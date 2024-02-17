#![allow(unused)]

#[macro_use]
extern crate serde;

use headers::HeaderMapExt;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client as ReqwestClient, Method, Response,
};
use std::{borrow::Cow, future::Future, num::NonZeroU32, sync::Arc};
use tokio::sync::RwLock;

pub mod error;
pub mod models;

#[cfg(feature = "fs")]
mod fs;

pub use error::B2Error;

const PREFIX: &str = "b2api/v3";
const AUTH_HEADER: HeaderName = HeaderName::from_static("authorization");

struct ClientState {
    /// The builder used to create the client.
    config: ClientBuilder,

    /// The authorization data returned from the B2 API `b2_authorize_account` endpoint
    account: crate::models::B2Authorized,

    /// The authorization header to use for requests
    auth: HeaderValue,
}

impl ClientState {
    fn check_capability(&self, capability: &'static str) -> Result<(), B2Error> {
        if !self.account.allowed(capability) {
            return Err(B2Error::MissingCapability(capability));
        }

        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{PREFIX}/{}", self.account.api.storage.api_url, path)
    }
}

/// A client for interacting with the B2 API
#[derive(Clone)]
pub struct Client {
    state: Arc<RwLock<ClientState>>,
    client: ReqwestClient,
}

/// A builder for creating a [`Client`]
#[derive(Clone)]
pub struct ClientBuilder {
    auth: HeaderValue,
    ua: Option<Cow<'static, str>>,
    max_retries: u8,
}

impl ClientBuilder {
    /// Creates a new client builder with the given key ID and application key.
    pub fn new(key_id: &str, app_key: &str) -> ClientBuilder {
        ClientBuilder {
            auth: models::create_auth_header(key_id, app_key),
            ua: None,
            max_retries: 5,
        }
    }

    /// Sets the `User-Agent` header to be used for requests.
    #[inline]
    pub fn user_agent(mut self, ua: impl Into<Cow<'static, str>>) -> Self {
        self.ua = Some(ua.into());
        self
    }

    /// Sets the maximum number of times to retry requests if they fail with a 401 Unauthorized error.
    #[inline]
    pub fn max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Builds and authorizes the client for first use.
    pub async fn authorize(self) -> Result<Client, B2Error> {
        let mut builder = reqwest::ClientBuilder::new().https_only(true);

        if let Some(ref ua) = self.ua {
            builder = builder.user_agent(ua.as_ref());
        }

        let client = builder.build()?;

        Ok(Client {
            state: Arc::new(RwLock::new(Client::do_auth(&client, self).await?)),
            client,
        })
    }
}

impl Client {
    async fn try_json_error(resp: Response) -> Result<Response, B2Error> {
        if !resp.status().is_success() {
            return Err(B2Error::B2ErrorMessage(resp.json().await?));
        }

        Ok(resp)
    }

    async fn json<T>(resp: reqwest::Response) -> Result<T, B2Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let text = Self::try_json_error(resp).await?.text().await?;

        println!("TEXT: {text}");

