use super::catalog::normalize_official_entries;
use super::catalog_projection::{
    provider_catalog_snapshot, record_failed_catalog_projection,
    refresh_provider_package_catalog_projection,
};
use super::*;
use sha2::{Digest, Sha256};

use crate::ports::RetainedFrontendModuleAssetInput;

pub struct InstallPluginCommand {
    pub actor_user_id: Uuid,
    pub package_root: String,
}

pub struct InstallOfficialPluginCommand {
    pub actor_user_id: Uuid,
    pub plugin_id: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
    pub risk_override: Option<PluginRiskOverride>,
}

pub struct InstallResolvedOfficialPluginCommand {
    pub actor_user_id: Uuid,
    pub plugin_id: String,
    pub plugin_type: String,
    pub minimum_host_version: String,
    pub source_kind: String,
    pub file_name: String,
    pub package_bytes: Vec<u8>,
    pub expected_checksum: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
    pub risk_override: Option<PluginRiskOverride>,
}

pub struct InstallOfficialExtensionCommand {
    pub actor_user_id: Uuid,
    pub artifact_id: String,
    pub expected_plugin_type: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
    pub risk_override: Option<PluginRiskOverride>,
}

pub struct InstallUploadedPluginCommand {
    pub actor_user_id: Uuid,
    pub file_name: String,
    pub package_bytes: Vec<u8>,
}

pub struct InstallExtensionNodePluginCommand {
    pub actor_user_id: Uuid,
    pub category: ExtensionCatalogCategory,
    pub file_name: String,
    pub package_bytes: Vec<u8>,
    pub source_kind: String,
}

pub struct EnsureBuiltinPluginCommand {
    pub actor_user_id: Uuid,
    pub package_root: String,
}

pub struct UpgradeLatestPluginFamilyCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
    pub risk_override: Option<PluginRiskOverride>,
}

fn official_package_risk_warnings(
    package_bytes: &[u8],
    expected_checksum: &str,
    intake: &PackageIntakeResult,
) -> Vec<String> {
    let actual = format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(package_bytes))
    );
    let mut warnings = Vec::new();
    if actual != expected_checksum {
        warnings.push(PLUGIN_RISK_CHECKSUM_MISMATCH.to_string());
    }
    match intake.signature_status.as_str() {
        "verified" => {}
        "unsigned" => warnings.push(PLUGIN_RISK_SIGNATURE_MISSING.to_string()),
        "unknown_key" => warnings.push(PLUGIN_RISK_SIGNING_KEY_UNKNOWN.to_string()),
        _ => warnings.push(PLUGIN_RISK_SIGNATURE_INVALID.to_string()),
    }
    warnings
}

#[derive(Debug, Clone)]
pub struct InstallPluginResult {
    pub installation: domain::PluginInstallationRecord,
    pub local_artifact: domain::PluginArtifactInstanceRecord,
    pub task: domain::PluginTaskRecord,
}

struct InstallSourceMetadata {
    source_kind: String,
    trust_level: String,
    checksum: Option<String>,
    signature_status: Option<String>,
    signature_algorithm: Option<String>,
    signing_key_id: Option<String>,
    package_bytes: Option<Vec<u8>>,
}

impl InstallSourceMetadata {
    fn uploaded_manual_install() -> Self {
        Self {
            source_kind: "uploaded".to_string(),
            trust_level: "checksum_only".to_string(),
            checksum: None,
            signature_status: Some("unsigned".to_string()),
            signature_algorithm: None,
            signing_key_id: None,
            package_bytes: None,
        }
    }
}

pub(super) async fn load_actor_context_for_user<R>(
    repository: &R,
    actor_user_id: Uuid,
) -> Result<domain::ActorContext>
where
    R: AuthRepository,
{
    let scope = repository.default_scope_for_user(actor_user_id).await?;
    repository
        .load_actor_context(actor_user_id, scope.tenant_id, scope.workspace_id, None)
        .await
}

pub(super) fn load_provider_package(path: impl AsRef<Path>) -> Result<ProviderPackage> {
    ProviderPackage::load_from_dir(path.as_ref()).map_err(map_framework_error)
}

