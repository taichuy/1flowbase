use super::*;

#[derive(Clone)]
pub(crate) struct MemoryPluginManagementRepository {
    pub(crate) actor: ActorContext,
    installations: Arc<RwLock<HashMap<Uuid, PluginInstallationRecord>>>,
    artifact_instances: Arc<RwLock<HashMap<(String, Uuid), PluginArtifactInstanceRecord>>>,
    catalog_projections: Arc<RwLock<HashMap<Uuid, PluginPackageCatalogProjectionRecord>>>,
    plugin_ids: Arc<RwLock<HashMap<String, Uuid>>>,
    assignments: Arc<RwLock<Vec<PluginAssignmentRecord>>>,
    tasks: Arc<RwLock<HashMap<Uuid, PluginTaskRecord>>>,
    instances: Arc<RwLock<HashMap<Uuid, ModelProviderInstanceRecord>>>,
    caches: Arc<RwLock<HashMap<Uuid, ModelProviderCatalogCacheRecord>>>,
    main_instances: Arc<RwLock<HashMap<(Uuid, String), domain::ModelProviderMainInstanceRecord>>>,
    node_contributions: Arc<RwLock<Vec<domain::NodeContributionRegistryEntry>>>,
    js_dependencies: Arc<RwLock<Vec<domain::JsDependencyRegistryEntry>>>,
    frontend_blocks: Arc<RwLock<Vec<domain::FrontendBlockCatalogEntry>>>,
    host_infrastructure_configs:
        Arc<RwLock<HashMap<(Uuid, String), HostInfrastructureProviderConfigRecord>>>,
    audit_events: Arc<RwLock<Vec<String>>>,
    artifact_snapshot_updates: Arc<RwLock<Vec<Uuid>>>,
    created_task_status_override: Arc<RwLock<Option<PluginTaskStatus>>>,
}

impl MemoryPluginManagementRepository {
    fn main_instance_key(workspace_id: Uuid, provider_code: &str) -> (Uuid, String) {
        (workspace_id, provider_code.to_string())
    }

