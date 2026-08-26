use thiserror::Error;

pub type FileStorageResult<T> = Result<T, FileStorageError>;

#[derive(Debug, Error)]
pub enum FileStorageError {
    #[error("unsupported file storage driver: {0}")]
    UnsupportedDriver(String),
    #[error("invalid file storage config: {0}")]
    InvalidConfig(&'static str),
    #[error("object not found")]
    ObjectNotFound,
    #[error("object changed while it was being read")]
    ObjectChanged,
    #[error("object length does not match the declared stream length")]
    ObjectLengthMismatch,
    #[error("object storage did not provide a stable read validator")]
    ObjectSnapshotUnavailable,
    #[error("object is too large for the fixed multipart stream contract")]
    ObjectTooLarge,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl FileStorageError {
    pub fn unsupported_driver(driver_type: impl Into<String>) -> Self {
        Self::UnsupportedDriver(driver_type.into())
    }
}
