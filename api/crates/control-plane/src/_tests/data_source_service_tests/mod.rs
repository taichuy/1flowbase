use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::data_source_contract::{
    DataSourceCatalogEntry, DataSourceConfigInput, DataSourceCreateRecordInput,
    DataSourceCreateRecordOutput, DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput,
    DataSourceDescribeResourceInput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourcePreviewReadInput,
    DataSourcePreviewReadOutput, DataSourceRecordScopeContext, DataSourceResourceDescriptor,
    DataSourceUpdateRecordInput, DataSourceUpdateRecordOutput,
};
use plugin_framework::provider_contract::PluginFormFieldSchema;
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    data_source::{
        CreateDataSourceInstanceCommand, DataSourceService, DiscoverDataSourceResourcesCommand,
        MapDataSourceResourceToModelCommand, PreviewDataSourceReadCommand,
        RotateDataSourceSecretCommand, UpdateDataSourceDefaultsCommand,
        ValidateDataSourceInstanceCommand,
    },
    ports::{
        AddModelFieldInput, AuthRepository, CreateDataSourceInstanceInput,
        CreateDataSourcePreviewSessionInput, CreateModelDefinitionInput,
        CreatePluginAssignmentInput, CreatePluginTaskInput, CreateScopeDataModelGrantInput,
        DataSourceCrudRuntimePort, DataSourceRepository, DataSourceRuntimePort,
        ModelDefinitionRepository, RotateDataSourceSecretInput, RotateDataSourceSecretOutput,
        UpdateDataSourceDefaultsInput, UpdateDataSourceInstanceConfigInput,
        UpdateDataSourceInstanceStatusInput, UpdateMainSourceDefaultsInput,
        UpdateModelDefinitionInput, UpdateModelDefinitionStatusInput, UpdateModelFieldInput,
        UpdatePluginArtifactSnapshotInput, UpdatePluginDesiredStateInput,
        UpdatePluginRuntimeSnapshotInput, UpdatePluginTaskStatusInput, UpdateProfileInput,
        UpdateScopeDataModelGrantInput, UpsertDataSourceCatalogCacheInput,
        UpsertDataSourceSecretInput, UpsertPluginArtifactInstanceInput,
        UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
    },
};
use domain::{
    ActorContext, AuditLogRecord, AuthenticatorRecord, DataModelScopeKind,
    DataSourceCatalogCacheRecord, DataSourceCatalogRefreshStatus, DataSourceDefaults,
    DataSourceInstanceRecord, DataSourceInstanceStatus, DataSourcePreviewSessionRecord,
    DataSourceSecretRecord, ModelDefinitionRecord, ModelFieldRecord, PermissionDefinition,
    PluginArtifactInstanceRecord, PluginArtifactStatus, PluginAssignmentRecord,
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord,
    PluginPackageCatalogProjectionRecord, PluginRuntimeStatus, PluginTaskRecord,
    PluginVerificationStatus, ScopeContext, ScopeDataModelGrantRecord, UserRecord,
};

fn tenant_id() -> Uuid {
    Uuid::from_u128(0x100)
}

fn workspace_id() -> Uuid {
    Uuid::from_u128(0x200)
}

fn user_id() -> Uuid {
    Uuid::from_u128(0x300)
}

fn installation_id() -> Uuid {
    Uuid::from_u128(0x400)
}

fn actor() -> ActorContext {
    ActorContext::root_in_scope(user_id(), tenant_id(), workspace_id(), "root")
}

