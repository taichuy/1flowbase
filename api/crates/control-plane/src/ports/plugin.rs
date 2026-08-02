use super::*;

#[derive(Debug, Clone)]
pub struct UpsertPluginInstallationInput {
    pub installation_id: Uuid,
    pub category: domain::ExtensionCategory,
    pub organization: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contract_version: String,
    pub protocol: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: String,
    pub verification_status: domain::PluginVerificationStatus,
    pub desired_state: domain::PluginDesiredState,
    pub expected_checksum: Option<String>,
    pub signature_status: domain::ExtensionSignatureStatus,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub metadata_json: serde_json::Value,
    pub is_system_reserved: bool,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CommitPluginInstallationInput {
    pub installation: UpsertPluginInstallationInput,
    pub artifact_instance: UpsertPluginArtifactInstanceInput,
    pub package_catalog: Option<UpsertPluginPackageCatalogProjectionInput>,
    pub node_contributions: ReplaceInstallationNodeContributionsInput,
    pub js_dependencies: ReplaceInstallationJsDependenciesInput,
    pub frontend_blocks: ReplaceInstallationFrontendBlocksInput,
}

#[derive(Debug, Clone)]
pub struct CreatePluginAssignmentInput {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreatePluginTaskInput {
    pub task_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub provider_code: String,
    pub task_kind: domain::PluginTaskKind,
    pub status: domain::PluginTaskStatus,
    pub status_message: Option<String>,
    pub detail_json: serde_json::Value,
    pub actor_user_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct UpdatePluginTaskStatusInput {
    pub task_id: Uuid,
    pub status: domain::PluginTaskStatus,
    pub status_message: Option<String>,
    pub detail_json: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdatePluginDesiredStateInput {
    pub installation_id: Uuid,
    pub desired_state: domain::PluginDesiredState,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpsertPluginArtifactInstanceInput {
    pub node_id: String,
    pub installation_id: Uuid,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub local_path: Option<String>,
    pub package_path: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub artifact_status: domain::PluginArtifactInstanceStatus,
    pub runtime_status: domain::PluginRuntimeStatus,
    pub availability_status: domain::PluginAvailabilityStatus,
    pub checked_at: time::OffsetDateTime,
    pub last_error: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct UpsertPluginPackageCatalogProjectionInput {
    pub installation_id: Uuid,
    pub package_code: String,
    pub package_version: String,
    pub catalog_snapshot_json: serde_json::Value,
    pub projection_status: domain::PluginPackageCatalogProjectionStatus,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct UpsertHostInfrastructureProviderConfigInput {
    pub installation_id: Uuid,
    pub extension_id: String,
    pub provider_code: String,
    pub config_ref: String,
    pub enabled_contracts: Vec<String>,
    pub config_json: serde_json::Value,
    pub status: domain::HostInfrastructureConfigStatus,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogSource {
    pub source_kind: String,
    pub source_label: String,
    pub registry_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialPluginCatalogFreshness {
    Fresh,
    Stale,
}

impl OfficialPluginCatalogFreshness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialPluginArtifact {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target: String,
    pub download_url: String,
    pub checksum: String,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialPluginI18nSummary {
    pub default_locale: String,
    pub available_locales: Vec<String>,
    pub bundles: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialPluginSourceEntry {
    pub plugin_id: String,
    pub plugin_type: String,
    pub provider_code: String,
    pub namespace: String,
    pub protocol: String,
    pub latest_version: String,
    pub minimum_host_version: String,
    pub icon: Option<String>,
    pub selected_artifact: OfficialPluginArtifact,
    pub i18n_summary: OfficialPluginI18nSummary,
    pub release_tag: String,
    pub trust_mode: String,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogSnapshot {
    pub source: OfficialPluginCatalogSource,
    pub freshness: OfficialPluginCatalogFreshness,
    pub entries: Vec<OfficialPluginSourceEntry>,
}

#[derive(Debug, Clone)]
pub struct DownloadedOfficialPluginPackage {
    pub file_name: String,
    pub package_bytes: Vec<u8>,
}

#[async_trait]
pub trait OfficialPluginSourcePort: Send + Sync {
    async fn list_official_catalog(&self) -> anyhow::Result<OfficialPluginCatalogSnapshot>;
    async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
        None
    }
    async fn download_plugin(
        &self,
        entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage>;
    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey>;
}

#[async_trait]
pub trait PluginRepository: Send + Sync {
    async fn commit_plugin_installation(
        &self,
        _input: &CommitPluginInstallationInput,
    ) -> anyhow::Result<domain::PluginInstallationRecord> {
        anyhow::bail!("unified plugin installation commit is not supported")
    }
    async fn upsert_installation(
        &self,
        input: &UpsertPluginInstallationInput,
    ) -> anyhow::Result<domain::PluginInstallationRecord>;
    async fn get_installation(
        &self,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::PluginInstallationRecord>>;
    async fn get_local_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::LocalPluginInstallationRecord>> {
        let Some(installation) = self.get_installation(installation_id).await? else {
            return Ok(None);
        };
        let Some(artifact) = self.get_artifact_instance(node_id, installation_id).await? else {
            return Ok(None);
        };
        Ok(Some(domain::LocalPluginInstallationRecord {
            installation,
            artifact,
        }))
    }
    async fn list_installations(&self) -> anyhow::Result<Vec<domain::PluginInstallationRecord>>;
    async fn upsert_plugin_package_catalog_projection(
        &self,
        input: &UpsertPluginPackageCatalogProjectionInput,
    ) -> anyhow::Result<domain::PluginPackageCatalogProjectionRecord>;
    async fn get_plugin_package_catalog_projection(
        &self,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::PluginPackageCatalogProjectionRecord>>;
    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> anyhow::Result<Vec<domain::PluginPackageCatalogProjectionRecord>>;
    async fn delete_installation(&self, installation_id: Uuid) -> anyhow::Result<()>;
    async fn list_pending_restart_host_extensions(
        &self,
    ) -> anyhow::Result<Vec<domain::PluginInstallationRecord>>;
    async fn update_desired_state(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> anyhow::Result<domain::PluginInstallationRecord>;
    async fn upsert_artifact_instance(
        &self,
        input: &UpsertPluginArtifactInstanceInput,
    ) -> anyhow::Result<domain::PluginArtifactInstanceRecord>;
    async fn get_artifact_instance(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::PluginArtifactInstanceRecord>>;
    async fn list_artifact_instances(
        &self,
        node_id: &str,
    ) -> anyhow::Result<Vec<domain::PluginArtifactInstanceRecord>>;
    async fn create_assignment(
        &self,
        input: &CreatePluginAssignmentInput,
    ) -> anyhow::Result<domain::PluginAssignmentRecord>;
    async fn list_assignments(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::PluginAssignmentRecord>>;
    async fn create_task(
        &self,
        input: &CreatePluginTaskInput,
    ) -> anyhow::Result<domain::PluginTaskRecord>;
    async fn update_task_status(
        &self,
        input: &UpdatePluginTaskStatusInput,
    ) -> anyhow::Result<domain::PluginTaskRecord>;
    async fn get_task(&self, task_id: Uuid) -> anyhow::Result<Option<domain::PluginTaskRecord>>;
    async fn list_tasks(&self) -> anyhow::Result<Vec<domain::PluginTaskRecord>>;
}

#[async_trait]
pub trait HostInfrastructureConfigRepository: Send + Sync {
    async fn upsert_host_infrastructure_provider_config(
        &self,
        input: &UpsertHostInfrastructureProviderConfigInput,
    ) -> anyhow::Result<domain::HostInfrastructureProviderConfigRecord>;

    async fn list_host_infrastructure_provider_configs(
        &self,
    ) -> anyhow::Result<Vec<domain::HostInfrastructureProviderConfigRecord>>;
}

#[derive(Debug, Clone)]
pub struct NodeContributionRegistryInput {
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub schema_ui: serde_json::Value,
    pub schema_version: String,
    pub output_schema: serde_json::Value,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    pub output_schema_snapshot: serde_json::Value,
    pub side_effect_policy: String,
    pub infra_contracts: Vec<String>,
    pub required_auth: Vec<String>,
    pub visibility: String,
    pub experimental: bool,
    pub dependency_installation_kind: String,
    pub dependency_plugin_version_range: String,
}

#[derive(Debug, Clone)]
pub struct ReplaceInstallationNodeContributionsInput {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub entries: Vec<NodeContributionRegistryInput>,
}

#[async_trait]
pub trait NodeContributionRepository: Send + Sync {
    async fn replace_installation_node_contributions(
        &self,
        input: &ReplaceInstallationNodeContributionsInput,
    ) -> anyhow::Result<()>;
    async fn list_node_contributions(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NodeContributionRegistryEntry>>;
}

#[derive(Debug, Clone)]
pub struct JsDependencyRegistryInput {
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub integrity: String,
    pub permissions: domain::JsDependencyPermissions,
}

#[derive(Debug, Clone)]
pub struct ReplaceInstallationJsDependenciesInput {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub entries: Vec<JsDependencyRegistryInput>,
}

#[async_trait]
pub trait JsDependencyRepository: Send + Sync {
    async fn replace_installation_js_dependencies(
        &self,
        input: &ReplaceInstallationJsDependenciesInput,
    ) -> anyhow::Result<()>;

    async fn list_workspace_js_dependencies(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::JsDependencyRegistryEntry>>;
}

#[derive(Debug, Clone)]
pub struct FrontendBlockCatalogRegistryInput {
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_template_version: Option<String>,
    pub code_template_language: Option<String>,
    pub code_modules: Vec<domain::FrontendBlockCodeModule>,
    pub context_contract: domain::FrontendBlockContextContract,
    pub permissions: domain::FrontendBlockPermissions,
    pub ui_capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReplaceInstallationFrontendBlocksInput {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub entries: Vec<FrontendBlockCatalogRegistryInput>,
}

#[async_trait]
pub trait FrontendBlockCatalogRepository: Send + Sync {
    async fn replace_installation_frontend_blocks(
        &self,
        input: &ReplaceInstallationFrontendBlocksInput,
    ) -> anyhow::Result<()>;

    async fn list_workspace_frontend_blocks(
        &self,
        node_id: &str,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::FrontendBlockCatalogEntry>>;
}

#[derive(Debug, Clone)]
pub struct CreatePluginWorkerLeaseInput {
    pub installation_id: Uuid,
    pub worker_key: String,
    pub status: domain::PluginWorkerStatus,
}

#[async_trait]
pub trait PluginWorkerRepository: Send + Sync {
    async fn create_worker_lease(
        &self,
        input: &CreatePluginWorkerLeaseInput,
    ) -> anyhow::Result<domain::PluginWorkerLeaseRecord>;
}

#[derive(Debug, Clone)]
pub struct UpsertHostExtensionInventoryInput {
    pub extension_id: String,
    pub version: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: domain::HostExtensionTrustLevel,
    pub activation_status: domain::HostExtensionActivationStatus,
    pub provides_contracts: Vec<String>,
    pub overrides_contracts: Vec<String>,
    pub registers_slots: Vec<String>,
    pub registers_storage: Vec<String>,
    pub last_error: Option<String>,
}

#[async_trait]
pub trait HostExtensionInventoryRepository: Send + Sync {
    async fn upsert_host_extension_inventory(
        &self,
        input: &UpsertHostExtensionInventoryInput,
    ) -> anyhow::Result<domain::HostExtensionInventoryRecord>;

    async fn list_host_extension_inventory(
        &self,
    ) -> anyhow::Result<Vec<domain::HostExtensionInventoryRecord>>;
}
