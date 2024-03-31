use reqwest::header::HeaderMap;
use smol_str::SmolStr;

use crate::models::{self, capabilities::B2CapabilitiesStringSet};

/// Identifier for a file to download, either by its file ID or file name.
///
/// Used in [`Client::download_file`](crate::Client::download_file).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DownloadFileBy<'a> {
    /// Download a file by its file ID.
    FileId(&'a str),

    /// Download a file by its file name.
    ///
    /// If a file has multiple versions, the most recent version will be downloaded.
    FileName(&'a str),
}

/// Parameters for creating a new application key.
#[derive(Default, Debug, Clone, typed_builder::TypedBuilder)]
#[builder(doc)]
pub struct CreateApplicationKey<'a> {
    /// The capabilities to grant to the new key.
    #[builder(default, setter(into))]
    pub capabilities: B2CapabilitiesStringSet,

    /// The name of the new key.
    pub key_name: &'a str,

    /// When provided, the key will expire after the given number of seconds,
    /// and will have `expirationTimestamp` set. Value must be a positive integer,
    /// and must be less than 1000 days (in seconds).
    #[builder(default, setter(into))]
    pub valid_duration_in_seconds: Option<u64>,

    /// When present, the new key can only access this bucket.
    ///
    /// When set, only these capabilities can be specified:
    /// `listAllBucketNames`, `listBuckets`, `readBuckets`, `readBucketEncryption`,
    /// `writeBucketEncryption`, `readBucketRetentions`, `writeBucketRetentions`,
    /// `listFiles`, `readFiles`, `shareFiles`, `writeFiles`, `deleteFiles`,
    /// `readFileLegalHolds`, `writeFileLegalHolds`, `readFileRetentions`,
    /// `writeFileRetentions`, and `bypassGovernance`.
    #[builder(default, setter(into))]
    pub bucket_id: Option<&'a str>,

    /// When present, restricts access to files whose names start with the prefix.
    ///
    /// You must set `bucketId` when setting this.
    #[builder(default, setter(into))]
    pub name_prefix: Option<&'a str>,
}

/// Parameters for listing B2 Buckets.
///
/// When using an authorization token that is restricted to a bucket,
/// you must include the `bucket_id` or `bucket_name` of that bucket in
/// the request, or the request will be denied.
///
/// Used in [`Client::list_buckets`](crate::Client::list_buckets).
#[derive(Default, Debug, Clone, typed_builder::TypedBuilder)]
#[builder(doc, mutators(
    /// Add a bucket type to the filter.
    pub fn bucket_type(&mut self, bucket_type: models::B2BucketType) {
        if !self.bucket_types.contains(&bucket_type) {
            self.bucket_types.push(bucket_type);
        }
    }

    /// Add multiple bucket types to the filter.
    pub fn bucket_types(&mut self, bucket_types: impl IntoIterator<Item = models::B2BucketType>) {
        for bucket_type in bucket_types {
            if !self.bucket_types.contains(&bucket_type) {
                self.bucket_types.push(bucket_type);
            }
        }
    }
))]
pub struct ListBuckets<'a> {
    /// When a bucket id is specified, the result will be a list containing just this bucket,
    /// if it's present in the account, or no buckets if the account does not have a bucket with this ID.
    #[builder(default, setter(into))]
    pub bucket_id: Option<&'a str>,

    /// When a bucket name is specified, the result will be a list containing just this bucket,
    /// if it's present in the account, or no buckets if the account does not have a bucket with this name.
    #[builder(default, setter(into))]
    pub bucket_name: Option<&'a str>,

    /// If present, B2 will use it as a filter for bucket types returned in the list buckets response.
    ///
    /// If not present, only buckets with bucket types [`allPublic`](models::B2BucketType::AllPublic),
    /// [`allPrivate`](models::B2BucketType::AllPrivate) and
    /// [`snapshot`](models::B2BucketType::Snapshot) will be returned.
    /// A special filter value of [`All`](models::B2BucketType::All) will return all bucket types.
    #[builder(default, via_mutators)]
    pub bucket_types: arrayvec::ArrayVec<models::B2BucketType, 6>, // 6 is the number of variants in B2BucketType
}

