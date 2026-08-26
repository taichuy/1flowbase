extern crate self as storage_object;

mod driver;
pub mod drivers;
mod errors;
mod registry;
mod types;

pub use driver::FileStorageDriver;
pub use drivers::{local::LocalFileStorageDriver, rustfs::RustfsFileStorageDriver};
pub use errors::{FileStorageError, FileStorageResult};
pub use registry::{builtin_driver_registry, FileStorageDriverRegistry};
pub use types::{
    DeleteObjectInput, FileStorageHealthcheck, FileStorageObjectSnapshot, FileStoragePutInput,
    FileStoragePutResult, FileStoragePutStreamInput, FileStorageStreamReader,
    GenerateAccessUrlInput, OpenReadInput, OpenReadResult, OpenReadStreamResult,
    VerifyReadUnchangedInput, FILE_STORAGE_STREAM_BUFFER_BYTES,
};

pub fn crate_name() -> &'static str {
    "storage-object"
}

#[cfg(test)]
mod _tests;