        Ok(serde_json::from_str(&text)?)
    }

    async fn do_auth(client: &ReqwestClient, config: ClientBuilder) -> Result<ClientState, B2Error> {
        use failsafe::{futures::CircuitBreaker, Config, Error as FailsafeError};

        let cb = Config::new().build();
        let mut attempts = 0;

        'try_auth: loop {
            let do_auth_inner = async {
                let resp = client
                    .get(format!("https://api.backblazeb2.com/{PREFIX}/b2_authorize_account"))
                    .header(AUTH_HEADER, &config.auth)
                    .send()
                    .await?;

                Client::json::<models::B2Authorized>(resp).await
            };

            return match cb.call(do_auth_inner).await {
                Ok(account) => Ok(ClientState {
                    config,
                    auth: HeaderValue::from_str(&account.auth_token)
                        .expect("Unable to use auth token in header value"),
                    account,
                }),
                Err(FailsafeError::Rejected) => {
                    attempts += 1;
                    if attempts >= config.max_retries {
                        return Err(B2Error::Unauthorized);
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                    continue 'try_auth;
                }
                Err(FailsafeError::Inner(e)) => Err(e),
            };
        }
    }

    /// Reauthorizes the client, updating the authorization token and account information.
    async fn reauthorize(&self) -> Result<(), B2Error> {
        let new_state = Self::do_auth(&self.client, self.state.read().await.config.clone()).await?;
        *self.state.write().await = new_state;
        Ok(())
    }

    /// Runs a request, reauthorizing if necessary.
    async fn run_request_with_reauth<'a, F, R, T>(&self, f: F) -> Result<T, B2Error>
    where
        F: Fn(Self) -> R + 'a,
        R: Future<Output = Result<T, B2Error>> + 'a,
    {
        let mut retried = false;
        loop {
            return match f(self.clone()).await {
                Ok(t) => Ok(t),
                Err(B2Error::B2ErrorMessage(e)) if !retried && e.status == 401 => {
                    self.reauthorize().await?;
                    retried = true;
                    continue;
                }
                Err(e) => Err(e),
            };
        }
    }

    fn inner_client(&self) -> &ReqwestClient {
        &self.client
    }

    /// Uses the `b2_get_file_info` API to get information about a file by its ID.
    pub async fn get_file_info(&self, file_id: &str) -> Result<models::B2FileInfo, B2Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct B2GetFileInfo<'a> {
            file_id: &'a str,
        }

        self.run_request_with_reauth(|b2| async move {
            let state = b2.state.read().await;

            state.check_capability("readFiles")?; // TODO: check if this is the right capability

            let resp = b2
                .client
                .request(Method::GET, "b2_get_file_info")
                .header(AUTH_HEADER, &state.auth)
                .query(&B2GetFileInfo { file_id })
                .send()
                .await?;

            Client::json(resp).await
        })
        .await
    }

    /// Uses the `b2_download_file_by_id` API to download a file by its ID, returning a [`DownloadedFile`],
    /// which is a wrapper around a [`reqwest::Response`] and the file's parsed headers.
    ///
    /// The `range` parameter can be used to download only a portion of the file. If `None`, the entire file will be downloaded.
    ///
    /// The `encryption` parameter is only required if the file is encrypted with server-side encryption with a customer-provided key (SSE-C).
    pub async fn download_file_by_id(
        &self,
        file_id: &str,
        range: Option<headers::Range>,
        encryption: Option<ServerSideEncryptionCustomer>,
    ) -> Result<DownloadedFile, B2Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct B2DownloadFileById<'a> {
            file_id: &'a str,
        }

        let (range, encryption) = (&range, &encryption);

        self.run_request_with_reauth(|b2| async move {
            let state = b2.state.read().await;

            state.check_capability("readFiles")?;

            let mut builder = b2
                .client
                .request(Method::GET, "b2_download_file_by_id")
                .query(&B2DownloadFileById { file_id })
                .header(AUTH_HEADER, &state.auth);

            if let Some(ref range) = range {
                let mut headers = reqwest::header::HeaderMap::new();
                headers.typed_insert(range.clone());
                builder = builder.headers(headers);
            }

            if let Some(ref encryption) = encryption {
                builder = builder.headers({
                    let mut headers = HeaderMap::new();
                    encryption.add_headers(&mut headers);
                    headers
                });
            }

            let resp = builder.send().await?;

            Ok(DownloadedFile {
                info: models::B2FileHeaders::parse(resp.headers())?,
                resp,
            })
        })
        .await
    }

    async fn get_b2_upload_url(
        &self,
        bucket_id: Option<&str>,
        in_parts: bool,
    ) -> Result<models::B2UploadUrl, B2Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct B2GetUploadUrlQuery<'a> {
            bucket_id: &'a str,
        }

        self.run_request_with_reauth(|b2| async move {
            let state = b2.state.read().await;

            state.check_capability("writeFiles")?;

            let builder = {
                let path = state.url(if in_parts { "b2_get_upload_part_url" } else { "b2_get_upload_url" });
                b2.client.request(Method::GET, path).header(AUTH_HEADER, &state.auth)
            };

            let Some(bucket_id) = bucket_id.or_else(|| state.account.api.storage.bucket_id.as_deref()) else {
                return Err(B2Error::MissingBucketId);
            };

            let resp = builder.query(&B2GetUploadUrlQuery { bucket_id }).send().await?;
            let url: models::B2UploadUrl = Self::json(resp).await?;

            Ok(url)
        })
        .await
    }

    async fn get_raw_upload_url(&self, bucket_id: Option<&str>, in_parts: bool) -> Result<RawUploadUrl, B2Error> {
        let url = self.get_b2_upload_url(bucket_id, in_parts).await?;

        Ok(RawUploadUrl {
            in_parts,
            client: self.clone(),
            auth: url.header(),
            url,
        })
    }

    /// Gets a URL for uploading files using the `b2_get_upload_url` API.
    ///
    /// If `bucket_id` is `None`, the client's default bucket will be used. If there is no default bucket, an error will be returned.
    ///
    /// The returned `UploadUrl` can be used to upload files to the B2 API for 24 hours. Only one file can be uploaded to a URL at a time.
    /// You may acquire multiple URLs to upload multiple files in parallel.
    pub async fn get_upload_url(&self, bucket_id: Option<&str>) -> Result<UploadUrl, B2Error> {
        Ok(UploadUrl(self.get_raw_upload_url(bucket_id, false).await?))
    }

    /// Gets a URL for uploading parts of a large file using the `b2_get_upload_part_url` API.
    ///
    /// If `bucket_id` is `None`, the client's default bucket will be used. If there is no default bucket, an error will be returned.
    ///
    /// The returned `UploadPartUrl` can be used to upload parts of a large file to the B2 API for 24 hours.
    /// Only one part can be uploaded to a URL at a time. You may acquire multiple URLs to upload multiple parts in parallel.
    pub async fn get_upload_part_url(&self, bucket_id: Option<&str>) -> Result<UploadPartUrl, B2Error> {
        Ok(UploadPartUrl(self.get_raw_upload_url(bucket_id, true).await?))
    }

    /// Prepares parts of a large file for uploading using the `b2_start_large_file` API.
    pub async fn start_large_file(&self, info: &NewFileInfo) -> Result<LargeFileUpload, B2Error> {
        let info = self
            .run_request_with_reauth(|b2| async move {
                let state = b2.state.read().await;

                state.check_capability("writeFiles")?;

                let resp = b2
                    .client
                    .request(Method::POST, state.url("b2_start_large_file"))
                    .header(AUTH_HEADER, &state.auth)
                    .headers({
                        let mut headers = HeaderMap::new();
                        info.add_headers(&mut headers, true);
                        headers
                    })
                    .send()
                    .await?;

                Client::json::<models::B2FileInfo>(resp).await
            })
            .await?;

        Ok(LargeFileUpload {
            client: self.clone(),
            info,
        })
    }
}

