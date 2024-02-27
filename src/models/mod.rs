//! Models for the B2 API.
//!
//! These types are largely read-only.

use reqwest::header::{HeaderMap, HeaderValue};
use smol_str::SmolStr;
use std::{collections::HashMap, sync::Arc};

pub mod capabilities;

use capabilities::{B2CapabilitiesStringSet, B2Capability};

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
    pub account_id: SmolStr,

    /// An authorization token to use with all calls, other than b2_authorize_account,
    /// that need an `authorization` header.
    ///
    /// **This authorization token is valid for at most 24 hours.**
    #[serde(alias = "authorizationToken")]
    pub auth_token: SmolStr,

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
    /// The URL to use for API calls.
    pub api_url: SmolStr,
    pub download_url: SmolStr,

    pub recommended_part_size: u64,
    pub absolute_minimum_part_size: u64,
    pub s3_api_url: SmolStr,

    #[serde(default)]
    pub capabilities: B2CapabilitiesStringSet,

    /// When present, access is restricted to one bucket.
    pub bucket_id: Option<SmolStr>,

    /// When `bucketId`` is set, and it is a valid bucket that has not been deleted,
    /// this field is set to the name of the bucket.
    ///
    /// It's possible that bucketId is set to a bucket that no longer exists,
    /// in which case this field will be null. It's also null when `bucketId`` is null.
    pub bucket_name: Option<SmolStr>,

    /// When present, access is restricted to files whose names start with the prefix.
    pub name_prefix: Option<Arc<str>>,
}

impl B2StorageApi {
    /// Checks if the storage API has the capability to perform the given action.
    pub fn contains(&self, capability: B2Capability) -> bool {
        self.capabilities.contains(capability)
    }
}

