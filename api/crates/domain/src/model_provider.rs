use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginVerificationStatus {
    Pending,
    Valid,
    Invalid,
}

impl PluginVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Valid => "valid",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginTaskKind {
    Install,
    Upgrade,
    Uninstall,
    Enable,
    Disable,
    Assign,
    Unassign,
    SwitchVersion,
}

impl PluginTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Upgrade => "upgrade",
            Self::Uninstall => "uninstall",
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::Assign => "assign",
            Self::Unassign => "unassign",
            Self::SwitchVersion => "switch_version",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginDesiredState {
    Disabled,
    PendingRestart,
    ActiveRequested,
}

impl PluginDesiredState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::PendingRestart => "pending_restart",
            Self::ActiveRequested => "active_requested",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginArtifactStatus {
    Missing,
    Staged,
    Ready,
    Corrupted,
    InstallIncomplete,
}

impl PluginArtifactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Staged => "staged",
            Self::Ready => "ready",
            Self::Corrupted => "corrupted",
            Self::InstallIncomplete => "install_incomplete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginArtifactInstanceStatus {
    Missing,
    Ready,
    Outdated,
    Mismatched,
    Corrupted,
    LoadFailed,
}

impl PluginArtifactInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Ready => "ready",
            Self::Outdated => "outdated",
            Self::Mismatched => "mismatched",
            Self::Corrupted => "corrupted",
            Self::LoadFailed => "load_failed",
        }
    }

    pub fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeStatus {
    Inactive,
    Active,
    LoadFailed,
}

impl PluginRuntimeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
            Self::LoadFailed => "load_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginAvailabilityStatus {
    Disabled,
    PendingRestart,
    ArtifactMissing,
    InstallIncomplete,
    LoadFailed,
    Available,
}

impl PluginAvailabilityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::PendingRestart => "pending_restart",
            Self::ArtifactMissing => "artifact_missing",
            Self::InstallIncomplete => "install_incomplete",
            Self::LoadFailed => "load_failed",
            Self::Available => "available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginTaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
    TimedOut,
}

impl PluginTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
            Self::TimedOut => "timed_out",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Canceled | Self::TimedOut
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInstallationRecord {
    pub id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contract_version: String,
    pub protocol: String,
    pub display_name: String,
    pub source_kind: String,
    pub trust_level: String,
    pub verification_status: PluginVerificationStatus,
    pub desired_state: PluginDesiredState,
    pub artifact_status: PluginArtifactStatus,
    pub runtime_status: PluginRuntimeStatus,
    pub availability_status: PluginAvailabilityStatus,
    pub package_path: Option<String>,
    pub installed_path: String,
    pub checksum: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub signature_status: Option<String>,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
    pub last_load_error: Option<String>,
    pub metadata_json: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginArtifactInstanceRecord {
    pub node_id: String,
    pub installation_id: Uuid,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub installed_path: Option<String>,
    pub artifact_status: PluginArtifactInstanceStatus,
    pub runtime_status: PluginRuntimeStatus,
    pub checked_at: OffsetDateTime,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPackageCatalogProjectionStatus {
    Ok,
    Missing,
    Failed,
}

impl PluginPackageCatalogProjectionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPackageCatalogProjectionRecord {
    pub installation_id: Uuid,
    pub package_code: String,
    pub package_version: String,
    pub catalog_snapshot_json: serde_json::Value,
    pub projection_status: PluginPackageCatalogProjectionStatus,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginAssignmentRecord {
    pub id: Uuid,
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub assigned_by: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginTaskRecord {
    pub id: Uuid,
    pub installation_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub provider_code: String,
    pub task_kind: PluginTaskKind,
    pub status: PluginTaskStatus,
    pub status_message: Option<String>,
    pub detail_json: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderInstanceStatus {
    Draft,
    Ready,
    Invalid,
    Disabled,
}

impl ModelProviderInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Invalid => "invalid",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderValidationStatus {
    Succeeded,
    Failed,
}

impl ModelProviderValidationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderDiscoveryMode {
    Static,
    Dynamic,
    Hybrid,
}

impl ModelProviderDiscoveryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderCatalogRefreshStatus {
    Idle,
    Refreshing,
    Ready,
    Failed,
}

impl ModelProviderCatalogRefreshStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Refreshing => "refreshing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderCatalogSource {
    Static,
    Dynamic,
    Hybrid,
}

impl ModelProviderCatalogSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderDistributionRule {
    #[default]
    None,
    RoundRobin,
}

impl ModelProviderDistributionRule {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RoundRobin => "round_robin",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderConfiguredModel {
    pub model_id: String,
    pub enabled: bool,
    pub context_window_override_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_multimodal: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderMainModelDistributionRule {
    pub model_id: String,
    pub distribution_rule: ModelProviderDistributionRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderMainModelDistributionRuleRecord {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub model_id: String,
    pub distribution_rule: ModelProviderDistributionRule,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderMainInstanceRecord {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Vec<ModelProviderMainModelDistributionRuleRecord>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderInstanceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub protocol: String,
    pub display_name: String,
    pub status: ModelProviderInstanceStatus,
    pub config_json: serde_json::Value,
    pub configured_models: Vec<ModelProviderConfiguredModel>,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: bool,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderPreviewSessionRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub instance_id: Option<Uuid>,
    pub config_fingerprint: String,
    pub models_json: serde_json::Value,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderCatalogCacheRecord {
    pub provider_instance_id: Uuid,
    pub model_discovery_mode: ModelProviderDiscoveryMode,
    pub refresh_status: ModelProviderCatalogRefreshStatus,
    pub source: ModelProviderCatalogSource,
    pub models_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderCatalogSourceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub source_kind: String,
    pub plugin_id: String,
    pub provider_code: String,
    pub display_name: String,
    pub base_url_ref: Option<String>,
    pub auth_secret_ref: Option<String>,
    pub protocol: String,
    pub status: String,
    pub last_sync_run_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCatalogSyncRunRecord {
    pub id: Uuid,
    pub catalog_source_id: Uuid,
    pub status: String,
    pub error_message_ref: Option<String>,
    pub discovered_count: i64,
    pub imported_count: i64,
    pub disabled_count: i64,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderCatalogEntryRecord {
    pub id: Uuid,
    pub provider_instance_id: Option<Uuid>,
    pub catalog_source_id: Uuid,
    pub upstream_model_id: String,
    pub display_label: String,
    pub protocol: String,
    pub capability_snapshot: serde_json::Value,
    pub parameter_schema_ref: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub pricing_ref: Option<String>,
    pub fetched_at: OffsetDateTime,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFailoverQueueTemplateRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFailoverQueueItemRecord {
    pub id: Uuid,
    pub queue_template_id: Uuid,
    pub sort_index: i32,
    pub provider_instance_id: Uuid,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub protocol: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFailoverQueueSnapshotRecord {
    pub id: Uuid,
    pub queue_template_id: Uuid,
    pub version: i64,
    pub items: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProviderSecretRecord {
    pub provider_instance_id: Uuid,
    pub encrypted_secret_json: serde_json::Value,
    pub secret_version: i32,
    pub updated_at: OffsetDateTime,
}