pub(super) fn load_plugin_manifest(path: impl AsRef<Path>) -> Result<PluginManifestV1> {
    let package_root = path.as_ref();
    let manifest_path = package_root.join("manifest.yaml");
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read plugin manifest at {}",
            manifest_path.display()
        )
    })?;
    let manifest = parse_plugin_manifest(&raw).map_err(map_framework_error)?;
    plugin_framework::validate_frontend_module_assets(package_root, &manifest)
        .map_err(map_framework_error)?;
    Ok(manifest)
}

trait InstallationProjectionIdentity {
    fn installation_id(&self) -> Uuid;
    fn provider_code(&self) -> &str;
    fn plugin_id(&self) -> &str;
    fn plugin_version(&self) -> &str;
}

#[derive(Debug, Clone)]
struct PreparedPluginInstallationInput {
    installation_id: Uuid,
    provider_code: String,
    plugin_id: String,
    plugin_version: String,
    contract_version: String,
    protocol: String,
    display_name: String,
    source_kind: String,
    trust_level: String,
    verification_status: domain::PluginVerificationStatus,
    desired_state: domain::PluginDesiredState,
    runtime_status: domain::PluginRuntimeStatus,
    availability_status: domain::PluginAvailabilityStatus,
    package_path: Option<String>,
    installed_path: String,
    checksum: Option<String>,
    manifest_fingerprint: Option<String>,
    signature_status: Option<String>,
    signature_algorithm: Option<String>,
    signing_key_id: Option<String>,
    last_load_error: Option<String>,
    metadata_json: serde_json::Value,
    actor_user_id: Uuid,
}

impl InstallationProjectionIdentity for PreparedPluginInstallationInput {
    fn installation_id(&self) -> Uuid {
        self.installation_id
    }
    fn provider_code(&self) -> &str {
        &self.provider_code
    }
    fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
    fn plugin_version(&self) -> &str {
        &self.plugin_version
    }
}

impl InstallationProjectionIdentity for domain::PluginInstallationRecord {
    fn installation_id(&self) -> Uuid {
        self.id
    }
    fn provider_code(&self) -> &str {
        &self.provider_code
    }
    fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
    fn plugin_version(&self) -> &str {
        &self.plugin_version
    }
}

mod plugin_settings_templates;
use plugin_settings_templates::prepare_installation_commit;
mod projections;
use projections::{
    build_frontend_block_sync_input, build_js_dependency_sync_input,
    build_node_contribution_sync_input,
};

// The contract and declaration identify a system service; legacy records may also use nil scope.
fn is_system_managed_service(installation: &domain::PluginInstallationRecord) -> bool {
    domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
        == domain::SYSTEM_SCOPE_ID
}

fn stable_plugin_unique_identifier(plugin_id: &str) -> String {
    plugin_id
        .split_once('@')
        .map(|(stable_id, _)| stable_id)
        .unwrap_or(plugin_id)
        .to_string()
}

fn stable_sha256_json(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)))
}

