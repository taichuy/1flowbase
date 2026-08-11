use async_trait::async_trait;

use crate::{
    errors::FileStorageResult,
    types::{
        DeleteObjectInput, FileStorageHealthcheck, FileStoragePutInput, FileStoragePutResult,
        FileStoragePutStreamInput, GenerateAccessUrlInput, OpenReadInput, OpenReadResult,
        OpenReadStreamResult, VerifyReadUnchangedInput,
    },
};

#[async_trait]
pub trait FileStorageDriver: Send + Sync {
    fn driver_type(&self) -> &'static str;

    fn validate_config(&self, config_json: &serde_json::Value) -> FileStorageResult<()>;

    async fn healthcheck(
        &self,
        config_json: &serde_json::Value,
    ) -> FileStorageResult<FileStorageHealthcheck>;

    async fn put_object(
        &self,
        input: FileStoragePutInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult>;

    async fn put_object_stream(
        &self,
        input: FileStoragePutStreamInput<'_>,
    ) -> FileStorageResult<FileStoragePutResult>;

    async fn delete_object(&self, input: DeleteObjectInput<'_>) -> FileStorageResult<()>;

    async fn open_read(&self, input: OpenReadInput<'_>) -> FileStorageResult<OpenReadResult>;

    async fn open_read_stream(
        &self,
        input: OpenReadInput<'_>,
    ) -> FileStorageResult<OpenReadStreamResult>;

    async fn verify_read_unchanged(
        &self,
        input: VerifyReadUnchangedInput<'_>,
    ) -> FileStorageResult<()>;

    async fn generate_access_url(
        &self,
        input: GenerateAccessUrlInput<'_>,
    ) -> FileStorageResult<Option<String>>;
}