/// Wrapper around a response and the file's parsed headers.
pub struct DownloadedFile {
    pub resp: reqwest::Response,
    pub info: models::B2FileHeaders,
}

#[derive(Debug, Serialize)]
pub struct ServerSideEncryptionCustomer {
    /// The algorithm to use when encrypting/decrypting a file using SSE-C encryption. The only currently supported value is AES256.
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Algorithm")]
    pub algorithm: String,

    /// The base64-encoded AES256 encryption key when encrypting/decrypting a file using SSE-C encryption.
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key")]
    pub key: String,

    /// The base64-encoded MD5 digest of the `X-Bz-Server-Side-Encryption-Customer-Key` when encrypting/decrypting a file using SSE-C encryption.
    #[serde(rename = "X-Bz-Server-Side-Encryption-Customer-Key-Md5")]
    pub key_md5: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ServerSideEncryption {
    /// SSE-B2 encryption
    Standard {
        /// The algorithm to use when encrypting/decrypting a file using SSE-B2 encryption. The only currently supported value is AES256.
        #[serde(rename = "X-Bz-Server-Side-Encryption")]
        algorithm: String,
    },

    /// SSE-C encryption
    Customer(ServerSideEncryptionCustomer),
}

/// Info about a new whole file to be uploaded.
///
/// See the documentation for [`NewFileInfo::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
pub struct NewFileInfo {
    /// The name of the new file.
    #[builder(setter(into))]
    file_name: String,