/// Parameters for creating a new bucket.
///
/// Used in [`Client::create_bucket`](crate::Client::create_bucket).
#[derive(Debug, typed_builder::TypedBuilder)]
#[builder(doc)]
pub struct CreateBucket<'a> {
    /// The name of the new bucket.
    pub bucket_name: &'a str,

    /// If true the bucket will be public, otherwise it will be private.
    #[builder(default)]
    pub public: bool,

    #[builder(default, setter(into))]
    pub bucket_info: Option<std::collections::HashMap<SmolStr, SmolStr>>,

    #[builder(default, setter(into))]
    pub cors_rules: Option<Vec<models::B2CorsRule>>,

    #[builder(default, setter(into))]
    pub default_retention: Option<&'a str>,

    #[builder(default, setter(into))]
    pub default_server_side_encryption: Option<sse::ServerSideEncryption>,

    #[builder(default, setter(into))]
    pub lifecycle_rules: Option<Vec<models::B2LifecycleRule>>,

    #[builder(default, setter(into))]
    pub replication: Option<models::B2ReplicationConfiguration>,

    /// If present, the Boolean value specifies whether the bucket has Object Lock enabled.
    ///
    /// Once Object Lock is enabled on a bucket, it cannot be disabled.
    #[builder(default, setter(into))]
    pub file_lock_enabled: Option<bool>,
}

#[derive(Debug, typed_builder::TypedBuilder)]
#[builder(doc)]
pub struct UpdateBucket<'a> {
    #[builder(default, setter(into))]
    pub if_revision_is: Option<u64>,

    pub bucket_id: &'a str,

    #[builder(default, setter(into))]
    pub bucket_type: Option<models::B2BucketType>,

    #[builder(default, setter(into))]
    pub bucket_info: Option<std::collections::HashMap<SmolStr, SmolStr>>,

    #[builder(default, setter(into))]
    pub cors_rules: Option<Vec<models::B2CorsRule>>,

    #[builder(default, setter(into))]
    pub default_retention: Option<&'a str>,

    #[builder(default, setter(into))]
    pub default_server_side_encryption: Option<sse::ServerSideEncryption>,

    #[builder(default, setter(into))]
    pub default_lifetime: Option<models::B2LifecycleRule>,

    /// If present, the Boolean value specifies whether the bucket has Object Lock enabled.
    ///
    /// Once Object Lock is enabled on a bucket, it cannot be disabled.
    ///
    /// A value of true will be accepted if you have `writeBucketRetentions` capability.
    /// But you cannot enable Object Lock on a restricted bucket (e.g. share buckets, snapshot)
    /// or on a bucket that contains source replication configuration.
    #[builder(default, setter(into))]
    pub file_lock_enabled: Option<bool>,
}

/// Parameters for listing files in a bucket.
///
/// Used in [`Client::list_files`](crate::Client::list_files).
#[derive(Default, Debug, Clone, Copy, Serialize, typed_builder::TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(doc)]
pub struct ListFiles<'a> {
    /// If `true`, list all versions of all files in the bucket.
    /// If `false`, list only the most recent versions of each file.
    #[serde(skip_serializing)]
    #[builder(default)]
    pub all_versions: bool,

    /// The ID of the bucket to list files in. If `None`, the client's default bucket will be used.
    #[builder(default, setter(into))]
    pub bucket_id: Option<&'a str>,

    /// The first file name to return. If `None`, the list will start at the beginning.
    #[builder(default, setter(into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_file_name: Option<&'a str>,

    /// The first file ID to return. If `None`, the list will start at the beginning.
    ///
    /// Only used if `all_versions` is `true`.
    #[builder(default, setter(into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_file_id: Option<&'a str>,

    /// The maximum number of files to return. If `None`, the maximum is 1000.
    ///
    /// If you set `max_file_count` to more than 1000 and more than 1000 are returned,
    /// the call will be billed as multiple transactions,
    /// as if you had made requests in a loop asking for 1000 at a time.
    #[builder(default, setter(into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_count: Option<usize>,

    /// The prefix to filter files by. If `None`, no prefix will be used.
    #[builder(default, setter(into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<&'a str>,

    /// The delimiter to use when filtering files. If `None`, no delimiter will be used.
    #[builder(default, setter(into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<&'a str>,
}

