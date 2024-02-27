//! Error Handling types for the B2 API.

use crate::models::capabilities::B2Capability;

/// The B2 API returns errors in a JSON format. This struct represents that format.
#[derive(Debug, Deserialize)]
pub struct B2ErrorMessage {
    /// The HTTP status code.
    pub status: u16,
    /// The B2 error code.
    pub code: String,
    /// The error message.
    pub message: String,
}

impl std::fmt::Display for B2ErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.status, self.code, self.message)
    }
}

impl std::error::Error for B2ErrorMessage {}

#[derive(Debug, thiserror::Error)]
pub enum B2Error {
    /// The B2 API returned an error.
    #[error("B2 Error Message: {0:?}")]
    B2ErrorMessage(#[from] B2ErrorMessage),

    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Reqwest Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Serde JSON Error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Unknown")]
    Unknown,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("B2 File Header Error: {0}")]
    B2FileHeaderError(#[from] B2FileHeaderError),

    #[error("Missing Bucket ID")]
    MissingBucketId,

    #[error("Invalid Part Sorting")]
    InvalidPartSorting,

    #[error("Missing Capability: {0:?}")]
    MissingCapability(B2Capability),

    #[error("Invalid Capability: {0:?}")]
    InvalidCapability(B2Capability),

    #[error("Missing File Name")]
    MissingFileName,

    #[error("Invalid File Id For Upload Url")]
    FileIdMismatch,

    #[error("Invalid/Mismatched Prefix")]
    InvalidPrefix,
}

#[derive(Debug, thiserror::Error)]
pub enum B2FileHeaderError {
    #[error("Missing Header: {0}")]
    MissingHeader(&'static str),

    #[error("Integer Parse Error: {0}")]
    IntegerParseError(#[from] std::num::ParseIntError),

    #[error("Bool Parse Error")]
    BoolParseError,

    #[error("String error: {0}")]
    ToStrError(#[from] reqwest::header::ToStrError),

    #[error("Invalid Timestamp")]
    InvalidTimestamp,

    #[error("Invalid Retention Mode")]
    InvalidRetentionMode,
}