    /// The length of the file in bytes.
    content_length: u64,

    /// The MIME type of the file.
    #[builder(default, setter(into))]
    content_type: Option<String>,

    /// The SHA1 hash of the file's contents as a hex string.
    #[builder(setter(into))]
    content_sha1: String,

    /// The server-side encryption to use when uploading the file.
    #[builder(default)]
    encryption: Option<ServerSideEncryption>,
}

/// Info about a new part of a large file to be uploaded.
///
/// See the documentation for [`NewPartInfo::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
pub struct NewPartInfo {
    /// The part number of the new large file part.
    #[builder(setter(into))]
    part_number: NonZeroU32,

    /// The length of the part in bytes.
    content_length: u64,

    /// The SHA1 hash of the part's contents as a hex string.
    #[builder(setter(into))]
    content_sha1: String,

    /// The server-side encryption to use when uploading the file.
    #[builder(default)]
    encryption: Option<ServerSideEncryption>,
}

macro_rules! h {
    ($headers:ident.$key:literal => $value:expr) => {
        $headers.insert(
            HeaderName::from_static($key), // NOTE: Header names must be lowercase
            HeaderValue::from_str($value).expect("Unable to use header value"),
        );
    };
}

impl ServerSideEncryptionCustomer {
    fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-server-side-encryption-customer-algorithm" => &self.algorithm);
        h!(headers."x-bz-server-side-encryption-customer-key" => &self.key);
        h!(headers."x-bz-server-side-encryption-customer-key-md5" => &self.key_md5);
    }
}

impl ServerSideEncryption {
    fn add_headers(&self, headers: &mut HeaderMap) {
        match self {
            ServerSideEncryption::Standard { algorithm } => {
                h!(headers."x-bz-server-side-encryption" => algorithm);
            }
            ServerSideEncryption::Customer(sse_c) => sse_c.add_headers(headers),
        }
    }
}

impl NewFileInfo {
    fn add_headers(&self, headers: &mut HeaderMap, parts: bool) {
        h!(headers."x-bz-file-name" => &self.file_name);
        h!(headers."content-type" => self.content_type.as_deref().unwrap_or("application/octet-stream"));

        if !parts {
            h!(headers."content-length" => &self.content_length.to_string());
            h!(headers."x-bz-content-sha1" => &self.content_sha1);
        }

        if let Some(ref encryption) = self.encryption {
            encryption.add_headers(headers);
        }
    }
}

impl NewPartInfo {
    fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-part-number" => &self.part_number.to_string());
        h!(headers."content-length" => &self.content_length.to_string());
        h!(headers."x-bz-content-sha1" => &self.content_sha1);

        if let Some(ref encryption) = self.encryption {
            encryption.add_headers(headers);
        }
    }
}

struct RawUploadUrl {
    in_parts: bool,
    client: Client,
    url: models::B2UploadUrl,
    auth: HeaderValue,
}

/// Temporarily acquired URL for uploading single files.
///
/// This is returned by [`Client::get_upload_url`].
///
/// The URL can be used to upload a file to the B2 API for 24 hours. Only one file can be uploaded to a URL at a time.
/// This is enforced via requiring mutable references to the URL when uploading a file.
pub struct UploadUrl(RawUploadUrl);

/// Temporarily acquired URL for uploading parts of a large file.
///
/// This is returned by [`Client::get_upload_part_url`].
///
/// The URL can be used to upload parts of a large file to the B2 API for 24 hours. Only one part can be uploaded to a URL at a time.
/// This is enforced via requiring mutable references to the URL when uploading a part.
pub struct UploadPartUrl(RawUploadUrl);