/// New file retention settings to apply to a file.
///
/// You can use [`FileRetention::builder`] or the other
/// provided methods to create a new `FileRetention` instance.
#[derive(Default, Debug, Clone, Serialize, typed_builder::TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(doc)]
pub struct FileRetention {
    /// The retention mode to use for the file.
    #[builder(default, setter(into))]
    pub mode: Option<models::B2FileRetentionMode>,

    /// Point at which the file will be unlocked and can be deleted.
    ///
    /// In millieconds since the Unix epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain_until_timestamp: Option<u64>,

    /// Bypasses governance mode retention settings to set the new retention settings.
    #[serde(skip_serializing)]
    pub bypass_governance: bool,
}

impl FileRetention {
    /// Creates a new `FileRetention` with the given retention mode.
    pub const fn new(mode: Option<models::B2FileRetentionMode>) -> Self {
        Self {
            mode,
            retain_until_timestamp: None,
            bypass_governance: false,
        }
    }

    /// Creates a new `FileRetention` with no retention settings.
    pub const fn none() -> Self {
        Self::new(None)
    }

    /// Creates a new `FileRetention` with [`Compliance`](models::B2FileRetentionMode::Compliance) retention settings.
    pub const fn compliance() -> Self {
        Self::new(Some(models::B2FileRetentionMode::Compliance))
    }

    /// Creates a new `FileRetention` with [`Governance`](models::B2FileRetentionMode::Governance) retention settings.
    pub const fn governance() -> Self {
        Self::new(Some(models::B2FileRetentionMode::Governance))
    }

    /// Bypasses governance mode retention settings to set the new retention settings.
    pub const fn bypass_governance(mut self) -> Self {
        self.bypass_governance = true;
        self
    }

    /// Sets the point at which the file will be unlocked and can be deleted.
    ///
    /// In millieconds since the Unix epoch.
    pub const fn retain_until_timestamp(mut self, timestamp: Option<u64>) -> Self {
        self.retain_until_timestamp = timestamp;
        self
    }
}

/// Info about a new whole file to be uploaded.
///
/// See the documentation for [`NewFileInfo::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
#[builder(doc, mutators(
    /// Sets the SSE-C encryption type with the given key.
    pub fn encrypt_custom_aes256(&mut self, key: &[u8]) {
        self.encryption = sse::ServerSideEncryption::customer_aes256(key);
    }

    pub fn encryption(&mut self, encryption: impl Into<sse::ServerSideEncryption>) {
        self.encryption = encryption.into();
    }
))]
pub struct NewFileInfo<'a> {
    /// The name of the new file.
    pub file_name: &'a str,

    /// The length of the file in bytes.
    pub content_length: u64,

    /// The MIME type of the file.
    ///
    /// Will default to `application/octet-stream` if not provided,
    /// or if specified to be `bz/x-auto` then the B2 API will attempt to
    /// determine the file's content type automatically.
    #[builder(default, setter(into))]
    pub content_type: Option<&'a str>,

    /// The SHA1 hash of the file's contents as a hex string.
    pub content_sha1: &'a str,

    /// The server-side encryption to use when uploading the file.
    #[builder(default, via_mutators)]
    pub encryption: sse::ServerSideEncryption,

    /// The file retention settings to apply to the file.
    #[builder(default, setter(into))]
    pub retention: Option<FileRetention>,

    /// Whether to apply a legal hold to the file.
    #[builder(default)]
    pub legal_hold: Option<bool>,
}

/// Info about a new large file to be uploaded.
///
/// This omits the `content_length`, `content_sha1` and `encryption` fields,
/// due to the file being uploaded in parts. Each part will have its own `content_length`,
/// `content_sha1` and `encryption` fields.
///
/// See the documentation for [`NewLargeFileInfo::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
#[builder(doc, mutators(
    /// Sets the SSE-C encryption type with the given key.
    pub fn encrypt_custom_aes256(&mut self, key: &[u8]) {
        self.encryption = sse::ServerSideEncryption::customer_aes256(key);
    }

    pub fn encryption(&mut self, encryption: impl Into<sse::ServerSideEncryption>) {
        self.encryption = encryption.into();
    }
))]
pub struct NewLargeFileInfo<'a> {
    /// The name of the new file.
    pub file_name: &'a str,

    /// The MIME type of the file.
    ///
    /// Will default to `application/octet-stream` if not provided,
    /// or if specified to be `bz/x-auto` then the B2 API will attempt to
    /// determine the file's content type automatically.
    #[builder(default, setter(into))]
    pub content_type: Option<&'a str>,

    /// The server-side encryption to use when uploading the file.
    #[builder(default, via_mutators)]
    pub encryption: sse::ServerSideEncryption,

    /// The file retention settings to apply to the file.
    #[builder(default, setter(into))]
    pub retention: Option<FileRetention>,

    /// Whether to apply a legal hold to the file.
    #[builder(default)]
    pub legal_hold: Option<bool>,
}

