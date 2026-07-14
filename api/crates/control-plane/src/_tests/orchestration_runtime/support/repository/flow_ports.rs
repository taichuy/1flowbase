use super::*;
use crate::ports::{
    CreateFileStorageInput, CreateFileTableRegistrationInput, FileManagementRepository,
    UpdateFileStorageBindingInput,
};
pub(crate) fn test_data_model_definition() -> domain::ModelDefinitionRecord {
    domain::ModelDefinitionRecord {
        id: Uuid::nil(),
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        code: "orders".to_string(),
        title: "Orders".to_string(),
        physical_table_name: "rtm_workspace_test_orders".to_string(),
        acl_namespace: "runtime_model:orders".to_string(),
        audit_namespace: "runtime_model:orders".to_string(),
        fields: vec![
            test_data_model_field("title", domain::ModelFieldKind::String, 0),
            test_data_model_field("status", domain::ModelFieldKind::Enum, 1),
        ],
        availability_status: domain::MetadataAvailabilityStatus::Available,
        status: domain::DataModelStatus::Published,
        protection: domain::DataModelProtection::default(),
    }
}

fn test_data_model_field(
    code: &str,
    field_kind: domain::ModelFieldKind,
    sort_order: i32,
) -> domain::ModelFieldRecord {
    domain::ModelFieldRecord {
        id: Uuid::now_v7(),
        data_model_id: Uuid::nil(),
        code: code.to_string(),
        title: code.to_string(),
        description: None,
        physical_column_name: code.to_string(),
        external_field_key: None,
        field_kind,
        is_system: false,
        is_writable: true,
        is_required: false,
        api_required: false,
        is_unique: false,
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    }
}

#[async_trait]
impl ApplicationRepository for InMemoryOrchestrationRuntimeRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let (actor_is_root, workspace_id) = {
            let inner = self.inner.lock().expect("runtime repo mutex poisoned");
            (inner.actor_is_root, inner.actor_workspace_id)
        };
        if actor_is_root {
            Ok(domain::ActorContext::root_in_scope(
                actor_user_id,
                Uuid::nil(),
                workspace_id,
                "root",
            ))
        } else {
            ApplicationRepository::load_actor_context_for_user(&self.flow, actor_user_id).await
        }
    }

    async fn load_role_console_policies_for_user(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self
            .inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .console_policies
            .clone())
    }

    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        ApplicationRepository::list_applications(
            &self.flow,
            workspace_id,
            actor_user_id,
            visibility,
        )
        .await
    }

    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::create_application(&self.flow, input).await
    }

    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::update_application(&self.flow, input).await
    }

    async fn delete_application(&self, input: &DeleteApplicationInput) -> Result<()> {
        ApplicationRepository::delete_application(&self.flow, input).await
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        ApplicationRepository::get_application(&self.flow, workspace_id, application_id).await
    }

    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> anyhow::Result<Option<domain::ApplicationRecord>> {
        let application = self.get_application(workspace_id, application_id).await?;
        Ok(application.filter(|application| {
            matches!(visibility, ApplicationVisibility::All)
                || application.created_by == actor_user_id
        }))
    }

    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        ApplicationRepository::list_application_tags(
            &self.flow,
            workspace_id,
            actor_user_id,
            visibility,
        )
        .await
    }

    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        ApplicationRepository::create_application_tag(&self.flow, input).await
    }

    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let application =
            ApplicationRepository::get_application(&self.flow, workspace_id, application_id)
                .await?;
        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        Ok(self
            .inner
            .lock()
            .expect("runtime repository mutex poisoned")
            .application_environment_variables
            .get(&application_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn replace_application_environment_variables(
        &self,
        input: &ReplaceApplicationEnvironmentVariablesInput,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let application = ApplicationRepository::get_application(
            &self.flow,
            input.workspace_id,
            input.application_id,
        )
        .await?;
        if application.is_none() {
            return Err(ControlPlaneError::NotFound("application").into());
        }

        let updated_at = OffsetDateTime::now_utc();
        let variables = input
            .variables
            .iter()
            .map(|variable| domain::ApplicationEnvironmentVariable {
                application_id: input.application_id,
                name: variable.name.clone(),
                value_type: variable.value_type.clone(),
                value: variable.value.clone(),
                description: variable.description.clone(),
                updated_at,
            })
            .collect::<Vec<_>>();
        self.inner
            .lock()
            .expect("runtime repository mutex poisoned")
            .application_environment_variables
            .insert(input.application_id, variables.clone());

        Ok(variables)
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        ApplicationRepository::append_audit_log(&self.flow, event).await
    }
}

#[async_trait]
impl ApplicationJsDependencySelectionRepository for InMemoryOrchestrationRuntimeRepository {
    async fn list_application_js_dependency_selections(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationJsDependencySelection>> {
        let mut selections = self
            .inner
            .lock()
            .expect("runtime repository mutex poisoned")
            .application_js_dependency_selections
            .values()
            .filter(|selection| {
                selection.workspace_id == workspace_id && selection.application_id == application_id
            })
            .cloned()
            .collect::<Vec<_>>();
        selections.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then(left.target.cmp(&right.target))
        });
        Ok(selections)
    }

    async fn replace_application_js_dependency_selection(
        &self,
        input: &ReplaceApplicationJsDependencySelectionInput,
    ) -> Result<domain::ApplicationJsDependencySelection> {
        let selection = domain::ApplicationJsDependencySelection {
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            installation_id: input.installation_id,
            provider_code: input.provider_code.clone(),
            plugin_id: input.plugin_id.clone(),
            plugin_version: input.plugin_version.clone(),
            alias: input.alias.clone(),
            package: input.package.clone(),
            version: input.version.clone(),
            target: input.target.clone(),
            artifact_path: input.artifact_path.clone(),
            artifact_hash: input.artifact_hash.clone(),
            integrity: input.integrity.clone(),
            permissions: input.permissions.clone(),
        };
        self.inner
            .lock()
            .expect("runtime repository mutex poisoned")
            .application_js_dependency_selections
            .insert(
                (
                    input.application_id,
                    input.target.clone(),
                    input.alias.clone(),
                ),
                selection.clone(),
            );
        Ok(selection)
    }
}

