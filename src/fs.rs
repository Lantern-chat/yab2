use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{io::SeekFrom, path::Path, sync::Arc};

use futures::FutureExt;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{mpsc, Mutex};

use futures::{
    future::TryFutureExt,
    stream::{self, StreamExt, TryStreamExt},
};

use bytes::{Bytes, BytesMut};
use reqwest::Body;

use sha1::{Digest, Sha1};

type DynError = Box<dyn Error + Send + Sync + 'static>;

use crate::*;

#[cfg(not(feature = "large_buffers"))]
const DEFAULT_BUF_SIZE: usize = 16 * 1024;

#[cfg(feature = "large_buffers")]
const DEFAULT_BUF_SIZE: usize = 64 * 1024;

async fn hash_chunk(file: &mut File, start: u64, end: u64) -> Result<String, B2Error> {
    file.seek(SeekFrom::Start(start)).await?;

    let mut sha1 = Sha1::new();

    let chunk_length = end - start;

    let mut read = 0;
    let mut buf = [0; DEFAULT_BUF_SIZE];

    while read < chunk_length {
        let remaining = (chunk_length - read).min(DEFAULT_BUF_SIZE as u64) as usize;

        let mut write_buf = &mut buf[..remaining];
        while !write_buf.is_empty() {
            file.read_buf(&mut write_buf).await?;
        }

        sha1.update(&buf[..remaining]);
        read += remaining as u64;
    }

    Ok(hex::encode(sha1.finalize()))
}

async fn forward_file_to_tx(
    file: &mut File,
    start: u64,
    end: u64,
    tx: mpsc::Sender<Result<Bytes, DynError>>,
) -> Result<(), B2Error> {
    file.seek(SeekFrom::Start(start)).await?;

    let chunk_length = end - start;
    let mut read = 0;

    while read < chunk_length {
        let remaining = (chunk_length - read).min(DEFAULT_BUF_SIZE as u64) as usize;

        let mut buf = BytesMut::with_capacity(remaining);

        // The buf won't resize unless these are equal, so stop it before then.
        while buf.len() < buf.capacity() {
            file.read_buf(&mut buf).await?;
        }

        assert_eq!(buf.len(), remaining);
        assert_eq!(buf.len(), buf.capacity());

        read += buf.len() as u64;

        if let Err(_) = tx.send(Ok(buf.freeze())).await {
            return Err(B2Error::Unknown);
        }
    }

    Ok(())
}

fn generate_file_upload_callback(file: Arc<Mutex<File>>, start: u64, end: u64) -> impl Fn() -> Body {
    move || {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<_, DynError>>(1);
        let body = Body::wrap_stream(tokio_stream::wrappers::ReceiverStream::new(rx));

        let file = file.clone();

        tokio::spawn(async move {
            let mut file = file.try_lock().expect("Unable to lock file");

            if let Err(e) = forward_file_to_tx(&mut file, start, end, tx.clone()).await {
                _ = tx.send(Err(e.into())).await;
            }
        });

        body
    }
}

/// Information for a new file to be uploaded.
///
/// See the documentation for [`NewFileFromPath::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
pub struct NewFileFromPath<'a> {
    pub path: &'a Path,

    /// The name of the new file.
    ///
    /// If not provided, the file name will be the same as the file
    /// name on the local file system.
    #[builder(default, setter(into))]
    pub file_name: Option<String>,

    /// The MIME type of the file.
    #[builder(default, setter(into))]
    pub content_type: Option<String>,

    /// The maximum number of connections to use when uploading the file.
    ///
    /// If set to 0, the default number of connections will be used.
    ///
    /// The default is currently a maximum of 8 connections,
    /// depending on the number of available threads.
    #[builder(default, setter(into))]
    pub max_simultaneous_uploads: u8,

    /// The server-side encryption to use when uploading the file.
    #[builder(default)]
    pub encryption: Option<sse::ServerSideEncryption>,
}

