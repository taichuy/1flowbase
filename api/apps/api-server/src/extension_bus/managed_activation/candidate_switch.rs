use super::*;
use control_plane_contracts::ports::ManagedInstallationSwitch;

impl ManagedExtensionComposition {
    /// The owned operation completes publication even if its protocol waiter is dropped.
    pub(crate) async fn switch_installation(
        self: &Arc<Self>,
        workspace_id: Uuid,
        current: Uuid,
        target: Uuid,
        audit: domain::AuditLogRecord,
        task: domain::PluginTaskRecord,
    ) -> Result<domain::PluginTaskRecord> {
        let permit = self
            .operations
            .admit(control_plane_contracts::ports::ManagedOwnedOperation::Candidate)?;
        let owner = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let result = owner.switch_candidate(workspace_id, current, target, audit).await;
            let mut detail = task.detail_json.clone();
            detail["migrated_instance_count"] = serde_json::json!(0);
            let finalized = owner.store.update_task_status(&control_plane_contracts::ports::UpdatePluginTaskStatusInput {
                task_id: task.id,
                status: if result.is_ok() { domain::PluginTaskStatus::Succeeded } else { domain::PluginTaskStatus::Failed },
                status_message: Some(match &result { Ok(()) => "switched".into(), Err(error) => error.to_string() }),
                detail_json: detail,
            }).await;
            if let Err(error) = &finalized {
                tracing::error!(task_id = %task.id, error = %error, "managed switch task finalization failed");
            }
            if let Err(error) = &result {
                tracing::warn!(task_id = %task.id, error = %error, "managed switch failed");
            }
            result?;
            finalized

        })
        .await
        .context("managed switch owner terminated")?
    }

    async fn switch_candidate(
        &self,
        workspace_id: Uuid,
        current: Uuid,
        target: Uuid,
        audit: domain::AuditLogRecord,
    ) -> Result<()> {
        let _assembly = self.assembly.lock().await;
        let old = self
            .snapshot(workspace_id)
            .await
            .context("managed workspace has no effective graph")?;
        let previous = self.snapshots.lock().await.clone();
        let owned = previous
            .current
            .values()
            .chain(previous.retained.values().flatten())
            .flat_map(|s| s.bindings.values().map(|b| b.handle.clone()))
            .collect::<Vec<_>>();
        drop(previous);
        let mut packages = self.prepare_packages(workspace_id).await?;
        let input = ManagedInstallationSwitch {
            workspace_id,
            current_installation_id: current,
            target_installation_id: target,
            node_id: self.node_id.clone(),
            installation_ids: packages
                .iter()
                .map(|p| p.installation.id)
                .chain([target])
                .collect(),
        };
        let lease = self.store.lock_managed_installation_switch(&input).await?;
        let mut expected = BTreeMap::new();
        for authority in lease.authority().snapshots() {
            let installation = lease
                .authority()
                .installation(authority.installation_id)
                .context("managed switch installation missing")?;
            expected.insert(
                (installation.id, workspace_id),
                (authority.revision, installation.updated_at),
            );
        }
        let target_installation = lease
            .authority()
            .installation(target)
            .context("managed candidate missing")?
            .clone();
        let target_authority = lease
            .authority()
            .snapshots()
            .iter()
            .find(|s| s.installation_id == target)
            .context("managed candidate authority missing")?
            .clone();
        lease.release().await?;
        // Source declarations are replaced only in this unpublished candidate, never in durable
        // assignment or the current graph. The candidate has its own explicit grants.
        packages.retain(|p| p.installation.id != current);
        packages.push(
            self.prepare_package(target_installation, target_authority)
                .await?,
        );
        let mut handles = Vec::new();
        let switched = async {
            let mut candidate_expected = BTreeMap::new();
            let candidate = self
                .prepare_snapshot(
                    workspace_id,
                    &packages,
                    &owned,
                    &mut candidate_expected,
                    &mut handles,
                )
                .await?;
            for (id, facts) in candidate_expected {
                if expected.get(&id) != Some(&facts) {
                    bail!("managed candidate changed during preparation");
                }
            }
            let affected = owned
                .iter()
                .filter(|handle| {
                    handle.identity().installation_id().as_str() == current.to_string()
                })
                .cloned()
                .collect::<Vec<_>>();
            if affected.is_empty() {
                bail!("managed source has no current executable bindings");
            }
            // The reversible runtime gate is independent of authority locks. Already admitted
            // event effects can acquire their final grant lease and finish while we drain.
            let drain = self.backend.drain_managed_contributions(&affected).await?;
            tokio::time::timeout(std::time::Duration::from_secs(5), drain.wait_drained())
                .await
                .context("managed upgrade drain timed out; current graph retained")??;
            let lease = self.store.lock_managed_installation_switch(&input).await?;
            Self::validate_candidate(&expected, lease.authority())?;
            let mut visible = self.snapshots.lock().await;
            visible.ensure_publication_capacity(
                &[(workspace_id, candidate.clone())].into_iter().collect(),
            )?;
            // All fallible compilation/activation/drain work precedes this short durable commit.
            // The owned task cannot be cancelled by a dropped request while committing.
            lease.commit(audit).await?;
            let retained = visible
                .retained
                .entry(old.graph.fingerprint().as_str().into())
                .or_default();
            if !retained.iter().any(|s| s.same_execution_snapshot(&old)) {
                retained.push(old.clone());
            }
            visible.current.insert(workspace_id, candidate);
            drop(visible);
            drop(drain);
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if switched.is_err() {
            for handle in handles {
                if let Err(error) = self.backend.deactivate_managed_contribution(&handle).await {
                    tracing::warn!(%error, "managed rejected candidate cleanup failed");
                }
            }
        }
        switched
    }
}
