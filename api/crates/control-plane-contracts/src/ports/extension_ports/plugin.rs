use std::collections::BTreeMap;

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
    pub settings_templates: Vec<PluginSettingsTemplateInput>,
    pub installation: UpsertPluginInstallationInput,
    pub artifact_instance: UpsertPluginArtifactInstanceInput,
    pub package_catalog: Option<UpsertPluginPackageCatalogProjectionInput>,
    pub node_contributions: ReplaceInstallationNodeContributionsInput,
    pub js_dependencies: ReplaceInstallationJsDependenciesInput,
    pub frontend_blocks: ReplaceInstallationFrontendBlocksInput,
    pub retained_frontend_module_assets: Vec<RetainedFrontendModuleAssetInput>,
}

/// Admission identity for one versioned installation, held through filesystem publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginInstallationContentIdentity {
    Legacy,
    ManagedArchive { checksum: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallationAdmission {
    pub plugin_id: String,
    pub content: PluginInstallationContentIdentity,
}

/// One installation's serialization owner. Commit uses its existing connection; callers must
/// finish or restore filesystem staging before releasing it. Dropping closes any held lease.
#[async_trait]
pub trait PluginInstallationLease: Send {
    /// Observe the existing version before its node artifact is replaced, using the lease.
    async fn node_runtime_is_active(&mut self, node_id: &str) -> anyhow::Result<bool>;
    async fn commit(
        &mut self,
        input: &CommitPluginInstallationInput,
    ) -> anyhow::Result<domain::PluginInstallationRecord>;
    async fn release(self: Box<Self>) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct RetainedFrontendModuleAssetInput {
    pub module_source: String,
    pub sha256: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RetainedFrontendModuleAsset {
    pub sha256: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
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

/// Activation failure may restore only the desired-state write made by that activation.
#[derive(Debug, Clone)]
pub struct CompareRestorePluginDesiredStateInput {
    pub installation_id: Uuid,
    pub expected_updated_at: time::OffsetDateTime,
    pub expected_desired_state: domain::PluginDesiredState,
    pub restore_desired_state: domain::PluginDesiredState,
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
pub struct CommitPluginFamilyUninstallInput {
    pub artifact_instances: Vec<UpsertPluginArtifactInstanceInput>,
    pub artifact_cleanups: Vec<CreatePluginArtifactCleanupInput>,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct CreatePluginArtifactCleanupInput {
    pub cleanup_id: Uuid,
    pub node_id: String,
    pub provider_code: String,
    pub tombstone_path: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct RecordPluginArtifactCleanupFailureInput {
    pub cleanup_id: Uuid,
    pub last_error: String,
    pub attempted_at: time::OffsetDateTime,
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
    async fn list_official_catalog_for_workspace(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<OfficialPluginCatalogSnapshot> {
        self.list_official_catalog().await
    }
    async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
        None
    }
    async fn download_plugin(
        &self,
        entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage>;
    async fn download_plugin_for_workspace(
        &self,
        _workspace_id: Uuid,
        entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage> {
        self.download_plugin(entry).await
    }
    fn trusted_public_keys(&self) -> Vec<extension_contracts::TrustedPublicKey>;
}

#[derive(Debug)]
pub struct VerifiedNativePluginArtifact {
    pub local_path: String,
    pub local_version: String,
    pub local_checksum: Option<String>,
    pub manifest_fingerprint: String,
    /// Reparse raw manifest bytes and return the entry only if the prepared identity matches.
    pub verify_manifest: fn(&str, &str, &VerifiedNativePluginArtifact) -> Option<String>,
    pub contribution_is_valid: fn(&str) -> bool,
}

pub enum NativePluginStartupOutcome<'a> {
    Active(&'a VerifiedNativePluginArtifact),
    LoadFailed(&'a str),
}

#[async_trait]
pub trait PluginRepository: Send + Sync {
    async fn apply_managed_plugin_settings_templates(
        &self,
        _installation_id: Uuid,
    ) -> anyhow::Result<()> {
        anyhow::bail!("managed settings template application is not supported")
    }
    async fn apply_native_plugin_settings_templates(
        &self,
        _target: &domain::NativePluginTarget,
    ) -> anyhow::Result<()> {
        anyhow::bail!("native settings template application is not supported")
    }
    async fn native_plugin_target_is_applied(
        &self,
        _target: &domain::NativePluginTarget,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("native settings template application is not supported")
    }
    async fn list_native_plugin_targets(&self) -> anyhow::Result<Vec<domain::NativePluginTarget>> {
        anyhow::bail!("native target selection is not supported")
    }
    /// Bootstrap only a legacy family without a target, rejecting multiple enabled candidates.
    async fn reconcile_legacy_native_plugin_target(
        &self,
        _installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::NativePluginTarget>> {
        anyhow::bail!("native target reconciliation is not supported")
    }
    /// Fences both desired completion and node runtime observation in one transaction.
    async fn complete_native_plugin_startup(
        &self,
        _target: &domain::NativePluginTarget,
        _node_id: &str,
        _outcome: NativePluginStartupOutcome<'_>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("native startup completion is not supported")
    }

    async fn begin_plugin_installation(
        &self,
        _admission: &PluginInstallationAdmission,
    ) -> anyhow::Result<Box<dyn PluginInstallationLease>> {
        anyhow::bail!("plugin installation lease is not implemented")
    }

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
    async fn get_current_local_installation(
        &self,
        node_id: &str,
        family: &domain::ExtensionCatalogIdentity,
    ) -> anyhow::Result<Option<domain::LocalPluginInstallationRecord>> {
        for installation in self.list_installations().await? {
            if installation.category != family.category()
                || installation.organization != family.organization()
                || installation.provider_code != family.artifact_id()
            {
                continue;
            }
            let Some(local) = self
                .get_local_installation(node_id, installation.id)
                .await?
            else {
                continue;
            };
            if local.artifact.is_current && local.artifact.artifact_status.is_ready() {
                return Ok(Some(local));
            }
        }
        Ok(None)
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
    /// Restore a rejected managed runtime activation only while its revision still owns selection.
    /// False means a newer selection or installation change must remain untouched.
    async fn compare_restore_desired_state(
        &self,
        _input: &CompareRestorePluginDesiredStateInput,
    ) -> anyhow::Result<bool> {
        anyhow::bail!("conditional managed desired-state restoration is not configured")
    }
    async fn upsert_artifact_instance(
        &self,
        input: &UpsertPluginArtifactInstanceInput,
    ) -> anyhow::Result<domain::PluginArtifactInstanceRecord>;
    /// Reconcile scanned facts only if the node artifact still matches the pre-scan observation.
    /// `None` means the caller must reread and rescan. Concurrent stores must implement the
    /// comparison and write atomically; the default is for non-concurrent implementations.
    async fn refresh_artifact_instance(
        &self,
        input: &RefreshPluginArtifactInstanceInput,
    ) -> anyhow::Result<Option<domain::PluginArtifactInstanceRecord>> {
        let installation = self.get_installation(input.installation_id).await?.ok_or(
            crate::ControlPlaneContractError::NotFound("plugin_installation"),
        )?;
        let existing = self
            .get_artifact_instance(&input.node_id, input.installation_id)
            .await?;
        if existing != input.observed_artifact {
            return Ok(None);
        }
        let is_host = installation.category == domain::ExtensionCategory::HostExtensions;
        let manifest_fingerprint = input.manifest_fingerprint.clone().or_else(|| {
            existing
                .as_ref()
                .and_then(|r| r.manifest_fingerprint.clone())
        });
        let is_managed_bus = installation.contract_version == "1flowbase.extension-bus/v1";
        let preserve_runtime = !input.reset_runtime_status
            && (installation.desired_state != domain::PluginDesiredState::Disabled
                || is_host
                || is_managed_bus);
        let runtime_status = existing
            .as_ref()
            .filter(|row| {
                preserve_runtime
                    && !domain::has_local_artifact_warning(row.last_error.as_deref())
                    && row.artifact_status.is_ready()
                    && input.artifact_status.is_ready()
                    && row.local_version == input.local_version
                    && row.local_checksum == input.local_checksum
                    && row.local_path == input.local_path
                    && row.manifest_fingerprint == manifest_fingerprint
                    && input.last_error.is_none()
            })
            .map(|row| row.runtime_status)
            .unwrap_or(domain::PluginRuntimeStatus::Inactive);
        let availability_status = derive_contract_availability_status(
            installation.desired_state,
            plugin_artifact_status(input.artifact_status),
            runtime_status,
        );
        let effective_last_error = if input.last_error.is_some() {
            input.last_error.clone()
        } else if runtime_status == domain::PluginRuntimeStatus::LoadFailed {
            existing.as_ref().and_then(|r| r.last_error.clone())
        } else {
            None
        };
        if let Some(record) = existing.as_ref().filter(|record| {
            !input.touch_checked_at
                && record.artifact_status == input.artifact_status
                && record.runtime_status == runtime_status
                && record.availability_status == availability_status
                && record.local_version == input.local_version
                && record.local_checksum == input.local_checksum
                && record.local_path == input.local_path
                && record.manifest_fingerprint == manifest_fingerprint
                && record.last_error == effective_last_error
        }) {
            return Ok(Some(record.clone()));
        }
        self.upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: input.node_id.clone(),
            installation_id: input.installation_id,
            local_version: input.local_version.clone(),
            local_checksum: input.local_checksum.clone(),
            local_path: input.local_path.clone(),
            package_path: existing.as_ref().and_then(|r| r.package_path.clone()),
            manifest_fingerprint,
            artifact_status: input.artifact_status,
            runtime_status,
            availability_status,
            checked_at: existing.as_ref().map_or(input.checked_at, |record| {
                input
                    .checked_at
                    .max(record.checked_at + time::Duration::microseconds(1))
            }),
            last_error: effective_last_error,
            is_current: existing.as_ref().is_none_or(|r| r.is_current),
        })
        .await
        .map(Some)
    }
    /// Atomically record a runtime status transition against the live installation desired state
    /// and live node artifact row.
    async fn record_artifact_runtime_status(
        &self,
        input: &RecordPluginRuntimeStatusInput,
    ) -> anyhow::Result<domain::PluginArtifactInstanceRecord> {
        if input.runtime_status == domain::PluginRuntimeStatus::Active {
            anyhow::bail!("use try_mark_active_artifact_instance to mark runtime active");
        }
        let installation = self.get_installation(input.installation_id).await?.ok_or(
            crate::ControlPlaneContractError::NotFound("plugin_installation"),
        )?;
        let existing = self
            .get_artifact_instance(&input.node_id, input.installation_id)
            .await?;
        let is_host = installation.category == domain::ExtensionCategory::HostExtensions;
        let effective_runtime = if installation.desired_state
            == domain::PluginDesiredState::Disabled
            && !is_host
            && input.runtime_status == domain::PluginRuntimeStatus::LoadFailed
        {
            domain::PluginRuntimeStatus::Inactive
        } else {
            input.runtime_status
        };
        let effective_error = if effective_runtime == domain::PluginRuntimeStatus::Inactive
            && input.runtime_status == domain::PluginRuntimeStatus::LoadFailed
        {
            None
        } else {
            input.last_error.clone()
        };
        let (artifact_status, availability_status) = derive_runtime_write_artifact_status(
            installation.desired_state,
            &installation.contract_version,
            effective_runtime,
            existing.as_ref().map(|r| r.artifact_status),
        );
        let last_error = effective_error.or_else(|| {
            existing
                .as_ref()
                .and_then(|row| row.last_error.as_deref())
                .filter(|error| domain::has_local_artifact_warning(Some(error)))
                .map(str::to_owned)
        });
        let (
            local_version,
            local_checksum,
            local_path,
            package_path,
            manifest_fingerprint,
            is_current,
        ) = existing
            .map(|r| {
                (
                    r.local_version,
                    r.local_checksum,
                    r.local_path,
                    r.package_path,
                    r.manifest_fingerprint,
                    r.is_current,
                )
            })
            .unwrap_or_else(|| {
                (
                    Some(installation.plugin_version.clone()),
                    installation.expected_checksum.clone(),
                    None,
                    None,
                    None,
                    false,
                )
            });
        self.upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: input.node_id.clone(),
            installation_id: input.installation_id,
            local_version,
            local_checksum,
            local_path,
            package_path,
            manifest_fingerprint,
            artifact_status,
            runtime_status: effective_runtime,
            availability_status,
            checked_at: input.checked_at,
            last_error,
            is_current,
        })
        .await
    }
    /// Commit activation only for the exact ready artifact that was loaded.
    async fn try_mark_active_artifact_instance(
        &self,
        input: &ActivatePluginArtifactInstanceInput,
    ) -> anyhow::Result<ActivatePluginArtifactOutcome> {
        use domain::PluginRuntimeStatus::{Active, Inactive};
        let installation = self.get_installation(input.installation_id).await?.ok_or(
            crate::ControlPlaneContractError::NotFound("plugin_installation"),
        )?;
        let disabled = installation.desired_state == domain::PluginDesiredState::Disabled
            && installation.category != domain::ExtensionCategory::HostExtensions;
        let Some(existing) = self
            .get_artifact_instance(&input.node_id, input.installation_id)
            .await?
        else {
            return Ok(if disabled {
                ActivatePluginArtifactOutcome::Disabled
            } else {
                ActivatePluginArtifactOutcome::ArtifactInvalidated
            });
        };
        // Like the durable adapter, reject stale row identity and installation checksum drift.
        // Final disk verification requires the durable adapter's node slot lock.
        let matches = !disabled
            && input.matches_ready_artifact(&existing)
            && installation
                .expected_checksum
                .as_ref()
                .is_none_or(|expected| existing.local_checksum.as_ref() == Some(expected));
        let runtime_status = if matches { Active } else { Inactive };
        let availability_status = derive_contract_availability_status(
            installation.desired_state,
            plugin_artifact_status(existing.artifact_status),
            runtime_status,
        );
        if matches
            || existing.runtime_status != Inactive
            || (disabled && existing.availability_status != availability_status)
        {
            let artifact = self
                .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
                    node_id: existing.node_id,
                    installation_id: existing.installation_id,
                    local_version: existing.local_version,
                    local_checksum: existing.local_checksum,
                    local_path: existing.local_path,
                    package_path: existing.package_path,
                    manifest_fingerprint: existing.manifest_fingerprint,
                    artifact_status: existing.artifact_status,
                    runtime_status,
                    availability_status,
                    checked_at: input.checked_at,
                    last_error: if matches { None } else { existing.last_error },
                    is_current: existing.is_current,
                })
                .await?;
            if matches {
                return Ok(ActivatePluginArtifactOutcome::Activated(artifact));
            }
        }
        Ok(if disabled {
            ActivatePluginArtifactOutcome::Disabled
        } else {
            ActivatePluginArtifactOutcome::ArtifactInvalidated
        })
    }
    async fn select_network_egress_current(
        &self,
        _node_id: &str,
        _installation_id: Uuid,
    ) -> anyhow::Result<Option<domain::PluginArtifactInstanceRecord>> {
        anyhow::bail!("network egress current-version selection is not supported")
    }
    async fn commit_plugin_family_uninstall(
        &self,
        _input: &CommitPluginFamilyUninstallInput,
    ) -> anyhow::Result<()> {
        anyhow::bail!("atomic plugin family uninstall is not supported")
    }
    async fn list_plugin_artifact_cleanups(
        &self,
        _node_id: &str,
    ) -> anyhow::Result<Vec<domain::PluginArtifactCleanupRecord>> {
        anyhow::bail!("plugin artifact cleanup reconciliation is not supported")
    }
    async fn complete_plugin_artifact_cleanup(&self, _cleanup_id: Uuid) -> anyhow::Result<()> {
        anyhow::bail!("plugin artifact cleanup reconciliation is not supported")
    }
    async fn record_plugin_artifact_cleanup_failure(
        &self,
        _input: &RecordPluginArtifactCleanupFailureInput,
    ) -> anyhow::Result<()> {
        anyhow::bail!("plugin artifact cleanup reconciliation is not supported")
    }
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
    /// Establish a first workspace selection atomically; never replace an existing family.
    async fn create_initial_assignment(
        &self,
        _input: &CreatePluginAssignmentInput,
    ) -> anyhow::Result<Option<domain::PluginAssignmentRecord>> {
        anyhow::bail!("initial plugin assignment is not supported")
    }
    async fn list_assignments(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::PluginAssignmentRecord>>;
    async fn list_assigned_installation_ids(&self) -> anyhow::Result<Vec<Uuid>>;
    /// Installations with a live consumer in any workspace. Historical import provenance is
    /// excluded, as are expired model-provider previews.
    async fn list_referenced_installation_ids(&self) -> anyhow::Result<Vec<Uuid>>;
    async fn installation_has_live_references(
        &self,
        installation_id: Uuid,
    ) -> anyhow::Result<bool> {
        Ok(self
            .list_referenced_installation_ids()
            .await?
            .contains(&installation_id))
    }
    /// Disable an active installation only while it is still unreferenced under the write fence.
    async fn retire_unreferenced_installation(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> anyhow::Result<Option<domain::PluginInstallationRecord>>;
    /// Undo a failed switch's temporary enable while the target is still unreferenced,
    /// even if no sibling is currently serving.
    async fn rollback_unreferenced_installation(
        &self,
        _input: &UpdatePluginDesiredStateInput,
    ) -> anyhow::Result<Option<domain::PluginInstallationRecord>> {
        anyhow::bail!("plugin activation rollback is not supported")
    }
    /// Atomically verify target readiness, migrate workspace provider instances + catalog caches,
    /// update the workspace assignment, and retire the previous version when unreferenced.
    async fn commit_provider_version_switch(
        &self,
        _input: &CommitProviderVersionSwitchInput,
    ) -> anyhow::Result<ProviderVersionSwitchOutcome> {
        anyhow::bail!("provider version switch is not supported")
    }
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
    pub ui_registration: String,
    /// Explicit opt-in to block management and page creation, independent of reusable UI registration.
    pub block_registration: bool,
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

/// Only the selected default and its published body are needed by the workspace catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendBlockCatalogDefaultTemplate {
    pub provider_code: String,
    pub contribution_code: String,
    pub published_revision: Option<FrontendBlockCatalogTemplateRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendBlockCatalogTemplateRevision {
    pub revision: i32,
    pub source: String,
    pub language: domain::UiCodeTemplateLanguage,
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

    async fn list_workspace_frontend_blocks_runtime(
        &self,
        node_id: &str,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::FrontendBlockCatalogEntry>> {
        let mut entries = self
            .list_workspace_frontend_blocks(node_id, workspace_id)
            .await?;
        for entry in &mut entries {
            for module in &mut entry.code_modules {
                module.type_declarations.clear();
            }
        }
        Ok(entries)
    }

    async fn list_system_frontend_blocks(
        &self,
        _node_id: &str,
    ) -> anyhow::Result<Vec<domain::FrontendBlockCatalogEntry>> {
        Ok(Vec::new())
    }

    async fn get_retained_frontend_module_asset(
        &self,
        _workspace_id: Uuid,
        _sha256: &str,
    ) -> anyhow::Result<Option<RetainedFrontendModuleAsset>> {
        Ok(None)
    }

    async fn list_active_ui_code_templates_for_catalog(
        &self,
    ) -> anyhow::Result<Vec<FrontendBlockCatalogDefaultTemplate>> {
        Ok(Vec::new())
    }
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

/// Verified fixed page source passed to the atomic plugin template application owner.
#[derive(Debug, Clone)]
pub struct PluginSettingsTemplateInput {
    pub feature_id: String,
    pub contribution_code: String,
    pub source: String,
    pub language: domain::UiCodeTemplateLanguage,
}

/// Read the workspace-UI declaration from the canonical, validated installed manifest metadata.
/// Storage and the assignment policy share this projection without depending on package intake.
pub fn has_managed_workspace_ui(installation: &domain::PluginInstallationRecord) -> bool {
    installation.contract_version == "1flowbase.extension-bus/v1"
        && installation.metadata_json["managed"]["execution_bindings"]
            .as_array()
            .is_some_and(|bindings| {
                bindings.iter().any(|binding| {
                    binding["execution_mode"] == "declarative_only"
                        && binding["payload"]["kind"] == "frontend_block"
                        && binding["payload"]["contribution_code"].is_string()
                })
            })
}

/// Required package-owner parser. Returns the fingerprint only for a valid installed manifest.
/// The repository reads the raw bytes and invokes this parser inside the final node slot lock.
pub type ValidateInstalledPluginManifest =
    fn(&domain::PluginInstallationRecord, &str) -> Option<String>;

#[derive(Debug, Clone)]
pub struct CommitProviderVersionSwitchInput {
    pub node_id: String,
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub current_installation_id: Uuid,
    pub target_installation_id: Uuid,
    pub target_local_version: Option<String>,
    pub target_local_checksum: Option<String>,
    pub target_local_path: String,
    pub target_manifest_fingerprint: Option<String>,
    pub validate_manifest: ValidateInstalledPluginManifest,
    pub target_protocol: String,
    pub model_discovery_mode: domain::ModelProviderDiscoveryMode,
    pub catalog_source: domain::ModelProviderCatalogSource,
    pub actor_user_id: Uuid,
}

impl CommitProviderVersionSwitchInput {
    pub fn matches_active_artifact(&self, artifact: &domain::PluginArtifactInstanceRecord) -> bool {
        artifact.runtime_status == domain::PluginRuntimeStatus::Active
            && matches_ready_artifact(
                artifact,
                &self.target_local_version,
                &self.target_local_checksum,
                &self.target_local_path,
                &self.target_manifest_fingerprint,
            )
    }
}

#[derive(Debug, Clone)]
pub struct ProviderVersionSwitchOutcome {
    pub migrated_instance_count: usize,
    pub retired_previous: Option<domain::PluginInstallationRecord>,
}

#[derive(Debug, Clone)]
pub struct RefreshPluginArtifactInstanceInput {
    pub node_id: String,
    pub installation_id: Uuid,
    /// Stored row before scanning disk, or `None` when it did not yet exist.
    pub observed_artifact: Option<domain::PluginArtifactInstanceRecord>,
    /// Bump the row clock after an explicit refresh even if all derived fields match.
    pub touch_checked_at: bool,
    pub local_version: Option<String>,
    pub local_checksum: Option<String>,
    pub local_path: Option<String>,
    pub manifest_fingerprint: Option<String>,
    pub artifact_status: domain::PluginArtifactInstanceStatus,
    pub last_error: Option<String>,
    pub reset_runtime_status: bool,
    pub checked_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct RecordPluginRuntimeStatusInput {
    pub node_id: String,
    pub installation_id: Uuid,
    pub runtime_status: domain::PluginRuntimeStatus,
    pub last_error: Option<String>,
    pub checked_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct ActivatePluginArtifactInstanceInput {
    pub node_id: String,
    pub installation_id: Uuid,
    pub expected_local_version: Option<String>,
    pub expected_local_checksum: Option<String>,
    pub expected_local_path: String,
    pub expected_manifest_fingerprint: Option<String>,
    pub validate_manifest: ValidateInstalledPluginManifest,
    pub checked_at: OffsetDateTime,
}

impl ActivatePluginArtifactInstanceInput {
    pub fn matches_ready_artifact(&self, artifact: &domain::PluginArtifactInstanceRecord) -> bool {
        matches_ready_artifact(
            artifact,
            &self.expected_local_version,
            &self.expected_local_checksum,
            &self.expected_local_path,
            &self.expected_manifest_fingerprint,
        )
    }
}

fn matches_ready_artifact(
    artifact: &domain::PluginArtifactInstanceRecord,
    local_version: &Option<String>,
    local_checksum: &Option<String>,
    local_path: &str,
    manifest_fingerprint: &Option<String>,
) -> bool {
    artifact.artifact_status.is_ready()
        && !domain::has_local_artifact_warning(artifact.last_error.as_deref())
        && artifact.local_version == *local_version
        && artifact.local_checksum == *local_checksum
        && artifact.local_path.as_deref() == Some(local_path)
        && artifact.manifest_fingerprint == *manifest_fingerprint
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "Return the committed artifact by value without a heap allocation"
)]
pub enum ActivatePluginArtifactOutcome {
    Activated(domain::PluginArtifactInstanceRecord),
    Disabled,
    ArtifactInvalidated,
}

/// Record runtime facts without promoting or replacing unverified disk facts.
pub fn derive_runtime_write_artifact_status(
    desired_state: domain::PluginDesiredState,
    contract_version: &str,
    effective_runtime: domain::PluginRuntimeStatus,
    existing_status: Option<domain::PluginArtifactInstanceStatus>,
) -> (
    domain::PluginArtifactInstanceStatus,
    domain::PluginAvailabilityStatus,
) {
    use domain::{PluginArtifactInstanceStatus as Instance, PluginRuntimeStatus as Runtime};
    let non_managed_load_failed = effective_runtime == Runtime::LoadFailed
        && contract_version != "1flowbase.extension-bus/v1";
    let artifact_status = match existing_status {
        Some(Instance::Ready) | None if non_managed_load_failed => Instance::LoadFailed,
        Some(status) => status,
        None => Instance::Missing,
    };
    let contract_artifact_status = match (effective_runtime, artifact_status) {
        (Runtime::LoadFailed, Instance::Ready | Instance::LoadFailed) => {
            domain::PluginArtifactStatus::Ready
        }
        _ => plugin_artifact_status(artifact_status),
    };
    let availability_status = derive_contract_availability_status(
        desired_state,
        contract_artifact_status,
        effective_runtime,
    );
    (artifact_status, availability_status)
}

pub fn plugin_artifact_status(
    status: domain::PluginArtifactInstanceStatus,
) -> domain::PluginArtifactStatus {
    use domain::PluginArtifactInstanceStatus as Instance;
    use domain::PluginArtifactStatus as Artifact;
    match status {
        Instance::Ready => Artifact::Ready,
        Instance::Missing | Instance::Outdated => Artifact::Missing,
        Instance::Mismatched | Instance::LoadFailed => Artifact::InstallIncomplete,
        Instance::Corrupted => Artifact::Corrupted,
    }
}

pub fn derive_contract_availability_status(
    desired_state: domain::PluginDesiredState,
    artifact_status: domain::PluginArtifactStatus,
    runtime_status: domain::PluginRuntimeStatus,
) -> domain::PluginAvailabilityStatus {
    match desired_state {
        domain::PluginDesiredState::Disabled => domain::PluginAvailabilityStatus::Disabled,
        domain::PluginDesiredState::PendingRestart => {
            if artifact_status == domain::PluginArtifactStatus::Ready {
                domain::PluginAvailabilityStatus::PendingRestart
            } else {
                domain::PluginAvailabilityStatus::ArtifactMissing
            }
        }
        domain::PluginDesiredState::ActiveRequested => match artifact_status {
            domain::PluginArtifactStatus::Missing => {
                domain::PluginAvailabilityStatus::ArtifactMissing
            }
            domain::PluginArtifactStatus::Staged
            | domain::PluginArtifactStatus::InstallIncomplete
            | domain::PluginArtifactStatus::Corrupted => {
                domain::PluginAvailabilityStatus::InstallIncomplete
            }
            domain::PluginArtifactStatus::Ready => match runtime_status {
                domain::PluginRuntimeStatus::Active => domain::PluginAvailabilityStatus::Available,
                domain::PluginRuntimeStatus::LoadFailed => {
                    domain::PluginAvailabilityStatus::LoadFailed
                }
                domain::PluginRuntimeStatus::Inactive => {
                    domain::PluginAvailabilityStatus::InstallIncomplete
                }
            },
        },
    }
}
