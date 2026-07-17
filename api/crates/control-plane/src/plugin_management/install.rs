use super::catalog::normalize_official_entries;
use super::catalog_projection::{
    record_failed_catalog_projection, refresh_provider_package_catalog_projection,
};
use super::*;
use sha2::{Digest, Sha256};

pub struct InstallPluginCommand {
    pub actor_user_id: Uuid,
    pub package_root: String,
}

pub struct InstallOfficialPluginCommand {
    pub actor_user_id: Uuid,
    pub plugin_id: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
}

pub struct InstallUploadedPluginCommand {
    pub actor_user_id: Uuid,
    pub file_name: String,
    pub package_bytes: Vec<u8>,
}

pub struct EnsureBuiltinPluginCommand {
    pub actor_user_id: Uuid,
    pub package_root: String,
}

pub struct UpgradeLatestPluginFamilyCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
    pub compatibility_override: Option<PluginCompatibilityOverride>,
}

#[derive(Debug, Clone)]
pub struct InstallPluginResult {
    pub installation: domain::PluginInstallationRecord,
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
    let manifest_path = path.as_ref().join("manifest.yaml");
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read plugin manifest at {}",
            manifest_path.display()
        )
    })?;
    parse_plugin_manifest(&raw).map_err(map_framework_error)
}

trait InstallationProjectionIdentity {
    fn installation_id(&self) -> Uuid;
    fn provider_code(&self) -> &str;
    fn plugin_id(&self) -> &str;
    fn plugin_version(&self) -> &str;
}

impl InstallationProjectionIdentity for UpsertPluginInstallationInput {
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

fn build_node_contribution_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationNodeContributionsInput {
    let plugin_unique_identifier = stable_plugin_unique_identifier(installation.plugin_id());
    let package_id = installation.plugin_id().to_string();

    ReplaceInstallationNodeContributionsInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .node_contributions
            .iter()
            .map(|entry| NodeContributionRegistryInput {
                plugin_unique_identifier: plugin_unique_identifier.clone(),
                package_id: package_id.clone(),
                contribution_code: entry.contribution_code.clone(),
                node_shell: entry.node_shell.clone(),
                category: entry.category.clone(),
                title: entry.title.clone(),
                description: entry.description.clone(),
                icon: entry.icon.clone(),
                schema_ui: entry.schema_ui.clone(),
                schema_version: entry.schema_version.clone(),
                output_schema: entry.output_schema.clone(),
                contribution_checksum: stable_sha256_json(
                    &serde_json::to_value(entry).unwrap_or_else(|_| json!({})),
                ),
                compiled_contribution_hash: stable_sha256_json(&json!({
                    "schema_version": entry.schema_version,
                    "node_shell": entry.node_shell,
                    "schema_ui": entry.schema_ui,
                    "output_schema": entry.output_schema,
                    "side_effect_policy": entry.side_effect_policy,
                    "infra_contracts": entry.infra_contracts,
                })),
                output_schema_snapshot: entry.output_schema.clone(),
                side_effect_policy: entry.side_effect_policy.clone(),
                infra_contracts: entry.infra_contracts.clone(),
                required_auth: entry.required_auth.clone(),
                visibility: entry.visibility.clone(),
                experimental: entry.experimental,
                dependency_installation_kind: entry.dependency.installation_kind.clone(),
                dependency_plugin_version_range: entry.dependency.plugin_version_range.clone(),
            })
            .collect(),
    }
}

fn build_js_dependency_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationJsDependenciesInput {
    ReplaceInstallationJsDependenciesInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .js_dependencies
            .iter()
            .flat_map(|dependency| {
                dependency.targets.iter().filter_map(|target| {
                    dependency.artifacts.get(target).map(|artifact_path| {
                        JsDependencyRegistryInput {
                            alias: dependency.alias.clone(),
                            package: dependency.package.clone(),
                            version: dependency.version.clone(),
                            target: target.clone(),
                            artifact_path: artifact_path.clone(),
                            integrity: dependency.integrity.clone(),
                            permissions: domain::JsDependencyPermissions {
                                network: dependency.permissions.network.clone(),
                                filesystem: dependency.permissions.filesystem.clone(),
                                env: dependency.permissions.env.clone(),
                            },
                        }
                    })
                })
            })
            .collect(),
    }
}