#[async_trait]
impl FlowRepository for InMemoryOrchestrationRuntimeRepository {
    async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        FlowRepository::get_or_create_editor_state(
            &self.flow,
            workspace_id,
            application_id,
            actor_user_id,
        )
        .await
    }

    async fn save_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: domain::FlowChangeKind,
        summary: &str,
    ) -> Result<domain::FlowEditorState> {
        FlowRepository::save_draft(
            &self.flow,
            workspace_id,
            application_id,
            actor_user_id,
            document,
            change_kind,
            summary,
        )
        .await
    }

    async fn restore_version(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
    ) -> Result<domain::FlowEditorState> {
        FlowRepository::restore_version(
            &self.flow,
            workspace_id,
            application_id,
            actor_user_id,
            version_id,
        )
        .await
    }

    async fn update_version_metadata(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
        summary: Option<String>,
        summary_is_custom: Option<bool>,
        is_user_protected: Option<bool>,
    ) -> Result<domain::FlowEditorState> {
        FlowRepository::update_version_metadata(
            &self.flow,
            workspace_id,
            application_id,
            actor_user_id,
            version_id,
            summary,
            summary_is_custom,
            is_user_protected,
        )
        .await
    }
}

#[async_trait]
impl ModelDefinitionRepository for InMemoryOrchestrationRuntimeRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        ApplicationRepository::load_actor_context_for_user(&self.flow, actor_user_id).await
    }

    async fn list_model_definitions(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        Ok(vec![test_data_model_definition()])
    }

    async fn get_model_definition(
        &self,
        _workspace_id: Uuid,
        model_id: Uuid,
    ) -> Result<Option<domain::ModelDefinitionRecord>> {
        Ok((model_id == Uuid::nil()).then(test_data_model_definition))
    }

    async fn create_model_definition(
        &self,
        _input: &CreateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn update_model_definition(
        &self,
        _input: &UpdateModelDefinitionInput,
    ) -> Result<domain::ModelDefinitionRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn add_model_field(
        &self,
        _input: &crate::ports::AddModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn update_model_field(
        &self,
        _input: &UpdateModelFieldInput,
    ) -> Result<domain::ModelFieldRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn delete_model_definition(&self, _actor_user_id: Uuid, _model_id: Uuid) -> Result<()> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
        _field_id: Uuid,
    ) -> Result<()> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
    ) -> Result<domain::ModelDefinitionRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let now = OffsetDateTime::now_utc();
        let grant = domain::ScopeDataModelGrantRecord {
            id: input.grant_id,
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_model_id: input.data_model_id,
            enabled: input.enabled,
            permission_profile: input.permission_profile,
            created_by: input.created_by,
            created_at: now,
            updated_at: now,
        };
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .scope_data_model_grants
            .push(grant.clone());
        Ok(grant)
    }

    async fn update_scope_data_model_grant(
        &self,
        input: &UpdateScopeDataModelGrantInput,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let grant = inner
            .scope_data_model_grants
            .iter_mut()
            .find(|grant| grant.id == input.grant_id && grant.data_model_id == input.data_model_id)
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        grant.enabled = input.enabled;
        grant.permission_profile = input.permission_profile;
        grant.updated_at = OffsetDateTime::now_utc();
        Ok(grant.clone())
    }

    async fn get_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<Option<domain::ScopeDataModelGrantRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .scope_data_model_grants
            .iter()
            .find(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .cloned())
    }

    async fn delete_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let index = inner
            .scope_data_model_grants
            .iter()
            .position(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        Ok(inner.scope_data_model_grants.remove(index))
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: domain::DataModelScopeKind,
        scope_id: Uuid,
    ) -> Result<Vec<domain::ScopeDataModelGrantRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .scope_data_model_grants
            .iter()
            .filter(|grant| grant.scope_kind == scope_kind && grant.scope_id == scope_id)
            .cloned()
            .collect())
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        ApplicationRepository::append_audit_log(&self.flow, event).await
    }
}

