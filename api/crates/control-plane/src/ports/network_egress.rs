use super::*;

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressProviderInput {
    pub provider_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
    pub secret_ref: String,
    pub lifecycle: domain::NetworkEgressProviderLifecycle,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateNetworkEgressProviderLifecycleInput {
    pub provider_id: Uuid,
    pub lifecycle: domain::NetworkEgressProviderLifecycle,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ReplaceNetworkEgressProjectionInput {
    pub provider_id: Uuid,
    pub health_status: domain::NetworkEgressHealthStatus,
    pub last_sync_error: Option<String>,
    pub synchronized_at: time::OffsetDateTime,
    pub egresses: Vec<domain::NetworkEgressProjectionRecord>,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct RecordNetworkEgressSyncFailureInput {
    pub provider_id: Uuid,
    pub last_sync_error: String,
    pub synchronized_at: time::OffsetDateTime,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressPoolInput {
    pub pool_id: Uuid,
    pub display_name: String,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateNetworkEgressPoolInput {
    pub pool_id: Uuid,
    pub display_name: String,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressPoolMemberInput {
    pub member_id: Uuid,
    pub pool_id: Uuid,
    pub provider_id: Uuid,
    pub provider_egress_key: String,
    pub enabled: bool,
    pub sequence: i32,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateNetworkEgressPoolMemberInput {
    pub pool_id: Uuid,
    pub member_id: Uuid,
    pub enabled: bool,
    pub sequence: i32,
    pub actor_user_id: Uuid,
}

#[async_trait]
pub trait NetworkEgressRepository: Send + Sync {
    async fn get_network_egress_provider(
        &self,
        provider_id: Uuid,
    ) -> anyhow::Result<Option<domain::NetworkEgressProviderRecord>>;
    async fn list_network_egress_providers(
        &self,
    ) -> anyhow::Result<Vec<domain::NetworkEgressProviderRecord>>;
    async fn create_network_egress_provider(
        &self,
        input: &CreateNetworkEgressProviderInput,
    ) -> anyhow::Result<domain::NetworkEgressProviderRecord>;
    async fn update_network_egress_provider_lifecycle(
        &self,
        input: &UpdateNetworkEgressProviderLifecycleInput,
    ) -> anyhow::Result<domain::NetworkEgressProviderRecord>;
    async fn list_network_egress_projections(
        &self,
        provider_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NetworkEgressProjectionRecord>>;
    async fn replace_network_egress_projection(
        &self,
        input: &ReplaceNetworkEgressProjectionInput,
    ) -> anyhow::Result<domain::NetworkEgressProviderRecord>;
    async fn record_network_egress_sync_failure(
        &self,
        input: &RecordNetworkEgressSyncFailureInput,
    ) -> anyhow::Result<domain::NetworkEgressProviderRecord>;
    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> anyhow::Result<()>;
}

#[async_trait]
pub trait NetworkEgressPoolRepository: Send + Sync {
    async fn get_network_egress_pool(
        &self,
        pool_id: Uuid,
    ) -> anyhow::Result<Option<domain::NetworkEgressPool>>;
    async fn list_network_egress_pools(&self) -> anyhow::Result<Vec<domain::NetworkEgressPool>>;
    async fn create_network_egress_pool(
        &self,
        input: &CreateNetworkEgressPoolInput,
    ) -> anyhow::Result<domain::NetworkEgressPool>;
    async fn update_network_egress_pool(
        &self,
        input: &UpdateNetworkEgressPoolInput,
    ) -> anyhow::Result<domain::NetworkEgressPool>;
    async fn delete_network_egress_pool(&self, pool_id: Uuid) -> anyhow::Result<()>;
    async fn list_network_egress_pool_members(
        &self,
        pool_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NetworkEgressPoolMember>>;
    async fn create_network_egress_pool_member(
        &self,
        input: &CreateNetworkEgressPoolMemberInput,
    ) -> anyhow::Result<domain::NetworkEgressPoolMember>;
    async fn update_network_egress_pool_member(
        &self,
        input: &UpdateNetworkEgressPoolMemberInput,
    ) -> anyhow::Result<domain::NetworkEgressPoolMember>;
    async fn delete_network_egress_pool_member(
        &self,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait NetworkEgressRuntimePort: Send + Sync {
    async fn sync_network_egresses(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<Vec<plugin_framework::EgressDescriptor>>;
}