    pub(crate) fn new(actor: ActorContext) -> Self {
        Self {
            actor,
            installations: Arc::new(RwLock::new(HashMap::new())),
            artifact_instances: Arc::new(RwLock::new(HashMap::new())),
            catalog_projections: Arc::new(RwLock::new(HashMap::new())),
            plugin_ids: Arc::new(RwLock::new(HashMap::new())),
            assignments: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            caches: Arc::new(RwLock::new(HashMap::new())),
            main_instances: Arc::new(RwLock::new(HashMap::new())),
            node_contributions: Arc::new(RwLock::new(Vec::new())),
            js_dependencies: Arc::new(RwLock::new(Vec::new())),
            frontend_blocks: Arc::new(RwLock::new(Vec::new())),
            host_infrastructure_configs: Arc::new(RwLock::new(HashMap::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            artifact_snapshot_updates: Arc::new(RwLock::new(Vec::new())),
            created_task_status_override: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) async fn audit_events(&self) -> Vec<String> {
        self.audit_events.read().await.clone()
    }

    pub(crate) async fn artifact_snapshot_update_count(&self) -> usize {
        self.artifact_snapshot_updates.read().await.len()
    }

    pub(crate) async fn remove_catalog_projection(&self, installation_id: Uuid) {
        self.catalog_projections
            .write()
            .await
            .remove(&installation_id);
    }

    pub(crate) async fn assignment_installation_id(&self, provider_code: &str) -> Uuid {
        self.assignments
            .read()
            .await
            .iter()
            .find(|assignment| assignment.provider_code == provider_code)
            .map(|assignment| assignment.installation_id)
            .unwrap()
    }

    pub(crate) async fn cache_refresh_statuses(&self) -> Vec<String> {
        let mut statuses = self
            .caches
            .read()
            .await
            .values()
            .map(|cache| cache.refresh_status.as_str().to_string())
            .collect::<Vec<_>>();
        statuses.sort();
        statuses
    }

    pub(crate) async fn set_created_task_status_override(&self, status: PluginTaskStatus) {
        *self.created_task_status_override.write().await = Some(status);
    }

    pub(crate) async fn host_infrastructure_config(
        &self,
        installation_id: Uuid,
        provider_code: &str,
    ) -> Option<HostInfrastructureProviderConfigRecord> {
        self.host_infrastructure_configs
            .read()
            .await
            .get(&(installation_id, provider_code.to_string()))
            .cloned()
    }

    pub(crate) async fn seed_instance_with_ready_cache(
        &self,
        installation_id: Uuid,
        provider_code: &str,
        display_name: &str,
    ) -> Uuid {
        let now = OffsetDateTime::now_utc();
        let instance_id = Uuid::now_v7();
        self.instances.write().await.insert(
            instance_id,
            ModelProviderInstanceRecord {
                id: instance_id,
                workspace_id: self.actor.current_workspace_id,
                installation_id,
                provider_code: provider_code.to_string(),
                protocol: "openai_compatible".to_string(),
                display_name: display_name.to_string(),
                status: ModelProviderInstanceStatus::Ready,
                config_json: json!({ "base_url": "https://api.example.com" }),
                configured_models: vec![],
                enabled_model_ids: vec![],
                included_in_main: true,
                created_by: self.actor.user_id,
                updated_by: self.actor.user_id,
                created_at: now,
                updated_at: now,
            },
        );
        self.caches.write().await.insert(
            instance_id,
            ModelProviderCatalogCacheRecord {
                provider_instance_id: instance_id,
                model_discovery_mode: ModelProviderDiscoveryMode::Hybrid,
                refresh_status: ModelProviderCatalogRefreshStatus::Ready,
                source: ModelProviderCatalogSource::Hybrid,
                models_json: json!([{ "model_id": "fixture_chat" }]),
                last_error_message: None,
                refreshed_at: Some(now),
                updated_at: now,
            },
        );
        instance_id
    }
}

#[async_trait]
impl AuthRepository for MemoryPluginManagementRepository {
    async fn find_authenticator(&self, _name: &str) -> Result<Option<AuthenticatorRecord>> {
        Ok(None)
    }

    async fn find_user_for_password_login(
        &self,
        _authenticator_name: &str,
        _identifier: &str,
    ) -> Result<Option<UserRecord>> {
        Ok(None)
    }

    async fn find_user_by_id(&self, _user_id: Uuid) -> Result<Option<UserRecord>> {
        Ok(None)
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<ScopeContext> {
        Ok(ScopeContext {
            tenant_id: self.actor.tenant_id,
            workspace_id: self.actor.current_workspace_id,
        })
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        self.load_actor_context(
            actor_user_id,
            self.actor.tenant_id,
            self.actor.current_workspace_id,
            None,
        )
        .await
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        _display_role: Option<&str>,
    ) -> Result<ActorContext> {
        let mut actor = self.actor.clone();
        actor.user_id = user_id;
        actor.tenant_id = tenant_id;
        actor.current_workspace_id = workspace_id;
        Ok(actor)
    }

    async fn update_password_hash(
        &self,
        _user_id: Uuid,
        _password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        Ok(1)
    }

    async fn update_profile(&self, _input: &UpdateProfileInput) -> Result<UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_user_meta(
        &self,
        _input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn bump_session_version(&self, _user_id: Uuid, _actor_id: Uuid) -> Result<i64> {
        Ok(1)
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl PluginRepository for MemoryPluginManagementRepository {
    async fn upsert_installation(
        &self,
        input: &UpsertPluginInstallationInput,
    ) -> Result<PluginInstallationRecord> {
        let now = OffsetDateTime::now_utc();
        let existing_id = self.plugin_ids.read().await.get(&input.plugin_id).copied();
        let id = existing_id.unwrap_or(input.installation_id);
        let mut installations = self.installations.write().await;
        let created_at = installations
            .get(&id)
            .map(|item| item.created_at)
            .unwrap_or(now);
        let record = PluginInstallationRecord {
            id,
            provider_code: input.provider_code.clone(),
            plugin_id: input.plugin_id.clone(),
            plugin_version: input.plugin_version.clone(),
            contract_version: input.contract_version.clone(),
            protocol: input.protocol.clone(),
            display_name: input.display_name.clone(),
            source_kind: input.source_kind.clone(),
            trust_level: input.trust_level.clone(),
            verification_status: input.verification_status,
            desired_state: input.desired_state,
            artifact_status: input.artifact_status,
            runtime_status: input.runtime_status,
            availability_status: input.availability_status,
            package_path: input.package_path.clone(),
            installed_path: input.installed_path.clone(),
            checksum: input.checksum.clone(),
            manifest_fingerprint: input.manifest_fingerprint.clone(),
            signature_status: input.signature_status.clone(),
            signature_algorithm: input.signature_algorithm.clone(),
            signing_key_id: input.signing_key_id.clone(),
            last_load_error: input.last_load_error.clone(),
            metadata_json: input.metadata_json.clone(),
            created_by: input.actor_user_id,
            created_at,
            updated_at: now,
        };
        installations.insert(id, record.clone());
        self.plugin_ids
            .write()
            .await
            .insert(input.plugin_id.clone(), id);
        Ok(record)
    }

    async fn get_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<PluginInstallationRecord>> {
        Ok(self
            .installations
            .read()
            .await
            .get(&installation_id)
            .cloned())
    }

    async fn list_installations(&self) -> Result<Vec<PluginInstallationRecord>> {
        Ok(self.installations.read().await.values().cloned().collect())
    }

    async fn upsert_plugin_package_catalog_projection(
        &self,
        input: &UpsertPluginPackageCatalogProjectionInput,
    ) -> Result<PluginPackageCatalogProjectionRecord> {
        let record = PluginPackageCatalogProjectionRecord {
            installation_id: input.installation_id,
            package_code: input.package_code.clone(),
            package_version: input.package_version.clone(),
            catalog_snapshot_json: input.catalog_snapshot_json.clone(),
            projection_status: input.projection_status,
            last_error_message: input.last_error_message.clone(),
            refreshed_at: input.refreshed_at,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.catalog_projections
            .write()
            .await
            .insert(input.installation_id, record.clone());
        Ok(record)
    }

    async fn get_plugin_package_catalog_projection(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<PluginPackageCatalogProjectionRecord>> {
        Ok(self
            .catalog_projections
            .read()
            .await
            .get(&installation_id)
            .cloned())
    }

    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> Result<Vec<PluginPackageCatalogProjectionRecord>> {
        Ok(self
            .catalog_projections
            .read()
            .await
            .values()
            .cloned()
            .collect())
    }

    async fn delete_installation(&self, installation_id: Uuid) -> Result<()> {
        let removed = self.installations.write().await.remove(&installation_id);
        let Some(_) = removed else {
            return Err(ControlPlaneError::NotFound("plugin_installation").into());
        };

        self.plugin_ids
            .write()
            .await
            .retain(|_, id| *id != installation_id);
        self.catalog_projections
            .write()
            .await
            .remove(&installation_id);
        self.assignments
            .write()
            .await
            .retain(|assignment| assignment.installation_id != installation_id);
        self.node_contributions
            .write()
            .await
            .retain(|entry| entry.installation_id != installation_id);
        self.js_dependencies
            .write()
            .await
            .retain(|entry| entry.installation_id != installation_id);
        self.frontend_blocks
            .write()
            .await
            .retain(|entry| entry.installation_id != installation_id);
        Ok(())
    }

    async fn list_pending_restart_host_extensions(&self) -> Result<Vec<PluginInstallationRecord>> {
        Ok(self
            .installations
            .read()
            .await
            .values()
            .filter(|installation| {
                matches!(
                    installation.desired_state,
                    PluginDesiredState::PendingRestart
                )
            })
            .cloned()
            .collect())
    }

    async fn update_desired_state(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> Result<PluginInstallationRecord> {
        let mut installations = self.installations.write().await;
        let installation = installations
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.desired_state = input.desired_state;
        installation.availability_status = input.availability_status;
        installation.updated_at = OffsetDateTime::now_utc();
        Ok(installation.clone())
    }

    async fn update_artifact_snapshot(
        &self,
        input: &UpdatePluginArtifactSnapshotInput,
    ) -> Result<PluginInstallationRecord> {
        self.artifact_snapshot_updates
            .write()
            .await
            .push(input.installation_id);
        let mut installations = self.installations.write().await;
        let installation = installations
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.artifact_status = input.artifact_status;
        installation.availability_status = input.availability_status;
        installation.package_path = input.package_path.clone();
        installation.installed_path = input.installed_path.clone();
        installation.checksum = input.checksum.clone();
        installation.manifest_fingerprint = input.manifest_fingerprint.clone();
        installation.updated_at = OffsetDateTime::now_utc();
        Ok(installation.clone())
    }

    async fn update_runtime_snapshot(
        &self,
        input: &UpdatePluginRuntimeSnapshotInput,
    ) -> Result<PluginInstallationRecord> {
        let mut installations = self.installations.write().await;
        let installation = installations
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.runtime_status = input.runtime_status;
        installation.availability_status = input.availability_status;
        installation.last_load_error = input.last_load_error.clone();
        installation.updated_at = OffsetDateTime::now_utc();
        Ok(installation.clone())
    }

    async fn upsert_artifact_instance(
        &self,
        input: &UpsertPluginArtifactInstanceInput,
    ) -> Result<PluginArtifactInstanceRecord> {
        let record = PluginArtifactInstanceRecord {
            node_id: input.node_id.clone(),
            installation_id: input.installation_id,
            local_version: input.local_version.clone(),
            local_checksum: input.local_checksum.clone(),
            installed_path: input.installed_path.clone(),
            artifact_status: input.artifact_status,
            runtime_status: input.runtime_status,
            checked_at: input.checked_at,
            last_error: input.last_error.clone(),
        };
        self.artifact_instances.write().await.insert(
            (record.node_id.clone(), record.installation_id),
            record.clone(),
        );
        Ok(record)
    }

    async fn get_artifact_instance(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<PluginArtifactInstanceRecord>> {
        Ok(self
            .artifact_instances
            .read()
            .await
            .get(&(node_id.to_string(), installation_id))
            .cloned())
    }

    async fn list_artifact_instances(
        &self,
        node_id: &str,
    ) -> Result<Vec<PluginArtifactInstanceRecord>> {
        Ok(self
            .artifact_instances
            .read()
            .await
            .values()
            .filter(|record| record.node_id == node_id)
            .cloned()
            .collect())
    }

    async fn create_assignment(
        &self,
        input: &CreatePluginAssignmentInput,
    ) -> Result<PluginAssignmentRecord> {
        let mut assignments = self.assignments.write().await;
        if let Some(existing) = assignments.iter_mut().find(|assignment| {
            assignment.workspace_id == input.workspace_id
                && assignment.provider_code == input.provider_code
        }) {
            existing.installation_id = input.installation_id;
            existing.assigned_by = input.actor_user_id;
            return Ok(existing.clone());
        }

        let record = PluginAssignmentRecord {
            id: Uuid::now_v7(),
            installation_id: input.installation_id,
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            assigned_by: input.actor_user_id,
            created_at: OffsetDateTime::now_utc(),
        };
        assignments.push(record.clone());
        Ok(record)
    }

    async fn list_assignments(&self, workspace_id: Uuid) -> Result<Vec<PluginAssignmentRecord>> {
        Ok(self
            .assignments
            .read()
            .await
            .iter()
            .filter(|assignment| assignment.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn create_task(&self, input: &CreatePluginTaskInput) -> Result<PluginTaskRecord> {
        let now = OffsetDateTime::now_utc();
        let status_override = *self.created_task_status_override.read().await;
        let record = PluginTaskRecord {
            id: input.task_id,
            installation_id: input.installation_id,
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            task_kind: input.task_kind,
            status: status_override.unwrap_or(input.status),
            status_message: input.status_message.clone(),
            detail_json: input.detail_json.clone(),
            created_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
            finished_at: None,
        };
        self.tasks.write().await.insert(record.id, record.clone());
        Ok(record)
    }

    async fn update_task_status(
        &self,
        input: &UpdatePluginTaskStatusInput,
    ) -> Result<PluginTaskRecord> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(&input.task_id)
            .ok_or(ControlPlaneError::NotFound("plugin_task"))?;
        task.status = input.status;
        task.status_message = input.status_message.clone();
        task.detail_json = input.detail_json.clone();
        task.updated_at = OffsetDateTime::now_utc();
        task.finished_at = input.status.is_terminal().then_some(task.updated_at);
        Ok(task.clone())
    }

    async fn get_task(&self, task_id: Uuid) -> Result<Option<PluginTaskRecord>> {
        Ok(self.tasks.read().await.get(&task_id).cloned())
    }

    async fn list_tasks(&self) -> Result<Vec<PluginTaskRecord>> {
        Ok(self.tasks.read().await.values().cloned().collect())
    }
}

#[async_trait]
impl HostInfrastructureConfigRepository for MemoryPluginManagementRepository {
    async fn upsert_host_infrastructure_provider_config(
        &self,
        input: &UpsertHostInfrastructureProviderConfigInput,
    ) -> Result<HostInfrastructureProviderConfigRecord> {
        let now = OffsetDateTime::now_utc();
        let key = (input.installation_id, input.provider_code.clone());
        let created_at = self
            .host_infrastructure_configs
            .read()
            .await
            .get(&key)
            .map(|record| record.created_at)
            .unwrap_or(now);
        let record = HostInfrastructureProviderConfigRecord {
            id: Uuid::now_v7(),
            installation_id: input.installation_id,
            extension_id: input.extension_id.clone(),
            provider_code: input.provider_code.clone(),
            config_ref: input.config_ref.clone(),
            enabled_contracts: input.enabled_contracts.clone(),
            config_json: input.config_json.clone(),
            status: input.status,
            updated_by: input.actor_user_id,
            created_at,
            updated_at: now,
        };
        self.host_infrastructure_configs
            .write()
            .await
            .insert(key, record.clone());
        Ok(record)
    }

    async fn list_host_infrastructure_provider_configs(
        &self,
    ) -> Result<Vec<HostInfrastructureProviderConfigRecord>> {
        Ok(self
            .host_infrastructure_configs
            .read()
            .await
            .values()
            .cloned()
            .collect())
    }
}

#[async_trait]
impl NodeContributionRepository for MemoryPluginManagementRepository {
    async fn replace_installation_node_contributions(
        &self,
        input: &ReplaceInstallationNodeContributionsInput,
    ) -> Result<()> {
        let mut rows = self.node_contributions.write().await;
        rows.retain(|entry| entry.installation_id != input.installation_id);
        rows.extend(
            input
                .entries
                .iter()
                .map(|entry| domain::NodeContributionRegistryEntry {
                    installation_id: input.installation_id,
                    provider_code: input.provider_code.clone(),
                    plugin_unique_identifier: entry.plugin_unique_identifier.clone(),
                    package_id: entry.package_id.clone(),
                    plugin_id: input.plugin_id.clone(),
                    plugin_version: input.plugin_version.clone(),
                    contribution_code: entry.contribution_code.clone(),
                    node_shell: entry.node_shell.clone(),
                    category: entry.category.clone(),
                    title: entry.title.clone(),
                    description: entry.description.clone(),
                    icon: entry.icon.clone(),
                    schema_ui: entry.schema_ui.clone(),
                    schema_version: entry.schema_version.clone(),
                    output_schema: entry.output_schema.clone(),
                    contribution_checksum: entry.contribution_checksum.clone(),
                    compiled_contribution_hash: entry.compiled_contribution_hash.clone(),
                    output_schema_snapshot: entry.output_schema_snapshot.clone(),
                    side_effect_policy: entry.side_effect_policy.clone(),
                    infra_contracts: entry.infra_contracts.clone(),
                    required_auth: entry.required_auth.clone(),
                    visibility: entry.visibility.clone(),
                    experimental: entry.experimental,
                    dependency_installation_kind: entry.dependency_installation_kind.clone(),
                    dependency_plugin_version_range: entry.dependency_plugin_version_range.clone(),
                    dependency_status: NodeContributionDependencyStatus::MissingPlugin,
                }),
        );
        Ok(())
    }

    async fn list_node_contributions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::NodeContributionRegistryEntry>> {
        let rows = self.node_contributions.read().await.clone();
        let assignments = self.assignments.read().await.clone();
        let installations = self.installations.read().await.clone();

        Ok(rows
            .into_iter()
            .map(|mut entry| {
                let assigned = assignments.iter().any(|assignment| {
                    assignment.workspace_id == workspace_id
                        && assignment.installation_id == entry.installation_id
                });
                let pinned_installation = installations.get(&entry.installation_id);
                entry.dependency_status = match (assigned, pinned_installation) {
                    (false, _) | (_, None) => NodeContributionDependencyStatus::MissingPlugin,
                    (true, Some(installation))
                        if matches!(installation.desired_state, PluginDesiredState::Disabled) =>
                    {
                        NodeContributionDependencyStatus::DisabledPlugin
                    }
                    (true, Some(_)) => NodeContributionDependencyStatus::Ready,
                };
                entry
            })
            .collect())
    }
}

#[async_trait]
impl JsDependencyRepository for MemoryPluginManagementRepository {
    async fn replace_installation_js_dependencies(
        &self,
        input: &ReplaceInstallationJsDependenciesInput,
    ) -> Result<()> {
        let mut rows = self.js_dependencies.write().await;
        rows.retain(|entry| entry.installation_id != input.installation_id);
        rows.extend(
            input
                .entries
                .iter()
                .map(|entry| domain::JsDependencyRegistryEntry {
                    installation_id: input.installation_id,
                    provider_code: input.provider_code.clone(),
                    plugin_id: input.plugin_id.clone(),
                    plugin_version: input.plugin_version.clone(),
                    alias: entry.alias.clone(),
                    package: entry.package.clone(),
                    version: entry.version.clone(),
                    target: entry.target.clone(),
                    artifact_path: entry.artifact_path.clone(),
                    integrity: entry.integrity.clone(),
                    permissions: entry.permissions.clone(),
                }),
        );
        Ok(())
    }

    async fn list_workspace_js_dependencies(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::JsDependencyRegistryEntry>> {
        let rows = self.js_dependencies.read().await.clone();
        let assignments = self.assignments.read().await.clone();

        Ok(rows
            .into_iter()
            .filter(|entry| {
                assignments.iter().any(|assignment| {
                    assignment.workspace_id == workspace_id
                        && assignment.installation_id == entry.installation_id
                })
            })
            .collect())
    }
}

#[async_trait]
impl FrontendBlockCatalogRepository for MemoryPluginManagementRepository {
    async fn replace_installation_frontend_blocks(
        &self,
        input: &ReplaceInstallationFrontendBlocksInput,
    ) -> Result<()> {
        let mut rows = self.frontend_blocks.write().await;
        rows.retain(|entry| entry.installation_id != input.installation_id);
        rows.extend(
            input
                .entries
                .iter()
                .map(|entry| domain::FrontendBlockCatalogEntry {
                    installation_id: input.installation_id,
                    provider_code: input.provider_code.clone(),
                    plugin_id: input.plugin_id.clone(),
                    plugin_version: input.plugin_version.clone(),
                    contribution_code: entry.contribution_code.clone(),
                    title: entry.title.clone(),
                    runtime: entry.runtime.clone(),
                    entry: entry.entry.clone(),
                    context_contract: entry.context_contract.clone(),
                    permissions: entry.permissions.clone(),
                    ui_capabilities: entry.ui_capabilities.clone(),
                }),
        );
        Ok(())
    }

    async fn list_workspace_frontend_blocks(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        let rows = self.frontend_blocks.read().await.clone();
        let assignments = self.assignments.read().await.clone();

        Ok(rows
            .into_iter()
            .filter(|entry| {
                assignments.iter().any(|assignment| {
                    assignment.workspace_id == workspace_id
                        && assignment.installation_id == entry.installation_id
                })
            })
            .collect())
    }
}

#[async_trait]
impl ModelProviderRepository for MemoryPluginManagementRepository {
    async fn create_instance(
        &self,
        input: &CreateModelProviderInstanceInput,
    ) -> Result<ModelProviderInstanceRecord> {
        let now = OffsetDateTime::now_utc();
        let included_in_main = match input.included_in_main {
            Some(value) => value,
            None => self
                .main_instances
                .read()
                .await
                .get(&Self::main_instance_key(
                    input.workspace_id,
                    &input.provider_code,
                ))
                .map(|record| record.auto_include_new_instances)
                .unwrap_or(domain::DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES),
        };
        let record = ModelProviderInstanceRecord {
            id: input.instance_id,
            workspace_id: input.workspace_id,
            installation_id: input.installation_id,
            provider_code: input.provider_code.clone(),
            protocol: input.protocol.clone(),
            display_name: input.display_name.clone(),
            status: input.status,
            config_json: input.config_json.clone(),
            configured_models: input.configured_models.clone(),
            enabled_model_ids: input.enabled_model_ids.clone(),
            included_in_main,
            created_by: input.created_by,
            updated_by: input.created_by,
            created_at: now,
            updated_at: now,
        };
        self.instances
            .write()
            .await
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn update_instance(
        &self,
        input: &UpdateModelProviderInstanceInput,
    ) -> Result<ModelProviderInstanceRecord> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&input.instance_id)
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        instance.display_name = input.display_name.clone();
        instance.status = input.status;
        instance.config_json = input.config_json.clone();
        instance.configured_models = input.configured_models.clone();
        instance.enabled_model_ids = input.enabled_model_ids.clone();
        instance.included_in_main = input.included_in_main;
        instance.updated_by = input.updated_by;
        instance.updated_at = OffsetDateTime::now_utc();
        Ok(instance.clone())
    }

    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<ModelProviderInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .get(&instance_id)
            .filter(|instance| instance.workspace_id == workspace_id)
            .cloned())
    }

    async fn list_instances(&self, workspace_id: Uuid) -> Result<Vec<ModelProviderInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .values()
            .filter(|instance| instance.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn list_instances_by_provider_code(
        &self,
        provider_code: &str,
    ) -> Result<Vec<ModelProviderInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .values()
            .filter(|instance| instance.provider_code == provider_code)
            .cloned()
            .collect())
    }

    async fn reassign_instances_to_installation(
        &self,
        input: &ReassignModelProviderInstancesInput,
    ) -> Result<Vec<ModelProviderInstanceRecord>> {
        let mut instances = self.instances.write().await;
        let mut migrated = Vec::new();
        for instance in instances.values_mut() {
            if instance.workspace_id == input.workspace_id
                && instance.provider_code == input.provider_code
            {
                instance.installation_id = input.target_installation_id;
                instance.protocol = input.target_protocol.clone();
                instance.updated_by = input.updated_by;
                instance.updated_at = OffsetDateTime::now_utc();
                migrated.push(instance.clone());
            }
        }
        Ok(migrated)
    }

    async fn upsert_catalog_cache(
        &self,
        input: &UpsertModelProviderCatalogCacheInput,
    ) -> Result<ModelProviderCatalogCacheRecord> {
        let record = ModelProviderCatalogCacheRecord {
            provider_instance_id: input.provider_instance_id,
            model_discovery_mode: input.model_discovery_mode,
            refresh_status: input.refresh_status,
            source: input.source,
            models_json: input.models_json.clone(),
            last_error_message: input.last_error_message.clone(),
            refreshed_at: input.refreshed_at,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.caches
            .write()
            .await
            .insert(record.provider_instance_id, record.clone());
        Ok(record)
    }

    async fn get_catalog_cache(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<ModelProviderCatalogCacheRecord>> {
        Ok(self.caches.read().await.get(&provider_instance_id).cloned())
    }

    async fn upsert_main_instance(
        &self,
        input: &crate::ports::UpsertModelProviderMainInstanceInput,
    ) -> Result<domain::ModelProviderMainInstanceRecord> {
        let now = OffsetDateTime::now_utc();
        let mut main_instances = self.main_instances.write().await;
        let key = Self::main_instance_key(input.workspace_id, &input.provider_code);
        let existing = main_instances.get(&key).cloned();
        let record = domain::ModelProviderMainInstanceRecord {
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            auto_include_new_instances: input.auto_include_new_instances,
            created_by: existing
                .as_ref()
                .map(|record| record.created_by)
                .unwrap_or(input.updated_by),
            updated_by: input.updated_by,
            created_at: existing
                .as_ref()
                .map(|record| record.created_at)
                .unwrap_or(now),
            updated_at: now,
        };
        main_instances.insert(key, record.clone());
        Ok(record)
    }

    async fn get_main_instance(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> Result<Option<domain::ModelProviderMainInstanceRecord>> {
        Ok(self
            .main_instances
            .read()
            .await
            .get(&Self::main_instance_key(workspace_id, provider_code))
            .cloned())
    }

    async fn create_preview_session(
        &self,
        input: &crate::ports::CreateModelProviderPreviewSessionInput,
    ) -> Result<domain::ModelProviderPreviewSessionRecord> {
        Ok(domain::ModelProviderPreviewSessionRecord {
            id: input.session_id,
            workspace_id: input.workspace_id,
            actor_user_id: input.actor_user_id,
            installation_id: input.installation_id,
            instance_id: input.instance_id,
            config_fingerprint: input.config_fingerprint.clone(),
            models_json: input.models_json.clone(),
            expires_at: input.expires_at,
            created_at: OffsetDateTime::now_utc(),
        })
    }

    async fn get_preview_session(
        &self,
        _workspace_id: Uuid,
        _session_id: Uuid,
    ) -> Result<Option<domain::ModelProviderPreviewSessionRecord>> {
        Ok(None)
    }

    async fn delete_preview_session(&self, _workspace_id: Uuid, _session_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn upsert_secret(
        &self,
        _input: &UpsertModelProviderSecretInput,
    ) -> Result<ModelProviderSecretRecord> {
        unimplemented!("not needed in plugin management tests")
    }

    async fn get_secret_json(
        &self,
        _provider_instance_id: Uuid,
        _master_key: &str,
    ) -> Result<Option<Value>> {
        Ok(None)
    }

    async fn get_secret_record(
        &self,
        _provider_instance_id: Uuid,
    ) -> Result<Option<ModelProviderSecretRecord>> {
        Ok(None)
    }

    async fn delete_instance(&self, workspace_id: Uuid, instance_id: Uuid) -> Result<()> {
        let mut instances = self.instances.write().await;
        let Some(instance) = instances.get(&instance_id).cloned() else {
            return Err(ControlPlaneError::NotFound("model_provider_instance").into());
        };
        if instance.workspace_id != workspace_id {
            return Err(ControlPlaneError::NotFound("model_provider_instance").into());
        }

        instances.remove(&instance_id);
        self.caches.write().await.remove(&instance_id);
        Ok(())
    }

    async fn count_instance_references(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
    ) -> Result<u64> {
        Ok(0)
    }
}
