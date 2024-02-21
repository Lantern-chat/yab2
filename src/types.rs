use reqwest::header::HeaderMap;

use crate::models;

/// Identifier for a file to download, either by its file ID or file name.
///
/// Used in [`Client::download_file`].
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

/// Parameters for listing files in a bucket.
///
/// Used in [`Client::list_files`].
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
pub struct NewFileInfo {
    /// The name of the new file.
    #[builder(setter(into))]
    pub file_name: String,

    /// The length of the file in bytes.
    pub content_length: u64,

    /// The MIME type of the file.
    ///
    /// Will default to `application/octet-stream` if not provided,
    /// or if specified to be `bz/x-auto` then the B2 API will attempt to
    /// determine the file's content type automatically.
    #[builder(default, setter(into))]
    pub content_type: Option<String>,

    /// The SHA1 hash of the file's contents as a hex string.
    #[builder(setter(into))]
    pub content_sha1: String,

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
pub struct NewLargeFileInfo {
    /// The name of the new file.
    #[builder(setter(into))]
    pub file_name: String,

    /// The MIME type of the file.
    ///
    /// Will default to `application/octet-stream` if not provided,
    /// or if specified to be `bz/x-auto` then the B2 API will attempt to
    /// determine the file's content type automatically.
    #[builder(default, setter(into))]
    pub content_type: Option<String>,

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
pub struct NewPartInfo {
    /// The part number of the new large file part.
    #[builder(setter(into))]
    pub part_number: std::num::NonZeroU32,

    /// The length of the part in bytes.
    pub content_length: u64,

    /// The SHA1 hash of the part's contents as a hex string.
    #[builder(setter(into))]
    pub content_sha1: String,

    /// The server-side encryption to use when uploading the file.
    #[builder(default, via_mutators)]
    pub encryption: sse::ServerSideEncryption,
}

impl NewFileInfo {
    pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-file-name" => &self.file_name);
        h!(headers."content-type" => self.content_type.as_deref().unwrap_or("application/octet-stream"));
        h!(headers."content-length" => &self.content_length.to_string());
        h!(headers."x-bz-content-sha1" => &self.content_sha1);

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

impl NewPartInfo {
    pub(crate) fn add_headers(&self, headers: &mut HeaderMap) {
        h!(headers."x-bz-part-number" => &self.part_number.to_string());
        h!(headers."content-length" => &self.content_length.to_string());
        h!(headers."x-bz-content-sha1" => &self.content_sha1);

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