/// Info about a new part of a large file to be uploaded.
///
/// See the documentation for [`NewPartInfo::builder`] for more information.
#[derive(Debug, typed_builder::TypedBuilder)]
#[builder(doc, mutators(
    /// Sets the SSE-C encryption type with the given key.
    pub fn encrypt_custom_aes256(&mut self, key: &[u8]) {
        self.encryption = sse::ServerSideEncryption::customer_aes256(key);
    }

    pub fn encryption(&mut self, encryption: impl Into<sse::ServerSideEncryption>) {
        self.encryption = encryption.into();
    }
))]
pub struct NewPartInfo<'a> {
    /// The part number of the new large file part.
    #[builder(setter(into))]
    pub part_number: std::num::NonZeroU32,

    /// The length of the part in bytes.
    pub content_length: u64,

    /// The SHA1 hash of the part's contents as a hex string.
    pub content_sha1: &'a str,

    /// The server-side encryption to use when uploading the file.
    #[builder(default, via_mutators)]
    pub encryption: sse::ServerSideEncryption,
}

impl NewFileInfo<'_> {
    pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-file-name" => &self.file_name);
        h!(headers."content-type" => self.content_type.unwrap_or("application/octet-stream"));
        h!(headers."content-length" => &self.content_length.to_string());
        h!(headers."x-bz-content-sha1" => self.content_sha1);

        if let Some(ref retention) = self.retention {
            if let Some(ref mode) = retention.mode {
                h!(headers."x-bz-file-retention-mode" => mode.as_ref());

                if let Some(timestamp) = retention.retain_until_timestamp {
                    h!(headers."x-bz-file-retention-retain-until-timestamp" => &timestamp.to_string());
                }
            }
        }

        if let Some(legal_hold) = self.legal_hold {
            h!(headers."x-bz-file-legal-hold" => if legal_hold { "on" } else { "off" });
        }

        self.encryption.add_headers(headers);
    }
}

impl NewPartInfo<'_> {
    pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-part-number" => &self.part_number.to_string());
        h!(headers."content-length" => &self.content_length.to_string());
        h!(headers."x-bz-content-sha1" => self.content_sha1);

        self.encryption.add_headers(headers);
    }
}

/// Server-Side Encryption (SSE) types and utilities
///
/// This module provides types and utilities for working with server-side encryption (SSE) in the B2 API.
///
/// The B2 API supports two types of server-side encryption:
/// - SSE-B2: encryption provided by Backblaze
/// - SSE-C: encryption provided by the client
///
/// SSE-B2 is the default encryption type, and is used when no encryption type is specified.
/// SSE-C is used when the client provides an encryption key and the necessary headers.
///
/// The types in this module are used to specify the encryption type and provide the necessary headers for SSE-C.
pub mod sse {
    use std::borrow::Cow;

    use reqwest::header::HeaderMap;

    /// Server-Side Encryption (SSE) with a customer-provided key (SSE-C)
    #[derive(Debug, Clone, Serialize)]
    pub struct ServerSideEncryptionCustomer {
        /// The algorithm to use when encrypting/decrypting a file using SSE-C encryption.
        ///
        /// The only currently supported value is `"AES256"`.
        pub algorithm: Cow<'static, str>,

        /// The base64-encoded AES256 encryption key when encrypting/decrypting a file using SSE-C encryption.
        pub key: String,

        /// The base64-encoded MD5 digest of the [`key`](ServerSideEncryptionCustomer::key) when encrypting/decrypting a file using SSE-C encryption.
        pub key_md5: String,
    }

