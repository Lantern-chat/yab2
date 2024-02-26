use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{io::SeekFrom, path::Path, sync::Arc};

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::{Mutex, OwnedMutexGuard};

use futures_util::stream::{self, StreamExt, TryStreamExt};
use futures_util::FutureExt;

use bytes::{Bytes, BytesMut};
use reqwest::Body;

use sha1::{Digest, Sha1};

type DynError = Box<dyn Error + Send + Sync + 'static>;

use crate::*;

#[cfg(not(feature = "large_buffers"))]
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

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

fn generate_file_upload_callback(file: Arc<Mutex<File>>, start: u64, end: u64) -> impl Fn() -> Body {
    move || {
        let num_chunks = (end - start).div_ceil(DEFAULT_BUF_SIZE as u64) as usize;

        // Pretty much guaranteed to be able to lock the file, so just do it.
        let file = Mutex::try_lock_owned(file.clone()).expect("Unable to lock file");

        struct State {
            file: OwnedMutexGuard<File>,
            chunk: u64,
        }

        Body::wrap_stream(stream::unfold(State { file, chunk: 0 }, move |mut state| async move {
            if state.chunk >= num_chunks as u64 {
                return None;
            }

            // avoid needing to deal with state in the error case
            let read_chunk = async {
                // only necessary on the first iteration
                if state.chunk == 0 {
                    state.file.seek(SeekFrom::Start(start)).await?;
                }

                let chunk_start = start + state.chunk * DEFAULT_BUF_SIZE as u64;
                let chunk_end = (chunk_start + DEFAULT_BUF_SIZE as u64).min(end);

                let remaining = (chunk_end - chunk_start) as usize;

                let mut buf = BytesMut::with_capacity(remaining);

                // The buf won't resize unless these are equal, so stop it before then.
                while buf.len() < buf.capacity() {
                    state.file.read_buf(&mut buf).await?;
                }

                assert_eq!(buf.len(), remaining);
                assert_eq!(buf.len(), buf.capacity());

                state.chunk += 1;

                Ok::<Bytes, DynError>(buf.freeze())
            };

            // give state back to the stream with result
            Some(match read_chunk.await {
                Ok(chunk) => (Ok(chunk), state),
                Err(e) => (Err(e), state),
            })
        }))
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
    pub file_name: Option<&'a str>,

    /// The MIME type of the file.
    #[builder(default, setter(into))]
    pub content_type: Option<&'a str>,

    /// The maximum number of connections to use when uploading the file.
    ///
    /// If set to 0, the default number of connections will be used.
    ///
    /// The default is currently a maximum of 4 connections,
    /// depending on the number of available threads.
    #[builder(default, setter(into))]
    pub max_simultaneous_uploads: u8,

    /// The server-side encryption to use when uploading the file.
    #[builder(default)]
    pub encryption: sse::ServerSideEncryption,

    /// The file retention settings to apply to the file.
    #[builder(default, setter(into))]
    pub retention: Option<FileRetention>,

    /// Whether to apply a legal hold to the file.
    #[builder(default)]
    pub legal_hold: Option<bool>,
}

impl Client {
    /// Acquires a new upload URL for the given bucket, then uploads the file at the given path.
    ///
    /// If the file is larger than the recommended part size, it will be uploaded in parts as a large file.
    /// Otherwise it will be uploaded as a single file, making use of the existing URL if provided.
    pub async fn upload_from_path(
        &self,
        info: &NewFileFromPath<'_>,
        bucket_id: Option<&str>,
        existing_url: Option<&mut UploadUrl>,
    ) -> Result<models::B2FileInfo, B2Error> {
        let mut file = tokio::fs::File::open(info.path).await?;

        let (metadata, recommended_part_size) = tokio::join!(file.metadata(), async {
            self.state.read().await.account.api.storage.recommended_part_size
        });

        let metadata = metadata?;
        let length = metadata.len();

        let file_name = match info.file_name {
            Some(name) => name.into(),
            None => info.path.file_name().ok_or(B2Error::MissingFileName)?.to_string_lossy(),
        };

        // small file, upload as a single file
        if length <= recommended_part_size {
            // Box the future to avoid bloating the stack too much, especially with large DEFAULT_BUF_SIZE
            let do_upload = Box::pin(async move {
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

                let whole_info = NewFileInfo {
                    file_name: &file_name,
                    content_type: info.content_type,
                    content_length,
                    content_sha1: &content_sha1,
                    encryption: info.encryption.clone(),
                    retention: info.retention.clone(),
                    legal_hold: info.legal_hold,
                };

                url.upload_file(&whole_info, generate_file_upload_callback(file, 0, length)).await
            });

            return do_upload.await;
        }

        let num_parts = length.div_ceil(recommended_part_size);

        let max_simultaneous_uploads = (num_parts as usize).min(match info.max_simultaneous_uploads {
            0 => match std::thread::available_parallelism() {
                Ok(threads) => threads.get().min(4),
                Err(_) => 1,
            },
            _ => info.max_simultaneous_uploads as usize,
        });

        let large = self
            .start_large_file(
                bucket_id,
                &NewLargeFileInfo {
                    file_name: &file_name,
                    content_type: info.content_type,
                    encryption: info.encryption.clone(),
                    retention: info.retention.clone(),
                    legal_hold: info.legal_hold,
                },
            )
            .boxed()
            .await?;

        struct SharedInfo {
            large: LargeFileUpload,
            part: AtomicU32,
            path: PathBuf,
            encryption: sse::ServerSideEncryption,
        }

        let info = Arc::new(SharedInfo {
            large,
            part: AtomicU32::new(0),
            path: info.path.to_owned(),
            encryption: info.encryption.clone(),
        });

        // inject the old file handle for the first iteration
        let old_files = stream::iter([Some(file)]).chain(stream::repeat_with(|| None));

        // use the old file handle for the first iteration, then open a new one for the rest and get the upload URL
        let files_and_urls = old_files.take(max_simultaneous_uploads).then(|old_file| async {
            let (url, file) = tokio::try_join!(info.large.get_upload_part_url(), async {
                Ok(match old_file {
                    Some(file) => file,
                    None => File::open(&info.path).await?,
                })
            })?;

            Ok::<_, B2Error>((info.clone(), Arc::new(Mutex::new(file)), url))
        });

        // for each file/url pair, upload the parts in parallel
        let do_uploads = files_and_urls.map_ok(|(info, file, mut url)| async move {
            // spawn in new task for real parallelism, at least when using the multi-threaded runtime
            let parts = tokio::spawn(async move {
                let mut parts = Vec::new();

                loop {
                    let part_number = info.part.fetch_add(1, Ordering::Relaxed);

                    if part_number as u64 >= num_parts {
                        break;
                    }

                    let start = part_number as u64 * recommended_part_size;
                    let end = (start + recommended_part_size).min(length);

                    let sha1 = {
                        let mut file = file.try_lock().expect("Unable to lock file");

                        hash_chunk(&mut file, start, end).await?
                    };

                    let part_info = NewPartInfo {
                        content_sha1: &sha1,
                        content_length: end - start,
                        part_number: unsafe { NonZeroU32::new_unchecked(part_number + 1) },
                        encryption: info.encryption.clone(),
                    };

                    let cb = generate_file_upload_callback(file.clone(), start, end);
                    let part = info.large.upload_part(&mut url, &part_info, cb).await?;

                    parts.push(Ok::<_, B2Error>(part));
                }

                Ok::<_, B2Error>(stream::iter(parts))
            });

            parts.await.expect("Unable to upload") // only really happens if panic occurs
        });

        // Box the future to avoid bloating the stack too much, especially with large DEFAULT_BUF_SIZE
        let mut parts = Box::pin(do_uploads)
            .try_buffer_unordered(max_simultaneous_uploads)
            .try_flatten_unordered(max_simultaneous_uploads)
            .try_collect::<Vec<_>>()
            .await?;

        parts.sort_unstable_by_key(|part| part.part_number);

        // done sharing the info now, can safely unwrap it
        let info = unsafe { Arc::try_unwrap(info).unwrap_unchecked() };

        info.large.finish(&parts).boxed().await
    }
}
