use std::pin::Pin;

use tokio::io::AsyncRead;

/// The largest buffer requested by the portable object-stream contract.
///
/// Drivers with protocol minimums (for example S3 multipart upload) may keep one fixed protocol
/// chunk in addition to this copy buffer, but neither allocation grows with the object size.
pub const FILE_STORAGE_STREAM_BUFFER_BYTES: usize = 256 * 1024;

pub type FileStorageStreamReader = Pin<Box<dyn AsyncRead + Send + Unpin + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStorageHealthcheck {
    pub reachable: bool,
    pub detail: Option<String>,
}

pub struct FileStoragePutInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
    pub content_type: Option<&'a str>,
    pub bytes: &'a [u8],
}

pub struct FileStoragePutStreamInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
    pub content_type: Option<&'a str>,
    pub content_length: u64,
    pub reader: FileStorageStreamReader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStoragePutResult {
    pub path: String,
    pub url: Option<String>,
    pub metadata_json: serde_json::Value,
}

pub struct DeleteObjectInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
}

pub struct OpenReadInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStorageObjectSnapshot {
    pub content_length: u64,
    pub validator: String,
}

pub struct OpenReadStreamResult {
    pub reader: FileStorageStreamReader,
    pub content_type: Option<String>,
    pub snapshot: FileStorageObjectSnapshot,
}

pub struct VerifyReadUnchangedInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
    pub snapshot: &'a FileStorageObjectSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReadResult {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

pub struct GenerateAccessUrlInput<'a> {
    pub config_json: &'a serde_json::Value,
    pub object_path: &'a str,
}