#[async_trait]
impl FileManagementRepository for InMemoryOrchestrationRuntimeRepository {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        ApplicationRepository::load_actor_context_for_user(&self.flow, actor_user_id).await
    }

    async fn find_file_table_by_code(&self, code: &str) -> Result<Option<domain::FileTableRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .file_tables_by_id
            .values()
            .find(|record| record.code == code)
            .cloned())
    }

    async fn get_file_table(&self, file_table_id: Uuid) -> Result<Option<domain::FileTableRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.file_tables_by_id.get(&file_table_id).cloned())
    }

    async fn create_file_storage(
        &self,
        input: &CreateFileStorageInput,
    ) -> Result<domain::FileStorageRecord> {
        let now = OffsetDateTime::now_utc();
        let record = domain::FileStorageRecord {
            id: input.storage_id,
            code: input.code.clone(),
            title: input.title.clone(),
            driver_type: input.driver_type.clone(),
            enabled: input.enabled,
            is_default: input.is_default,
            config_json: input.config_json.clone(),
            rule_json: input.rule_json.clone(),
            health_status: domain::FileStorageHealthStatus::Unknown,
            last_health_error: None,
            created_by: input.actor_user_id,
            updated_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
        };
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .file_storages_by_id
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn create_file_table_registration(
        &self,
        input: &CreateFileTableRegistrationInput,
    ) -> Result<domain::FileTableRecord> {
        let now = OffsetDateTime::now_utc();
        let record = domain::FileTableRecord {
            id: input.file_table_id,
            code: input.code.clone(),
            title: input.title.clone(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            model_definition_id: input.model_definition_id,
            bound_storage_id: input.bound_storage_id,
            is_builtin: input.is_builtin,
            is_default: input.is_default,
            status: "active".to_string(),
            created_by: input.actor_user_id,
            updated_by: input.actor_user_id,
            created_at: now,
            updated_at: now,
        };
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .file_tables_by_id
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn list_file_storages(&self) -> Result<Vec<domain::FileStorageRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.file_storages_by_id.values().cloned().collect())
    }

    async fn get_default_file_storage(&self) -> Result<Option<domain::FileStorageRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .file_storages_by_id
            .values()
            .find(|record| record.is_default)
            .cloned())
    }

    async fn get_file_storage(
        &self,
        storage_id: Uuid,
    ) -> Result<Option<domain::FileStorageRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.file_storages_by_id.get(&storage_id).cloned())
    }

    async fn list_visible_file_tables(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FileTableRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .file_tables_by_id
            .values()
            .filter(|record| {
                matches!(record.scope_kind, domain::FileTableScopeKind::System)
                    || record.scope_id == workspace_id
            })
            .cloned()
            .collect())
    }

    async fn update_file_table_binding(
        &self,
        input: &UpdateFileStorageBindingInput,
    ) -> Result<domain::FileTableRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = inner
            .file_tables_by_id
            .get_mut(&input.file_table_id)
            .ok_or(ControlPlaneError::NotFound("file_table"))?;
        record.bound_storage_id = input.bound_storage_id;
        record.updated_by = input.actor_user_id;
        record.updated_at = OffsetDateTime::now_utc();
        Ok(record.clone())
    }
}

