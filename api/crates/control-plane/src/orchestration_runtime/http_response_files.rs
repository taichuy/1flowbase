use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use orchestration_runtime::execution_engine::{
    HttpResponseFilePersistInput, HttpResponseFilePersister,
};

use super::OrchestrationRuntimeService;
use crate::{
    file_management::{FileUploadService, UploadFileCommand},
    ports::{FileManagementRepository, ModelDefinitionRepository},
};

const DEFAULT_HTTP_RESPONSE_FILE_TABLE_CODE: &str = "attachments";

pub(super) struct RuntimeHttpResponseFilePersister<R> {
    repository: R,
    registry: Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    actor: domain::ActorContext,
}

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: FileManagementRepository + ModelDefinitionRepository + Clone + Send + Sync + 'static,
{
    pub(super) fn http_response_file_persister(
        &self,
        actor: domain::ActorContext,
    ) -> Option<RuntimeHttpResponseFilePersister<R>> {
        self.file_storage_registry
            .as_ref()
            .map(|registry| RuntimeHttpResponseFilePersister {
                repository: self.repository.clone(),
                registry: registry.clone(),
                runtime_engine: self.runtime_engine.clone(),
                actor,
            })
    }
}

#[async_trait]
impl<R> HttpResponseFilePersister for RuntimeHttpResponseFilePersister<R>
where
    R: FileManagementRepository + ModelDefinitionRepository + Clone + Send + Sync + 'static,
{
    async fn persist_http_response_file(
        &self,
        input: HttpResponseFilePersistInput<'_>,
    ) -> Result<serde_json::Value> {
        let file_table = self
            .repository
            .find_file_table_by_code(DEFAULT_HTTP_RESPONSE_FILE_TABLE_CODE)
            .await?
            .ok_or_else(|| anyhow!("default HTTP response file table not found"))?;
        let uploaded = FileUploadService::new(
            self.repository.clone(),
            self.registry.clone(),
            self.runtime_engine.clone(),
        )
        .upload(UploadFileCommand {
            actor: self.actor.clone(),
            target: crate::file_management::FileUploadTarget::Table(file_table.id),
            original_filename: input.filename.to_string(),
            content_type: Some(input.content_type.to_string()),
            bytes: input.bytes.to_vec(),
        })
        .await?;
        let mut record = uploaded.record;

        if let Some(object) = record.as_object_mut() {
            let meta = object
                .entry("meta")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(meta_object) = meta.as_object_mut() {
                meta_object.insert(
                    "source".to_string(),
                    serde_json::json!("http_request_response"),
                );
                meta_object.insert("node_id".to_string(), serde_json::json!(input.node_id));
            }
        }

        Ok(record)
    }
}