impl std::ops::Deref for UploadUrl {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0.client
    }
}

impl RawUploadUrl {
    /// Actually performs the upload, with automatic reauthorization if necessary.
    async fn do_upload<F, T>(&mut self, f: F) -> Result<T, B2Error>
    where
        F: Fn(&Self) -> reqwest::RequestBuilder,
        T: serde::de::DeserializeOwned,
    {
        loop {
            let res = async { Client::json(f(self).send().await?).await };

            return match res.await {
                Err(B2Error::B2ErrorMessage(e)) if e.status == 401 => {
                    let url = self.client.get_b2_upload_url(Some(&self.url.bucket_id), self.in_parts).await?;

                    self.auth = url.header();
                    self.url = url;

                    continue;
                }
                res => res,
            };
        }
    }
}

impl UploadUrl {
    /// Uploads a file to the B2 API using the URL acquired from [`Client::get_upload_url`].
    ///
    /// The `file` parameter is a closure that returns a value to be converted into a `reqwest::Body`.
    /// This method may need to retry the request if the URL or authorization token has expired, therefore
    /// it is recommended the body-creation closure be cheap to call multiple times.
    pub async fn upload_file<F, B>(&mut self, info: &NewFileInfo, file: F) -> Result<models::B2FileInfo, B2Error>
    where
        F: Fn() -> B,
        B: Into<reqwest::Body>,
    {
        self.0
            .do_upload(|url| {
                let client = url.client.inner_client();
                client
                    .request(reqwest::Method::POST, &url.url.upload_url)
                    .header(AUTH_HEADER, &url.auth)
                    .headers({
                        let mut headers = HeaderMap::new();
                        info.add_headers(&mut headers, false);
                        headers
                    })
                    .body(file())
            })
            .await
    }

    /// Uploads a file to the B2 API using the URL acquired from [`Client::get_upload_url`].
    ///
    /// The `bytes` parameter is a value to be converted into a `bytes::Bytes`.
    pub async fn upload_file_bytes(
        &mut self,
        info: &NewFileInfo,
        bytes: impl Into<bytes::Bytes>,
    ) -> Result<models::B2FileInfo, B2Error> {
        let bytes = bytes.into();
        self.upload_file(info, || bytes.clone()).await
    }
}

/// A large file that is being uploaded in parts.
///
/// Any [`UploadPartUrl`] can be used to upload a part of the file. Once all parts have been uploaded,
/// call [`LargeFile::finish`] to complete the upload.
pub struct LargeFileUpload {
    client: Client,
    info: models::B2FileInfo,
}

impl LargeFileUpload {
    /// Equivalent to [`Client::start_large_file`].
    pub async fn start(client: &Client, info: &NewFileInfo) -> Result<LargeFileUpload, B2Error> {
        client.start_large_file(info).await
    }

    /// Uploads a part of a large file to the given upload URL. Once all parts have been uploaded,
    /// call [`LargeFile::finish`] to complete the upload.
    ///
    /// Parts can be uploaded in parallel, so long as each url is only used for one part at a time.
    ///
    /// The `body` parameter is a closure that returns a value to be converted into a [`reqwest::Body`], and
    /// may need to be called multiple times if the request needs to be retried. Therefore, it is recommended
    /// the body-creation closure be cheap to call multiple times.
    pub async fn upload_part<F, B>(
        &self,
        url: &mut UploadPartUrl,
        info: &NewPartInfo,
        body: F,
    ) -> Result<models::B2PartInfo, B2Error>
    where
        F: Fn() -> B,
        B: Into<reqwest::Body>,
    {
        url.0
            .do_upload(|url| {
                let client = url.client.inner_client();
                client
                    .request(reqwest::Method::POST, &url.url.upload_url)
                    .header(AUTH_HEADER, &url.auth)
                    .headers({
                        let mut headers = HeaderMap::new();
                        info.add_headers(&mut headers);
                        headers
                    })
                    .body(body())
            })
            .await
    }

