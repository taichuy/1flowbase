use super::*;

pub(super) type BootSnapshotPinHook =
    Box<dyn FnOnce(&BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>) -> Result<()> + Send>;
pub(super) type BootEventCatalogBuilder = Box<
    dyn FnOnce(
            BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<domain::event_center::EventCatalogSnapshot>>
                    + Send,
            >,
        > + Send,
>;

impl ManagedExtensionComposition {
    pub(super) async fn rebuild_installation_owned(&self, installation_id: Uuid) -> Result<()> {
        let _assembly = self.assembly.lock().await;
        let workspaces = self
            .store
            .contribution_authority_workspaces(installation_id)
            .await?;
        if workspaces.is_empty() {
            bail!("managed installation requires a workspace assignment");
        }
        self.rebuild_scopes(workspaces, Some(installation_id), false, None, None)
            .await
            .map(|_| ())
    }

    pub(super) async fn rebuild_workspaces_owned_with_event_catalog_and_pin(
        &self,
        workspaces: std::collections::BTreeSet<Uuid>,
        verify_all_active_scopes: bool,
        build_event_catalog: Option<BootEventCatalogBuilder>,
        pin_after_commit: Option<BootSnapshotPinHook>,
    ) -> Result<BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>> {
        let _assembly = self.assembly.lock().await;
        self.rebuild_scopes(
            workspaces,
            None,
            verify_all_active_scopes,
            build_event_catalog,
            pin_after_commit,
        )
        .await
    }

    // Called with the assembly lock held by the candidate operation owner.
    async fn rebuild_scopes(
        &self,
        workspaces: impl IntoIterator<Item = Uuid>,
        installation_id: Option<Uuid>,
        verify_all_active_scopes: bool,
        build_event_catalog: Option<BootEventCatalogBuilder>,
        pin_after_commit: Option<BootSnapshotPinHook>,
    ) -> Result<BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>> {
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
        let workspaces: Vec<_> = workspaces.into_iter().collect();
        // Capture the old graph's authority before filtering packages by desired state.
        let mut expected = BTreeMap::new();
        for &workspace in &workspaces {
            if let Some(requested) = installation_id {
                let lease = self
                    .store
                    .lock_installation_contribution_authority(requested, workspace)
                    .await?;
                let installation = lease
                    .installation(requested)
                    .context("missing locked installation")?;
                expected.insert(
                    (requested, workspace),
                    (lease.snapshot().revision, installation.updated_at),
                );
                lease.release().await?;
            }
            if let Some(prior) = published.get(&workspace) {
                let prior_ids: std::collections::BTreeSet<_> = prior
                    .bindings
                    .values()
                    .map(|binding| binding.installation.id)
                    .filter(|prior_id| Some(*prior_id) != installation_id)
                    .collect();
                for prior_id in prior_ids {
                    if let Some(facts) = self
                        .store
                        .observe_prior_managed_scope(prior_id, workspace)
                        .await?
                    {
                        expected.insert((prior_id, workspace), facts);
                    }
                }
            }
        }
        let mut packages_by_scope = BTreeMap::new();
        for &workspace in &workspaces {
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
                if !candidate.bindings.is_empty() {
                    candidates.insert(workspace_id, candidate);
                }
            }
            // Refreshed scopes without a candidate are removed from the new generation.
            // Their old bindings still fence the commit and drain from retained state.
            let previous = workspaces
                .iter()
                .filter_map(|workspace| published.remove(workspace).map(|old| (*workspace, old)))
                .collect::<BTreeMap<_, _>>();
            published.extend(
                candidates
                    .iter()
                    .map(|(workspace, candidate)| (*workspace, candidate.clone())),
            );
            let prepared_host = self
                .prepare_host_generation(services, published.clone())
                .await?;
            let event_catalog = match prepared_host.as_ref() {
                Some((_, prepared)) => Some(prepared.event_catalog.clone()),
                None => match build_event_catalog {
                    Some(builder) => Some(builder(published.clone()).await?),
                    None => None,
                },
            };
            let permission_catalog = prepared_host
                .as_ref()
                .map(|(_, prepared)| prepared.permission_catalog.clone());
            let scopes = expected.keys().copied().collect::<Vec<_>>();
            let mut lease = self
                .store
                .lock_contribution_authority_batch(
                    &workspaces,
                    &scopes,
                    verify_all_active_scopes,
                    Some(control_plane_contracts::ports::ManagedArtifactVerification {
                        node_id: &self.node_id,
                        validate_manifest: control_plane::plugin_management::validate_installed_managed_manifest,
                    }),
                )
                .await?;
            Self::validate_candidate(&expected, lease.as_ref())?;
            Self::validate_loaded_artifacts(packages_by_scope.values().flatten(), lease.as_ref())?;
            Self::validate_package_files(packages_by_scope.values().flatten()).await?;
            // The authority batch remains locked until the complete candidate set is written to
            // visible state (and boot-pinned when applicable). Readers remain blocked on
            // `self.snapshots` until `release` commits; any commit error restores prior state.
            let mut visible = self.snapshots.lock().await;
            lease
                .stage_managed_settings_templates(template_ids, permission_catalog, event_catalog)
                .await?;
            let rollback_visible = visible.clone();
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
            #[cfg(test)]
            let hook = self.before_authority_release.lock().unwrap().take();
            #[cfg(test)]
            if let Some(hook) = hook {
                if let Err(error) = hook.await {
                    *visible = rollback_visible;
                    return Err(error);
                }
            }
            if let Err(error) = lease.release().await {
                *visible = rollback_visible;
                return Err(error);
            }
            if let Some(pin) = pin_after_commit {
                pin(&published)?;
            }
            for (workspace, snapshot) in &published {
                self.register_candidate_lifecycle_sources(*workspace, snapshot);
            }
            if let Some((publisher, prepared)) = prepared_host {
                publisher.publish(prepared);
            }
            drop(visible);
            self.reclaim_unreferenced_retained_locked().await;
            self.notify_reclamation();
            Ok::<_, anyhow::Error>(published)
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
            Ok(published) => Ok(published),
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
            let fence = (package.authority.revision, package.installation.updated_at);
            let observed = expected
                .entry((package.installation.id, workspace_id))
                .or_insert(fence);
            if *observed != fence {
                bail!("managed installation changed during preparation");
            }
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
            // Share one immutable installation record across contributions in this snapshot.
            let installation = Arc::new(package.installation.clone());
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
                        installation: Arc::clone(&installation),
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

    pub(super) fn validate_loaded_artifacts<'a>(
        packages: impl IntoIterator<Item = &'a PreparedPackage>,
        lease: &dyn ContributionAuthorityLease,
    ) -> Result<()> {
        for package in packages {
            let loaded = &package.local_artifact;
            if !lease
                .node_artifact(package.installation.id)
                .is_some_and(|live| {
                    live.artifact_status.is_ready()
                        && !domain::has_local_artifact_warning(live.last_error.as_deref())
                        && !domain::has_local_artifact_warning(loaded.last_error.as_deref())
                        && live.local_path.is_some()
                        && live.local_version == loaded.local_version
                        && live.local_checksum == loaded.local_checksum
                        && live.local_path == loaded.local_path
                        && live.package_path == loaded.package_path
                        && live.manifest_fingerprint == loaded.manifest_fingerprint
                        && live.manifest_fingerprint.as_deref()
                            == Some(package.artifact_fingerprint.as_str())
                })
            {
                bail!("managed candidate node artifact changed during preparation");
            }
        }
        Ok(())
    }

    // Run under the same lease without borrowing the non-Sync transaction across file I/O.
    pub(super) async fn validate_package_files<'a>(
        packages: impl IntoIterator<Item = &'a PreparedPackage>,
    ) -> Result<()> {
        for package in packages {
            control_plane::plugin_management::validate_installed_plugin_artifact(
                &package.installation,
                &package.local_artifact,
            )
            .await?;
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
                let execution_scope =
                    domain::managed_installation_scope(&installation, workspace_id);
                if execution_scope == domain::SYSTEM_SCOPE_ID {
                    // An explicit workspace assignment may expose a system package's
                    // declarative UI. Its execution and authority stay in SYSTEM_SCOPE;
                    // the frontend resolver still checks assignment and frontend grants.
                    continue;
                }
                if execution_scope != workspace_id {
                    bail!("managed execution scope disagrees with workspace assignment");
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
        control_plane::plugin_management::validate_installed_plugin_artifact(
            &installation,
            &local.artifact,
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
            local_artifact: local.artifact,
            manifest: managed,
            artifact_fingerprint: ManagedArtifactFingerprint::from_bytes(&raw),
            authority,
        })
    }
}
