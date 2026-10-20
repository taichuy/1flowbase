use super::*;

impl ManagedExtensionComposition {
    pub(super) async fn rebuild_installation_owned(&self, installation_id: Uuid) -> Result<()> {
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
        let mut packages_by_scope = BTreeMap::new();
        for workspace in workspaces {
            packages_by_scope.insert(workspace, self.prepare_packages(workspace).await?);
        }
        let services = if self.publisher().is_some() {
            self.candidate_services(
                packages_by_scope
                    .get(&domain::SYSTEM_SCOPE_ID)
                    .map(Vec::as_slice),
            )
            .await?
        } else {
            Vec::new()
        };
        let interface_module = self
            .publisher()
            .map(|publisher| publisher.interface_module(&services))
            .transpose()?;
        let template_ids = if packages_by_scope.contains_key(&domain::SYSTEM_SCOPE_ID) {
            services
                .iter()
                .map(|service| service.installation_id)
                .collect()
        } else {
            Vec::new()
        };
        let mut candidates = BTreeMap::new();
        let mut expected = BTreeMap::new();
        let mut new_handles = Vec::new();
        let result = async {
            for (workspace_id, packages) in &packages_by_scope {
                let workspace_id = *workspace_id;
                let candidate = self
                    .prepare_snapshot(
                        workspace_id,
                        packages,
                        &owned_handles,
                        &mut expected,
                        &mut new_handles,
                        interface_module.as_ref(),
                    )
                    .await?;
                candidates.insert(workspace_id, candidate);
            }
            // A disabled target may no longer appear in the new graph. Lock its assignment and
            // installation too, so removal and a concurrent re-enable cannot cross publication.
            for workspace_id in candidates.keys() {
                if let std::collections::btree_map::Entry::Vacant(e) =
                    expected.entry((installation_id, *workspace_id))
                {
                    let lease = self
                        .store
                        .lock_installation_contribution_authority(installation_id, *workspace_id)
                        .await?;
                    let installation = lease
                        .installation(installation_id)
                        .context("missing locked installation")?;
                    e.insert((lease.snapshot().revision, installation.updated_at));
                    lease.release().await?;
                }
            }
            let mut generation_snapshots = published.clone();
            generation_snapshots.extend(candidates.clone());
            let prepared_host = self
                .prepare_host_generation(services, generation_snapshots)
                .await?;
            let permission_catalog = prepared_host
                .as_ref()
                .map(|(_, prepared)| prepared.permission_catalog.clone());
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

            lease
                .commit_managed_settings_templates(template_ids, permission_catalog)
                .await?;
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
            if let Some((publisher, prepared)) = prepared_host {
                publisher.publish(prepared);
            }
            drop(visible);
            self.notify_reclamation();
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

    pub(super) async fn prepare_snapshot(
        &self,
        workspace_id: Uuid,
        packages: &[PreparedPackage],
        owned_handles: &[ManagedExecutionHandle],
        expected: &mut BTreeMap<(Uuid, Uuid), (i64, time::OffsetDateTime)>,
        new_handles: &mut Vec<ManagedExecutionHandle>,
        interface_module: Option<&ModuleDescriptor>,
    ) -> Result<Arc<ManagedWorkspaceSnapshot>> {
        let mut modules = self.base_modules.clone();
        if let Some(module) = interface_module.or_else(|| self.interface_module.get()) {
            modules.push(module.clone());
        }
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
                let interface_protocol = package
                    .manifest
                    .execution_bindings
                    .iter()
                    .find(|binding| binding.contribution_id == contribution.contribution_id)
                    .context("managed execution binding missing")?
                    .interface_protocol;
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
                        interface_protocol,
                        installation: package.installation.clone(),
                    },
                );
            }
        }
        let lifecycle_plan = self.compile_event_plan(&graph, &bindings)?;
        let visible = self.snapshots.lock().await;
        let lifetime = visible
            .current
            .values()
            .chain(visible.retained.values().flatten())
            .find(|snapshot| {
                snapshot.graph.fingerprint() == graph.fingerprint()
                    && snapshot.bindings.len() == bindings.len()
                    && bindings.iter().all(|(id, binding)| {
                        snapshot
                            .bindings
                            .get(id)
                            .is_some_and(|old| old.handle == binding.handle)
                    })
            })
            .map(|snapshot| snapshot.lifetime.clone())
            .unwrap_or_else(|| {
                Arc::new(SnapshotLifetime::with_reclamation_notify(
                    self.reclamation.notify.clone(),
                ))
            });
        lifetime.ensure_open()?;
        let snapshot = Arc::new(ManagedWorkspaceSnapshot {
            lifetime,
            graph,
            authority,
            bindings,
            lifecycle_plan,
        });
        for binding in snapshot.bindings.values() {
            let key = serde_json::to_string(&governance::target(
                &snapshot,
                binding,
                self.execution_epoch,
            ))?;
            if let Some(retired) = visible.retired_targets.get(&key) {
                retired.ensure_open()?;
            }
        }
        Ok(snapshot)
    }

    pub(super) fn validate_candidate(
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

    pub(super) async fn prepare_packages(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PreparedPackage>> {
        let installations = if workspace_id == domain::SYSTEM_SCOPE_ID {
            self.store
                .list_installations()
                .await?
                .into_iter()
                .filter(|installation| {
                    domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                        == domain::SYSTEM_SCOPE_ID
                })
                .collect::<Vec<_>>()
        } else {
            let mut installations = Vec::new();
            for assignment in self.store.list_assignments(workspace_id).await? {
                let installation = self
                    .store
                    .get_installation(assignment.installation_id)
                    .await?
                    .context("assigned installation missing")?;
                if domain::managed_installation_scope(&installation, workspace_id) != workspace_id {
                    bail!("system managed service cannot have workspace assignment");
                }
                installations.push(installation);
            }
            installations
        };
        let mut packages = Vec::new();
        for installation in installations {
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

    pub(super) async fn prepare_package(
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
}
