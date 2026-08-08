mod artifact_instance;
mod bootstrap;
mod catalog;
mod catalog_projection;
mod extension_installation;
mod family;
mod filesystem;
mod install;
mod package_router;

use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use access_control::ensure_permission;
use anyhow::{Context, Result};
use plugin_framework::{
    compute_manifest_fingerprint, intake_package_bytes, parse_plugin_manifest,
    provider_contract::CURRENT_PROVIDER_CONTRACT, provider_package::ProviderPackage,
    PackageIntakePolicy, PackageIntakeResult, PluginConsumptionKind, PluginExecutionMode,
    PluginManifestV1,
};
use semver::Version;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    host_extension::{
        ensure_root_actor, ensure_uploaded_host_extensions_enabled, is_host_extension_installation,
        is_model_provider_installation, plugin_code_from_plugin_id,
    },
    i18n::{
        merge_i18n_catalog, plugin_namespace, trim_json_bundles, I18nCatalog, RequestedLocales,
    },
    plugin_lifecycle::derive_availability_status,
    ports::{
        AuthRepository, CacheStore, CommitPluginInstallationInput, CreatePluginAssignmentInput,
        CreatePluginTaskInput, ExtensionInstallationRepository, FrontendBlockCatalogRegistryInput,
        FrontendBlockCatalogRepository, JsDependencyRegistryInput, JsDependencyRepository,
        ModelProviderRepository, NodeContributionRegistryInput, NodeContributionRepository,
        OfficialPluginArtifact, OfficialPluginSourceEntry, OfficialPluginSourcePort,
        PluginRepository, ProviderRuntimePort, ReassignModelProviderInstancesInput,
        ReplaceInstallationFrontendBlocksInput, ReplaceInstallationJsDependenciesInput,
        ReplaceInstallationNodeContributionsInput, RoleConsolePolicyReader,
        UpdatePluginDesiredStateInput, UpdatePluginTaskStatusInput,
        UpsertModelProviderCatalogCacheInput, UpsertPluginArtifactInstanceInput,
        UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
    },
    state_transition::ensure_plugin_task_transition,
};

pub use artifact_instance::*;
pub use bootstrap::*;
pub use catalog::*;
pub use catalog_projection::*;
pub use extension_installation::*;
pub use family::*;
pub use install::*;
pub use package_router::{route_plugin_package, RoutedPluginPackageKind};

pub struct PluginManagementService<R, H> {
    repository: R,
    runtime: H,
    official_source: Arc<dyn OfficialPluginSourcePort>,
    install_root: PathBuf,
    node_id: String,
    host_version: String,
    allow_uploaded_host_extensions: bool,
    use_case: PluginManagementUseCase,
    model_routing_cache_store: Option<Arc<dyn CacheStore>>,
}

#[derive(Clone)]
enum PluginManagementUseCase {
    BusinessActions,
    PluginConsoleOperation {
        policy_reader: Arc<dyn RoleConsolePolicyReader>,
        group: domain::ConsolePolicyGroup,
        operation_id: domain::ConsoleOperationId,
    },
    ModelProviderConsoleOperation {
        policy_reader: Arc<dyn RoleConsolePolicyReader>,
        group: domain::ConsolePolicyGroup,
        operation_id: domain::ConsoleOperationId,
    },
}

