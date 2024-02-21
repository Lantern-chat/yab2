//! Models for the B2 API.
//!
//! These types are largely read-only.

use reqwest::header::{HeaderMap, HeaderValue};
use std::{collections::HashMap, sync::Arc};

/// Creates the authorization header and token
///
/// **NOTE**: The account ID can be used in place of the master application key ID.
pub fn create_auth_header(key_id: &str, key: &str) -> HeaderValue {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    HeaderValue::from_str(&format!("Basic {}", STANDARD.encode(format!("{key_id}:{key}"))))
        .expect("Unable to create auth header value")
}

#[derive(Debug, Deserialize)]
pub struct B2Authorized {
    /// The identifier for the account.
    #[serde(alias = "accountId")]
    pub account_id: Box<str>,

    /// An authorization token to use with all calls, other than b2_authorize_account,
    /// that need an `authorization` header.
    ///
    /// **This authorization token is valid for at most 24 hours.**
    #[serde(alias = "authorizationToken")]
    pub auth_token: Box<str>,

    #[serde(alias = "apiInfo")]
    pub api: B2ApiInfo,

    #[serde(default, alias = "applicationKeyExpirationTimestamp")]
    pub expiration: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct B2ApiInfo {
    #[serde(alias = "storageApi")]
    pub storage: B2StorageApi,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2StorageApi {
    pub api_url: Box<str>,
    pub download_url: Box<str>,
    pub recommended_part_size: u64,
    pub absolute_minimum_part_size: u64,
    pub s3_api_url: Box<str>,

    /// A list of strings, each one naming a capability the key has. Possibilities are:
    /// `listKeys`, `writeKeys`, `deleteKeys`, `listBuckets`, `writeBuckets`,
    /// `deleteBuckets`, `listFiles`, `readFiles`, `shareFiles`, `writeFiles`, and `deleteFiles`.
    #[serde(default)]
    pub capabilities: Vec<Box<str>>,

    /// When present, access is restricted to one bucket.
    pub bucket_id: Option<Box<str>>,

    /// When `bucketId`` is set, and it is a valid bucket that has not been deleted,
    /// this field is set to the name of the bucket.
    ///
    /// It's possible that bucketId is set to a bucket that no longer exists,
    /// in which case this field will be null. It's also null when `bucketId`` is null.
    pub bucket_name: Option<Box<str>>,

    /// When present, access is restricted to files whose names start with the prefix.
    pub name_prefix: Option<Arc<str>>,
}

impl B2StorageApi {
    /// Checks if the storage API has the capability to perform the given action.
    pub fn contains(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| capability.eq_ignore_ascii_case(c))
    }
}

impl B2Authorized {
    /// Checks if the authorized account has the capability to perform the given action.
    pub fn allowed(&self, capability: &str) -> bool {
        self.api.storage.contains(capability)
    }
}

/// When you upload a file to B2, you must call `b2_get_upload_url` first to get the URL for uploading.
/// Then, you use `b2_upload_file` on this URL to upload your file.
///
/// An upload url and upload authorization token are valid for 24 hours or until the endpoint
/// rejects an upload, see `b2_upload_file`. You can upload as many files to this URL as you need.
///
/// To achieve faster upload speeds, request multiple upload urls and upload your files
/// to these different endpoints in parallel.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2UploadUrl {
    /// The identifier for the bucket, if doing a simple upload.
    #[serde(default)]
    pub bucket_id: Option<Box<str>>,

    /// The identifier for the file, if doing a large file upload.
    #[serde(default)]
    pub file_id: Option<Box<str>>,

    /// The URL that can be used to upload files to this bucket, see b2_upload_file.
    pub upload_url: Box<str>,

    /// The authorization token that must be used when uploading files to this bucket.
    ///
    /// This token is valid for 24 hours or until the uploadUrl endpoint rejects an upload,
    /// see b2_upload_file
    pub authorization_token: Box<str>,
}

impl B2UploadUrl {
    pub fn header(&self) -> HeaderValue {
        HeaderValue::from_str(&self.authorization_token).expect("Unable to use auth token in header value")
    }
}

