use super::*;

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressProviderInput {
    pub provider_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    pub secret_ref: String,
    pub lifecycle: domain::NetworkEgressProviderLifecycle,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateStaticHttpProxyPoolMemberInput {
    pub provider_id: Uuid,
    pub member_id: Uuid,
    pub pool_id: Uuid,
    pub display_name: String,
    pub description: String,
    pub secret_ref: String,
    pub plaintext_secret_json: serde_json::Value,
    pub master_key: String,
    pub enabled: bool,
    pub sequence: i32,
    pub synchronized_at: time::OffsetDateTime,
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
pub struct UpsertNetworkEgressProviderSecretInput {
    pub provider_id: Uuid,
    pub secret_ref: String,
    pub plaintext_secret_json: serde_json::Value,
    pub master_key: String,
    pub secret_version: i32,
}

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressPoolInput {
    pub pool_id: Uuid,
    pub display_name: String,
    pub owner_provider_id: Option<Uuid>,
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

#[derive(Debug, Clone)]
pub struct RecordNetworkEgressPoolMemberProbeInput {
    pub pool_id: Uuid,
    pub member_id: Uuid,
    pub status: domain::NetworkEgressPoolMemberProbeStatus,
    pub http_status: domain::NetworkEgressPoolMemberProbeStatus,
    pub https_status: domain::NetworkEgressPoolMemberProbeStatus,
    pub latency_ms: i32,
    pub exit_ip: Option<String>,
    pub exit_region: Option<String>,
    /// A closed, operator-safe code such as `connection_failed`; never a runtime error string.
    pub error_code: Option<String>,
    pub probed_at: time::OffsetDateTime,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateNetworkEgressRouteInput {
    pub route_id: Uuid,
    pub workspace_id: Uuid,
    pub selector: domain::NetworkEgressConsumerSelector,
    pub pool_id: Uuid,
    pub pool_member_ids: Vec<Uuid>,
    pub enabled: bool,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateNetworkEgressRouteInput {
    pub workspace_id: Uuid,
    pub route_id: Uuid,
    pub pool_member_ids: Vec<Uuid>,
    pub enabled: bool,
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
    /// Used only to compensate an initial provider creation whose first synchronization failed.
    /// Existing providers retain their unhealthy state so operators can inspect and retry them.
    async fn delete_network_egress_provider(&self, provider_id: Uuid) -> anyhow::Result<()>;
    async fn create_static_http_proxy_pool_member(
        &self,
        input: &CreateStaticHttpProxyPoolMemberInput,
    ) -> anyhow::Result<domain::NetworkEgressPoolMember>;
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
    async fn upsert_network_egress_provider_secret(
        &self,
        input: &UpsertNetworkEgressProviderSecretInput,
    ) -> anyhow::Result<domain::NetworkEgressProviderSecretRecord>;
    async fn resolve_network_egress_provider_secret_json(
        &self,
        provider_id: Uuid,
        secret_ref: &str,
        master_key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;
    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> anyhow::Result<()>;
}

/// Sensitive material is intentionally only constructed at the runner-provisioning boundary.
/// It has no serde implementation and redacts its payload in diagnostics.
#[derive(Clone, PartialEq)]
pub struct NetworkEgressSecretMaterial {
    pub secret_ref: String,
    pub secret_json: serde_json::Value,
}

impl std::fmt::Debug for NetworkEgressSecretMaterial {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NetworkEgressSecretMaterial")
            .field("secret_ref", &self.secret_ref)
            .field("secret_json", &"<redacted>")
            .finish()
    }
}

#[async_trait]
pub trait NetworkEgressSecretResolver: Send + Sync {
    async fn resolve_for_runner(
        &self,
        provider: &domain::NetworkEgressProviderRecord,
    ) -> anyhow::Result<Option<NetworkEgressSecretMaterial>>;
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
    async fn record_network_egress_pool_member_probe(
        &self,
        input: &RecordNetworkEgressPoolMemberProbeInput,
    ) -> anyhow::Result<domain::NetworkEgressPoolMember>;
    async fn delete_network_egress_pool_member(
        &self,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait NetworkEgressRouteRepository: Send + Sync {
    async fn list_network_egress_routes(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NetworkEgressRoute>>;
    async fn create_network_egress_route(
        &self,
        input: &CreateNetworkEgressRouteInput,
    ) -> anyhow::Result<domain::NetworkEgressRoute>;
    async fn update_network_egress_route(
        &self,
        input: &UpdateNetworkEgressRouteInput,
    ) -> anyhow::Result<domain::NetworkEgressRoute>;
    async fn delete_network_egress_route(
        &self,
        workspace_id: Uuid,
        route_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn find_enabled_network_egress_route(
        &self,
        workspace_id: Uuid,
        selector: &domain::NetworkEgressConsumerSelector,
    ) -> anyhow::Result<Option<domain::NetworkEgressRoute>>;
    async fn is_network_egress_pool_member_referenced(
        &self,
        member_id: Uuid,
    ) -> anyhow::Result<bool>;
}

#[async_trait]
pub trait NetworkEgressRuntimePort: Send + Sync {
    async fn sync_network_egresses(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        secret: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<Vec<plugin_framework::EgressDescriptor>>;
}