fn prepare_installation_commit_blocking(
    package_root: &Path,
    node_id: &str,
    mut installation: PreparedPluginInstallationInput,
    manifest: &PluginManifestV1,
    package_catalog: Option<UpsertPluginPackageCatalogProjectionInput>,
) -> Result<CommitPluginInstallationInput> {
    if let Some(service) = &manifest.managed_service {
        installation.metadata_json["managed_service"] = serde_json::to_value(service)?;
        installation.metadata_json["managed_service_permissions"] = serde_json::json!({"secrets": manifest.permissions.secrets, "storage": manifest.permissions.storage});
    }
    installation.metadata_json["communication_commands"] =
        serde_json::to_value(&manifest.communication_commands)?;
    if !manifest.notification_types.is_empty() {
        installation.metadata_json["notification_types"] =
            serde_json::to_value(&manifest.notification_types)?;
    }
    if !manifest.authentication_providers.is_empty() {
        installation.metadata_json["authentication_providers"] =
            serde_json::to_value(&manifest.authentication_providers)?;
    }
    installation.metadata_json["binding_targets"] =
        serde_json::to_value(&manifest.binding_targets)?;
    if let Some(managed) = &manifest.managed {
        installation.metadata_json["managed"] = serde_json::to_value(managed)?;
    }
    let settings_templates =
        plugin_settings_templates::prepare_settings_templates(package_root, manifest)?;
    let package_kind = route_plugin_package(manifest)?;
    let category = match &package_kind {
        RoutedPluginPackageKind::HostExtension => domain::ExtensionCategory::HostExtensions,
        RoutedPluginPackageKind::ModelProviderRuntime
        | RoutedPluginPackageKind::DataSourceRuntime
        | RoutedPluginPackageKind::NetworkEgressProviderRuntime
        | RoutedPluginPackageKind::ProviderDistributionRuleRuntime => {
            domain::ExtensionCategory::RuntimeExtensions
        }
        RoutedPluginPackageKind::CapabilityPlugin => domain::ExtensionCategory::CapabilityPlugins,
        RoutedPluginPackageKind::ManagedContributions => match manifest.consumption_kind {
            PluginConsumptionKind::RuntimeExtension => domain::ExtensionCategory::RuntimeExtensions,
            PluginConsumptionKind::CapabilityPlugin => domain::ExtensionCategory::CapabilityPlugins,
            PluginConsumptionKind::HostExtension => domain::ExtensionCategory::HostExtensions,
        },
    };
    let artifact_instance = UpsertPluginArtifactInstanceInput {
        node_id: node_id.to_string(),
        installation_id: installation.installation_id,
        local_version: Some(installation.plugin_version.clone()),
        local_checksum: installation.checksum.clone(),
        local_path: Some(installation.installed_path.clone()),
        package_path: installation.package_path.clone(),
        manifest_fingerprint: installation.manifest_fingerprint.clone(),
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: installation.runtime_status,
        availability_status: installation.availability_status,
        checked_at: OffsetDateTime::now_utc(),
        last_error: installation.last_load_error.clone(),
        // Network egress is system/node scoped: a successful install selects this version for
        // new proxy creation while retained installations remain bound to existing providers.
        is_current: matches!(
            package_kind,
            RoutedPluginPackageKind::NetworkEgressProviderRuntime
        ),
    };
    let node_contributions = build_node_contribution_sync_input(&installation, manifest);
    let js_dependencies = build_js_dependency_sync_input(&installation, manifest);
    let frontend_blocks = build_frontend_block_sync_input(&installation, manifest);
    let mut retained_frontend_module_assets = Vec::new();
    for module in manifest
        .block_contributions
        .iter()
        .flat_map(|entry| &entry.code_modules)
        .chain(
            manifest
                .settings_pages
                .iter()
                .flat_map(|entry| &entry.code_modules),
        )
    {
        for asset in &module.assets {
            retained_frontend_module_assets.push(RetainedFrontendModuleAssetInput {
                module_source: module.source.clone(),
                sha256: asset.sha256.clone(),
                media_type: asset.media_type.clone(),
                bytes: plugin_framework::load_frontend_module_asset(package_root, asset)
                    .map_err(map_framework_error)?,
            });
        }
    }
    let root = UpsertPluginInstallationInput {
        installation_id: installation.installation_id,
        category,
        organization: manifest.publisher_namespace.clone(),
        provider_code: installation.provider_code,
        plugin_id: installation.plugin_id,
        plugin_version: installation.plugin_version,
        contract_version: installation.contract_version,
        protocol: installation.protocol,
        display_name: installation.display_name,
        source_kind: installation.source_kind.clone(),
        trust_level: installation.trust_level,
        verification_status: installation.verification_status,
        desired_state: installation.desired_state,
        expected_checksum: installation.checksum,
        signature_status: extension_signature_status(installation.signature_status.as_deref()),
        signature_algorithm: installation.signature_algorithm,
        signing_key_id: installation.signing_key_id,
        metadata_json: installation.metadata_json,
        is_system_reserved: installation.source_kind == "builtin",
        actor_user_id: installation.actor_user_id,
    };
    Ok(CommitPluginInstallationInput {
        settings_templates,
        installation: root,
        artifact_instance,
        package_catalog,
        node_contributions,
        js_dependencies,
        frontend_blocks,
        retained_frontend_module_assets,
    })
}

