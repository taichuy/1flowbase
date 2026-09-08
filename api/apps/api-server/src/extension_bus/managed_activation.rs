//! Workspace-scoped immutable managed graph assembly and fresh execution admission.
mod candidate_switch;
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
    pub(crate) lifecycle_plan: Option<EffectiveLifecycleSubscriberPlan>,
}

#[derive(Clone, Default)]
struct ManagedSnapshots {
    current: BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
    // Publication and retention share one lock: no delivery can see a new graph before its
    // predecessor is retained. P09 owns explicit retirement after backlog/in-flight disposition.
    retained: BTreeMap<String, Vec<Arc<ManagedWorkspaceSnapshot>>>,
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
    native_lifecycle_plan: std::sync::OnceLock<EffectiveLifecycleSubscriberPlan>,
    /// One assembly owner; immutable snapshots remain valid while callers hold their Arc.
    snapshots: Mutex<ManagedSnapshots>,
    execution_epoch: Uuid,
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
            native_lifecycle_plan: Default::default(),
            snapshots: Mutex::new(ManagedSnapshots::default()),
            execution_epoch: Uuid::now_v7(),
            assembly: Mutex::new(()),
        }
    }

    pub(crate) async fn snapshot(
        &self,
        workspace_id: Uuid,
    ) -> Option<Arc<ManagedWorkspaceSnapshot>> {
        self.snapshots
            .lock()
            .await
            .current
            .get(&workspace_id)
            .cloned()
    }

    pub(crate) async fn rebuild_installation(&self, installation_id: Uuid) -> Result<()> {
        let _assembly = self.assembly.lock().await;
        let previous_state = self.snapshots.lock().await.clone();
        let mut published = previous_state.current.clone();
        let owned_handles = previous_state
            .current
            .values()
            .chain(previous_state.retained.values().flatten())
            .flat_map(|snapshot| {
                snapshot
                    .bindings
                    .values()
                    .map(|binding| binding.handle.clone())
            })
            .collect::<Vec<_>>();
        drop(previous_state);
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
                let candidate = self
                    .prepare_snapshot(
                        workspace_id,
                        &packages,
                        &owned_handles,
                        &mut expected,
                        &mut new_handles,
                    )
                    .await?;
                candidates.insert(workspace_id, candidate);
            }
            // A disabled target may no longer appear in the new graph. Lock its assignment and
            // installation too, so removal and a concurrent re-enable cannot cross publication.
            for workspace_id in candidates.keys() {
                if !expected.contains_key(&(installation_id, *workspace_id)) {
                    let lease = self
                        .store
                        .lock_installation_contribution_authority(installation_id, *workspace_id)
                        .await?;
                    let installation = lease
                        .installation(installation_id)
                        .context("missing locked installation")?;
                    expected.insert(
                        (installation_id, *workspace_id),
                        (lease.snapshot().revision, installation.updated_at),
                    );
                    lease.release().await?;
                }
            }
            let removed = published
                .iter()
                .filter(|(workspace, _)| candidates.contains_key(workspace))
                .flat_map(|(_, snapshot)| snapshot.bindings.values())
                .filter(|binding| {
                    !candidates.values().any(|candidate| {
                        candidate
                            .bindings
                            .values()
                            .any(|next| next.handle == binding.handle)
                    })
                })
                .map(|binding| binding.handle.clone())
                .collect::<Vec<_>>();
            let drain = if removed.is_empty() {
                None
            } else {
                Some(self.backend.drain_managed_contributions(&removed).await?)
            };
            if let Some(drain) = &drain {
                tokio::time::timeout(std::time::Duration::from_secs(5), drain.wait_drained())
                    .await
                    .context("managed graph drain timed out; current graph retained")??;
            }
            let scopes = expected.keys().copied().collect::<Vec<_>>();
            let lease = self
                .store
                .lock_contribution_authority_batch(&scopes)
                .await?;
            Self::validate_candidate(&expected, lease.as_ref())?;
            // The authority batch remains locked until the complete candidate set is visible.
            // Readers continue using the prior immutable snapshot during preparation.
            let previous = candidates
                .keys()
                .filter_map(|workspace| {
                    published
                        .get(workspace)
                        .map(|value| (*workspace, value.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            for (workspace, candidate) in &candidates {
                published.insert(*workspace, candidate.clone());
            }
            let mut visible = self.snapshots.lock().await;
            let original = visible.clone();
            for old in previous.values() {
                if candidates
                    .values()
                    .any(|candidate| candidate.same_execution_snapshot(old))
                {
                    continue;
                }
                let retained = visible
                    .retained
                    .entry(old.graph.fingerprint().as_str().into())
                    .or_default();
                if !retained
                    .iter()
                    .any(|snapshot| snapshot.same_execution_snapshot(old))
                {
                    retained.push(old.clone());
                }
            }
            visible.current = published.clone();
            if let Err(error) = lease.release().await {
                *visible = original;
                return Err(error);
            }
            Ok::<_, anyhow::Error>(previous)
        }
        .await;
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
            Ok(_) => Ok(()),
        }
    }

    async fn prepare_snapshot(
        &self,
        workspace_id: Uuid,
        packages: &[PreparedPackage],
        owned_handles: &[ManagedExecutionHandle],
        expected: &mut BTreeMap<(Uuid, Uuid), (i64, time::OffsetDateTime)>,
        new_handles: &mut Vec<ManagedExecutionHandle>,
    ) -> Result<Arc<ManagedWorkspaceSnapshot>> {
        let mut modules = self.base_modules.clone();
        let mut authority = ManagedGraphAuthority::new(self.policy.identity());
        for package in packages {
            let mut module = package.manifest.module.clone();
            // Both values are host-owned. Preserve per-contribution requirements exactly.
            module.activation = ModuleActivationDeclaration::Active;
            module.granted_permissions.clear();
            authority.managed_modules.insert(module.module_id.clone());
            if module
                .extension_points
                .iter()
                .any(|point| !point.is_managed_composition_event(&module.module_id))
            {
                bail!("managed point declaration exceeds host namespace admission");
            }
            for contribution in &module.contributions {
                let permissions = self.policy.effective_permissions(
                    &package.installation,
                    workspace_id,
                    contribution,
                    &package.authority,
                )?;
                let subject = managed_subject(
                    package.installation.id,
                    workspace_id,
                    contribution.contribution_id.clone(),
                )?;
                if authority
                    .contributions
                    .insert(
                        contribution.contribution_id.clone(),
                        ManagedContributionAuthority {
                            subject,
                            revision: package.authority.revision,
                            permissions,
                        },
                    )
                    .is_some()
                {
                    bail!("duplicate managed contribution identity");
                }
            }
            expected.insert(
                (package.installation.id, workspace_id),
                (package.authority.revision, package.installation.updated_at),
            );
            modules.push(module);
        }
        let graph = Arc::new(compile_extension_graph_with_authority(modules, &authority)?);
        for receipt in graph.contribution_receipts() {
            if authority
                .contributions
                .contains_key(&receipt.descriptor().contribution_id)
                && receipt.status() != &ContributionResolutionStatus::Active
            {
                bail!("managed contribution is inactive in the effective graph");
            }
        }
        let mut bindings = BTreeMap::new();
        for package in packages {
            for contribution in &package.manifest.module.contributions {
                let identity = ManagedExecutionIdentity::new(
                    ManagedInstallationId::new(package.installation.id.to_string())?,
                    ManagedWorkspaceId::new(workspace_id.to_string())?,
                    contribution.contribution_id.clone(),
                    package.artifact_fingerprint.clone(),
                    package
                        .manifest
                        .execution_binding_fingerprint(&contribution.contribution_id)?,
                );
                let handle = self
                    .backend
                    .activate_managed_contribution(RuntimeManagedActivation {
                        plugin_id: package.installation.plugin_id.clone(),
                        artifact: RuntimeArtifactReference::new(
                            package.installation.id.to_string(),
                        )?,
                        identity,
                    })
                    .await?;
                if !owned_handles.contains(&handle) {
                    new_handles.push(handle.clone());
                }
                bindings.insert(
                    contribution.contribution_id.clone(),
                    ManagedContributionBinding {
                        handle,
                        descriptor: contribution.clone(),
                        installation: package.installation.clone(),
                    },
                );
            }
        }
        let lifecycle_plan = self.compile_event_plan(&graph, &bindings)?;
        Ok(Arc::new(ManagedWorkspaceSnapshot {
            graph,
            authority,
            bindings,
            lifecycle_plan,
        }))
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
            packages.push(self.prepare_package(installation, authority).await?);
        }
        packages.sort_by_key(|package| package.installation.id);
        Ok(packages)
    }

    async fn prepare_package(
        &self,
        installation: domain::PluginInstallationRecord,
        authority: domain::PluginContributionAuthoritySnapshot,
    ) -> Result<PreparedPackage> {
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
        Ok(PreparedPackage {
            installation,
            manifest: managed,
            artifact_fingerprint: ManagedArtifactFingerprint::from_bytes(&raw),
            authority,
        })
    }

    /// Execute against the invocation's retained plan, with fresh authority at every admission.
    /// A newer published workspace snapshot must never substitute the requested binding.
    pub(crate) async fn execute_hook(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        mut invocation: extension_contracts::ManagedHookInvocation,
        input: extension_contracts::ManagedCreateHookInput,
    ) -> Result<extension_contracts::ManagedHookOutcome> {
        let binding = snapshot
            .bindings
            .get(contribution_id)
            .context("managed hook contribution is absent from the frozen snapshot")?;
        let workspace_id = Uuid::parse_str(binding.handle.identity().workspace_id().as_str())?;
        if principal.workspace_id != workspace_id.to_string()
            || invocation.graph_fingerprint != snapshot.graph.fingerprint().as_str()
            || binding.descriptor.point_id.as_str() != input.point_id()
            || binding.descriptor.contract_version.as_str() != "1"
        {
            bail!("managed hook frozen plan or scope mismatch");
        }
        let frozen_authority = snapshot
            .authority
            .contributions
            .get(contribution_id)
            .context("managed hook frozen authority is missing")?;
        if &frozen_authority.subject != binding.handle.identity().subject() {
            bail!("managed hook frozen authority subject mismatch");
        }
        // The adapter supplies invocation/registry identity; the composition owns grant identity.
        invocation.authority_revision = frozen_authority.revision;
        let lease = self
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        self.validate_current_binding(binding, workspace_id, lease.as_ref())?;
        let admitted = self
            .backend
            .admit_managed_hook(runtime_core::runtime_backend::RuntimeManagedHookRequest {
                handle: binding.handle.clone(),
                principal,
                invocation,
                input,
            })
            .await?;
        lease.release().await?;
        Ok(admitted.await?)
    }

    fn validate_current_binding(
        &self,
        binding: &ManagedContributionBinding,
        workspace_id: Uuid,
        lease: &dyn ContributionAuthorityLease,
    ) -> Result<()> {
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
        Ok(())
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
        self.validate_current_binding(binding, workspace_id, lease.as_ref())?;
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

impl ManagedExtensionComposition {
    pub(crate) fn attach_native_lifecycle_plan(
        &self,
        plan: EffectiveLifecycleSubscriberPlan,
    ) -> Result<()> {
        self.native_lifecycle_plan
            .set(plan)
            .map_err(|_| anyhow::anyhow!("native lifecycle plan already attached"))
    }

    fn compile_event_plan(
        &self,
        graph: &EffectiveExtensionGraph,
        bindings: &BTreeMap<ContributionId, ManagedContributionBinding>,
    ) -> Result<Option<EffectiveLifecycleSubscriberPlan>> {
        let event_bindings = bindings
            .values()
            .filter(|binding| {
                binding
                    .descriptor
                    .required_permissions
                    .iter()
                    .any(|p| p.as_str() == "event.subscribe")
            })
            .collect::<Vec<_>>();
        if event_bindings.is_empty() {
            return Ok(None);
        }
        let mut subscriber_bindings = self
            .native_lifecycle_plan
            .get()
            .context("managed events require native lifecycle plan binding")?
            .bindings();
        for binding in event_bindings {
            let (contract_id, contract_version) = match binding.descriptor.point_id.as_str() {
                extension_contracts::MANAGED_CREATE_EVENT_POINT => {
                    (extension_contracts::MANAGED_CREATE_EVENT_ID, "v1")
                }
                extension_contracts::MANAGED_PROCESSED_EVENT_ID => {
                    (extension_contracts::MANAGED_PROCESSED_EVENT_ID, "1")
                }
                _ => bail!("managed event subscription contract is not opened"),
            };
            let id = format!(
                "managed.{}.{}",
                binding.handle.identity().workspace_id().as_str(),
                binding.descriptor.contribution_id.as_str()
            );
            subscriber_bindings.push(LifecycleSubscriberBinding {
                contribution_id: binding.descriptor.contribution_id.clone(),
                subscription_id: id.clone(),
                point_id: binding.descriptor.point_id.clone(),
                fact_contract_id: contract_id.into(),
                fact_contract_version: contract_version.into(),
                handler_id: id,
                handler_version: format!(
                    "{}:{}:{}:{}",
                    self.execution_epoch,
                    binding.handle.identity().artifact_fingerprint().as_str(),
                    binding.handle.identity().binding_fingerprint().as_str(),
                    binding.handle.generation()
                ),
            });
        }
        Ok(Some(compile_lifecycle_subscriber_plan(
            graph,
            subscriber_bindings,
        )?))
    }

    /// Resolve the complete persisted target, never a handler from the currently published graph.
    /// An epoch in handler_version prevents a restarted host's mount counter from impersonating
    /// an old executable generation. Missing historical graphs remain durably paused by P06.
    pub(crate) async fn event_snapshot_for_graph(
        &self,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<Option<Arc<ManagedWorkspaceSnapshot>>> {
        use control_plane_contracts::ports::{
            LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason,
        };
        let snapshots = self.snapshots.lock().await;
        let candidates = snapshots
            .current
            .values()
            .chain(
                snapshots
                    .retained
                    .get(&record.graph_fingerprint)
                    .into_iter()
                    .flatten(),
            )
            .filter(|snapshot| snapshot.graph.fingerprint().as_str() == record.graph_fingerprint);
        let mut graph_available = false;
        for snapshot in candidates {
            graph_available = true;
            if snapshot.lifecycle_plan.as_ref().is_some_and(|plan| {
                plan.subscribers().iter().any(|s| {
                    s.subscriber_id == record.subscriber_id
                        && s.handler_id == record.handler_id
                        && s.handler_version == record.handler_version
                        && s.fact_contract_id == record.contract_id
                        && s.fact_contract_version == record.contract_version
                })
            }) {
                return Ok(Some(snapshot.clone()));
            }
        }
        if graph_available {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
            )
            .into());
        }
        Ok(None)
    }

    /// A record can reach a native handler only after its exact target is found in the frozen
    /// combined plan. Managed handlers additionally acquire current subject authority at admission.
    pub(crate) async fn deliver_event_record(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<bool> {
        if snapshot.graph.fingerprint().as_str() != record.graph_fingerprint {
            bail!("managed event frozen graph mismatch");
        }
        let subscriber = snapshot
            .lifecycle_plan
            .as_ref()
            .context("managed lifecycle plan missing")?
            .subscribers()
            .iter()
            .find(|s| {
                s.subscriber_id == record.subscriber_id
                    && s.handler_id == record.handler_id
                    && s.handler_version == record.handler_version
                    && s.fact_contract_id == record.contract_id
                    && s.fact_contract_version == record.contract_version
            })
            .ok_or(control_plane_contracts::ports::LifecycleDeliveryBlocked(control_plane_contracts::ports::LifecycleDeliveryPauseReason::FrozenHandlerUnavailable))?;
        if !matches!(
            subscriber.contributor_module_kind,
            ModuleKind::Runtime | ModuleKind::Capability
        ) {
            return Ok(false);
        }
        let binding = snapshot
            .bindings
            .get(&subscriber.contribution_id)
            .ok_or(control_plane_contracts::ports::LifecycleDeliveryBlocked(
            control_plane_contracts::ports::LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
        ))?;
        let delivery = decode_event_delivery(record)?;
        let workspace_id = Uuid::parse_str(binding.handle.identity().workspace_id().as_str())?;
        if delivery.workspace_id != workspace_id.to_string() {
            bail!("managed event cross-workspace delivery rejected");
        }
        let frozen = snapshot
            .authority
            .contributions
            .get(&subscriber.contribution_id)
            .context("managed event frozen authority missing")?;
        if &frozen.subject != binding.handle.identity().subject() {
            bail!("managed event authority subject mismatch");
        }
        let lease = self
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        self.validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
        let admitted = self
            .backend
            .admit_managed_event(runtime_core::runtime_backend::RuntimeManagedEventRequest {
                handle: binding.handle.clone(),
                graph_fingerprint: record.graph_fingerprint.clone(),
                authority_revision: lease.snapshot().revision,
                deadline_unix_ms: ((time::OffsetDateTime::now_utc() + time::Duration::seconds(10))
                    .unix_timestamp_nanos()
                    / 1_000_000) as i64,
                delivery: delivery.clone(),
            })
            .await
            .map_err(|error| -> anyhow::Error {
                // No worker has started: the already validated frozen binding cannot be admitted.
                // In-flight worker/process failures below remain retryable within the retry budget.
                if matches!(&error, runtime_core::runtime_backend::RuntimeBackendError::Contract(_) | runtime_core::runtime_backend::RuntimeBackendError::Unavailable(_) | runtime_core::runtime_backend::RuntimeBackendError::MissingBackend | runtime_core::runtime_backend::RuntimeBackendError::UnsupportedOperation(_)) {
                    tracing::warn!(%error, "frozen managed event handler admission unavailable");
                    control_plane_contracts::ports::LifecycleDeliveryBlocked(control_plane_contracts::ports::LifecycleDeliveryPauseReason::FrozenHandlerUnavailable).into()
                } else { error.into() }
            })?;
        lease.release().await?;
        match admitted.await? {
            extension_contracts::ManagedEventOutcome::Acknowledged => {
                if binding
                    .descriptor
                    .required_permissions
                    .iter()
                    .any(|p| p.as_str() == "plugin_data.owned.write")
                {
                    bail!("managed event owned effect required before acknowledgement");
                }
            }
            extension_contracts::ManagedEventOutcome::ApplyProcessed { effect } => {
                if delivery.contract_id != extension_contracts::MANAGED_PROCESSED_EVENT_ID
                    || delivery.contract_version != "1"
                    || effect != delivery.payload
                    || !binding
                        .descriptor
                        .required_permissions
                        .iter()
                        .any(|p| p.as_str() == "plugin_data.owned.write")
                {
                    bail!("managed event processed effect contract mismatch");
                }
                let lease = self
                    .store
                    .lock_contribution_authority(binding.handle.identity().subject())
                    .await?;
                self.validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
                lease
                    .commit_processed_model(
                        binding.handle.identity().subject().clone(),
                        Uuid::parse_str(&delivery.event_id)?,
                        effect,
                        ((time::OffsetDateTime::now_utc() + time::Duration::seconds(10))
                            .unix_timestamp_nanos()
                            / 1_000_000) as i64,
                    )
                    .await?;
            }
            extension_contracts::ManagedEventOutcome::Failed { classification } => {
                bail!("managed event handler failed: {classification}")
            }
            extension_contracts::ManagedEventOutcome::Publish { publication } => {
                // New publication gets the currently effective target set (new subscriptions only
                // see facts published after activation); the E consumer binding remains frozen.
                let current = self
                    .snapshot(workspace_id)
                    .await
                    .context("managed publisher workspace is inactive")?;
                let plan = current
                    .publication_plan(&publication.contract_id, &publication.contract_version)
                    .unwrap_or_else(
                        || control_plane_contracts::ports::LifecyclePublicationPlan {
                            graph_fingerprint: current.graph.fingerprint().as_str().into(),
                            subscribers: Vec::new(),
                        },
                    );
                let lease = self
                    .store
                    .lock_contribution_authority(binding.handle.identity().subject())
                    .await?;
                self.validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
                control_plane::managed_event_publication::publish_managed_event(
                    lease,
                    binding.handle.identity(),
                    &binding.descriptor,
                    &delivery,
                    publication,
                    plan,
                )
                .await?;
            }
        }
        Ok(true)
    }
}

impl ManagedWorkspaceSnapshot {
    fn same_execution_snapshot(&self, other: &Self) -> bool {
        self.graph.fingerprint() == other.graph.fingerprint()
            && self.bindings.len() == other.bindings.len()
            && self.bindings.iter().all(|(id, binding)| {
                other
                    .bindings
                    .get(id)
                    .is_some_and(|old| old.handle == binding.handle)
            })
    }

    pub(crate) fn publication_plan(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<control_plane_contracts::ports::LifecyclePublicationPlan> {
        let plan = self.lifecycle_plan.as_ref()?;
        let subscribers = plan
            .subscribers()
            .iter()
            .filter(|s| {
                s.fact_contract_id == contract_id && s.fact_contract_version == contract_version
            })
            .map(
                |s| control_plane_contracts::ports::LifecycleSubscriberTarget {
                    subscriber_id: s.subscriber_id.clone(),
                    handler_id: s.handler_id.clone(),
                    handler_version: s.handler_version.clone(),
                },
            )
            .collect::<Vec<_>>();
        if subscribers.is_empty() {
            return None;
        }
        Some(control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: self.graph.fingerprint().as_str().into(),
            subscribers,
        })
    }
}

fn decode_event_delivery(
    record: &control_plane_contracts::ports::LifecycleOutboxRecord,
) -> Result<extension_contracts::ManagedEventDelivery> {
    use extension_contracts::*;
    let delivery = match (
        record.contract_id.as_str(),
        record.contract_version.as_str(),
    ) {
        (MANAGED_CREATE_EVENT_ID, "v1") => {
            let fact: AfterCommitFact<
                control_plane_contracts::ports::ModelDefinitionCommittedFact,
            > = serde_json::from_slice(&record.canonical_payload)?;
            if fact.fact_id().as_str() != record.event_id.to_string()
                || fact.transaction_id().as_str() != record.transaction_id.to_string()
                || fact.contract().contract_id.as_str() != record.contract_id
                || fact.contract().contract_version.as_str() != record.contract_version
                || fact.payload().scope_kind != domain::DataModelScopeKind::Workspace
            {
                bail!("managed Create fact identity or workspace mismatch");
            }
            ManagedEventDelivery {
                event_id: record.event_id.to_string(),
                contract_id: record.contract_id.clone(),
                contract_version: record.contract_version.clone(),
                workspace_id: fact.payload().scope_id.to_string(),
                causation_id: record.event_id.to_string(),
                correlation_id: record.event_id.to_string(),
                payload: ManagedEventPayload {
                    model_id: fact.payload().model_definition_id.to_string(),
                    status: ManagedEventStatus::Committed,
                    result_reference: None,
                },
            }
        }
        (MANAGED_PROCESSED_EVENT_ID, "1") => {
            let fact: ManagedEventFact = serde_json::from_slice(&record.canonical_payload)?;
            if fact.event_id != record.event_id.to_string()
                || fact.transaction_id != record.transaction_id.to_string()
                || fact.contract_id != record.contract_id
                || fact.contract_version != record.contract_version
                || fact.workspace_id != fact.publisher.workspace_id().as_str()
            {
                bail!("managed processed fact identity mismatch");
            }
            ManagedEventDelivery {
                event_id: fact.event_id,
                contract_id: fact.contract_id,
                contract_version: fact.contract_version,
                workspace_id: fact.workspace_id,
                causation_id: fact.causation_id,
                correlation_id: fact.correlation_id,
                payload: fact.payload,
            }
        }
        _ => bail!("managed event contract is not opened"),
    };
    delivery.validate()?;
    Ok(delivery)
}

/// Weak link prevents the store's shared catalog from retaining its own composition owner.
pub(crate) struct ManagedWorkspacePublicationSource(
    pub(crate) std::sync::Weak<ManagedExtensionComposition>,
);
#[async_trait::async_trait]
impl control_plane_contracts::ports::WorkspaceLifecyclePublicationSource
    for ManagedWorkspacePublicationSource
{
    async fn plan_for_workspace(
        &self,
        workspace_id: Uuid,
        contract_id: &str,
        contract_version: &str,
    ) -> Result<Option<control_plane_contracts::ports::LifecyclePublicationPlan>> {
        let owner = self
            .0
            .upgrade()
            .context("managed publication owner unavailable")?;
        let snapshot = owner.snapshot(workspace_id).await;
        Ok(snapshot.and_then(|s| s.publication_plan(contract_id, contract_version)))
    }
}

impl ManagedExtensionComposition {
    fn validate_event_current_binding(
        &self,
        binding: &ManagedContributionBinding,
        workspace_id: Uuid,
        lease: &dyn ContributionAuthorityLease,
    ) -> Result<()> {
        use control_plane_contracts::ports::{
            LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason,
        };
        let installation =
            lease
                .installation(binding.installation.id)
                .ok_or(LifecycleDeliveryBlocked(
                    LifecycleDeliveryPauseReason::InstallationInactive,
                ))?;
        if installation.desired_state != domain::PluginDesiredState::ActiveRequested {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::InstallationInactive,
            )
            .into());
        }
        if installation.metadata_json != binding.installation.metadata_json
            || installation.organization != binding.installation.organization
            || installation.provider_code != binding.installation.provider_code
            || installation.plugin_version != binding.installation.plugin_version
        {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
            )
            .into());
        }
        self.validate_current_binding(binding, workspace_id, lease)
            .map_err(|error| {
                if matches!(
                    error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                    Some(control_plane::errors::ControlPlaneError::PermissionDenied(
                        _
                    ))
                ) {
                    LifecycleDeliveryBlocked(LifecycleDeliveryPauseReason::AuthorityRevoked).into()
                } else {
                    error
                }
            })
    }
}