    pub async fn upload_part_bytes(
        &self,
        url: &mut UploadPartUrl,
        info: &NewPartInfo,
        bytes: impl Into<bytes::Bytes>,
    ) -> Result<models::B2PartInfo, B2Error> {
        let bytes = bytes.into();
        self.upload_part(url, info, || bytes.clone()).await
    }

    /// Converts the parts that have been uploaded into a single B2 file.
    ///
    /// It may be that the call to finish a large file succeeds, but you don't know it because the
    /// request timed out, or the connection was broken. In that case, retrying will result in a
    /// 400 Bad Request response because the file is already finished. If that happens, we recommend
    /// calling `b2_get_file_info`/[`Client::get_file_info`] to see if the file is there. If the file is there,
    /// you can count the upload as a success.
    ///
    /// `parts` must be sorted by `part_number`.
    pub async fn finish(self, parts: &[models::B2PartInfo]) -> Result<models::B2FileInfo, B2Error> {
        // check if parts are sorted by part_number
        if parts.windows(2).any(|w| w[0].part_number >= w[1].part_number) {
            return Err(B2Error::InvalidPartSorting);
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct B2FinishLargeFile<'a> {
            file_id: &'a str,
            part_sha1_array: Vec<&'a str>,
        }

        let body = &B2FinishLargeFile {
            file_id: &self.info.file_id,
            part_sha1_array: parts.iter().map(|part| &*part.content_sha1).collect(),
        };

        self.client
            .run_request_with_reauth(|b2| async move {
                let state = b2.state.read().await;

                let resp = b2
                    .client
                    .request(reqwest::Method::POST, state.url("b2_finish_large_file"))
                    .header(AUTH_HEADER, &state.auth)
                    .json(&body)
                    .send()
                    .await?;

                Client::json(resp).await
            })
            .await
    }

    /// Cancels the upload of a large file, and deletes all of the parts that have been uploaded.
    ///
    /// This will return an error if there is no active upload with the given file ID.
    pub async fn cancel(self) -> Result<models::B2CancelledFileInfo, B2Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct B2CancelLargeFile<'a> {
            file_id: &'a str,
        }

        let body = &B2CancelLargeFile {
            file_id: &self.info.file_id,
        };

        self.client
            .run_request_with_reauth(|b2| async move {
                let state = b2.state.read().await;

                let resp = b2
                    .client
                    .request(reqwest::Method::POST, state.url("b2_cancel_large_file"))
                    .header(AUTH_HEADER, &state.auth)
                    .json(&body)
                    .send()
                    .await?;

                Client::json(resp).await
            })
            .await
    }
}

impl std::ops::Deref for LargeFileUpload {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn test_auth() {
        dotenv::dotenv().ok();

        let app_id = std::env::var("APP_ID").expect("APP_ID not found in .env");
        let app_key = std::env::var("APP_KEY").expect("APP_KEY not found in .env");

        let client = ClientBuilder::new(&app_id, &app_key).authorize().await.unwrap();

        // must be mut because `upload_file` requires exclusive access to the url
        let mut upload = client.get_upload_url(None).await.unwrap();

        let mut file = tokio::fs::OpenOptions::new().read(true).open("Cargo.toml").await.unwrap();
        let meta = file.metadata().await.unwrap();

        let mut bytes = Vec::with_capacity(meta.len() as usize);
        file.read_to_end(&mut bytes).await.unwrap();

        let bytes = bytes::Bytes::from(bytes); // bytes

        let info = NewFileInfo::builder()
            .file_name("testing/Cargo.toml".to_owned())
            .content_length(meta.len())
            .content_type("text/plain".to_owned())
            .content_sha1(hex::encode({
                use sha1::{Digest, Sha1};

                let mut hasher = Sha1::new();
                hasher.update(&bytes);
                hasher.finalize()
            }))
            .build();

        upload.upload_file_bytes(&info, bytes).await.unwrap();

        println!("{:#?}", client.state.read().await.account);
    }
}