fn build_frontend_block_sync_input(
    installation: &impl InstallationProjectionIdentity,
    manifest: &PluginManifestV1,
) -> ReplaceInstallationFrontendBlocksInput {
    ReplaceInstallationFrontendBlocksInput {
        installation_id: installation.installation_id(),
        provider_code: installation.provider_code().to_string(),
        plugin_id: installation.plugin_id().to_string(),
        plugin_version: installation.plugin_version().to_string(),
        entries: manifest
            .block_contributions
            .iter()
            .map(|block| FrontendBlockCatalogRegistryInput {
                contribution_code: block.contribution_code.clone(),
                title: block.title.clone(),
                runtime: block.runtime.clone(),
                entry: block.entry.clone(),
                code_template: block.code_template.clone(),
                code_template_version: block.code_template_version.clone(),
                code_template_language: block.code_template_language.clone(),
                code_modules: block
                    .code_modules
                    .iter()
                    .map(|code_module| domain::FrontendBlockCodeModule {
                        source: code_module.source.clone(),
                        type_declarations: code_module.type_declarations.clone(),
                    })
                    .collect(),
                context_contract: domain::FrontendBlockContextContract {
                    primitives: block.context_contract.primitives.clone(),
                    input_schema: block.context_contract.input_schema.clone(),
                },
                permissions: domain::FrontendBlockPermissions {
                    network: block.permissions.network.clone(),
                    storage: block.permissions.storage.clone(),
                    secrets: block.permissions.secrets.clone(),
                },
                ui_capabilities: block.ui_capabilities.clone(),
            })
            .collect(),
    }
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
    format!("sha256:{:x}", Sha256::digest(bytes))
}

