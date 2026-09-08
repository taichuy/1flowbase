//! Workspace-scoped immutable managed graph assembly and fresh execution admission.
use anyhow::{bail, Context, Result};
use control_plane::{
    plugin_management::{ready_current_node_plugin_installation, HostContributionGrantPolicy},
    ports::{ContributionAuthorityLease, PluginContributionAuthorityRepository, PluginRepository},
};
use plugin_framework::{extension_bus::*, ManagedManifest, PluginManifestV1};
use runtime_core::runtime_backend::{
    RuntimeArtifactReference, RuntimeBackend, RuntimeExecutionPrincipal, RuntimeManagedActivation,
    RuntimeManagedCapabilityRequest,
};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use storage_durable_postgres::MainDurableStore;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ManagedContributionBinding {
    pub(crate) handle: ManagedExecutionHandle,
    pub(crate) descriptor: ContributionDescriptor,
    installation: domain::PluginInstallationRecord,
}

pub(crate) struct ManagedWorkspaceSnapshot {
    pub(crate) graph: Arc<EffectiveExtensionGraph>,
    pub(crate) authority: ManagedGraphAuthority,
    pub(crate) bindings: BTreeMap<ContributionId, ManagedContributionBinding>,
}

struct PreparedPackage {
    installation: domain::PluginInstallationRecord,
    manifest: ManagedManifest,
    artifact_fingerprint: ManagedArtifactFingerprint,
    authority: domain::PluginContributionAuthoritySnapshot,
}

pub(crate) struct ManagedExtensionComposition {
    store: MainDurableStore,
    node_id: String,
    backend: Arc<dyn RuntimeBackend>,
    base_modules: Vec<ModuleDescriptor>,
    policy: HostContributionGrantPolicy,
    /// One assembly owner; immutable snapshots remain valid while callers hold their Arc.
    snapshots: Mutex<BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>>,
    assembly: Mutex<()>,
}

impl ManagedExtensionComposition {
    pub(crate) fn new(
        store: MainDurableStore,
        node_id: String,
        backend: Arc<dyn RuntimeBackend>,
        base_modules: Vec<ModuleDescriptor>,
    ) -> Self {
        Self {
            store,
            node_id,
            backend,
            base_modules,
            policy: HostContributionGrantPolicy::root_composition(),
            snapshots: Mutex::new(BTreeMap::new()),
            assembly: Mutex::new(()),
        }
    }

    pub(crate) async fn snapshot(
        &self,
        workspace_id: Uuid,
    ) -> Option<Arc<ManagedWorkspaceSnapshot>> {
        self.snapshots.lock().await.get(&workspace_id).cloned()
    }

