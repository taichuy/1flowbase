mod access_service;
mod backup_export;
mod backup_restore;
mod bootstrap;
mod preview;
mod storage_service;
pub use preview::FilePreviewProjector;
mod table_service;
mod template;
mod upload_service;

pub use access_service::*;
pub use backup_export::*;
pub use backup_restore::*;
pub use bootstrap::*;
pub use storage_service::*;
pub use table_service::*;
pub use template::*;
pub use upload_service::*;

#[cfg(test)]
mod _tests;

mod resource_import;
pub use resource_import::*;

async fn visible_file_table(
    repository: &impl crate::ports::FileManagementRepository,
    file_table_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
) -> anyhow::Result<domain::FileTableRecord> {
    repository
        .get_file_table(file_table_id)
        .await?
        .filter(|table| match table.scope_kind {
            domain::FileTableScopeKind::System => table.scope_id == domain::SYSTEM_SCOPE_ID,
            domain::FileTableScopeKind::Workspace => table.scope_id == workspace_id,
        })
        .ok_or_else(|| crate::errors::ControlPlaneError::NotFound("file_table").into())
}
