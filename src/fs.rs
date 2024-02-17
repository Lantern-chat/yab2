use sha1::{Digest, Sha1};
use std::error::Error;
use std::{io::SeekFrom, path::Path, sync::Arc};
use tokio::sync::Mutex;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use reqwest::Body;

type DynError = Box<dyn Error + Send + Sync + 'static>;

use crate::*;

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

async fn hash_chunk(file: &mut File, start: u64, end: u64) -> Result<String, B2Error> {
    file.seek(SeekFrom::Start(start)).await?;

    let mut sha1 = Sha1::new();

    let chunk_length = end - start;

    let mut read = 0;
    let mut buf = [0; DEFAULT_BUF_SIZE];

    while read < chunk_length {
        let remaining = (chunk_length - read).min(DEFAULT_BUF_SIZE as u64) as usize;
        let n = file.read(&mut buf[..remaining]).await?;

        if n == 0 {
            break;
        }

        sha1.update(&buf[..n]);
        read += n as u64;
    }

    Ok(hex::encode(sha1.finalize()))
}

async fn forward_file_to_tx(
    file: &mut File,
    start: u64,
    end: u64,
    tx: tokio::sync::mpsc::Sender<Result<Vec<u8>, DynError>>,
) -> Result<(), B2Error> {
    file.seek(SeekFrom::Start(start)).await?;

    let chunk_length = end - start;

    let mut read = 0;
    let mut buf = [0; DEFAULT_BUF_SIZE];

    while read < chunk_length {
        let remaining = (chunk_length - read).min(DEFAULT_BUF_SIZE as u64) as usize;
        let n = file.read(&mut buf[..remaining]).await?;

        if n == 0 {
            break;
        }

        tx.send(Ok(buf[..n].to_vec())).await.map_err(|_| B2Error::Unknown)?;
        read += n as u64;
    }

    Ok(())
}

fn generate_file_upload_callback(file: Arc<Mutex<File>>, start: u64, end: u64) -> impl Fn() -> Body {
    move || {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<_, DynError>>(1);
        let body = Body::wrap_stream(tokio_stream::wrappers::ReceiverStream::new(rx));

        let file = file.clone();

        tokio::spawn(async move {
            let mut file = file.lock().await;

            if let Err(e) = forward_file_to_tx(&mut file, start, end, tx.clone()).await {
                tx.send(Err(e.into())).await;
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
    path: &'a Path,

    /// The name of the new file.
    ///
    /// If not provided, the file name will be the same as the file
    /// name on the local file system.
    #[builder(default, setter(into))]
    file_name: Option<String>,

    /// The MIME type of the file.
    #[builder(default, setter(into))]
    content_type: Option<String>,

    /// The maximum number of connections to use when uploading the file.
    ///
    /// If set to 0, the default number of connections will be used.
    #[builder(default, setter(into))]
    max_simultaneous_uploads: u8,

    /// The server-side encryption to use when uploading the file.
    #[builder(default)]
    encryption: Option<sse::ServerSideEncryption>,
}

impl Client {
    /// Acquires a new upload URL for the given bucket, then uploads the file at the given path.
    ///
    /// If the file is larger than the recommended part size, it will be uploaded in parts as a large file.
    /// Otherwise it will be uploaded as a single file, making use of the existing URL if provided.
    pub async fn upload_from_path(
        &mut self,
        mut info: NewFileFromPath<'_>,
        bucket_id: Option<&str>,
        existing_url: Option<&mut UploadUrl>,
    ) -> Result<models::B2FileInfo, B2Error> {
        let mut file = tokio::fs::File::open(info.path).await?;

        let (metadata, part_size) = tokio::join!(file.metadata(), async {
            self.state.read().await.account.api.storage.recommended_part_size
        });

        let metadata = metadata?;
        let length = metadata.len();

        let file_name = match info.file_name {
            Some(name) => name,
            None => info.path.file_name().ok_or(B2Error::MissingFileName)?.to_string_lossy().into_owned(),
        };

        if length <= part_size {
            let mut url = self.get_upload_url(bucket_id).await?;

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

        let whole_info = NewLargeFileInfo::builder().file_name(file_name).content_type(info.content_type).build();

        let large = self.start_large_file(&whole_info).await?;

        unimplemented!("WIP")
    }
}