impl Client {
    /// Acquires a new upload URL for the given bucket, then uploads the file at the given path.
    ///
    /// If the file is larger than the recommended part size, it will be uploaded in parts as a large file.
    /// Otherwise it will be uploaded as a single file, making use of the existing URL if provided.
    pub async fn upload_from_path(
        &self,
        mut info: NewFileFromPath<'_>,
        bucket_id: Option<&str>,
        existing_url: Option<&mut UploadUrl>,
    ) -> Result<models::B2FileInfo, B2Error> {
        let mut file = tokio::fs::File::open(info.path).await?;

        let (metadata, recommended_part_size) = tokio::join!(file.metadata(), async {
            self.state.read().await.account.api.storage.recommended_part_size
        });

        let metadata = metadata?;
        let length = metadata.len();

        let file_name = match info.file_name.take() {
            Some(name) => name,
            None => info.path.file_name().ok_or(B2Error::MissingFileName)?.to_string_lossy().into_owned(),
        };

        // small file, upload as a single file
        if length <= recommended_part_size {
            let mut new_url; // store the new URL if we have to get one
            let url = match existing_url {
                Some(url) => url,
                None => {
                    new_url = Some(self.get_upload_url(bucket_id).await?);

                    new_url.as_mut().unwrap()
                }
            };

            let content_length = metadata.len();
            let content_sha1 = hash_chunk(&mut file, 0, length).await?;

            let file = Arc::new(Mutex::new(file));
            let whole_info = NewFileInfo::builder()
                .file_name(file_name)
                .content_type(info.content_type)
                .content_length(content_length)
                .content_sha1(content_sha1)
                .build();

            return url.upload_file(&whole_info, generate_file_upload_callback(file, 0, length)).await;
        }

        drop(file); // TODO: Reuse the file handle somehow?

        let max_simultaneous_uploads = match info.max_simultaneous_uploads {
            0 => match std::thread::available_parallelism() {
                Ok(threads) => threads.get().min(8),
                Err(_) => 1,
            },
            _ => info.max_simultaneous_uploads as usize,
        };

        let whole_info =
            NewLargeFileInfo::builder().file_name(file_name).content_type(info.content_type.take()).build();

        let large = self.start_large_file(bucket_id, &whole_info).await?;

        let num_parts = 1 + (length - 1) / recommended_part_size;

        struct SharedInfo {
            large: LargeFileUpload,
            path: PathBuf,
            encryption: Option<sse::ServerSideEncryption>,
            part: AtomicU32,
        }

        let info = Arc::new(SharedInfo {
            large,
            path: info.path.to_owned(),
            encryption: info.encryption,
            part: AtomicU32::new(0),
        });

        let files_and_urls = stream::iter(0..max_simultaneous_uploads).then(|_| async {
            let (file, url) = tokio::try_join!(
                File::open(&info.path).map_err(B2Error::from),
                info.large.get_upload_part_url()
            )?;

            Ok::<_, B2Error>((info.clone(), Arc::new(Mutex::new(file)), url))
        });

        let doing_uploads = files_and_urls.map_ok(|(info, file, mut url)| async move {
            let parts = tokio::spawn(async move {
                let mut parts = Vec::new();

                loop {
                    let part_number = info.part.fetch_add(1, Ordering::SeqCst);

                    if part_number as u64 >= num_parts {
                        break;
                    }

                    let start = part_number as u64 * recommended_part_size;
                    let end = (start + recommended_part_size).min(length);

                    let content_length = end - start;

                    let sha1 = {
                        let mut file = file.try_lock().expect("Unable to lock file");

                        hash_chunk(&mut file, start, end).await?
                    };

                    let part_info = NewPartInfo::builder()
                        .content_sha1(sha1)
                        .content_length(content_length)
                        .part_number(unsafe { NonZeroU32::new_unchecked(part_number + 1) })
                        .encryption(info.encryption.clone())
                        .build();

                    let cb = generate_file_upload_callback(file.clone(), start, end);
                    let part = info.large.upload_part(&mut url, &part_info, cb).await?;

                    parts.push(Ok::<_, B2Error>(part));
                }

                Ok::<_, B2Error>(stream::iter(parts))
            });

            parts.await.expect("Unable to upload") // only really happens if panic occurs
        });

        let mut parts = doing_uploads
            .try_buffer_unordered(max_simultaneous_uploads)
            .try_flatten_unordered(max_simultaneous_uploads)
            .try_collect::<Vec<_>>()
            .boxed()
            .await?;

        parts.sort_unstable_by_key(|part| part.part_number);

        Arc::try_unwrap(info).unwrap_or_else(|_| unreachable!()).large.finish(&parts).await
    }
}