impl B2Authorized {
    /// Checks if the authorized account has the capability to perform the given action.
    pub fn allowed(&self, capability: B2Capability) -> bool {
        self.api.storage.contains(capability)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ApplicationKey {
    /// The account that this application key is for.
    pub account_id: SmolStr,

    /// The name assigned when the key was created.
    pub key_name: SmolStr,

    /// The ID of the application key.
    pub application_key_id: SmolStr,

    #[serde(default)]
    pub bucket_id: Option<SmolStr>,

    /// A list of strings, each one naming a capability the key has.
    #[serde(default)]
    pub capabilities: B2CapabilitiesStringSet,

    /// When present, restricts access to files whose names start with the prefix.
    #[serde(default)]
    pub name_prefix: Option<SmolStr>,

    /// When present and set to s3, the key can be used to sign requests to the S3 Compatible API.
    #[serde(default)]
    pub options: Vec<SmolStr>,

    /// When present, says when this key will expire, in milliseconds since 1970.
    #[serde(default = "u64::max_value")]
    pub expiration_timestamp: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ListedApplicationKey {
    pub keys: Vec<B2ApplicationKey>,

    /// Set if there are more keys beyond the ones that were returned. Pass this value
    /// as `startApplicationKeyId` in the next query to continue listing keys.
    ///
    /// Note that this value may not be a valid application key ID,
    /// but can still be used as the starting point for the next query.
    #[serde(default)]
    pub next_application_key_id: Option<SmolStr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum B2BucketType {
    All,
    AllPublic,
    AllPrivate,
    Restricted,
    Shared,
    Snapshot,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2Bucket {
    /// The account that the bucket is in.
    pub account_id: SmolStr,

    /// The unique ID of the bucket.
    pub bucket_id: SmolStr,

    /// The unique name of the bucket
    pub bucket_name: SmolStr,
    pub bucket_type: B2BucketType,

    /// A counter that is updated every time the bucket is modified,
    /// and can be used with the `ifRevisionIs` parameter to
    /// `b2_update_bucket` to prevent colliding, simultaneous updates.
    pub revision: u64,

    #[serde(default)]
    pub bucket_info: HashMap<SmolStr, SmolStr>,

    #[serde(default)]
    pub cors_rules: Vec<B2CorsRule>,

    #[serde(default)]
    pub lifecycle_rules: Vec<B2LifecycleRule>,

    /// When present and set to s3, the bucket can be accessed through the S3 Compatible API.
    #[serde(default)]
    pub options: Vec<SmolStr>,

    #[serde(default)]
    pub replication_configuration: Option<B2ReplicationConfiguration>,
    pub default_server_side_encryption: B2ServerSideEncryption,
    pub file_lock_configuration: B2FileLockConfiguration,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileLockConfiguration {
    pub is_client_authorized_to_read: bool,
    #[serde(default)]
    pub value: Option<B2FileLockConfigurationValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2FileLockConfigurationValue {
    pub default_retention: B2FileRetention,
    pub is_file_lock_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2DefaultServerSideEncryption {
    pub is_client_authorized_to_read: bool,
    #[serde(default)]
    pub value: Option<B2ServerSideEncryption>,
}

/// See [CORS Rules](https://www.backblaze.com/docs/cloud-storage-cross-origin-resource-sharing-rules) for more information.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2CorsRule {
    pub cors_rule_name: SmolStr,

    #[serde(default)]
    pub allowed_origins: Vec<SmolStr>,
    #[serde(default)]
    pub allowed_operations: Vec<SmolStr>,
    #[serde(default)]
    pub allowed_headers: Vec<SmolStr>,
    #[serde(default)]
    pub expose_headers: Vec<SmolStr>,
    #[serde(default)]
    pub max_age_seconds: u64,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct B2ReplicationConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_replication_source: Option<B2ReplicationSourceArray>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_replication_destination: Option<B2ReplicationDestination>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ReplicationSourceArray {
    pub replication_rules: Vec<B2ReplicationSource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ReplicationSource {
    pub destination_bucket_id: SmolStr,
    pub file_name_prefix: SmolStr,
    pub include_existing_files: bool,
    pub is_enabled: bool,
    pub priority: u64,
    pub replication_rule_name: SmolStr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2ReplicationDestination {
    pub source_to_destination_key_mapping: SmolStr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct B2LifecycleRule {
    pub days_from_hiding_to_deleting: u64,
    pub days_from_uploading_to_hiding: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name_prefix: Option<SmolStr>,
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
    pub bucket_id: Option<SmolStr>,

    /// The identifier for the file, if doing a large file upload.
    #[serde(default)]
    pub file_id: Option<SmolStr>,

    /// The URL that can be used to upload files to this bucket, see b2_upload_file.
    pub upload_url: SmolStr,

    /// The authorization token that must be used when uploading files to this bucket.
    ///
    /// This token is valid for 24 hours or until the uploadUrl endpoint rejects an upload,
    /// see b2_upload_file
    pub authorization_token: SmolStr,
}

impl B2UploadUrl {
    pub fn header(&self) -> HeaderValue {
        HeaderValue::from_str(&self.authorization_token).expect("Unable to use auth token in header value")
    }
}

#[derive(Debug)]
pub enum B2FileEncryptionHeaders {
    B2 { algorithm: SmolStr },
    Customer { algorithm: SmolStr, key_md5: SmolStr },
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

/// The state of a file.
///
/// The B2 API describes these are "actions", but they are more like states,
/// with the real values of "start", "upload", "hide", and "folder".
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

/// The state of a file's replication.
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
    pub value: Option<SmolStr>,
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
    pub algorithm: Option<SmolStr>,

    #[serde(default)]
    pub mode: Option<SmolStr>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct B2FileInfo {
    pub account_id: Option<SmolStr>,
    pub file_id: SmolStr,
    pub file_name: SmolStr,
    pub action: Option<B2FileAction>,
    pub bucket_id: SmolStr,
    pub content_length: u64,
    pub content_sha1: Option<SmolStr>,
    pub content_type: Option<SmolStr>,
    pub file_info: HashMap<SmolStr, SmolStr>,
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
    pub next_file_name: Option<SmolStr>,

    /// The ID of the next file, if there are more files to list.
    #[serde(default)]
    pub next_file_id: Option<SmolStr>,
}

/// Response from `b2_cancel_large_file`
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2CancelledFileInfo {
    pub file_id: SmolStr,
    pub file_name: SmolStr,
    pub bucket_id: SmolStr,
    pub account_id: SmolStr,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct B2PartInfo {
    pub file_id: SmolStr,
    pub part_number: u64,
    pub content_length: u64,
    pub content_sha1: SmolStr,

    #[serde(default)]
    pub server_side_encryption: B2ServerSideEncryption,

    pub upload_timestamp: u64,
}

use headers::{CacheControl, ContentDisposition, ContentLength, ContentType, Expires, HeaderMapExt};

pub struct B2FileHeaders {
    pub content_length: ContentLength,
    pub content_type: ContentType,
    pub file_id: SmolStr,
    pub file_name: SmolStr,
    pub file_sha1: SmolStr,
    pub info: HeaderMap,
    pub upload_timestamp: u64,

    pub content_disposition: Option<ContentDisposition>,
    pub content_language: Option<SmolStr>,
    pub expires: Option<Expires>,
    pub cache_control: Option<CacheControl>,
    pub encryption: Option<B2FileEncryptionHeaders>,

    pub retention_mode: Option<B2FileRetentionMode>,
    pub retain_until: Option<u64>,
    pub legal_hold: Option<bool>,
    pub unauthorized_to_read: Option<SmolStr>,
}

use crate::error::B2FileHeaderError;

impl B2FileHeaders {
    pub(crate) fn parse(headers: &HeaderMap) -> Result<B2FileHeaders, B2FileHeaderError> {
        #[rustfmt::skip] macro_rules! p {
            [@$key:literal] => { headers.typed_get().ok_or(B2FileHeaderError::MissingHeader($key))? };
            [$key:literal] => { headers.get($key).ok_or(B2FileHeaderError::MissingHeader($key))? };
            [$key:literal as str] => { p![$key].to_str()? };
            [$key:literal as SmolStr] => { SmolStr::from(p![$key].to_str()?) };
            [$key:literal as Option<SmolStr>] => { headers.get($key).map(|h| h.to_str().map(SmolStr::from)).transpose()? };
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
            file_id: p!["x-bz-file-id" as SmolStr],
            file_name: p!["x-bz-file-name" as SmolStr],
            file_sha1: p!["x-bz-content-sha1" as SmolStr],
            info,
            upload_timestamp: p!["x-bz-upload-timestamp" as str].parse()?,
            content_disposition: headers.typed_get(),
            content_language: p!["content-language" as Option<SmolStr>],
            expires: headers.typed_get(),
            cache_control: headers.typed_get(),

            encryption: match p!["x-bz-server-side-encryption" as Option<SmolStr>] {
                Some(algorithm) => Some(B2FileEncryptionHeaders::B2 { algorithm }),
                None => match p!["x-bz-server-side-encryption-customer-algorithm" as Option<SmolStr>] {
                    Some(algorithm) => Some(B2FileEncryptionHeaders::Customer {
                        algorithm,
                        key_md5: p!["x-bz-server-side-encryption-customer-key-md5" as SmolStr],
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
            unauthorized_to_read: p!["x-bz-client-unauthorized-to-read" as Option<SmolStr>],
        })
    }
}
