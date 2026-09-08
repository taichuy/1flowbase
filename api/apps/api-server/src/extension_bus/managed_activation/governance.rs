use super::*;
use control_plane::plugin_management::ManagedExecutionGovernancePort;
use control_plane_contracts::ports::*;

pub(super) fn target(
    snapshot: &ManagedWorkspaceSnapshot,
    binding: &ManagedContributionBinding,
    epoch: Uuid,
) -> ManagedFrozenExecutionTarget {
    ManagedFrozenExecutionTarget {
        graph_fingerprint: snapshot.graph.fingerprint().as_str().into(),
        handler_id: format!(
            "managed.{}.{}",
            binding.handle.identity().workspace_id().as_str(),
            binding.descriptor.contribution_id.as_str()
        ),
        handler_version: format!(
            "{}:{}:{}:{}:{}",
            epoch,
            binding.handle.identity().artifact_fingerprint().as_str(),
            binding.handle.identity().binding_fingerprint().as_str(),
            binding.handle.generation(),
            binding.installation.id
        ),
    }
}
fn delivery_projection(record: &LifecycleOutboxRecord) -> ManagedLifecycleDelivery {
    ManagedLifecycleDelivery {
        event_id: record.event_id,
        subscriber_id: record.subscriber_id.clone(),
        target: ManagedFrozenExecutionTarget {
            graph_fingerprint: record.graph_fingerprint.clone(),
            handler_id: record.handler_id.clone(),
            handler_version: record.handler_version.clone(),
        },
        status: match record.status {
            LifecycleOutboxStatus::Pending => "pending",
            LifecycleOutboxStatus::Claimed => "claimed",
            LifecycleOutboxStatus::Paused => "paused",
            LifecycleOutboxStatus::Delivered => "delivered",
        }
        .into(),
        pause_reason: record.pause_reason.map(|r| r.as_str().into()),
        ownership: if managed_handler_installation(&record.handler_version).is_some() {
            "verified"
        } else {
            "unknown_legacy"
        }
        .into(),
    }
}
// This local port owner retains the shared composition across detached retirement work.
// Implementing the foreign port on Arc<ManagedExtensionComposition> violates orphan rules.
struct ManagedCompositionGovernance(Arc<ManagedExtensionComposition>);

impl ManagedExtensionComposition {
    pub(crate) fn governance(self: &Arc<Self>) -> Arc<dyn ManagedExecutionGovernancePort> {
        Arc::new(ManagedCompositionGovernance(self.clone()))
    }

