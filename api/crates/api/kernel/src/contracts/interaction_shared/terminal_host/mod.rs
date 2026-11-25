//! Typed terminal operations on the same concrete host allocation.
use crate::app_state::ApiState;
use std::sync::Arc;
#[async_trait::async_trait]
pub trait NativeResourceImportPort: Send + Sync {
    async fn import_for_principal(
        &self,
        state: Arc<ApiState>,
        principal: &interface_runtime::UserPrincipal,
        resource_uri: &str,
        file_table_id: uuid::Uuid,
    ) -> Result<extension_contracts::FileImportResponse, extension_contracts::FileImportError>;
}
#[async_trait::async_trait]
pub trait TerminalHostOperations: Send + Sync {
    fn local_terminal_disabled(&self) -> bool;
    fn jobs(&self) -> Arc<crate::local_command::jobs::JobRegistry>;
    async fn read_file(
        &self,
        actor: domain::ActorContext,
        file_table_id: uuid::Uuid,
        record_id: uuid::Uuid,
    ) -> anyhow::Result<control_plane::file_management::FileContent>;
    async fn import_resource(
        self: Arc<Self>,
        principal: &interface_runtime::UserPrincipal,
        resource_uri: &str,
        file_table_id: uuid::Uuid,
    ) -> Result<extension_contracts::FileImportResponse, extension_contracts::FileImportError>;
}
#[async_trait::async_trait]
impl TerminalHostOperations for ApiState {
    fn local_terminal_disabled(&self) -> bool {
        self.local_terminal_disabled
    }

    fn jobs(&self) -> Arc<crate::local_command::jobs::JobRegistry> {
        self.terminal_jobs.clone()
    }
    async fn read_file(
        &self,
        actor: domain::ActorContext,
        file_table_id: uuid::Uuid,
        record_id: uuid::Uuid,
    ) -> anyhow::Result<control_plane::file_management::FileContent> {
        control_plane::file_management::FileAccessService::new(
            self.store.clone(),
            self.file_storage_registry.clone(),
            self.runtime_engine.clone(),
            self.infrastructure.ephemeral_kv_store(),
        )
        .read(actor, file_table_id, record_id)
        .await
    }
    async fn import_resource(
        self: Arc<Self>,
        principal: &interface_runtime::UserPrincipal,
        resource_uri: &str,
        file_table_id: uuid::Uuid,
    ) -> Result<extension_contracts::FileImportResponse, extension_contracts::FileImportError> {
        let port = self.native_resource_import.clone();
        port.import_for_principal(self, principal, resource_uri, file_table_id)
            .await
    }
}
