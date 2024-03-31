//! A Pool of [`UploadUrl`]s that can be used to upload files in parallel,
//! reusing the same URLs, and reducing the number of requests to the B2 API.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::{collections::VecDeque, sync::Weak};

use parking_lot::Mutex;
use tokio::sync::Semaphore;

use crate::{B2Error, Client, UploadUrl};

struct PoolInner {
    bucket_id: Option<String>,
    client: Client,
    sem: Semaphore,
    urls: Mutex<VecDeque<UploadUrl>>,
}

/// A pool of `UploadUrl`s that can be used to upload files in parallel,
/// reusing the same URLs, and reducing the number of requests to the B2 API.
///
/// The number of URLs in the pool is limited by the `max_urls` parameter passed to [`Pool::new`].
///
/// `Pool` also implements `Deref` to `Client`, so it can be used as a drop-in replacement for `Client`.
#[derive(Clone)]
pub struct Pool(Arc<PoolInner>);

/// A pooled `UploadUrl` that will be returned to the pool when dropped.
///
/// Will not prevent the pool from being dropped itself.
pub struct PooledUploadUrl {
    pool: Weak<PoolInner>,
    url: Option<UploadUrl>,
}

impl Pool {
    /// Creates a new pool with the given client and bucket ID.
    ///
    /// If `bucket_id` is `None`, the pool will use the default bucket for the authorized account.
    pub fn new(client: Client, bucket_id: Option<&str>, max_urls: u8) -> Self {
        Self(Arc::new(PoolInner {
            bucket_id: bucket_id.map(str::to_owned),
            client,
            sem: Semaphore::new(max_urls as usize),
            urls: Mutex::new(VecDeque::new()),
        }))
    }

    /// Acquires an `UploadUrl` from the pool, or gets a new one from the B2 API if the pool is empty.
    ///
    /// Can more or less be used as a drop-in replacement for [`Client::get_upload_url`].
    pub async fn get_pooled_upload_url(&self) -> Result<PooledUploadUrl, B2Error> {
        match self.0.sem.acquire().await {
            Ok(permit) => permit.forget(),
            Err(_) => return Err(B2Error::Unknown), // closed semaphore
        }

        let inner = &self.0;

        if let Some(url) = inner.urls.lock().pop_front() {
            return Ok(PooledUploadUrl {
                pool: Arc::downgrade(inner),
                url: Some(url),
            });
        }

        let new_url = inner.client.get_upload_url(inner.bucket_id.as_deref()).await?;

        Ok(PooledUploadUrl {
            pool: Arc::downgrade(&self.0),
            url: Some(new_url),
        })
    }

    /// Increases the size of the pool by the given amount.
    ///
    /// Should be used carefully. This is irreversible.
    pub fn increase_pool_size(&self, size: usize) {
        self.0.sem.add_permits(size);
    }
}

impl Deref for Pool {
    type Target = Client;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0.client
    }
}

impl Deref for PooledUploadUrl {
    type Target = UploadUrl;

    #[inline]
    fn deref(&self) -> &Self::Target {
        debug_assert!(self.url.is_some());

        // SAFETY: These should never be `None` until after `Drop`
        unsafe { self.url.as_ref().unwrap_unchecked() }
    }
}

impl DerefMut for PooledUploadUrl {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        debug_assert!(self.url.is_some());

        // SAFETY: These should never be `None` until after `Drop`
        unsafe { self.url.as_mut().unwrap_unchecked() }
    }
}

impl Drop for PooledUploadUrl {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            // SAFETY: This should never be `None` until after `Drop`
            pool.urls.lock().push_back(unsafe { self.url.take().unwrap_unchecked() });
            pool.sem.add_permits(1);
        }
    }
}