    async fn installation_deliveries(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
    ) -> Result<Vec<LifecycleOutboxRecord>> {
        let installation = self
            .store
            .get_installation(installation_id)
            .await?
            .context("managed installation missing")?;
        let manifest: plugin_framework::ManagedManifest =
            serde_json::from_value(installation.metadata_json["managed"].clone())?;
        let ids = manifest
            .module
            .contributions
            .iter()
            .map(|c| format!("managed.{workspace_id}.{}", c.contribution_id.as_str()))
            .collect::<Vec<_>>();
        Ok(self
            .store
            .managed_lifecycle_deliveries(&ids)
            .await?
            .into_iter()
            .filter(|record| {
                managed_handler_installation(&record.handler_version)
                    .is_none_or(|owner| owner == installation_id)
            })
            .collect())
    }
    async fn execution_state(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
    ) -> Result<ManagedExecutionState> {
        let deliveries = self
            .installation_deliveries(workspace_id, installation_id)
            .await?
            .iter()
            .map(delivery_projection)
            .collect();
        let snapshots = self.snapshots.lock().await;
        let mut executions = Vec::<ManagedExecutionReference>::new();
        let mut counted = std::collections::BTreeSet::new();
        for (snapshot, is_current) in snapshots
            .current
            .values()
            .map(|s| (s, true))
            .chain(snapshots.retained.values().flatten().map(|s| (s, false)))
        {
            for binding in snapshot.bindings.values().filter(|b| {
                b.installation.id == installation_id
                    && b.handle.identity().workspace_id().as_str() == workspace_id.to_string()
            }) {
                let expected = target(snapshot, binding, self.execution_epoch);
                let references = if counted.insert((
                    serde_json::to_string(&expected)?,
                    Arc::as_ptr(&snapshot.lifetime) as usize,
                )) {
                    snapshot.lifetime.reference_count()
                } else {
                    0
                };
                if let Some(execution) = executions
                    .iter_mut()
                    .find(|execution| execution.target == expected)
                {
                    execution.current |= is_current;
                    execution.frozen_reference_count += references;
                } else {
                    executions.push(ManagedExecutionReference {
                        target: expected,
                        current: is_current,
                        frozen_reference_count: references,
                    });
                }
            }
        }
        Ok(ManagedExecutionState {
            installation_id,
            workspace_id,
            executions,
            deliveries,
        })
    }
    async fn resume_delivery(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        input: ResumeManagedLifecycleDelivery,
    ) -> Result<ManagedExecutionState> {
        if managed_handler_installation(&input.expected.handler_version) != Some(installation_id) {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_frozen_target_unverifiable",
            )
            .into());
        }
        let record = self
            .installation_deliveries(workspace_id, installation_id)
            .await?
            .into_iter()
            .find(|r| {
                r.event_id == input.event_id
                    && r.subscriber_id == input.subscriber_id
                    && delivery_projection(r).target == input.expected
            })
            .ok_or(control_plane::errors::ControlPlaneError::Conflict(
                "managed_paused_target_missing",
            ))?;
        if record.status != LifecycleOutboxStatus::Paused {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_target_not_paused",
            )
            .into());
        }
        let snapshot = self.event_snapshot_for_graph(&record).await?.ok_or(
            control_plane::errors::ControlPlaneError::Conflict("managed_frozen_graph_unavailable"),
        )?;
        let _reference = snapshot.freeze_reference()?;
        let binding = snapshot
            .bindings
            .values()
            .find(|binding| target(&snapshot, binding, self.execution_epoch) == input.expected)
            .context("managed frozen handler unavailable")?;
        let lease = self
            .store
            .lock_contribution_authority(binding.handle.identity().subject())
            .await?;
        self.validate_event_current_binding(binding, workspace_id, lease.as_ref())?;
        // Admission verifies the original executable digest and mount, without starting a worker.
        let admitted = self
            .backend
            .admit_managed_event(runtime_core::runtime_backend::RuntimeManagedEventRequest {
                handle: binding.handle.clone(),
                graph_fingerprint: record.graph_fingerprint.clone(),
                authority_revision: lease.snapshot().revision,
                deadline_unix_ms: ((time::OffsetDateTime::now_utc() + time::Duration::seconds(10))
                    .unix_timestamp_nanos()
                    / 1_000_000) as i64,
                delivery: decode_event_delivery(&record)?,
            })
            .await?;
        drop(admitted);
        lease.commit_resume_managed_delivery(input).await?;
        self.execution_state(workspace_id, installation_id).await
    }
    async fn retire_execution(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        expected: ManagedFrozenExecutionTarget,
    ) -> Result<ManagedExecutionState> {
        if managed_handler_installation(&expected.handler_version) != Some(installation_id) {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_frozen_target_unverifiable",
            )
            .into());
        }
        let assembly = self.assembly.lock().await;
        let visible = self.snapshots.lock().await;
        let retirement_key = serde_json::to_string(&expected)?;
        visible.ensure_retirement_capacity(&retirement_key)?;
        let matches = |snapshot: &ManagedWorkspaceSnapshot| {
            snapshot.bindings.values().any(|binding| {
                binding.installation.id == installation_id
                    && binding.handle.identity().workspace_id().as_str() == workspace_id.to_string()
                    && target(snapshot, binding, self.execution_epoch) == expected
            })
        };
        if visible.current.values().any(|s| matches(s)) {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_execution_is_current",
            )
            .into());
        }
        let snapshots = visible
            .retained
            .values()
            .flatten()
            .filter(|s| matches(s))
            .cloned()
            .collect::<Vec<_>>();
        if snapshots.is_empty() {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_retained_target_missing",
            )
            .into());
        }
        let mut retirement_markers = BTreeMap::new();
        for snapshot in &snapshots {
            for binding in snapshot.bindings.values() {
                retirement_markers.insert(
                    serde_json::to_string(&target(snapshot, binding, self.execution_epoch))?,
                    snapshot.lifetime.clone(),
                );
            }
        }
        if visible.retired_targets.len()
            + retirement_markers
                .keys()
                .filter(|key| !visible.retired_targets.contains_key(*key))
                .count()
            > MAX_RETIRED_TARGETS
        {
            bail!("managed retired target capacity exhausted");
        }
        let shared_handles = visible
            .current
            .values()
            .chain(visible.retained.values().flatten().filter(|candidate| {
                !snapshots
                    .iter()
                    .any(|retired| candidate.same_execution_snapshot(retired))
            }))
            .flat_map(|snapshot| {
                snapshot
                    .bindings
                    .values()
                    .map(|binding| binding.handle.clone())
            })
            .collect::<Vec<_>>();
        let mut lifetimes = Vec::new();
        let mut retirements = Vec::new();
        for snapshot in &snapshots {
            if !lifetimes
                .iter()
                .any(|lifetime| Arc::ptr_eq(lifetime, &snapshot.lifetime))
            {
                retirements.push(snapshot.lifetime.close()?);
                lifetimes.push(snapshot.lifetime.clone());
            }
        }
        drop(visible);
        // Every retained snapshot carrying this exact target must be closed and unreferenced.
        for retirement in &retirements {
            retirement.ensure_unreferenced()?;
        }
        let mut handles = Vec::new();
        for binding in snapshots
            .iter()
            .flat_map(|snapshot| snapshot.bindings.values())
        {
            if !handles.contains(&binding.handle) {
                handles.push(binding.handle.clone());
            }
        }
        // Other graphs may still own the same runtime handle. Their admissions remain open;
        // the retiring graph's own calls are covered by its explicit snapshot references.
        let exclusive_handles = handles
            .iter()
            .filter(|handle| !shared_handles.contains(handle))
            .cloned()
            .collect::<Vec<_>>();
        let drain = self
            .backend
            .drain_managed_contributions(&exclusive_handles)
            .await?;
        tokio::time::timeout(std::time::Duration::from_secs(5), drain.wait_drained())
            .await
            .context("managed retirement drain timed out")??;
        // Freeze is closed before this query. Existing publication leases have finished their
        // commit/rollback, so an empty durable result cannot be invalidated by an old publisher.
        for snapshot in &snapshots {
            for binding in snapshot.bindings.values() {
                let scope = Uuid::parse_str(binding.handle.identity().workspace_id().as_str())?;
                if self
                    .installation_deliveries(scope, binding.installation.id)
                    .await?
                    .iter()
                    .any(|record| {
                        record.status != LifecycleOutboxStatus::Delivered
                            && (managed_handler_installation(&record.handler_version).is_none()
                                || (record.graph_fingerprint == expected.graph_fingerprint
                                    && snapshot.lifecycle_plan.as_ref().is_some_and(|plan| {
                                        plan.subscribers().iter().any(|s| {
                                            s.handler_id == record.handler_id
                                                && s.handler_version == record.handler_version
                                        })
                                    })))
                    })
                {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "managed_execution_has_durable_deliveries",
                    )
                    .into());
                }
            }
            if let Some(plan) = &snapshot.lifecycle_plan {
                let ids = plan
                    .subscribers()
                    .iter()
                    .map(|subscriber| subscriber.subscriber_id.clone())
                    .collect::<Vec<_>>();
                for record in self.store.managed_lifecycle_deliveries(&ids).await? {
                    if record.status != LifecycleOutboxStatus::Delivered
                        && record.graph_fingerprint == expected.graph_fingerprint
                        && plan.subscribers().iter().any(|subscriber| {
                            subscriber.handler_id == record.handler_id
                                && subscriber.handler_version == record.handler_version
                        })
                        && decode_event_delivery(&record)?.workspace_id == workspace_id.to_string()
                    {
                        return Err(control_plane::errors::ControlPlaneError::Conflict(
                            "managed_execution_has_durable_deliveries",
                        )
                        .into());
                    }
                }
            }
        }
        let mut visible = self.snapshots.lock().await;
        for retained in visible.retained.values_mut() {
            retained.retain(|s| {
                !snapshots
                    .iter()
                    .any(|retired| s.same_execution_snapshot(retired))
            });
        }
        visible
            .retained
            .retain(|_, snapshots| !snapshots.is_empty());
        let still_owned = visible
            .current
            .values()
            .chain(visible.retained.values().flatten())
            .flat_map(|s| s.bindings.values().map(|b| b.handle.clone()))
            .collect::<Vec<_>>();
        for retirement in &retirements {
            retirement.retire();
        }
        visible.retired_targets.extend(retirement_markers);
        drop(visible);
        for handle in handles.into_iter().filter(|h| !still_owned.contains(h)) {
            self.backend
                .deactivate_managed_contribution(&handle)
                .await?;
        }
        drop(drain);
        drop(assembly);
        self.execution_state(workspace_id, installation_id).await
    }
}
#[async_trait::async_trait]
impl ManagedExecutionGovernancePort for ManagedCompositionGovernance {
    async fn managed_execution_state(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
    ) -> Result<ManagedExecutionState> {
        self.0.execution_state(workspace_id, installation_id).await
    }
    async fn resume_managed_delivery(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        input: ResumeManagedLifecycleDelivery,
    ) -> Result<ManagedExecutionState> {
        self.0
            .resume_delivery(workspace_id, installation_id, input)
            .await
    }
    async fn retire_managed_execution(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
        target: ManagedFrozenExecutionTarget,
    ) -> Result<ManagedExecutionState> {
        let permit = self
            .0
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Retirement)?;
        let owner = self.0.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result=owner.retire_execution(workspace_id, installation_id, target).await;
            if let Err(error)=&result { tracing::warn!(%workspace_id,%installation_id,%error,"managed retirement owner failed"); }
            result
        })
        .await
        .context("managed retirement owner terminated")?
    }
}
#[async_trait::async_trait]
impl ManagedArtifactRemovalGuard for ManagedExtensionComposition {
    async fn guard_managed_artifact_removal(
        &self,
        installation_ids: &[Uuid],
    ) -> Result<Box<dyn Send + Sync>> {
        let operation = self
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Retirement)?;
        let assembly = self.assembly.clone().lock_owned().await;
        let visible = self.snapshots.lock().await;
        if visible
            .current
            .values()
            .chain(visible.retained.values().flatten())
            .any(|s| {
                s.bindings
                    .values()
                    .any(|b| installation_ids.contains(&b.installation.id))
            })
        {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "managed_artifact_has_execution_references",
            )
            .into());
        }
        drop(visible);
        for installation_id in installation_ids {
            // Historical authorities are needed after assignment removal and host restart.
            let workspaces = self
                .store
                .managed_installation_workspaces(*installation_id)
                .await?;
            for workspace in workspaces {
                if self
                    .installation_deliveries(workspace, *installation_id)
                    .await?
                    .iter()
                    .any(|r| r.status != LifecycleOutboxStatus::Delivered)
                {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "managed_artifact_has_durable_deliveries",
                    )
                    .into());
                }
            }
        }
        Ok(Box::new((assembly, operation)))
    }
}
