use anyhow::Result;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{FileManagementRepository, ModelDefinitionRepository},
};

pub struct UploadFileCommand {
    pub actor: domain::ActorContext,
    pub target: FileUploadTarget,
    pub original_filename: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub enum FileUploadTarget {
    Default,
    Table(Uuid),
}

#[derive(Debug)]
pub struct UploadedFileView {
    pub file_table_id: Uuid,
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
        let file_table = match command.target {
            FileUploadTarget::Default => self
                .repository
                .list_visible_file_tables(command.actor.current_workspace_id)
                .await?
                .into_iter()
                .find(|table| table.is_default && table.status == "active")
                .ok_or(ControlPlaneError::NotFound("default_file_table"))?,
            FileUploadTarget::Table(file_table_id) => self
                .repository
                .get_file_table(file_table_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("file_table"))?,
        };
        let storage = match file_table.bound_storage_id {
            Some(storage_id) => self.repository.get_file_storage(storage_id).await?,
            None => self.repository.get_default_file_storage().await?,
        }
        .ok_or(ControlPlaneError::NotFound("file_storage"))?;
        if !storage.enabled {
            return Err(ControlPlaneError::Conflict("file_storage_disabled").into());
        }
        let model = self
            .repository
            .get_model_definition(
                command.actor.current_workspace_id,
                file_table.model_definition_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        if model.availability_status != domain::MetadataAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("model_definition_unavailable").into());
        }
        match model.status {
            domain::DataModelStatus::Published => {}
            domain::DataModelStatus::Draft => {
                return Err(
                    runtime_core::runtime_engine::RuntimeModelError::not_published(&model.code)
                        .into(),
                )
            }
            domain::DataModelStatus::Disabled => {
                return Err(
                    runtime_core::runtime_engine::RuntimeModelError::disabled(&model.code).into(),
                )
            }
            domain::DataModelStatus::Broken => {
                return Err(
                    runtime_core::runtime_engine::RuntimeModelError::broken(&model.code).into(),
                )
            }
        }
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

        // Reject disabled or mismatched scope authority before object storage has side effects.
        // The record owner repeats this check when it creates the metadata record.
        runtime_core::runtime_acl::resolve_access_scope(
            &command.actor,
            runtime_core::runtime_acl::RuntimeDataAction::Create,
            model.id,
            Some(&scope_grant),
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

        let size = command.bytes.len() as u64;
        let stored = driver
            .put_object(storage_object::FileStoragePutInput {
                config_json: &storage.config_json,
                object_path: &object_path,
                content_type: Some(mimetype.as_str()),
                bytes: command.bytes,
            })
            .await?;
        let metadata = domain::file_management::UploadedFileMetadata {
            id: file_id,
            title: command.original_filename.clone(),
            filename: command.original_filename,
            extname: (!extname.is_empty()).then_some(extname),
            size,
            mimetype,
            path: stored.path,
            meta: stored.metadata_json,
            url: Some(domain::file_management::file_content_url(
                file_table.id,
                file_id,
            )),
            storage_id: storage.id,
        };
        // This host-only entry covers registered legacy file tables as well as core/file/v1.
        let record = self
            .runtime_engine
            .create_uploaded_file_record(runtime_core::runtime_engine::RuntimeUploadedFileInput {
                actor: command.actor,
                model_code: model.code,
                metadata,
                scope_grant: Some(scope_grant),
            })
            .await?;

        Ok(UploadedFileView {
            file_table_id: file_table.id,
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
