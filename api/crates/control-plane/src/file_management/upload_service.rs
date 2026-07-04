use anyhow::Result;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{FileManagementRepository, ModelDefinitionRepository},
};

pub struct UploadFileCommand {
    pub actor: domain::ActorContext,
    pub file_table_id: Uuid,
    pub original_filename: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub struct UploadedFileView {
    pub record: serde_json::Value,
    pub storage_id: Uuid,
}

pub struct FileUploadService<R> {
    repository: R,
    registry: std::sync::Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: std::sync::Arc<runtime_core::runtime_engine::RuntimeEngine>,
}

impl<R> FileUploadService<R>
where
    R: FileManagementRepository + ModelDefinitionRepository + Clone,
{
    pub fn new(
        repository: R,
        registry: std::sync::Arc<storage_object::FileStorageDriverRegistry>,
        runtime_engine: std::sync::Arc<runtime_core::runtime_engine::RuntimeEngine>,
    ) -> Self {
        Self {
            repository,
            registry,
            runtime_engine,
        }
    }

    pub async fn upload(&self, command: UploadFileCommand) -> Result<UploadedFileView> {
        let file_table = self
            .repository
            .get_file_table(command.file_table_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("file_table"))?;
        let storage = self
            .repository
            .get_file_storage(file_table.bound_storage_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("file_storage"))?;
        let model = self
            .repository
            .get_model_definition(
                command.actor.current_workspace_id,
                file_table.model_definition_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let driver = self
            .registry
            .get(&storage.driver_type)
            .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;
        let scope_grant =
            crate::model_definition::ModelDefinitionService::new(self.repository.clone())
                .load_runtime_scope_grant(
                    &command.actor,
                    model.id,
                    runtime_core::runtime_acl::RuntimeDataAction::Create,
                )
                .await?
                .ok_or(
                    runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(
                        "data_model_scope_not_granted",
                    ),
                )?;

        let file_id = Uuid::now_v7();
        let extname = std::path::Path::new(&command.original_filename)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let mimetype = command
            .content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let object_path = build_object_path(
            &file_table,
            &model.code,
            file_id,
            extname.as_str(),
            OffsetDateTime::now_utc(),
        );

        let stored = driver
            .put_object(storage_object::FileStoragePutInput {
                config_json: &storage.config_json,
                object_path: &object_path,
                content_type: Some(mimetype.as_str()),
                bytes: &command.bytes,
            })
            .await?;
        let mut payload = serde_json::Map::from_iter([
            (
                "title".to_string(),
                serde_json::Value::String(command.original_filename.clone()),
            ),
            (
                "filename".to_string(),
                serde_json::Value::String(command.original_filename),
            ),
            ("size".to_string(), serde_json::json!(command.bytes.len())),
            ("mimetype".to_string(), serde_json::Value::String(mimetype)),
            ("path".to_string(), serde_json::Value::String(stored.path)),
            ("meta".to_string(), stored.metadata_json),
            (
                "storage_id".to_string(),
                serde_json::Value::String(storage.id.to_string()),
            ),
        ]);
        if !extname.is_empty() {
            payload.insert("extname".to_string(), serde_json::Value::String(extname));
        }
        if let Some(url) = stored.url {
            payload.insert("url".to_string(), serde_json::Value::String(url));
        }

        let record = self
            .runtime_engine
            .create_record(runtime_core::runtime_engine::RuntimeCreateInput {
                actor: command.actor,
                model_code: model.code,
                payload: serde_json::Value::Object(payload),
                scope_grant: Some(scope_grant),
            })
            .await?;

        Ok(UploadedFileView {
            record,
            storage_id: storage.id,
        })
    }
}

fn build_object_path(
    file_table: &domain::FileTableRecord,
    model_code: &str,
    file_id: Uuid,
    extname: &str,
    now: OffsetDateTime,
) -> String {
    let scope_prefix = match file_table.scope_kind {
        domain::FileTableScopeKind::System => "system",
        domain::FileTableScopeKind::Workspace => "workspace",
    };
    let file_name = if extname.is_empty() {
        file_id.to_string()
    } else {
        format!("{file_id}.{extname}")
    };
    let month: u8 = now.month().into();

    format!(
        "{scope_prefix}/{model_code}/{:04}/{:02}/{file_name}",
        now.year(),
        month
    )
}