    impl ServerSideEncryptionCustomer {
        /// Creates a new `ServerSideEncryptionCustomer` with the given key,
        /// automatically computing the MD5 digest and encoding.
        pub fn aes256(key: &[u8]) -> Self {
            use base64::{engine::general_purpose::STANDARD, Engine as _};
            use md5::{Digest, Md5};

            Self {
                algorithm: Cow::Borrowed("AES256"),
                key: STANDARD.encode(key),
                key_md5: STANDARD.encode(Md5::new().chain_update(key).finalize()),
            }
        }
    }

    /// Server-Side Encryption (SSE) types
    ///
    /// Implements various `From` conversions for easy construction from other types.
    #[derive(Default, Debug, Clone, Serialize)]
    #[serde(tag = "mode")]
    pub enum ServerSideEncryption {
        /// The default encryption type of the bucket, which will either be SSE-B2 or no encryption.
        #[default]
        Default,

        /// SSE-B2 encryption, the default encryption type.
        #[serde(rename = "SSE-B2")]
        Standard {
            /// The algorithm to use when encrypting/decrypting a file using SSE-B2 encryption.
            ///
            /// The only currently supported value is `"AES256"`, which can be
            /// easily constructed using [`ServerSideEncryption::standard_aes256`].
            algorithm: Cow<'static, str>,
        },

        /// SSE-C encryption, allowing the client to provide an encryption key.
        ///
        /// This variant can be easily constructed using [`ServerSideEncryption::customer_aes256`].
        #[serde(rename = "SSE-C")]
        Customer(ServerSideEncryptionCustomer),
    }

    impl ServerSideEncryption {
        /// Creates a new `ServerSideEncryption` using the default encryption type of the bucket,
        /// which will either be SSE-B2 or no encryption.
        pub const fn default() -> Self {
            Self::Default
        }

        /// Creates a new `ServerSideEncryption` with the default SSE-B2 encryption algorithm of AES256.
        pub const fn standard_aes256() -> Self {
            Self::Standard {
                algorithm: Cow::Borrowed("AES256"),
            }
        }

        /// Creates a new `ServerSideEncryption` with the given SSE-C encryption key for the AES256 algorithm.
        pub fn customer_aes256(key: &[u8]) -> Self {
            Self::Customer(ServerSideEncryptionCustomer::aes256(key))
        }

        /// Returns `true` if the encryption type is `Default`.
        pub const fn is_default(&self) -> bool {
            matches!(self, Self::Default)
        }

        /// Returns `true` if the encryption type is `Standard`.
        pub const fn is_standard(&self) -> bool {
            matches!(self, Self::Standard { .. })
        }

        /// Returns `true` if the encryption type is `Customer`.
        pub const fn is_customer(&self) -> bool {
            matches!(self, Self::Customer { .. })
        }
    }

    impl From<ServerSideEncryptionCustomer> for ServerSideEncryption {
        fn from(sse_c: ServerSideEncryptionCustomer) -> Self {
            ServerSideEncryption::Customer(sse_c)
        }
    }

    impl From<Option<ServerSideEncryptionCustomer>> for ServerSideEncryption {
        fn from(sse_c: Option<ServerSideEncryptionCustomer>) -> Self {
            sse_c.map_or(ServerSideEncryption::Default, ServerSideEncryption::Customer)
        }
    }

    impl From<Option<ServerSideEncryption>> for ServerSideEncryption {
        fn from(sse: Option<ServerSideEncryption>) -> Self {
            sse.unwrap_or(ServerSideEncryption::Default)
        }
    }

    impl From<()> for ServerSideEncryption {
        fn from(_: ()) -> Self {
            ServerSideEncryption::Default
        }
    }

    impl ServerSideEncryptionCustomer {
        pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
            h!(headers."x-bz-server-side-encryption-customer-algorithm" => &self.algorithm);
            h!(headers."x-bz-server-side-encryption-customer-key" => &self.key);
            h!(headers."x-bz-server-side-encryption-customer-key-md5" => &self.key_md5);
        }
    }

    impl ServerSideEncryption {
        pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
            match self {
                ServerSideEncryption::Default => {}
                ServerSideEncryption::Standard { algorithm } => {
                    h!(headers."x-bz-server-side-encryption" => algorithm);
                }
                ServerSideEncryption::Customer(sse_c) => sse_c.add_headers(headers),
            }
        }
    }
}