#[derive(Debug)]
pub enum B2FileEncryptionHeaders {
    B2 { algorithm: Box<str> },
    Customer { algorithm: Box<str>, key_md5: Box<str> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum B2FileRetentionMode {
    Governance,
    Compliance,
}

impl AsRef<str> for B2FileRetentionMode {
    fn as_ref(&self) -> &str {
        match self {
            B2FileRetentionMode::Governance => "governance",
            B2FileRetentionMode::Compliance => "compliance",
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum B2FileAction {
    #[serde(alias = "start")]
    Started,
    #[serde(alias = "upload")]
    Uploaded,
    #[serde(alias = "hide")]
    Hidden,
    #[serde(alias = "folder")]
    Folder,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum B2ReplicationStatus {
    Pending,
    Completed,
    Failed,
    Replica,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2LegalHold {
    pub is_client_authorized_to_read: bool,

    #[serde(default)]
    pub value: Option<Box<str>>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileRetention {
    pub is_client_authorized_to_read: bool,

    #[serde(default)]
    pub value: Option<B2FileRetentionValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileRetentionValue {
    pub mode: B2FileRetentionMode,
    pub retain_until_timestamp: u64,
}

#[derive(Default, Debug, Deserialize)]
pub struct B2ServerSideEncryption {
    #[serde(default)]
    pub algorithm: Option<Box<str>>,

    #[serde(default)]
    pub mode: Option<Box<str>>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct B2FileInfo {
    pub account_id: Option<Box<str>>,
    pub file_id: Box<str>,
    pub file_name: Box<str>,
    pub action: Option<B2FileAction>,
    pub bucket_id: Box<str>,
    pub content_length: u64,
    pub content_sha1: Option<Box<str>>,
    pub content_type: Option<Box<str>>,
    pub file_info: HashMap<Box<str>, Box<str>>,
    pub file_retention: B2FileRetention,
    pub legal_hold: B2LegalHold,
    pub replication_status: Option<B2ReplicationStatus>,
    pub server_side_encryption: B2ServerSideEncryption,
    pub upload_timestamp: u64,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileInfoList {
    pub files: Vec<B2FileInfo>,

    /// The name of the next file, if there are more files to list.
    #[serde(default)]
    pub next_file_name: Option<Box<str>>,

    /// The ID of the next file, if there are more files to list.
    #[serde(default)]
    pub next_file_id: Option<Box<str>>,
}

/// Response from `b2_cancel_large_file`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2CancelledFileInfo {
    pub file_id: Box<str>,
    pub file_name: Box<str>,
    pub bucket_id: Box<str>,
    pub account_id: Box<str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2PartInfo {
    pub file_id: Box<str>,
    pub part_number: u64,
    pub content_length: u64,
    pub content_sha1: Box<str>,

    #[serde(default)]
    pub server_side_encryption: B2ServerSideEncryption,

    pub upload_timestamp: u64,
}

use headers::{CacheControl, ContentDisposition, ContentLength, ContentType, Expires, HeaderMapExt};

pub struct B2FileHeaders {
    pub content_length: ContentLength,
    pub content_type: ContentType,
    pub file_id: Box<str>,
    pub file_name: Box<str>,
    pub file_sha1: Box<str>,
    pub info: HeaderMap,
    pub upload_timestamp: u64,

    pub content_disposition: Option<ContentDisposition>,
    pub content_language: Option<Box<str>>,
    pub expires: Option<Expires>,
    pub cache_control: Option<CacheControl>,
    pub encryption: Option<B2FileEncryptionHeaders>,

    pub retention_mode: Option<B2FileRetentionMode>,
    pub retain_until: Option<u64>,
    pub legal_hold: Option<bool>,
    pub unauthorized_to_read: Option<Box<str>>,
}

use crate::error::B2FileHeaderError;

impl B2FileHeaders {
    pub(crate) fn parse(headers: &HeaderMap) -> Result<B2FileHeaders, B2FileHeaderError> {
        #[rustfmt::skip] macro_rules! p {
            [@$key:literal] => { headers.typed_get().ok_or(B2FileHeaderError::MissingHeader($key))? };
            [$key:literal] => { headers.get($key).ok_or(B2FileHeaderError::MissingHeader($key))? };
            [$key:literal as str] => { p![$key].to_str()? };
            [$key:literal as Box<str>] => { Box::from(p![$key].to_str()?) };
            [$key:literal as Option<Box<str>>] => { headers.get($key).map(|h| h.to_str().map(Box::from)).transpose()? };
        }

        let mut info = HeaderMap::new();
        for (name, value) in headers.iter() {
            if name.as_str().starts_with("x-bz-info-") {
                info.append(name, value.clone());
            }
        }

        Ok(B2FileHeaders {
            content_length: p![@"content-length"],
            content_type: p![@"content-type"],
            file_id: p!["x-bz-file-id" as Box<str>],
            file_name: p!["x-bz-file-name" as Box<str>],
            file_sha1: p!["x-bz-content-sha1" as Box<str>],
            info,
            upload_timestamp: p!["x-bz-upload-timestamp" as str].parse()?,
            content_disposition: headers.typed_get(),
            content_language: p!["content-language" as Option<Box<str>>],
            expires: headers.typed_get(),
            cache_control: headers.typed_get(),

            encryption: match p!["x-bz-server-side-encryption" as Option<Box<str>>] {
                Some(algorithm) => Some(B2FileEncryptionHeaders::B2 { algorithm }),
                None => match p!["x-bz-server-side-encryption-customer-algorithm" as Option<Box<str>>] {
                    Some(algorithm) => Some(B2FileEncryptionHeaders::Customer {
                        algorithm,
                        key_md5: p!["x-bz-server-side-encryption-customer-key-md5" as Box<str>],
                    }),
                    None => None,
                },
            },

            retention_mode: match headers.get("x-bz-file-retention-mode") {
                None => None,
                Some(rm) => Some(match rm.to_str()? {
                    rm if rm.eq_ignore_ascii_case("governance") => B2FileRetentionMode::Governance,
                    rm if rm.eq_ignore_ascii_case("compliance") => B2FileRetentionMode::Compliance,
                    _ => return Err(B2FileHeaderError::InvalidRetentionMode),
                }),
            },

            retain_until: headers
                .get("x-bz-file-retention-retain-until-timestamp")
                .map(|h| Ok::<_, B2FileHeaderError>(h.to_str()?.parse()?))
                .transpose()?,

            legal_hold: {
                match headers.get("x-bz-file-legal-hold") {
                    None => None,
                    Some(header) => Some(match header.to_str()? {
                        "true" | "yes" => true,
                        "false" | "no" => false,
                        _ => return Err(B2FileHeaderError::BoolParseError),
                    }),
                }
            },
            unauthorized_to_read: p!["x-bz-client-unauthorized-to-read" as Option<Box<str>>],
        })
    }
}