    pub(crate) async fn rebuild_installation(&self, installation_id: Uuid) -> Result<()> {
        let _assembly = self.assembly.lock().await;
        let mut published = self.snapshots.lock().await.clone();
        let workspaces = self
            .store
            .contribution_authority_workspaces(installation_id)
            .await?;
        if workspaces.is_empty() {
            bail!("managed installation requires a workspace assignment");
        }
        let mut candidates = BTreeMap::new();
        let mut expected = BTreeMap::new();
        let mut new_handles = Vec::new();
        let result = async {
            for workspace_id in workspaces {
                let packages = self.prepare_packages(workspace_id).await?;
                let mut modules = self.base_modules.clone();
                let mut authority = ManagedGraphAuthority::new(self.policy.identity());
                for package in &packages {
                    let mut module = package.manifest.module.clone();
                    // Both values are host-owned. Preserve per-contribution requirements exactly.
                    module.activation = ModuleActivationDeclaration::Active;
                    module.granted_permissions.clear();
                    authority.managed_modules.insert(module.module_id.clone());
                    if !module.extension_points.is_empty() { bail!("managed point declarations require a separate host namespace admission"); }
                    for contribution in &module.contributions {
                        let permissions = self.policy.effective_permissions(&package.installation, workspace_id, contribution, &package.authority)?;
                        let subject = managed_subject(package.installation.id, workspace_id, contribution.contribution_id.clone())?;
                        if authority.contributions.insert(contribution.contribution_id.clone(), ManagedContributionAuthority { subject, revision: package.authority.revision, permissions }).is_some() {
                            bail!("duplicate managed contribution identity");
                        }
                    }
                    expected.insert((package.installation.id, workspace_id), (package.authority.revision, package.installation.updated_at));
                    modules.push(module);
                }
                let graph = Arc::new(compile_extension_graph_with_authority(modules, &authority)?);
                for receipt in graph.contribution_receipts() {
                    if authority.contributions.contains_key(&receipt.descriptor().contribution_id) && receipt.status() != &ContributionResolutionStatus::Active {
                        bail!("managed contribution is inactive in the effective graph");
                    }
                }
                let mut bindings = BTreeMap::new();
                for package in &packages {
                    for contribution in &package.manifest.module.contributions {
                        let identity = ManagedExecutionIdentity::new(
                            ManagedInstallationId::new(package.installation.id.to_string())?, ManagedWorkspaceId::new(workspace_id.to_string())?,
                            contribution.contribution_id.clone(), package.artifact_fingerprint.clone(), package.manifest.execution_binding_fingerprint(&contribution.contribution_id)?,
                        );
                        let handle = self.backend.activate_managed_contribution(RuntimeManagedActivation {
                            plugin_id: package.installation.plugin_id.clone(), artifact: RuntimeArtifactReference::new(package.installation.id.to_string())?, identity,
                        }).await?;
                        let retained = published.get(&workspace_id).is_some_and(|old| old.bindings.values().any(|binding| binding.handle == handle));
                        if !retained { new_handles.push(handle.clone()); }
                        bindings.insert(contribution.contribution_id.clone(), ManagedContributionBinding { handle, descriptor: contribution.clone(), installation: package.installation.clone() });
                    }
                }
                candidates.insert(workspace_id, Arc::new(ManagedWorkspaceSnapshot { graph, authority, bindings }));
            }
            // A disabled target may no longer appear in the new graph. Lock its assignment and
            // installation too, so removal and a concurrent re-enable cannot cross publication.
            for workspace_id in candidates.keys() {
                if !expected.contains_key(&(installation_id, *workspace_id)) {
                    let lease = self.store.lock_installation_contribution_authority(installation_id, *workspace_id).await?;
                    let installation = lease.installation(installation_id).context("missing locked installation")?;
                    expected.insert((installation_id, *workspace_id), (lease.snapshot().revision, installation.updated_at));
                    lease.release().await?;
                }
            }
            let scopes = expected.keys().copied().collect::<Vec<_>>();
            let lease = self.store.lock_contribution_authority_batch(&scopes).await?;
            Self::validate_candidate(&expected, lease.as_ref())?;
            // The authority batch remains locked until the complete candidate set is visible.
            // Readers continue using the prior immutable snapshot during preparation.
            let previous = candidates.keys().filter_map(|workspace| published.get(workspace).map(|value| (*workspace, value.clone()))).collect::<BTreeMap<_, _>>();
            for (workspace, candidate) in &candidates { published.insert(*workspace, candidate.clone()); }
            let mut visible = self.snapshots.lock().await;
            let original = visible.clone();
            *visible = published.clone();
            if let Err(error) = lease.release().await {
                *visible = original;
                return Err(error);
            }
            Ok::<_, anyhow::Error>(previous)
        }.await;
        match result {
            Err(error) => {
                for handle in new_handles {
                    if let Err(cleanup) =
                        self.backend.deactivate_managed_contribution(&handle).await
                    {
                        tracing::warn!(%cleanup, "managed candidate cleanup failed");
                    }
                }
                Err(error)
            }
            Ok(previous) => {
                // Replacing a published graph does not revoke already-admitted work. The runtime
                // scope owns its bounded drain; new calls obtain the newly published binding.
                let retained = published
                    .values()
                    .flat_map(|snapshot| {
                        snapshot
                            .bindings
                            .values()
                            .map(|binding| binding.handle.clone())
                    })
                    .collect::<Vec<_>>();
                drop(published);
                for old in previous.values() {
                    for binding in old.bindings.values() {
                        if !retained.contains(&binding.handle) {
                            self.backend
                                .deactivate_managed_contribution(&binding.handle)
                                .await?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn validate_candidate(
        expected: &BTreeMap<(Uuid, Uuid), (i64, time::OffsetDateTime)>,
        lease: &dyn ContributionAuthorityLease,
    ) -> Result<()> {
        if lease.snapshots().len() != expected.len() {
            bail!("managed candidate authority scope changed");
        }
        for snapshot in lease.snapshots() {
            let pinned = expected
                .get(&(snapshot.installation_id, snapshot.workspace_id))
                .context("managed candidate scope changed")?;
            let installation = lease
                .installation(snapshot.installation_id)
                .context("managed candidate installation disappeared")?;
            if snapshot.revision != pinned.0 || installation.updated_at != pinned.1 {
                bail!("managed candidate authority or installation revision changed");
            }
        }
        Ok(())
    }

    async fn prepare_packages(&self, workspace_id: Uuid) -> Result<Vec<PreparedPackage>> {
        let assignments = self.store.list_assignments(workspace_id).await?;
        let mut packages = Vec::new();
        for assignment in assignments {
            let installation = self
                .store
                .get_installation(assignment.installation_id)
                .await?
                .context("assigned installation missing")?;
            if installation.contract_version != "1flowbase.extension-bus/v1"
                || installation.desired_state != domain::PluginDesiredState::ActiveRequested
            {
                continue;
            }
            let local = ready_current_node_plugin_installation(
                &self.store,
                &self.node_id,
                Path::new(""),
                installation.id,
            )
            .await?;
            let raw = tokio::fs::read(
                local
                    .local_path()
                    .context("managed artifact has no path")?
                    .to_string()
                    + "/manifest.yaml",
            )
            .await?;
            let manifest: PluginManifestV1 =
                plugin_framework::parse_plugin_manifest(std::str::from_utf8(&raw)?)?;
            let managed = manifest.managed.context("managed declaration required")?;
            if manifest.publisher_namespace != installation.organization
                || manifest.version != installation.plugin_version
                || managed.module.module_id.as_str() != installation.provider_code
                || serde_json::to_value(&managed)? != installation.metadata_json["managed"]
            {
                bail!("managed installed bytes and durable identity disagree");
            }
            let lease = self
                .store
                .lock_installation_contribution_authority(installation.id, workspace_id)
                .await?;
            if lease
                .installation(installation.id)
                .context("missing locked installation")?
                .updated_at
                != installation.updated_at
            {
                bail!("managed installation changed during preparation");
            }
            let authority = lease.snapshot().clone();
            lease.release().await?;
            packages.push(PreparedPackage {
                installation,
                manifest: managed,
                artifact_fingerprint: ManagedArtifactFingerprint::from_bytes(&raw),
                authority,
            });
        }
        packages.sort_by_key(|package| package.installation.id);
        Ok(packages)
    }

    pub(crate) async fn execute(
        &self,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if principal.workspace_id != workspace_id.to_string() {
            bail!("managed execution workspace mismatch");
        }
        let snapshot = self
            .snapshot(workspace_id)
            .await
            .context("managed workspace is not activated")?;
        let binding = snapshot
            .bindings
            .get(contribution_id)
            .context("managed contribution is not activated")?;
        let lease = self
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        let installation = lease
            .installation(binding.installation.id)
            .context("managed installation missing")?;
        if installation.desired_state != domain::PluginDesiredState::ActiveRequested
            || installation.metadata_json != binding.installation.metadata_json
            || installation.organization != binding.installation.organization
            || installation.provider_code != binding.installation.provider_code
            || installation.plugin_version != binding.installation.plugin_version
        {
            bail!("managed installation is disabled or changed");
        }
        self.policy.effective_permissions(
            installation,
            workspace_id,
            &binding.descriptor,
            lease.snapshot(),
        )?;
        let admitted = self
            .backend
            .admit_managed_capability_execute(RuntimeManagedCapabilityRequest {
                handle: binding.handle.clone(),
                principal,
                config_payload,
                input_payload,
            })
            .await?;
        // On release failure the already-admitted future drops here, releasing its execution lease.
        lease.release().await?;
        Ok(admitted.await?)
    }
}

fn managed_subject(
    installation_id: Uuid,
    workspace_id: Uuid,
    contribution_id: ContributionId,
) -> Result<ManagedContributionSubject> {
    Ok(ManagedContributionSubject::new(
        ManagedInstallationId::new(installation_id.to_string())?,
        ManagedWorkspaceId::new(workspace_id.to_string())?,
        contribution_id,
    ))
}