async fn commit_prepared_installation<R>(
    repository: &R,
    node_id: &str,
    installation: UpsertPluginInstallationInput,
    manifest: &PluginManifestV1,
    package_catalog: Option<UpsertPluginPackageCatalogProjectionInput>,
) -> Result<domain::PluginInstallationRecord>
where
    R: PluginRepository,
{
    let artifact_instance = UpsertPluginArtifactInstanceInput {
        node_id: node_id.to_string(),
        installation_id: installation.installation_id,
        local_version: Some(installation.plugin_version.clone()),
        local_checksum: installation.checksum.clone(),
        installed_path: Some(installation.installed_path.clone()),
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: domain::PluginRuntimeStatus::Inactive,
        checked_at: OffsetDateTime::now_utc(),
        last_error: None,
    };
    let node_contributions = build_node_contribution_sync_input(&installation, manifest);
    let js_dependencies = build_js_dependency_sync_input(&installation, manifest);
    let frontend_blocks = build_frontend_block_sync_input(&installation, manifest);
    repository
        .commit_plugin_installation_projection(&CommitPluginInstallationProjectionInput {
            installation,
            artifact_instance,
            package_catalog,
            node_contributions,
            js_dependencies,
            frontend_blocks,
        })
        .await
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
        let mut staged_installation =
            filesystem::StagedArtifactPath::prepare_directory(package_root, &install_path)?;
        let manifest_fingerprint =
            compute_manifest_fingerprint(&staged_installation.staged_path().join("manifest.yaml"))
                .await
                .map_err(map_framework_error)?;
        staged_installation.activate()?;

        let installation_input = UpsertPluginInstallationInput {
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
            artifact_status: domain::PluginArtifactStatus::Ready,
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
                "selection_mode": manifest.selection_mode,
                "block_contributions": manifest
                    .block_contributions
                    .iter()
                    .map(|entry| entry.contribution_code.clone())
                    .collect::<Vec<_>>(),
            }),
            actor_user_id: command.actor_user_id,
        };
        let installation_result = commit_prepared_installation(
            &self.repository,
            &self.node_id,
            installation_input,
            &manifest,
            None,
        )
        .await;
        let installation = match installation_result {
            Ok(installation) => {
                staged_installation.finish();
                installation
            }
            Err(error) => {
                let _ = staged_installation.rollback();
                return Err(error);
            }
        };

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
        for installation in self.repository.list_installations().await? {
            let local_artifact = self
                .refresh_current_node_artifact_snapshot(&installation)
                .await?;
            if !is_model_provider_installation(&installation) {
                continue;
            }
            if !local_artifact.artifact_status.is_ready() {
                continue;
            }
            let Some(installed_path) = local_artifact.installed_path.as_deref() else {
                continue;
            };

            match load_provider_package(installed_path) {
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
                }
            }
        }

        Ok(())
    }

    pub async fn install_uploaded_plugin(
        &self,
        command: InstallUploadedPluginCommand,
    ) -> Result<InstallPluginResult> {
        let file_name = command.file_name.clone();
        let intake = intake_package_bytes(
            &command.package_bytes,
            &PackageIntakePolicy {
                source_kind: "uploaded".to_string(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(file_name.clone()),
            },
        )
        .await?;
        self.ensure_model_provider_package_kind(route_plugin_package(&intake.manifest)?)?;
        self.install_intake_result(
            command.actor_user_id,
            intake,
            Some(command.package_bytes),
            json!({
                "install_kind": "upload",
                "file_name": file_name,
            }),
        )
        .await
    }

    pub async fn install_official_plugin(
        &self,
        command: InstallOfficialPluginCommand,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let snapshot = self.official_source.list_official_catalog().await?;
        let entry = snapshot
            .entries
            .into_iter()
            .find(|item| item.plugin_id == command.plugin_id)
            .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
        if self.is_model_provider_console_operation() && entry.plugin_type != "model_provider" {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        let compatibility_override = validate_official_plugin_compatibility_override(
            &entry,
            &self.host_version,
            command.compatibility_override.as_ref(),
        )?;
        let downloaded = self.official_source.download_plugin(&entry).await?;
        let intake = intake_package_bytes(
            &downloaded.package_bytes,
            &PackageIntakePolicy {
                source_kind: snapshot.source.source_kind.clone(),
                trust_mode: entry.trust_mode.clone(),
                expected_artifact_sha256: Some(entry.selected_artifact.checksum.clone()),
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(downloaded.file_name.clone()),
            },
        )
        .await?;
        self.ensure_model_provider_package_kind(route_plugin_package(&intake.manifest)?)?;
        let result = async {
            let mut detail_json = json!({
                "install_kind": "official_source",
                "plugin_id": command.plugin_id,
                "file_name": downloaded.file_name,
            });
            if let Some(compatibility_override) = compatibility_override.clone() {
                detail_json["compatibility_override"] = compatibility_override;
            }
            let install = self
                .install_intake_result(
                    command.actor_user_id,
                    intake,
                    Some(downloaded.package_bytes.clone()),
                    detail_json,
                )
                .await?;
            if is_host_extension_installation(&install.installation) {
                return Ok::<InstallPluginResult, anyhow::Error>(install);
            }
            self.enable_plugin(EnablePluginCommand {
                actor_user_id: command.actor_user_id,
                installation_id: install.installation.id,
            })
            .await?;
            let task = self
                .assign_plugin(AssignPluginCommand {
                    actor_user_id: command.actor_user_id,
                    installation_id: install.installation.id,
                })
                .await?;
            let installation = self
                .repository
                .get_installation(install.installation.id)
                .await?
                .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
            Ok::<InstallPluginResult, anyhow::Error>(InstallPluginResult { installation, task })
        }
        .await;
        result
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
        let official_entry = self.official_source.list_official_catalog().await?.entries;
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
                    && installation.plugin_version == official_entry.latest_version
            });
        let target = match installed_target {
            Some(installation) => installation,
            None => {
                let downloaded = self
                    .official_source
                    .download_plugin(&official_entry)
                    .await?;
                let snapshot = self.official_source.list_official_catalog().await?;
                let snapshot_entry = normalize_official_entries(snapshot.entries)
                    .into_iter()
                    .find(|entry| entry.provider_code == command.provider_code)
                    .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
                let intake = intake_package_bytes(
                    &downloaded.package_bytes,
                    &PackageIntakePolicy {
                        source_kind: snapshot.source.source_kind.clone(),
                        trust_mode: snapshot_entry.trust_mode.clone(),
                        expected_artifact_sha256: Some(
                            snapshot_entry.selected_artifact.checksum.clone(),
                        ),
                        trusted_public_keys: self.official_source.trusted_public_keys(),
                        original_filename: Some(downloaded.file_name.clone()),
                    },
                )
                .await?;
                let mut detail_json = json!({
                    "install_kind": "official_upgrade",
                    "plugin_id": snapshot_entry.plugin_id,
                    "provider_code": snapshot_entry.provider_code,
                    "file_name": downloaded.file_name,
                });
                if let Some(compatibility_override) = compatibility_override.clone() {
                    detail_json["compatibility_override"] = compatibility_override;
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

    async fn install_intake_result(
        &self,
        actor_user_id: Uuid,
        intake: PackageIntakeResult,
        package_bytes: Option<Vec<u8>>,
        detail_json: serde_json::Value,
    ) -> Result<InstallPluginResult> {
        let package_root = intake.extracted_root.clone();
        let result = self
            .install_plugin_with_metadata(
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
            .await;
        let _ = fs::remove_dir_all(&package_root);
        result
    }

    async fn install_plugin_with_metadata(
        &self,
        command: InstallPluginCommand,
        source_metadata: InstallSourceMetadata,
        detail_json: serde_json::Value,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: None,
                workspace_id: None,
                provider_code: "pending_provider".to_string(),
                task_kind: domain::PluginTaskKind::Install,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".to_string()),
                detail_json: detail_json.clone(),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".to_string()),
                detail_json.clone(),
            )
            .await?;

        let installation_result = async {
            let manifest = load_plugin_manifest(&command.package_root)?;
            let package_kind = route_plugin_package(&manifest)?;
            let plugin_code = plugin_code_from_plugin_id(&manifest.plugin_id)?;
            let package_id = manifest.versioned_plugin_id().map_err(map_framework_error)?;
            let install_path = self
                .install_root
                .join("installed")
                .join(&plugin_code)
                .join(&manifest.version);
            let package_archive_path = self
                .install_root
                .join("packages")
                .join(&plugin_code)
                .join(format!("{package_id}.1flowbasepkg"));
            let mut staged_package = source_metadata
                .package_bytes
                .as_deref()
                .map(|bytes| filesystem::StagedArtifactPath::prepare_file(bytes, &package_archive_path))
                .transpose()?;
            let mut staged_installation = filesystem::StagedArtifactPath::prepare_directory(
                Path::new(&command.package_root),
                &install_path,
            )?;
            let manifest_fingerprint =
                compute_manifest_fingerprint(&staged_installation.staged_path().join("manifest.yaml"))
                    .await
                    .map_err(map_framework_error)?;
            write_artifact_marker(
                staged_installation.staged_path(),
                &package_id,
                &manifest.version,
                source_metadata.checksum.as_deref(),
                Some(&manifest_fingerprint),
            )?;
            staged_installation.activate()?;
            if let Some(package) = staged_package.as_mut() {
                package.activate()?;
            }
            let database_result = async {
                match package_kind {
                RoutedPluginPackageKind::HostExtension => {
                    ensure_root_actor(&actor)?;
                    ensure_uploaded_host_extensions_enabled(self.allow_uploaded_host_extensions)?;
                    let mut metadata_json = json!({});
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = self
                        .repository
                        .upsert_installation(&UpsertPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: plugin_code.clone(),
                            plugin_id: package_id.clone(),
                            plugin_version: manifest.version.clone(),
                            contract_version: manifest.contract_version.clone(),
                            protocol: manifest.runtime.protocol.clone(),
                            display_name: manifest.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::PendingRestart,
                            artifact_status: domain::PluginArtifactStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::PendingRestart,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        })
                        .await?;
                    self.repository
                        .append_audit_log(&audit_log(
                            Some(actor.current_workspace_id),
                            Some(command.actor_user_id),
                            "plugin_installation",
                            Some(installation.id),
                            "plugin.installed",
                            plugin_install_audit_detail(&installation, &detail_json, true),
                        ))
                        .await?;
                    Ok::<(domain::PluginInstallationRecord, bool), anyhow::Error>((
                        installation,
                        false,
                    ))
                }
                RoutedPluginPackageKind::ModelProviderRuntime => {
                    let installed_package = load_provider_package(&install_path)?;
                    let mut metadata_json = json!({
                        "help_url": installed_package.provider.help_url,
                        "default_base_url": installed_package.provider.default_base_url,
                        "model_discovery_mode": format!("{:?}", installed_package.provider.model_discovery_mode).to_ascii_lowercase(),
                        "icon": installed_package.manifest.icon,
                        "supported_model_types": ["llm"],
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = self
                        .repository
                        .upsert_installation(&UpsertPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: installed_package.provider.provider_code.clone(),
                            plugin_id: installed_package.identifier(),
                            plugin_version: installed_package.manifest.version.clone(),
                            contract_version: installed_package.manifest.contract_version.clone(),
                            protocol: installed_package.provider.protocol.clone(),
                            display_name: installed_package.provider.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            artifact_status: domain::PluginArtifactStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        })
                        .await?;
                    refresh_provider_package_catalog_projection(
                        &self.repository,
                        &installation,
                        &installed_package,
                    )
                    .await?;
                    let manifest = load_plugin_manifest(&install_path)?;
                    self.repository
                        .replace_installation_node_contributions(
                            &build_node_contribution_sync_input(&installation, &manifest),
                        )
                        .await?;
                    self.repository
                        .replace_installation_js_dependencies(&build_js_dependency_sync_input(
                            &installation,
                            &manifest,
                        ))
                        .await?;
                    self.repository
                        .replace_installation_frontend_blocks(&build_frontend_block_sync_input(
                            &installation,
                            &manifest,
                        ))
                        .await?;
                    self.repository
                        .append_audit_log(&audit_log(
                            Some(actor.current_workspace_id),
                            Some(command.actor_user_id),
                            "plugin_installation",
                            Some(installation.id),
                            "plugin.installed",
                            plugin_install_audit_detail(&installation, &detail_json, false),
                        ))
                        .await?;
                    Ok::<(domain::PluginInstallationRecord, bool), anyhow::Error>((
                        installation,
                        false,
                    ))
                }
                RoutedPluginPackageKind::DataSourceRuntime => {
                    let installed_package =
                        plugin_framework::DataSourcePackage::load_from_dir(&install_path)
                            .map_err(map_framework_error)?;
                    let mut metadata_json = json!({
                        "supported_resource_kinds": installed_package.definition.resource_kinds,
                        "auth_modes": installed_package.definition.auth_modes,
                        "capabilities": installed_package.definition.capabilities,
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = self
                        .repository
                        .upsert_installation(&UpsertPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: installed_package.definition.source_code.clone(),
                            plugin_id: installed_package.identifier(),
                            plugin_version: installed_package.manifest.version.clone(),
                            contract_version: installed_package.manifest.contract_version.clone(),
                            protocol: "data_source".to_string(),
                            display_name: installed_package.definition.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            artifact_status: domain::PluginArtifactStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        })
                        .await?;
                    self.repository
                        .append_audit_log(&audit_log(
                            Some(actor.current_workspace_id),
                            Some(command.actor_user_id),
                            "plugin_installation",
                            Some(installation.id),
                            "plugin.installed",
                            plugin_install_audit_detail(&installation, &detail_json, false),
                        ))
                        .await?;
                    Ok::<(domain::PluginInstallationRecord, bool), anyhow::Error>((
                        installation,
                        false,
                    ))
                }
                RoutedPluginPackageKind::CapabilityPlugin => {
                    let manifest = load_plugin_manifest(&install_path)?;
                    let mut metadata_json = json!({
                        "node_contributions": manifest
                            .node_contributions
                            .iter()
                            .map(|entry| entry.contribution_code.clone())
                            .collect::<Vec<_>>(),
                        "block_contributions": manifest
                            .block_contributions
                            .iter()
                            .map(|entry| entry.contribution_code.clone())
                            .collect::<Vec<_>>(),
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation_input = UpsertPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: stable_plugin_unique_identifier(&manifest.plugin_id),
                            plugin_id: manifest.versioned_plugin_id().map_err(map_framework_error)?,
                            plugin_version: manifest.version.clone(),
                            contract_version: manifest.contract_version.clone(),
                            protocol: manifest.runtime.protocol.clone(),
                            display_name: manifest.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            artifact_status: domain::PluginArtifactStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        };
                    let installation = commit_prepared_installation(
                        &self.repository,
                        &self.node_id,
                        installation_input,
                        &manifest,
                        None,
                    )
                    .await?;
                    let _ = self.repository.append_audit_log(&audit_log(
                            Some(actor.current_workspace_id),
                            Some(command.actor_user_id),
                            "plugin_installation",
                            Some(installation.id),
                            "plugin.installed",
                            plugin_install_audit_detail(&installation, &detail_json, false),
                        )).await;
                    Ok::<(domain::PluginInstallationRecord, bool), anyhow::Error>((
                        installation,
                        true,
                    ))
                }
                }
            }
            .await;
            if database_result.is_err() {
                let _ = staged_installation.rollback();
                if let Some(package) = staged_package.as_mut() {
                    let _ = package.rollback();
                }
            } else {
                staged_installation.finish();
                if let Some(package) = staged_package {
                    package.finish();
                }
            }
            database_result
        }
        .await;

        match installation_result {
            Ok((installation, artifact_committed)) => {
                if !artifact_committed {
                    if let Err(error) = self.record_ready_current_node_artifact(&installation).await
                    {
                        let _ = self
                            .transition_task(
                                &running_task,
                                domain::PluginTaskStatus::Failed,
                                Some(error.to_string()),
                                json!({
                                    "installation_id": installation.id,
                                    "provider_code": installation.provider_code,
                                }),
                            )
                            .await;
                        return Err(error);
                    }
                }
                let installed_message = if is_host_extension_installation(&installation) {
                    "installed; restart required"
                } else {
                    "installed"
                };
                let mut task_detail_json = json!({
                    "installation_id": installation.id,
                    "provider_code": installation.provider_code,
                    "plugin_id": installation.plugin_id,
                    "installed_path": installation.installed_path,
                });
                if let Some(compatibility_override) =
                    detail_json.get("compatibility_override").cloned()
                {
                    task_detail_json["compatibility_override"] = compatibility_override;
                }
                let task = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Succeeded,
                        Some(installed_message.to_string()),
                        task_detail_json,
                    )
                    .await?;
                Ok(InstallPluginResult { installation, task })
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        detail_json,
                    )
                    .await;
                Err(error)
            }
        }
    }
}