pub(super) fn extension_signature_status(value: Option<&str>) -> domain::ExtensionSignatureStatus {
    match value {
        Some("verified" | "builtin") => domain::ExtensionSignatureStatus::Verified,
        Some("unknown_key") => domain::ExtensionSignatureStatus::UnknownKey,
        Some("invalid" | "malformed_signature") => domain::ExtensionSignatureStatus::Invalid,
        _ => domain::ExtensionSignatureStatus::Missing,
    }
}

pub(super) fn map_model_discovery_mode(
    mode: plugin_framework::provider_contract::ModelDiscoveryMode,
) -> domain::ModelProviderDiscoveryMode {
    match mode {
        plugin_framework::provider_contract::ModelDiscoveryMode::Static => {
            domain::ModelProviderDiscoveryMode::Static
        }
        plugin_framework::provider_contract::ModelDiscoveryMode::Dynamic => {
            domain::ModelProviderDiscoveryMode::Dynamic
        }
        plugin_framework::provider_contract::ModelDiscoveryMode::Hybrid => {
            domain::ModelProviderDiscoveryMode::Hybrid
        }
    }
}

pub(super) fn map_catalog_source(
    mode: plugin_framework::provider_contract::ModelDiscoveryMode,
) -> domain::ModelProviderCatalogSource {
    match mode {
        plugin_framework::provider_contract::ModelDiscoveryMode::Static => {
            domain::ModelProviderCatalogSource::Static
        }
        plugin_framework::provider_contract::ModelDiscoveryMode::Dynamic => {
            domain::ModelProviderCatalogSource::Dynamic
        }
        plugin_framework::provider_contract::ModelDiscoveryMode::Hybrid => {
            domain::ModelProviderCatalogSource::Hybrid
        }
    }
}