pub const PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM: &str = "below_minimum_host_version";
pub const PLUGIN_RISK_CHECKSUM_MISMATCH: &str = "checksum_mismatch";
pub const PLUGIN_RISK_SIGNATURE_MISSING: &str = "signature_missing";
pub const PLUGIN_RISK_SIGNATURE_INVALID: &str = "signature_invalid";
pub const PLUGIN_RISK_SIGNING_KEY_UNKNOWN: &str = "signing_key_unknown";
const PLUGIN_HOST_COMPATIBILITY_COMPATIBLE: &str = "compatible";
const PLUGIN_HOST_VERSION_BELOW_MINIMUM_CONFLICT: &str = "plugin_host_version_below_minimum";
const PLUGIN_COMPATIBILITY_OVERRIDE_INVALID: &str = "plugin_compatibility_override";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCompatibilityOverride {
    pub reason: String,
    pub acknowledged_current_host_version: String,
    pub acknowledged_minimum_host_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRiskOverride {
    pub reason: String,
    pub acknowledged_warnings: Vec<String>,
}

pub(super) fn validate_plugin_risk_override(
    warnings: &[String],
    risk_override: Option<&PluginRiskOverride>,
) -> Result<Option<serde_json::Value>> {
    if warnings.is_empty() {
        return Ok(None);
    }
    let Some(risk_override) = risk_override else {
        return Err(ControlPlaneError::Conflict("plugin_risk_confirmation_required").into());
    };
    let mut expected = warnings.to_vec();
    expected.sort();
    expected.dedup();
    let mut acknowledged = risk_override.acknowledged_warnings.clone();
    acknowledged.sort();
    acknowledged.dedup();
    if risk_override.reason.trim().is_empty() || acknowledged != expected {
        return Err(ControlPlaneError::InvalidInput("plugin_risk_override").into());
    }
    Ok(Some(json!({
        "reason": risk_override.reason,
        "acknowledged_warnings": expected,
    })))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialPluginHostCompatibility {
    pub minimum_host_version: String,
    pub current_host_version: String,
    pub status: String,
    pub warning_reason: Option<String>,
}

impl<R, H> PluginManagementService<R, H> {
    pub fn new(
        repository: R,
        runtime: H,
        official_source: Arc<dyn OfficialPluginSourcePort>,
        install_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repository,
            runtime,
            official_source,
            install_root: install_root.into(),
            node_id: String::new(),
            host_version: current_plugin_host_version(),
            allow_uploaded_host_extensions: true,
            use_case: PluginManagementUseCase::BusinessActions,
            model_routing_cache_store: None,
        }
        .with_default_node_id()
    }

    pub fn with_allow_uploaded_host_extensions(mut self, allow: bool) -> Self {
        self.allow_uploaded_host_extensions = allow;
        self
    }

    pub fn with_model_routing_cache_store(mut self, cache_store: Arc<dyn CacheStore>) -> Self {
        self.model_routing_cache_store = Some(cache_store);
        self
    }

    async fn invalidate_model_routing_catalog(&self, workspace_id: Uuid) {
        if let Some(cache_store) = self.model_routing_cache_store.as_deref() {
            crate::orchestration_runtime::compile_context::invalidate_model_provider_routing_catalog(
                cache_store,
                workspace_id,
            )
            .await;
        }
    }

    fn with_default_node_id(mut self) -> Self {
        if self.node_id.is_empty() {
            self.node_id = format!("local:{}", self.install_root.display());
        }
        self
    }

    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        let node_id = node_id.into();
        let node_id = node_id.trim();
        if !node_id.is_empty() {
            self.node_id = node_id.to_string();
        }
        self
    }

    pub fn for_plugin_console_operation(
        mut self,
        group: domain::ConsolePolicyGroup,
        operation_id: &'static str,
    ) -> Self
    where
        R: RoleConsolePolicyReader + Clone + 'static,
    {
        self.use_case = PluginManagementUseCase::PluginConsoleOperation {
            policy_reader: Arc::new(self.repository.clone()),
            group,
            operation_id: domain::ConsoleOperationId::try_from(operation_id)
                .expect("compiled plugin operation id must be valid"),
        };
        self
    }

    pub fn for_model_provider_console_operation(mut self, operation_id: &'static str) -> Self
    where
        R: RoleConsolePolicyReader + Clone + 'static,
    {
        self.use_case = PluginManagementUseCase::ModelProviderConsoleOperation {
            policy_reader: Arc::new(self.repository.clone()),
            group: domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation_id: domain::ConsoleOperationId::try_from(operation_id)
                .expect("compiled model-provider plugin operation id must be valid"),
        };
        self
    }

    pub fn for_extension_center_console_operation(mut self, operation_id: &'static str) -> Self
    where
        R: RoleConsolePolicyReader + Clone + 'static,
    {
        self.use_case = PluginManagementUseCase::PluginConsoleOperation {
            policy_reader: Arc::new(self.repository.clone()),
            group: domain::ConsolePolicyGroup::settings_feature("system.extension-center")
                .expect("compiled extension-center settings group must be valid"),
            operation_id: domain::ConsoleOperationId::try_from(operation_id)
                .expect("compiled extension-center operation id must be valid"),
        };
        self
    }

    async fn ensure_use_case_permission(
        &self,
        actor: &domain::ActorContext,
        business_permission: &str,
    ) -> Result<()> {
        match &self.use_case {
            PluginManagementUseCase::BusinessActions => {
                ensure_permission(actor, business_permission)
                    .map_err(ControlPlaneError::PermissionDenied)?;
                Ok(())
            }
            PluginManagementUseCase::PluginConsoleOperation {
                policy_reader,
                group,
                operation_id,
            }
            | PluginManagementUseCase::ModelProviderConsoleOperation {
                policy_reader,
                group,
                operation_id,
            } => {
                if actor.is_root {
                    return Ok(());
                }
                let policies = policy_reader
                    .load_role_console_policies_for_user(actor)
                    .await?;
                if domain::effective_console_simple_operation(&policies, group, operation_id) {
                    Ok(())
                } else {
                    Err(ControlPlaneError::PermissionDenied("permission_denied").into())
                }
            }
        }
    }

    fn is_model_provider_console_operation(&self) -> bool {
        matches!(
            &self.use_case,
            PluginManagementUseCase::ModelProviderConsoleOperation { .. }
        )
    }

    fn ensure_model_provider_target(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<()> {
        if self.is_model_provider_console_operation()
            && !is_model_provider_installation(installation)
        {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        Ok(())
    }

    fn ensure_model_provider_package_kind(&self, kind: RoutedPluginPackageKind) -> Result<()> {
        if self.is_model_provider_console_operation()
            && kind != RoutedPluginPackageKind::ModelProviderRuntime
        {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        Ok(())
    }
}

pub fn current_plugin_host_version() -> String {
    option_env!("FLOWBASE_API_SERVER_VERSION")
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}

fn parse_semver(value: &str) -> Option<Version> {
    let version = value.trim().trim_start_matches('v');
    Version::parse(version).ok()
}

pub fn official_plugin_host_compatibility(
    minimum_host_version: &str,
    current_host_version: &str,
) -> OfficialPluginHostCompatibility {
    let is_below_minimum = match (
        parse_semver(current_host_version),
        parse_semver(minimum_host_version),
    ) {
        (Some(current), Some(minimum)) => current < minimum,
        _ => false,
    };
    let status = if is_below_minimum {
        PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM
    } else {
        PLUGIN_HOST_COMPATIBILITY_COMPATIBLE
    };

    OfficialPluginHostCompatibility {
        minimum_host_version: minimum_host_version.to_string(),
        current_host_version: current_host_version.to_string(),
        status: status.to_string(),
        warning_reason: is_below_minimum
            .then(|| PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM.to_string()),
    }
}

pub(super) fn validate_official_plugin_compatibility_override(
    entry: &OfficialPluginSourceEntry,
    current_host_version: &str,
    compatibility_override: Option<&PluginCompatibilityOverride>,
) -> Result<Option<serde_json::Value>> {
    validate_plugin_compatibility_requirement(
        &entry.minimum_host_version,
        current_host_version,
        compatibility_override,
    )
}

pub(super) fn validate_plugin_compatibility_requirement(
    minimum_host_version: &str,
    current_host_version: &str,
    compatibility_override: Option<&PluginCompatibilityOverride>,
) -> Result<Option<serde_json::Value>> {
    let compatibility =
        official_plugin_host_compatibility(minimum_host_version, current_host_version);
    if compatibility.status != PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM {
        return Ok(None);
    }

    let Some(compatibility_override) = compatibility_override else {
        return Err(ControlPlaneError::Conflict(PLUGIN_HOST_VERSION_BELOW_MINIMUM_CONFLICT).into());
    };
    if compatibility_override.reason != PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM
        || compatibility_override.acknowledged_current_host_version != current_host_version
        || compatibility_override.acknowledged_minimum_host_version != minimum_host_version
    {
        return Err(ControlPlaneError::InvalidInput(PLUGIN_COMPATIBILITY_OVERRIDE_INVALID).into());
    }

    Ok(Some(json!({
        "reason": compatibility_override.reason,
        "acknowledged_current_host_version": compatibility_override.acknowledged_current_host_version,
        "acknowledged_minimum_host_version": compatibility_override.acknowledged_minimum_host_version,
    })))
}

fn merge_install_detail_metadata(
    metadata_json: &mut serde_json::Value,
    detail_json: &serde_json::Value,
) {
    for key in [
        "install_kind",
        "official_plugin_id",
        "bootstrap_source",
        "domain_binding_owner",
    ] {
        if let Some(value) = detail_json.get(key).cloned() {
            metadata_json[key] = value;
        }
    }
    if let Some(compatibility_override) = detail_json.get("compatibility_override").cloned() {
        metadata_json["compatibility_override"] = compatibility_override;
    }
    if let Some(risk_override) = detail_json.get("risk_override").cloned() {
        metadata_json["risk_override"] = risk_override;
    }
    if let Some(risk_warnings) = detail_json.get("risk_warnings").cloned() {
        metadata_json["risk_warnings"] = risk_warnings;
    }
}

fn plugin_install_audit_detail(
    installation: &domain::PluginInstallationRecord,
    detail_json: &serde_json::Value,
    restart_required: bool,
) -> serde_json::Value {
    let mut audit_detail = json!({
        "provider_code": installation.provider_code,
        "plugin_id": installation.plugin_id,
    });
    if restart_required {
        audit_detail["restart_required"] = json!(true);
    }
    if let Some(compatibility_override) = detail_json.get("compatibility_override").cloned() {
        audit_detail["compatibility_override"] = compatibility_override;
    }
    if let Some(risk_override) = detail_json.get("risk_override").cloned() {
        audit_detail["risk_override"] = risk_override;
    }
    if let Some(risk_warnings) = detail_json.get("risk_warnings").cloned() {
        audit_detail["risk_warnings"] = risk_warnings;
    }
    audit_detail
}