#[async_trait]
impl PluginRepository for InMemoryOrchestrationRuntimeRepository {
    async fn upsert_installation(
        &self,
        _input: &crate::ports::UpsertPluginInstallationInput,
    ) -> Result<domain::PluginInstallationRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn get_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.installations_by_id.get(&installation_id).cloned())
    }

    async fn list_installations(&self) -> Result<Vec<domain::PluginInstallationRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner.installations_by_id.values().cloned().collect())
    }

    async fn upsert_plugin_package_catalog_projection(
        &self,
        _input: &crate::ports::UpsertPluginPackageCatalogProjectionInput,
    ) -> Result<domain::PluginPackageCatalogProjectionRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn get_plugin_package_catalog_projection(
        &self,
        _installation_id: Uuid,
    ) -> Result<Option<domain::PluginPackageCatalogProjectionRecord>> {
        Ok(None)
    }

    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> Result<Vec<domain::PluginPackageCatalogProjectionRecord>> {
        Ok(Vec::new())
    }

    async fn delete_installation(&self, installation_id: Uuid) -> Result<()> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        if inner.installations_by_id.remove(&installation_id).is_some() {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("plugin_installation").into())
        }
    }

    async fn list_pending_restart_host_extensions(
        &self,
    ) -> Result<Vec<domain::PluginInstallationRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .installations_by_id
            .values()
            .filter(|installation| {
                matches!(
                    installation.desired_state,
                    domain::PluginDesiredState::PendingRestart
                )
            })
            .cloned()
            .collect())
    }

    async fn update_desired_state(
        &self,
        input: &crate::ports::UpdatePluginDesiredStateInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let installation = inner
            .installations_by_id
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.desired_state = input.desired_state;
        installation.availability_status = input.availability_status;
        Ok(installation.clone())
    }

    async fn update_artifact_snapshot(
        &self,
        input: &crate::ports::UpdatePluginArtifactSnapshotInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let installation = inner
            .installations_by_id
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.artifact_status = input.artifact_status;
        installation.availability_status = input.availability_status;
        installation.package_path = input.package_path.clone();
        installation.installed_path = input.installed_path.clone();
        installation.checksum = input.checksum.clone();
        installation.manifest_fingerprint = input.manifest_fingerprint.clone();
        Ok(installation.clone())
    }

    async fn update_runtime_snapshot(
        &self,
        input: &crate::ports::UpdatePluginRuntimeSnapshotInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let installation = inner
            .installations_by_id
            .get_mut(&input.installation_id)
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        installation.runtime_status = input.runtime_status;
        installation.availability_status = input.availability_status;
        installation.last_load_error = input.last_load_error.clone();
        Ok(installation.clone())
    }

    async fn upsert_artifact_instance(
        &self,
        input: &crate::ports::UpsertPluginArtifactInstanceInput,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let record = domain::PluginArtifactInstanceRecord {
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
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        inner.artifact_instances_by_key.insert(
            (record.node_id.clone(), record.installation_id),
            record.clone(),
        );
        Ok(record)
    }

    async fn get_artifact_instance(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .artifact_instances_by_key
            .get(&(node_id.to_string(), installation_id))
            .cloned())
    }

    async fn list_artifact_instances(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::PluginArtifactInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .artifact_instances_by_key
            .values()
            .filter(|record| record.node_id == node_id)
            .cloned()
            .collect())
    }

    async fn create_assignment(
        &self,
        _input: &crate::ports::CreatePluginAssignmentInput,
    ) -> Result<domain::PluginAssignmentRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn list_assignments(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::PluginAssignmentRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .assignments_by_workspace
            .get(&workspace_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn create_task(
        &self,
        _input: &crate::ports::CreatePluginTaskInput,
    ) -> Result<domain::PluginTaskRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn update_task_status(
        &self,
        _input: &crate::ports::UpdatePluginTaskStatusInput,
    ) -> Result<domain::PluginTaskRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn get_task(&self, _task_id: Uuid) -> Result<Option<domain::PluginTaskRecord>> {
        Ok(None)
    }

    async fn list_tasks(&self) -> Result<Vec<domain::PluginTaskRecord>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl NodeContributionRepository for InMemoryOrchestrationRuntimeRepository {
    async fn replace_installation_node_contributions(
        &self,
        _input: &crate::ports::ReplaceInstallationNodeContributionsInput,
    ) -> Result<()> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn list_node_contributions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::NodeContributionRegistryEntry>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .node_contributions_by_workspace
            .get(&workspace_id)
            .cloned()
            .unwrap_or_default())
    }
}

