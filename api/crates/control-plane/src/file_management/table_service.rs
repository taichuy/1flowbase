use anyhow::Result;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    file_management::{CreateWorkspaceFileTableCommand, FileTableProvisioningService},
    ports::{
        DeleteFileTableInput, FileManagementRepository, ModelDefinitionRepository,
        UpdateFileStorageBindingInput,
    },
};

pub struct BindFileTableStorageCommand {
    pub actor_user_id: Uuid,
    pub file_table_id: Uuid,
    pub bound_storage_id: Uuid,
}

pub struct CreateFileTableCommand {
    pub actor_user_id: Uuid,
    pub code: String,
    pub title: String,
}

pub struct DeleteFileTableCommand {
    pub actor_user_id: Uuid,
    pub file_table_id: Uuid,
}

#[derive(Debug)]
pub struct FileTableWithStorageTitle {
    pub table: domain::FileTableRecord,
    pub bound_storage_title: Option<String>,
}

pub struct FileTableService<R> {
    repository: R,
}

impl<R> FileTableService<R>
where
    R: FileManagementRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_tables(&self, actor_user_id: Uuid) -> Result<Vec<FileTableWithStorageTitle>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;

        let tables = self
            .repository
            .list_visible_file_tables(actor.current_workspace_id)
            .await?;
        let mut visible_tables = Vec::with_capacity(tables.len());
        for table in tables {
            visible_tables.push(self.with_storage_title(table).await?);
        }
        Ok(visible_tables)
    }

    pub async fn bind_storage(
        &self,
        command: BindFileTableStorageCommand,
    ) -> Result<FileTableWithStorageTitle> {
        let actor = FileManagementRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
        .await?;
        if !actor.is_root {
            return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
        }

        let table = self
            .repository
            .update_file_table_binding(&UpdateFileStorageBindingInput {
                actor_user_id: command.actor_user_id,
                file_table_id: command.file_table_id,
                bound_storage_id: command.bound_storage_id,
            })
            .await?;
        self.with_storage_title(table).await
    }

    pub async fn delete_table(&self, command: DeleteFileTableCommand) -> Result<()> {
        let actor = FileManagementRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
        .await?;
        if !actor.is_root {
            return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
        }

        self.repository
            .delete_file_table(&DeleteFileTableInput {
                actor_user_id: command.actor_user_id,
                file_table_id: command.file_table_id,
            })
            .await
    }

    async fn with_storage_title(
        &self,
        table: domain::FileTableRecord,
    ) -> Result<FileTableWithStorageTitle> {
        let bound_storage_title = self
            .repository
            .get_file_storage(table.bound_storage_id)
            .await?
            .map(|storage| storage.title);
        Ok(FileTableWithStorageTitle {
            table,
            bound_storage_title,
        })
    }
}

impl<R> FileTableService<R>
where
    R: FileManagementRepository + ModelDefinitionRepository + Clone,
{
    pub async fn create_table(
        &self,
        command: CreateFileTableCommand,
    ) -> Result<FileTableWithStorageTitle> {
        let actor = FileManagementRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
        .await?;

        let default_storage = self
            .repository
            .get_default_file_storage()
            .await?
            .ok_or(ControlPlaneError::NotFound("file_storage"))?;

        let table = FileTableProvisioningService::new(self.repository.clone())
            .create_workspace_file_table(CreateWorkspaceFileTableCommand {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                code: command.code,
                title: command.title,
                default_storage_id: default_storage.id,
            })
            .await?;
        self.with_storage_title(table).await
    }
}