fn seeded_installation() -> PluginInstallationRecord {
    PluginInstallationRecord {
        id: installation_id(),
        provider_code: "acme_hubspot_source".to_string(),
        plugin_id: "acme_hubspot_source@0.1.0".to_string(),
        plugin_version: "0.1.0".to_string(),
        contract_version: "1flowbase.data_source/v1".to_string(),
        protocol: "stdio_json".to_string(),
        display_name: "Acme HubSpot Source".to_string(),
        source_kind: "uploaded".to_string(),
        trust_level: "unverified".to_string(),
        verification_status: PluginVerificationStatus::Valid,
        desired_state: PluginDesiredState::ActiveRequested,
        artifact_status: PluginArtifactStatus::Ready,
        runtime_status: PluginRuntimeStatus::Active,
        availability_status: PluginAvailabilityStatus::Available,
        package_path: None,
        installed_path: format!(
            "{}/src/_tests/fixtures/acme_data_source",
            env!("CARGO_MANIFEST_DIR")
        ),
        checksum: None,
        manifest_fingerprint: None,
        signature_status: None,
        signature_algorithm: None,
        signing_key_id: None,
        last_load_error: None,
        metadata_json: json!({}),
        created_by: user_id(),
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[derive(Clone)]
struct InMemoryDataSourceRepository {
    actor: ActorContext,
    console_policies: Arc<RwLock<Vec<domain::RoleConsolePolicy>>>,
    installations: Arc<RwLock<HashMap<Uuid, PluginInstallationRecord>>>,
    artifact_instances: Arc<RwLock<HashMap<(String, Uuid), PluginArtifactInstanceRecord>>>,
    assignments: Arc<RwLock<Vec<PluginAssignmentRecord>>>,
    instances: Arc<RwLock<HashMap<Uuid, DataSourceInstanceRecord>>>,
    secrets: Arc<RwLock<HashMap<Uuid, Value>>>,
    secret_records: Arc<RwLock<HashMap<Uuid, DataSourceSecretRecord>>>,
    caches: Arc<RwLock<HashMap<Uuid, DataSourceCatalogCacheRecord>>>,
    main_source_defaults: Arc<RwLock<HashMap<Uuid, DataSourceDefaults>>>,
    preview_sessions: Arc<RwLock<HashMap<Uuid, DataSourcePreviewSessionRecord>>>,
    models: Arc<RwLock<HashMap<Uuid, ModelDefinitionRecord>>>,
    grants: Arc<RwLock<Vec<ScopeDataModelGrantRecord>>>,
    audit_logs: Arc<RwLock<Vec<AuditLogRecord>>>,
}

impl Default for InMemoryDataSourceRepository {
    fn default() -> Self {
        let actor = actor();
        let installation = seeded_installation();
        let assignment = PluginAssignmentRecord {
            id: Uuid::now_v7(),
            installation_id: installation.id,
            workspace_id: actor.current_workspace_id,
            provider_code: installation.provider_code.clone(),
            assigned_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
        };
        Self {
            actor,
            console_policies: Arc::new(RwLock::new(Vec::new())),
            installations: Arc::new(RwLock::new(HashMap::from([(
                installation.id,
                installation,
            )]))),
            artifact_instances: Arc::new(RwLock::new(HashMap::new())),
            assignments: Arc::new(RwLock::new(vec![assignment])),
            instances: Arc::new(RwLock::new(HashMap::new())),
            secrets: Arc::new(RwLock::new(HashMap::new())),
            secret_records: Arc::new(RwLock::new(HashMap::new())),
            caches: Arc::new(RwLock::new(HashMap::new())),
            main_source_defaults: Arc::new(RwLock::new(HashMap::new())),
            preview_sessions: Arc::new(RwLock::new(HashMap::new())),
            models: Arc::new(RwLock::new(HashMap::new())),
            grants: Arc::new(RwLock::new(Vec::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl InMemoryDataSourceRepository {
    fn with_actor(actor: ActorContext) -> Self {
        Self {
            actor,
            ..Self::default()
        }
    }

    async fn preview_session_count(&self) -> usize {
        self.preview_sessions.read().await.len()
    }

    async fn stored_secret_json(&self, instance_id: Uuid) -> Value {
        self.secrets
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap_or_else(|| json!({}))
    }

    async fn audit_events(&self) -> Vec<AuditLogRecord> {
        self.audit_logs.read().await.clone()
    }

    async fn mapped_models(&self) -> Vec<ModelDefinitionRecord> {
        self.models.read().await.values().cloned().collect()
    }

    async fn cached_catalog(&self, instance_id: Uuid) -> Option<DataSourceCatalogCacheRecord> {
        self.caches.read().await.get(&instance_id).cloned()
    }

    async fn set_console_policies(&self, policies: Vec<domain::RoleConsolePolicy>) {
        *self.console_policies.write().await = policies;
    }
}

#[async_trait]
impl AuthRepository for InMemoryDataSourceRepository {
    async fn find_authenticator(&self, _id: Uuid) -> Result<Option<AuthenticatorRecord>> {
        Ok(None)
    }

    async fn find_user_for_password_login(
        &self,
        _authenticator_id: Uuid,
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
        self.audit_logs.write().await.push(event.clone());
        Ok(())
    }
}

#[async_trait]
impl crate::ports::PluginRepository for InMemoryDataSourceRepository {
    async fn upsert_installation(
        &self,
        _input: &UpsertPluginInstallationInput,
    ) -> Result<PluginInstallationRecord> {
        anyhow::bail!("not implemented")
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
        _input: &UpsertPluginPackageCatalogProjectionInput,
    ) -> Result<PluginPackageCatalogProjectionRecord> {
        anyhow::bail!("not implemented")
    }

    async fn get_plugin_package_catalog_projection(
        &self,
        _installation_id: Uuid,
    ) -> Result<Option<PluginPackageCatalogProjectionRecord>> {
        Ok(None)
    }

    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> Result<Vec<PluginPackageCatalogProjectionRecord>> {
        Ok(Vec::new())
    }

    async fn delete_installation(&self, _installation_id: Uuid) -> Result<()> {
        anyhow::bail!("not implemented")
    }

    async fn list_pending_restart_host_extensions(&self) -> Result<Vec<PluginInstallationRecord>> {
        Ok(Vec::new())
    }

    async fn update_desired_state(
        &self,
        _input: &UpdatePluginDesiredStateInput,
    ) -> Result<PluginInstallationRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_artifact_snapshot(
        &self,
        _input: &UpdatePluginArtifactSnapshotInput,
    ) -> Result<PluginInstallationRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_runtime_snapshot(
        &self,
        _input: &UpdatePluginRuntimeSnapshotInput,
    ) -> Result<PluginInstallationRecord> {
        anyhow::bail!("not implemented")
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
        _input: &CreatePluginAssignmentInput,
    ) -> Result<PluginAssignmentRecord> {
        anyhow::bail!("not implemented")
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

    async fn create_task(&self, _input: &CreatePluginTaskInput) -> Result<PluginTaskRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_task_status(
        &self,
        _input: &UpdatePluginTaskStatusInput,
    ) -> Result<PluginTaskRecord> {
        anyhow::bail!("not implemented")
    }

    async fn get_task(&self, _task_id: Uuid) -> Result<Option<PluginTaskRecord>> {
        Ok(None)
    }

    async fn list_tasks(&self) -> Result<Vec<PluginTaskRecord>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl DataSourceRepository for InMemoryDataSourceRepository {
    async fn list_instances(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: crate::ports::DataSourceInstanceVisibility,
    ) -> Result<Vec<DataSourceInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .values()
            .filter(|instance| match visibility {
                crate::ports::DataSourceInstanceVisibility::Own => {
                    instance.workspace_id == workspace_id && instance.created_by == actor_user_id
                }
                crate::ports::DataSourceInstanceVisibility::ScopeAll => {
                    instance.workspace_id == workspace_id
                }
            })
            .cloned()
            .collect())
    }

    async fn load_role_console_policies_for_user(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.console_policies.read().await.clone())
    }

    async fn create_instance(
        &self,
        input: &CreateDataSourceInstanceInput,
    ) -> Result<DataSourceInstanceRecord> {
        let record = DataSourceInstanceRecord {
            id: input.instance_id,
            workspace_id: input.workspace_id,
            installation_id: input.installation_id,
            source_code: input.source_code.clone(),
            display_name: input.display_name.clone(),
            status: input.status,
            config_json: input.config_json.clone(),
            metadata_json: input.metadata_json.clone(),
            secret_ref: None,
            secret_version: None,
            defaults: input.defaults,
            created_by: input.created_by,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        self.instances
            .write()
            .await
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn update_instance_status(
        &self,
        input: &UpdateDataSourceInstanceStatusInput,
    ) -> Result<DataSourceInstanceRecord> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&input.instance_id)
            .expect("instance should exist for test");
        instance.status = input.status;
        instance.metadata_json = input.metadata_json.clone();
        instance.updated_at = OffsetDateTime::now_utc();
        Ok(instance.clone())
    }

    async fn update_instance_defaults(
        &self,
        input: &UpdateDataSourceDefaultsInput,
    ) -> Result<DataSourceInstanceRecord> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&input.instance_id)
            .expect("instance should exist for test");
        instance.defaults = input.defaults;
        instance.updated_at = OffsetDateTime::now_utc();
        Ok(instance.clone())
    }

    async fn get_main_source_defaults(&self, workspace_id: Uuid) -> Result<DataSourceDefaults> {
        Ok(self
            .main_source_defaults
            .read()
            .await
            .get(&workspace_id)
            .copied()
            .unwrap_or_default())
    }

    async fn update_main_source_defaults(
        &self,
        input: &UpdateMainSourceDefaultsInput,
    ) -> Result<DataSourceDefaults> {
        self.main_source_defaults
            .write()
            .await
            .insert(input.workspace_id, input.defaults);
        Ok(input.defaults)
    }

    async fn update_instance_config(
        &self,
        input: &UpdateDataSourceInstanceConfigInput,
    ) -> Result<DataSourceInstanceRecord> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&input.instance_id)
            .expect("instance should exist for test");
        instance.config_json = input.config_json.clone();
        instance.updated_at = OffsetDateTime::now_utc();
        Ok(instance.clone())
    }

    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<DataSourceInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .get(&instance_id)
            .filter(|instance| instance.workspace_id == workspace_id)
            .cloned())
    }

    async fn get_instance_for_visibility(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        actor_user_id: Uuid,
        visibility: crate::ports::DataSourceInstanceVisibility,
    ) -> Result<Option<DataSourceInstanceRecord>> {
        Ok(self
            .instances
            .read()
            .await
            .get(&instance_id)
            .filter(|instance| {
                instance.workspace_id == workspace_id
                    && (matches!(
                        visibility,
                        crate::ports::DataSourceInstanceVisibility::ScopeAll
                    ) || instance.created_by == actor_user_id)
            })
            .cloned())
    }

    async fn upsert_secret(
        &self,
        input: &UpsertDataSourceSecretInput,
    ) -> Result<DataSourceSecretRecord> {
        let record = DataSourceSecretRecord {
            data_source_instance_id: input.data_source_instance_id,
            secret_ref: input.secret_ref.clone(),
            encrypted_secret_json: input.plaintext_secret_json.clone(),
            secret_version: input.secret_version,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.secrets.write().await.insert(
            input.data_source_instance_id,
            input.plaintext_secret_json.clone(),
        );
        self.secret_records
            .write()
            .await
            .insert(input.data_source_instance_id, record.clone());
        if let Some(instance) = self
            .instances
            .write()
            .await
            .get_mut(&input.data_source_instance_id)
        {
            instance.secret_ref = Some(record.secret_ref.clone());
            instance.secret_version = Some(record.secret_version);
        }
        Ok(record)
    }

    async fn rotate_secret(
        &self,
        input: &RotateDataSourceSecretInput,
    ) -> Result<RotateDataSourceSecretOutput> {
        let mut secret_records = self.secret_records.write().await;
        let secret_version = secret_records
            .get(&input.data_source_instance_id)
            .map(|record| record.secret_version + 1)
            .unwrap_or(1);
        let existing_secret_json = self
            .secrets
            .read()
            .await
            .get(&input.data_source_instance_id)
            .cloned();
        let secret_json = merge_rotated_secret_values(
            existing_secret_json.as_ref(),
            &input.plaintext_secret_json,
        );
        let record = DataSourceSecretRecord {
            data_source_instance_id: input.data_source_instance_id,
            secret_ref: input.secret_ref.clone(),
            encrypted_secret_json: secret_json.clone(),
            secret_version,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.secrets
            .write()
            .await
            .insert(input.data_source_instance_id, secret_json);
        secret_records.insert(input.data_source_instance_id, record.clone());
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&input.data_source_instance_id)
            .expect("instance should exist for test");
        instance.secret_ref = Some(record.secret_ref.clone());
        instance.secret_version = Some(record.secret_version);
        instance.config_json = refresh_test_secret_reference_versions(
            &instance.config_json,
            &record.secret_ref,
            record.secret_version,
        );
        instance.updated_at = OffsetDateTime::now_utc();
        Ok(RotateDataSourceSecretOutput {
            secret: record,
            instance: instance.clone(),
        })
    }

    async fn get_secret_record(&self, instance_id: Uuid) -> Result<Option<DataSourceSecretRecord>> {
        Ok(self.secret_records.read().await.get(&instance_id).cloned())
    }

    async fn get_secret_json(&self, instance_id: Uuid, _master_key: &str) -> Result<Option<Value>> {
        Ok(self.secrets.read().await.get(&instance_id).cloned())
    }

    async fn upsert_catalog_cache(
        &self,
        input: &UpsertDataSourceCatalogCacheInput,
    ) -> Result<DataSourceCatalogCacheRecord> {
        let record = DataSourceCatalogCacheRecord {
            data_source_instance_id: input.data_source_instance_id,
            refresh_status: input.refresh_status,
            catalog_json: input.catalog_json.clone(),
            last_error_message: input.last_error_message.clone(),
            refreshed_at: input.refreshed_at,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.caches
            .write()
            .await
            .insert(record.data_source_instance_id, record.clone());
        Ok(record)
    }

    async fn get_catalog_cache(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<DataSourceCatalogCacheRecord>> {
        let belongs_to_workspace = self
            .instances
            .read()
            .await
            .get(&instance_id)
            .is_some_and(|instance| instance.workspace_id == workspace_id);
        if !belongs_to_workspace {
            return Ok(None);
        }
        Ok(self.caches.read().await.get(&instance_id).cloned())
    }

    async fn create_preview_session(
        &self,
        input: &CreateDataSourcePreviewSessionInput,
    ) -> Result<DataSourcePreviewSessionRecord> {
        let record = DataSourcePreviewSessionRecord {
            id: input.session_id,
            workspace_id: input.workspace_id,
            actor_user_id: input.actor_user_id,
            data_source_instance_id: input.data_source_instance_id,
            config_fingerprint: input.config_fingerprint.clone(),
            preview_json: input.preview_json.clone(),
            expires_at: input.expires_at,
            created_at: OffsetDateTime::now_utc(),
        };
        self.preview_sessions
            .write()
            .await
            .insert(record.id, record.clone());
        Ok(record)
    }
}

#[async_trait]
impl ModelDefinitionRepository for InMemoryDataSourceRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        AuthRepository::load_actor_context_for_user(self, actor_user_id).await
    }

    async fn list_model_definitions(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<ModelDefinitionRecord>> {
        Ok(self.models.read().await.values().cloned().collect())
    }

    async fn get_model_definition(
        &self,
        workspace_id: Uuid,
        model_id: Uuid,
    ) -> Result<Option<ModelDefinitionRecord>> {
        Ok(self
            .models
            .read()
            .await
            .get(&model_id)
            .filter(|model| {
                workspace_id.is_nil()
                    || !matches!(model.scope_kind, DataModelScopeKind::Workspace)
                    || model.scope_id == workspace_id
            })
            .cloned())
    }

    async fn get_data_source_defaults(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
    ) -> Result<DataSourceDefaults> {
        self.instances
            .read()
            .await
            .get(&data_source_instance_id)
            .filter(|instance| instance.workspace_id == workspace_id)
            .map(|instance| instance.defaults)
            .ok_or_else(|| {
                control_plane::errors::ControlPlaneError::NotFound("data_source_instance").into()
            })
    }

    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        let model = ModelDefinitionRecord {
            id: Uuid::now_v7(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_source_instance_id: input.data_source_instance_id,
            source_kind: input.source_kind,
            external_resource_key: input.external_resource_key.clone(),
            external_table_id: input.external_table_id.clone(),
            external_capability_snapshot: input.external_capability_snapshot.clone(),
            code: input.code.clone(),
            title: input.title.clone(),
            physical_table_name: format!("rtm_workspace_{}", input.code),
            acl_namespace: format!("state_model.{}", input.code),
            audit_namespace: format!("audit.state_model.{}", input.code),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: input.status,
            protection: input.protection.clone(),
        };
        self.models.write().await.insert(model.id, model.clone());
        Ok(model)
    }

    async fn update_model_definition(
        &self,
        _input: &UpdateModelDefinitionInput,
    ) -> Result<ModelDefinitionRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_model_definition_status(
        &self,
        _input: &UpdateModelDefinitionStatusInput,
    ) -> Result<ModelDefinitionRecord> {
        anyhow::bail!("not implemented")
    }

    async fn add_model_field(&self, input: &AddModelFieldInput) -> Result<ModelFieldRecord> {
        let mut models = self.models.write().await;
        let model = models
            .get_mut(&input.model_id)
            .expect("model should exist for test");
        let field = ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: input.model_id,
            code: input.code.clone(),
            title: input.title.clone(),
            description: input.description.clone(),
            physical_column_name: input
                .physical_column_name
                .clone()
                .unwrap_or_else(|| format!("col_{}", input.code)),
            external_field_key: input.external_field_key.clone(),
            field_kind: input.field_kind,
            is_system: input.is_system,
            is_writable: input.is_writable,
            is_required: input.is_required,
            api_required: input.api_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_target_model_id: input.relation_target_model_id,
            relation_options: input.relation_options.clone(),
            sort_order: model.fields.len() as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        };
        model.fields.push(field.clone());
        Ok(field)
    }

    async fn update_model_field(&self, _input: &UpdateModelFieldInput) -> Result<ModelFieldRecord> {
        anyhow::bail!("not implemented")
    }

    async fn delete_model_definition(&self, _actor_user_id: Uuid, _model_id: Uuid) -> Result<()> {
        anyhow::bail!("not implemented")
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
        _field_id: Uuid,
    ) -> Result<()> {
        anyhow::bail!("not implemented")
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        _model_id: Uuid,
    ) -> Result<ModelDefinitionRecord> {
        anyhow::bail!("not implemented")
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> Result<ScopeDataModelGrantRecord> {
        let now = OffsetDateTime::now_utc();
        let grant = ScopeDataModelGrantRecord {
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
        self.grants.write().await.push(grant.clone());
        Ok(grant)
    }

    async fn update_scope_data_model_grant(
        &self,
        _input: &UpdateScopeDataModelGrantInput,
    ) -> Result<ScopeDataModelGrantRecord> {
        anyhow::bail!("not implemented")
    }

    async fn get_scope_data_model_grant(
        &self,
        _data_model_id: Uuid,
        _grant_id: Uuid,
    ) -> Result<Option<ScopeDataModelGrantRecord>> {
        Ok(None)
    }

    async fn delete_scope_data_model_grant(
        &self,
        _data_model_id: Uuid,
        _grant_id: Uuid,
    ) -> Result<ScopeDataModelGrantRecord> {
        anyhow::bail!("not implemented")
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: DataModelScopeKind,
        scope_id: Uuid,
    ) -> Result<Vec<ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .read()
            .await
            .iter()
            .filter(|grant| grant.scope_kind == scope_kind && grant.scope_id == scope_id)
            .cloned()
            .collect())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_logs.write().await.push(event.clone());
        Ok(())
    }
}

fn refresh_test_secret_reference_versions(
    value: &Value,
    secret_ref: &str,
    secret_version: i32,
) -> Value {
    match value {
        Value::Object(object) if is_test_secret_reference_marker(value) => {
            let mut updated = object.clone();
            if updated
                .get("secret_ref")
                .and_then(Value::as_str)
                .map(|value| value == secret_ref)
                .unwrap_or(false)
            {
                updated.insert("secret_version".to_string(), json!(secret_version));
            }
            Value::Object(updated)
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| {
                    (
                        key.clone(),
                        refresh_test_secret_reference_versions(child, secret_ref, secret_version),
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    refresh_test_secret_reference_versions(item, secret_ref, secret_version)
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn is_test_secret_reference_marker(value: &Value) -> bool {
    value
        .as_object()
        .map(|object| object.contains_key("secret_ref") && object.contains_key("secret_version"))
        .unwrap_or(false)
}

fn merge_rotated_secret_values(existing: Option<&Value>, incoming: &Value) -> Value {
    let Some(incoming_object) = incoming.as_object() else {
        return incoming.clone();
    };
    let mut merged_object = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in incoming_object {
        if key != "__config_secret_values" {
            merged_object.insert(key.clone(), value.clone());
        }
    }

    let mut marker_values = existing
        .and_then(|value| value.get("__config_secret_values"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(incoming_marker_values) = incoming_object
        .get("__config_secret_values")
        .and_then(Value::as_object)
    {
        for (key, value) in incoming_marker_values {
            marker_values.insert(key.clone(), value.clone());
        }
    }
    if !marker_values.is_empty() {
        merged_object.insert(
            "__config_secret_values".to_string(),
            Value::Object(marker_values),
        );
    }

    Value::Object(merged_object)
}

#[derive(Clone)]
struct StubDataSourceRuntime {
    preview_inputs: Arc<RwLock<Vec<DataSourcePreviewReadInput>>>,
    describe_inputs: Arc<RwLock<Vec<DataSourceDescribeResourceInput>>>,
    echo_secret_output: bool,
}

impl StubDataSourceRuntime {
    fn ready() -> Self {
        Self {
            preview_inputs: Arc::new(RwLock::new(Vec::new())),
            describe_inputs: Arc::new(RwLock::new(Vec::new())),
            echo_secret_output: false,
        }
    }

    fn echoing_secret() -> Self {
        Self {
            preview_inputs: Arc::new(RwLock::new(Vec::new())),
            describe_inputs: Arc::new(RwLock::new(Vec::new())),
            echo_secret_output: true,
        }
    }

    async fn last_preview_input(&self) -> Option<DataSourcePreviewReadInput> {
        self.preview_inputs.read().await.last().cloned()
    }

    async fn last_describe_input(&self) -> Option<DataSourceDescribeResourceInput> {
        self.describe_inputs.read().await.last().cloned()
    }
}

#[async_trait]
impl DataSourceRuntimePort for StubDataSourceRuntime {
    async fn ensure_loaded(&self, _installation: &PluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn validate_config(
        &self,
        _installation: &PluginInstallationRecord,
        _config_json: Value,
        secret_json: Value,
    ) -> Result<Value> {
        if self.echo_secret_output {
            let secret = secret_json["client_secret"].as_str().unwrap_or_default();
            return Ok(json!({
                "ok": true,
                "echoed": secret_json["client_secret"].clone(),
                "authorization": format!("Bearer {secret}"),
                "nested": {
                    "token": secret_json["client_secret"].clone(),
                    "authorization": format!("Token {secret}"),
                }
            }));
        }
        Ok(json!({ "ok": true }))
    }

    async fn test_connection(
        &self,
        _installation: &PluginInstallationRecord,
        _config_json: Value,
        _secret_json: Value,
    ) -> Result<Value> {
        Ok(json!({ "status": "ok" }))
    }

    async fn discover_catalog(
        &self,
        _installation: &PluginInstallationRecord,
        _config_json: Value,
        secret_json: Value,
    ) -> Result<Value> {
        if self.echo_secret_output {
            let secret = secret_json["client_secret"].as_str().unwrap_or_default();
            return Ok(serde_json::to_value(vec![DataSourceCatalogEntry {
                resource_key: "contacts".to_string(),
                display_name: "Contacts".to_string(),
                resource_kind: "object".to_string(),
                capabilities: Default::default(),
                metadata: json!({
                    "authorization": format!("Bearer {secret}"),
                    "nested": {
                        "token": secret_json["client_secret"].clone(),
                    },
                }),
            }])?);
        }
        Ok(serde_json::to_value(vec![DataSourceCatalogEntry {
            resource_key: "contacts".to_string(),
            display_name: "Contacts".to_string(),
            resource_kind: "object".to_string(),
            capabilities: Default::default(),
            metadata: json!({}),
        }])?)
    }

    async fn describe_resource(
        &self,
        _installation: &PluginInstallationRecord,
        input: DataSourceDescribeResourceInput,
    ) -> Result<DataSourceResourceDescriptor> {
        let echoed_secret = input.connection.secret_json["client_secret"].clone();
        self.describe_inputs.write().await.push(input.clone());
        if self.echo_secret_output {
            let secret = echoed_secret.as_str().unwrap_or_default();
            return Ok(DataSourceResourceDescriptor {
                resource_key: format!("{}-{secret}", input.resource_key),
                primary_key: Some("id".to_string()),
                fields: vec![
                    PluginFormFieldSchema {
                        key: "id".to_string(),
                        label: format!("Record {secret} ID"),
                        field_type: "string".to_string(),
                        control: None,
                        group: None,
                        order: Some(0),
                        advanced: None,
                        required: Some(true),
                        send_mode: None,
                        enabled_by_default: None,
                        description: Some(format!("Primary key {secret}")),
                        placeholder: None,
                        default_value: None,
                        min: None,
                        max: None,
                        step: None,
                        precision: None,
                        unit: None,
                        options: vec![],
                        visible_when: vec![],
                        disabled_when: vec![],
                    },
                    PluginFormFieldSchema {
                        key: format!("properties.email.{secret}"),
                        label: format!("Email {secret}"),
                        field_type: "email".to_string(),
                        control: None,
                        group: None,
                        order: Some(1),
                        advanced: None,
                        required: Some(false),
                        send_mode: None,
                        enabled_by_default: None,
                        description: None,
                        placeholder: None,
                        default_value: Some(json!({ "echoed": secret })),
                        min: None,
                        max: None,
                        step: None,
                        precision: None,
                        unit: None,
                        options: vec![],
                        visible_when: vec![],
                        disabled_when: vec![],
                    },
                ],
                supports_preview_read: true,
                supports_import_snapshot: false,
                capabilities: plugin_framework::data_source_contract::DataSourceCrudCapabilities {
                    supports_list: true,
                    supports_get: true,
                    supports_filter: true,
                    supports_scope_filter: true,
                    ..Default::default()
                },
                metadata: json!({
                    "display_name": format!("Contacts {secret}"),
                    "authorization": format!("Bearer {secret}"),
                    "nested": {
                        "token": echoed_secret,
                    },
                }),
            });
        }
        Ok(DataSourceResourceDescriptor {
            resource_key: input.resource_key,
            primary_key: Some("id".to_string()),
            fields: vec![
                PluginFormFieldSchema {
                    key: "id".to_string(),
                    label: "Record ID".to_string(),
                    field_type: "string".to_string(),
                    control: None,
                    group: None,
                    order: Some(0),
                    advanced: None,
                    required: Some(true),
                    send_mode: None,
                    enabled_by_default: None,
                    description: None,
                    placeholder: None,
                    default_value: None,
                    min: None,
                    max: None,
                    step: None,
                    precision: None,
                    unit: None,
                    options: vec![],
                    visible_when: vec![],
                    disabled_when: vec![],
                },
                PluginFormFieldSchema {
                    key: "properties.email".to_string(),
                    label: "Email".to_string(),
                    field_type: "email".to_string(),
                    control: None,
                    group: None,
                    order: Some(1),
                    advanced: None,
                    required: Some(false),
                    send_mode: None,
                    enabled_by_default: None,
                    description: None,
                    placeholder: None,
                    default_value: None,
                    min: None,
                    max: None,
                    step: None,
                    precision: None,
                    unit: None,
                    options: vec![],
                    visible_when: vec![],
                    disabled_when: vec![],
                },
            ],
            supports_preview_read: true,
            supports_import_snapshot: false,
            capabilities: plugin_framework::data_source_contract::DataSourceCrudCapabilities {
                supports_list: true,
                supports_get: true,
                supports_filter: true,
                supports_scope_filter: true,
                ..Default::default()
            },
            metadata: json!({ "display_name": "Contacts" }),
        })
    }

    async fn preview_read(
        &self,
        _installation: &PluginInstallationRecord,
        input: DataSourcePreviewReadInput,
    ) -> Result<DataSourcePreviewReadOutput> {
        let echoed_secret = input.connection.secret_json["client_secret"].clone();
        self.preview_inputs.write().await.push(input);
        if self.echo_secret_output {
            let secret = echoed_secret.as_str().unwrap_or_default();
            return Ok(DataSourcePreviewReadOutput {
                rows: vec![json!({
                    "id": "1",
                    "token": echoed_secret,
                    "authorization": format!("Bearer {secret}"),
                    "nested": { "secret": echoed_secret },
                    "items": [echoed_secret]
                })],
                next_cursor: None,
            });
        }
        Ok(DataSourcePreviewReadOutput {
            rows: vec![json!({ "id": "1", "email": "person@example.com" })],
            next_cursor: None,
        })
    }
}

mod console_policy;
mod crud_runtime;
mod defaults_and_mapping;
mod instance_validation;
mod runtime_redaction;
mod secret_rotation;