#[async_trait]
impl ModelProviderRepository for InMemoryOrchestrationRuntimeRepository {
    async fn create_instance(
        &self,
        _input: &crate::ports::CreateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn update_instance(
        &self,
        _input: &crate::ports::UpdateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .instances_by_id
            .get(&instance_id)
            .filter(|record| record.workspace_id == workspace_id)
            .cloned())
    }

    async fn list_instances(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .instances_by_id
            .values()
            .filter(|record| record.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn list_instances_by_provider_code(
        &self,
        provider_code: &str,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .instances_by_id
            .values()
            .filter(|record| record.provider_code == provider_code)
            .cloned()
            .collect())
    }

    async fn reassign_instances_to_installation(
        &self,
        _input: &crate::ports::ReassignModelProviderInstancesInput,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn upsert_catalog_cache(
        &self,
        _input: &crate::ports::UpsertModelProviderCatalogCacheInput,
    ) -> Result<domain::ModelProviderCatalogCacheRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn get_catalog_cache(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderCatalogCacheRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .caches_by_instance_id
            .get(&provider_instance_id)
            .cloned())
    }

    async fn list_catalog_entries_for_provider_instance(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .catalog_entries_by_instance_id
            .get(&provider_instance_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn upsert_secret(
        &self,
        _input: &crate::ports::UpsertModelProviderSecretInput,
    ) -> Result<domain::ModelProviderSecretRecord> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn upsert_main_instance(
        &self,
        input: &crate::ports::UpsertModelProviderMainInstanceInput,
    ) -> Result<domain::ModelProviderMainInstanceRecord> {
        let now = OffsetDateTime::now_utc();
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = Self::main_instance_key(input.workspace_id, &input.provider_code);
        let existing = inner.main_instances_by_provider.get(&key).cloned();
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
            model_distribution_rules: Vec::new(),
        };
        inner.main_instances_by_provider.insert(key, record.clone());
        Ok(record)
    }

    async fn get_main_instance(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> Result<Option<domain::ModelProviderMainInstanceRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .main_instances_by_provider
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

    async fn get_secret_json(
        &self,
        provider_instance_id: Uuid,
        _master_key: &str,
    ) -> Result<Option<Value>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .secret_json_by_instance_id
            .get(&provider_instance_id)
            .cloned())
    }

    async fn get_secret_record(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderSecretRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .secret_json_by_instance_id
            .get(&provider_instance_id)
            .map(|secret| domain::ModelProviderSecretRecord {
                provider_instance_id,
                encrypted_secret_json: secret.clone(),
                secret_version: 1,
                updated_at: OffsetDateTime::now_utc(),
            }))
    }

    async fn delete_instance(&self, _workspace_id: Uuid, _instance_id: Uuid) -> Result<()> {
        unimplemented!("not needed in orchestration runtime tests")
    }

    async fn count_instance_references(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
    ) -> Result<u64> {
        Ok(0)
    }
}