pub(super) fn map_framework_error(
    error: plugin_framework::error::PluginFrameworkError,
) -> anyhow::Error {
    use plugin_framework::error::PluginFrameworkErrorKind;

    match error.kind() {
        PluginFrameworkErrorKind::InvalidAssignment
        | PluginFrameworkErrorKind::InvalidProviderPackage
        | PluginFrameworkErrorKind::InvalidProviderContract
        | PluginFrameworkErrorKind::Serialization => {
            ControlPlaneError::InvalidInput("provider_package").into()
        }
        PluginFrameworkErrorKind::Io | PluginFrameworkErrorKind::RuntimeContract => {
            ControlPlaneError::UpstreamUnavailable("provider_runtime").into()
        }
    }
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository
        + FrontendBlockCatalogRepository,
    H: ProviderRuntimePort,
{
    pub async fn ensure_builtin_plugin(
        &self,
        command: EnsureBuiltinPluginCommand,
    ) -> Result<domain::PluginInstallationRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_root_actor(&actor)?;

        let package_root = Path::new(&command.package_root);
        let manifest = load_plugin_manifest(package_root)?;
        if manifest.source_kind != "builtin"
            || manifest.trust_level != "verified_official"
            || manifest.consumption_kind != PluginConsumptionKind::CapabilityPlugin
            || manifest.execution_mode != PluginExecutionMode::DeclarativeOnly
            || manifest.selection_mode != "auto_activate"
            || !manifest
                .binding_targets
                .iter()
                .any(|target| target == "workspace")
            || !manifest
                .slot_codes
                .iter()
                .any(|slot| slot == "frontend_block")
        {
            return Err(ControlPlaneError::InvalidInput("builtin_plugin_manifest").into());
        }

        let provider_code = manifest
            .plugin_code()
            .map_err(map_framework_error)?
            .to_string();
        let plugin_id = manifest
            .versioned_plugin_id()
            .map_err(map_framework_error)?;
        let installation_id = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .find(|installation| installation.plugin_id == plugin_id)
            .map(|installation| installation.id)
            .unwrap_or_else(Uuid::now_v7);
        let install_path = self
            .install_root
            .join("installed")
            .join(&provider_code)
            .join(&manifest.version);
        let lease = self
            .repository
            .begin_plugin_installation(&crate::ports::PluginInstallationAdmission {
                plugin_id: plugin_id.clone(),
                content: crate::ports::PluginInstallationContentIdentity::Legacy,
            })
            .await?;
        let staged_installation =
            filesystem::StagedArtifactPath::prepare_directory(package_root, &install_path)?;
        let manifest_fingerprint =
            compute_manifest_fingerprint(&staged_installation.staged_path().join("manifest.yaml"))
                .await
                .map_err(map_framework_error)?;
        write_artifact_marker(
            staged_installation.staged_path(),
            &plugin_id,
            &manifest.version,
            None,
            Some(&manifest_fingerprint),
            Some("builtin"),
            Some("verified_official"),
            Some("builtin"),
            None,
            None,
        )?;

        let installation_input = PreparedPluginInstallationInput {
            installation_id,
            provider_code,
            plugin_id,
            plugin_version: manifest.version.clone(),
            contract_version: manifest.contract_version.clone(),
            protocol: manifest.runtime.protocol.clone(),
            display_name: manifest.display_name.clone(),
            source_kind: "builtin".to_string(),
            trust_level: "verified_official".to_string(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: install_path.display().to_string(),
            checksum: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            signature_status: Some("builtin".to_string()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({
                "install_kind": "builtin_bootstrap",
                "plugin_type": "capability_plugin",
                "selection_mode": manifest.selection_mode,
                "block_contributions": manifest
                    .block_contributions
                    .iter()
                    .map(|entry| entry.contribution_code.clone())
                    .collect::<Vec<_>>(),
            }),
            actor_user_id: command.actor_user_id,
        };
        let installation_input = prepare_installation_commit(
            staged_installation.staged_path(),
            &self.node_id,
            installation_input,
            &manifest,
            None,
        )
        .await?;
        let installation = super::managed_installation::publish_installation(
            lease,
            staged_installation,
            None,
            installation_input,
        )
        .await?;

        Ok(installation)
    }

    pub async fn install_plugin(
        &self,
        command: InstallPluginCommand,
    ) -> Result<InstallPluginResult> {
        let package_root = command.package_root.clone();
        self.install_plugin_with_metadata(
            command,
            InstallSourceMetadata::uploaded_manual_install(),
            json!({
                "install_kind": "uploaded_manual_install",
                "package_root": package_root,
            }),
        )
        .await
    }

    pub async fn reconcile_all_installations(&self) -> Result<()> {
        self.reconcile_pending_artifact_removals().await?;
        let assigned_installation_ids = self
            .repository
            .list_assigned_installation_ids()
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        for installation in self.repository.list_installations().await? {
            let local_artifact = self
                .refresh_current_node_artifact_snapshot(&installation)
                .await?;
            if !local_artifact.artifact_status.is_ready() {
                continue;
            }
            if local_artifact.local_path.is_none() {
                continue;
            }
            let local_installation = domain::LocalPluginInstallationRecord {
                installation: installation.clone(),
                artifact: local_artifact,
            };

            if is_model_provider_installation(&installation) {
                match crate::installed_provider_package::load_installed_provider_package(
                    &local_installation,
                ) {
                    Ok(package) => {
                        refresh_provider_package_catalog_projection(
                            &self.repository,
                            &installation,
                            &package,
                        )
                        .await?;
                    }
                    Err(error) => {
                        record_failed_catalog_projection(&self.repository, &installation, &error)
                            .await?;
                        continue;
                    }
                }
            }
            if installation.desired_state == domain::PluginDesiredState::ActiveRequested
                && (is_system_managed_service(&installation)
                    || assigned_installation_ids.contains(&installation.id))
            {
                match self.runtime.activate_plugin(&local_installation).await {
                    Ok(()) => {
                        self.mark_current_node_runtime_status(
                            &installation,
                            domain::PluginRuntimeStatus::Active,
                            None,
                        )
                        .await?;
                    }
                    Err(error) => {
                        self.mark_current_node_runtime_status(
                            &installation,
                            domain::PluginRuntimeStatus::LoadFailed,
                            Some(error.to_string()),
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn ensure_initial_managed_assignment(
        &self,
        actor_user_id: Uuid,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<()> {
        if installation.contract_version != "1flowbase.extension-bus/v1"
            || !crate::plugin_management::family::supports_workspace_assignment(installation)
        {
            return Ok(());
        }
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        // Establish first-install scope without replacing an existing family selection,
        // including when another install wins concurrently. Assignment grants no capability.
        if self
            .repository
            .create_initial_assignment(&CreatePluginAssignmentInput {
                installation_id: installation.id,
                workspace_id: actor.current_workspace_id,
                provider_code: installation.provider_code.clone(),
                actor_user_id,
            })
            .await?
            .is_some()
        {
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "plugin_assignment",
                    Some(installation.id),
                    "plugin.assigned",
                    json!({"provider_code":installation.provider_code}),
                ))
                .await?;
            self.invalidate_model_routing_catalog(actor.current_workspace_id)
                .await;
        }
        Ok(())
    }

    async fn enable_and_assign_model_provider_installation(
        &self,
        actor_user_id: Uuid,
        installation_id: Uuid,
    ) -> Result<InstallPluginResult> {
        let before = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_initial_managed_assignment(actor_user_id, &before)
            .await?;
        let enabled = self
            .enable_plugin(EnablePluginCommand {
                actor_user_id,
                installation_id,
            })
            .await?;
        // Managed version selection is owned by the validated switch operation. Its
        // first assignment already exists; generic install must not overwrite a family.
        let task = if before.contract_version == "1flowbase.extension-bus/v1" {
            enabled
        } else {
            self.assign_plugin(AssignPluginCommand {
                actor_user_id,
                installation_id,
            })
            .await?
        };
        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let local_artifact = self
            .repository
            .get_artifact_instance(&self.node_id, installation.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_artifact_instance"))?;
        Ok(InstallPluginResult {
            installation,
            local_artifact,
            task,
        })
    }

    pub async fn install_official_extension(
        &self,
        command: InstallOfficialExtensionCommand,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let snapshot = self
            .official_source
            .list_official_catalog_for_workspace(actor.current_workspace_id)
            .await?;
        let entry = snapshot
            .entries
            .into_iter()
            .find(|item| item.plugin_id == command.artifact_id)
            .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
        if entry.plugin_type != command.expected_plugin_type {
            return Err(ControlPlaneError::InvalidInput("extension_catalog_category").into());
        }
        let compatibility_override = validate_official_plugin_compatibility_override(
            &entry,
            &self.host_version,
            command.compatibility_override.as_ref(),
        )?;
        let downloaded = self
            .official_source
            .download_plugin_for_workspace(actor.current_workspace_id, &entry)
            .await?;
        let intake = intake_package_bytes(
            &downloaded.package_bytes,
            &PackageIntakePolicy {
                source_kind: snapshot.source.source_kind,
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(downloaded.file_name.clone()),
            },
        )
        .await?;
        let risk_warnings = official_package_risk_warnings(
            &downloaded.package_bytes,
            &entry.selected_artifact.checksum,
            &intake,
        );
        let risk_override =
            validate_plugin_risk_override(&risk_warnings, command.risk_override.as_ref())?;
        if catalog_plugin_type(&intake.manifest)? != entry.plugin_type {
            return Err(ControlPlaneError::Conflict("extension_catalog_package_mismatch").into());
        }
        let mut detail_json = json!({
            "install_kind": "extension_center_official_install",
            "plugin_id": command.artifact_id,
            "file_name": downloaded.file_name,
            "risk_warnings": risk_warnings,
        });
        if let Some(value) = compatibility_override {
            detail_json["compatibility_override"] = value;
        }
        if let Some(value) = risk_override {
            detail_json["risk_override"] = value;
        }
        self.install_intake_result(
            command.actor_user_id,
            intake,
            Some(downloaded.package_bytes),
            detail_json,
        )
        .await
    }

    pub async fn upgrade_latest(
        &self,
        command: UpgradeLatestPluginFamilyCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let current = self
            .load_current_family_installation(actor.current_workspace_id, &command.provider_code)
            .await?;
        self.ensure_model_provider_target(&current)?;
        let official_entry = self
            .official_source
            .list_official_catalog_for_workspace(actor.current_workspace_id)
            .await?
            .entries;
        let official_entry = normalize_official_entries(official_entry)
            .into_iter()
            .find(|entry| entry.provider_code == command.provider_code)
            .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
        if self.is_model_provider_console_operation()
            && official_entry.plugin_type != "model_provider"
        {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        let compatibility_override = validate_official_plugin_compatibility_override(
            &official_entry,
            &self.host_version,
            command.compatibility_override.as_ref(),
        )?;
        let installed_target = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .find(|installation| {
                installation.provider_code == command.provider_code
                    && installation.category == current.category
                    && installation.scope_id == current.scope_id
                    && installation.organization == current.organization
                    && installation.plugin_version == official_entry.latest_version
            });
        let target = match installed_target {
            Some(installation) => installation,
            None => {
                let downloaded = self
                    .official_source
                    .download_plugin_for_workspace(actor.current_workspace_id, &official_entry)
                    .await?;
                let snapshot = self
                    .official_source
                    .list_official_catalog_for_workspace(actor.current_workspace_id)
                    .await?;
                let snapshot_entry = normalize_official_entries(snapshot.entries)
                    .into_iter()
                    .find(|entry| entry.provider_code == command.provider_code)
                    .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
                let intake = intake_package_bytes(
                    &downloaded.package_bytes,
                    &PackageIntakePolicy {
                        source_kind: snapshot.source.source_kind.clone(),
                        trust_mode: "allow_unsigned".to_string(),
                        expected_artifact_sha256: None,
                        trusted_public_keys: self.official_source.trusted_public_keys(),
                        original_filename: Some(downloaded.file_name.clone()),
                    },
                )
                .await?;
                let risk_warnings = official_package_risk_warnings(
                    &downloaded.package_bytes,
                    &snapshot_entry.selected_artifact.checksum,
                    &intake,
                );
                let risk_override =
                    validate_plugin_risk_override(&risk_warnings, command.risk_override.as_ref())?;
                let mut detail_json = json!({
                    "install_kind": "official_upgrade",
                    "plugin_id": snapshot_entry.plugin_id,
                    "provider_code": snapshot_entry.provider_code,
                    "file_name": downloaded.file_name,
                });
                if let Some(compatibility_override) = compatibility_override.clone() {
                    detail_json["compatibility_override"] = compatibility_override;
                }
                detail_json["risk_warnings"] = json!(risk_warnings);
                if let Some(risk_override) = risk_override {
                    detail_json["risk_override"] = risk_override;
                }
                self.install_intake_result(
                    command.actor_user_id,
                    intake,
                    Some(downloaded.package_bytes.clone()),
                    detail_json,
                )
                .await?
                .installation
            }
        };
        self.ensure_model_provider_target(&target)?;
        if current.id == target.id {
            return Err(ControlPlaneError::Conflict("plugin_version_already_current").into());
        }

        self.switch_family_installation(
            &actor,
            &command.provider_code,
            &current,
            &target,
            command.actor_user_id,
            compatibility_override.as_ref(),
        )
        .await
    }

    pub(super) async fn install_intake_result(
        &self,
        actor_user_id: Uuid,
        intake: PackageIntakeResult,
        package_bytes: Option<Vec<u8>>,
        detail_json: serde_json::Value,
    ) -> Result<InstallPluginResult> {
        let package_root = intake.extracted_root.clone();
        let result = async {
            plugin_framework::ensure_package_runtime_target_compatible(
                &package_root,
                &intake.manifest,
            )?;
            self.install_plugin_with_metadata(
                InstallPluginCommand {
                    actor_user_id,
                    package_root: package_root.display().to_string(),
                },
                InstallSourceMetadata {
                    source_kind: intake.source_kind,
                    trust_level: intake.trust_level,
                    checksum: intake.checksum,
                    signature_status: Some(intake.signature_status),
                    signature_algorithm: intake.signature_algorithm,
                    signing_key_id: intake.signing_key_id,
                    package_bytes,
                },
                detail_json,
            )
            .await
        }
        .await;
        let _ = fs::remove_dir_all(&package_root);
        result
    }
}

mod source_installation;

#[path = "install/installation_commit.rs"]
mod installation_commit;
