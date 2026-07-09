use std::{collections::HashMap, fs, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    i18n::RequestedLocales,
    model_provider::{
        CreateModelProviderInstanceCommand, DeleteModelProviderInstanceCommand,
        ModelProviderConfiguredModelInput, ModelProviderService, PreviewModelProviderModelsCommand,
        UpdateModelProviderInstanceCommand, UpdateModelProviderMainInstanceCommand,
    },
    ports::{
        AuthRepository, CreateModelProviderInstanceInput, CreateModelProviderPreviewSessionInput,
        CreatePluginAssignmentInput, CreatePluginTaskInput, ModelProviderRepository,
        PluginRepository, ProviderRuntimeInvocationOutput, ProviderRuntimePort,
        ReassignModelProviderInstancesInput, UpdateModelProviderInstanceInput,
        UpdatePluginArtifactSnapshotInput, UpdatePluginDesiredStateInput,
        UpdatePluginRuntimeSnapshotInput, UpdatePluginTaskStatusInput, UpdateProfileInput,
        UpsertModelProviderCatalogCacheInput, UpsertModelProviderMainInstanceInput,
        UpsertModelProviderSecretInput, UpsertPluginArtifactInstanceInput,
        UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
    },
};
use domain::{
    ActorContext, AuditLogRecord, AuthenticatorRecord, ModelProviderCatalogCacheRecord,
    ModelProviderCatalogRefreshStatus, ModelProviderInstanceRecord, ModelProviderInstanceStatus,
    ModelProviderMainInstanceRecord, ModelProviderPreviewSessionRecord, ModelProviderSecretRecord,
    PermissionDefinition, PluginArtifactInstanceRecord, PluginArtifactStatus,
    PluginAssignmentRecord, PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord,
    PluginPackageCatalogProjectionRecord, PluginPackageCatalogProjectionStatus,
    PluginRuntimeStatus, PluginTaskRecord, PluginVerificationStatus, ScopeContext, UserRecord,
};
use plugin_framework::provider_contract::{
    ProviderInvocationInput, ProviderInvocationResult, ProviderModelDescriptor, ProviderModelSource,
};
use time::OffsetDateTime;

use super::plugin_management::support::{actor_with_permissions, create_provider_fixture};

#[derive(Clone)]
struct MemoryModelProviderRepository {
    actor: ActorContext,
    installations: Arc<RwLock<HashMap<Uuid, PluginInstallationRecord>>>,
    artifact_instances: Arc<RwLock<HashMap<(String, Uuid), PluginArtifactInstanceRecord>>>,
    catalog_projections: Arc<RwLock<HashMap<Uuid, PluginPackageCatalogProjectionRecord>>>,
    assignments: Arc<RwLock<Vec<PluginAssignmentRecord>>>,
    tasks: Arc<RwLock<HashMap<Uuid, PluginTaskRecord>>>,
    instances: Arc<RwLock<HashMap<Uuid, ModelProviderInstanceRecord>>>,
    caches: Arc<RwLock<HashMap<Uuid, ModelProviderCatalogCacheRecord>>>,
    preview_sessions: Arc<RwLock<HashMap<Uuid, ModelProviderPreviewSessionRecord>>>,
    secrets: Arc<RwLock<HashMap<Uuid, (ModelProviderSecretRecord, Value)>>>,
    main_instances: Arc<RwLock<HashMap<(Uuid, String), ModelProviderMainInstanceRecord>>>,
    references: Arc<RwLock<HashMap<Uuid, u64>>>,
    audit_events: Arc<RwLock<Vec<String>>>,
    artifact_snapshot_updates: Arc<RwLock<Vec<Uuid>>>,
}

impl MemoryModelProviderRepository {
    fn main_instance_key(workspace_id: Uuid, provider_code: &str) -> (Uuid, String) {
        (workspace_id, provider_code.to_string())
    }

    fn new(actor: ActorContext) -> Self {
        Self {
            actor,
            installations: Arc::new(RwLock::new(HashMap::new())),
            artifact_instances: Arc::new(RwLock::new(HashMap::new())),
            catalog_projections: Arc::new(RwLock::new(HashMap::new())),
            assignments: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            caches: Arc::new(RwLock::new(HashMap::new())),
            preview_sessions: Arc::new(RwLock::new(HashMap::new())),
            secrets: Arc::new(RwLock::new(HashMap::new())),
            main_instances: Arc::new(RwLock::new(HashMap::new())),
            references: Arc::new(RwLock::new(HashMap::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            artifact_snapshot_updates: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn seed_installation(
        &self,
        install_path: &str,
        desired_state: PluginDesiredState,
        assigned: bool,
    ) -> Uuid {
        let installation_id = Uuid::now_v7();
        let installation = PluginInstallationRecord {
            id: installation_id,
            provider_code: "fixture_provider".to_string(),
            plugin_id: "fixture_provider@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contract_version: "1flowbase.provider/v1".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Fixture Provider".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "unverified".to_string(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: if matches!(desired_state, PluginDesiredState::Disabled) {
                PluginRuntimeStatus::Inactive
            } else {
                PluginRuntimeStatus::Active
            },
            availability_status: if matches!(desired_state, PluginDesiredState::Disabled) {
                PluginAvailabilityStatus::Disabled
            } else {
                PluginAvailabilityStatus::Available
            },
            package_path: None,
            installed_path: install_path.to_string(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            created_by: self.actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        self.installations
            .write()
            .await
            .insert(installation_id, installation);
        self.upsert_plugin_package_catalog_projection(&UpsertPluginPackageCatalogProjectionInput {
            installation_id,
            package_code: "fixture_provider".to_string(),
            package_version: "0.1.0".to_string(),
            catalog_snapshot_json: json!({
                "manifest": {
                    "icon": null,
                },
                "provider": {
                    "display_name": "Fixture Provider",
                    "help_url": "https://example.com/help",
                    "default_base_url": "https://api.example.com",
                    "model_discovery_mode": "hybrid",
                    "supports_model_fetch_without_credentials": false,
                    "form_schema": [
                        {
                            "key": "base_url",
                            "type": "string",
                            "required": true
                        }
                    ],
                    "parameter_form": null,
                    "predefined_models": [
                        {
                            "model_id": "fixture_chat",
                            "display_name": "Fixture Chat",
                            "source": "static",
                            "supports_streaming": true,
                            "supports_tool_call": false,
                            "supports_multimodal": false,
                            "context_window": null,
                            "max_output_tokens": null,
                            "provider_metadata": {}
                        }
                    ]
                },
                "i18n": {
                    "default_locale": "en_US",
                    "bundles": {
                        "en_US": {
                            "provider": {
                                "label": "Fixture Provider"
                            },
                            "models": {
                                "fixture_chat": {
                                    "label": "Fixture Chat",
                                    "description": "Fixture chat model"
                                }
                            }
                        },
                        "zh_Hans": {
                            "provider": {
                                "label": "示例供应商"
                            },
                            "models": {
                                "fixture_chat": {
                                    "label": "示例聊天模型",
                                    "description": "示例聊天模型"
                                }
                            }
                        }
                    }
                }
            }),
            projection_status: PluginPackageCatalogProjectionStatus::Ok,
            last_error_message: None,
            refreshed_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
        if assigned {
            self.assignments.write().await.push(PluginAssignmentRecord {
                id: Uuid::now_v7(),
                installation_id,
                workspace_id: self.actor.current_workspace_id,
                provider_code: "fixture_provider".to_string(),
                assigned_by: self.actor.user_id,
                created_at: OffsetDateTime::now_utc(),
            });
        }
        installation_id
    }

    async fn secret_json(&self, instance_id: Uuid) -> Value {
        self.secrets
            .read()
            .await
            .get(&instance_id)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| json!({}))
    }

    async fn set_reference_count(&self, instance_id: Uuid, count: u64) {
        self.references.write().await.insert(instance_id, count);
    }

    async fn set_instance_status(&self, instance_id: Uuid, status: ModelProviderInstanceStatus) {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&instance_id)
            .expect("instance should exist for test");
        instance.status = status;
    }

    async fn audit_events(&self) -> Vec<String> {
        self.audit_events.read().await.clone()
    }

    async fn artifact_snapshot_update_count(&self) -> usize {
        self.artifact_snapshot_updates.read().await.len()
    }

    async fn remove_catalog_projection(&self, installation_id: Uuid) {
        self.catalog_projections
            .write()
            .await
            .remove(&installation_id);
    }

    async fn installation(&self, installation_id: Uuid) -> PluginInstallationRecord {
        self.installations
            .read()
            .await
            .get(&installation_id)
            .cloned()
            .expect("installation should exist for test")
    }
}

#[async_trait]
impl AuthRepository for MemoryModelProviderRepository {
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
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl PluginRepository for MemoryModelProviderRepository {
    async fn upsert_installation(
        &self,
        input: &UpsertPluginInstallationInput,
    ) -> Result<PluginInstallationRecord> {
        let record = PluginInstallationRecord {
            id: input.installation_id,
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
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        self.installations
            .write()
            .await
            .insert(record.id, record.clone());
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
        if self
            .installations
            .write()
            .await
            .remove(&installation_id)
            .is_some()
        {
            self.catalog_projections
                .write()
                .await
                .remove(&installation_id);
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("plugin_installation").into())
        }
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
        let record = PluginTaskRecord {
            id: input.task_id,
            installation_id: input.installation_id,
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            task_kind: input.task_kind,
            status: input.status,
            status_message: input.status_message.clone(),
            detail_json: input.detail_json.clone(),
            created_by: input.actor_user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
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
impl ModelProviderRepository for MemoryModelProviderRepository {
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

    async fn upsert_secret(
        &self,
        input: &UpsertModelProviderSecretInput,
    ) -> Result<ModelProviderSecretRecord> {
        let record = ModelProviderSecretRecord {
            provider_instance_id: input.provider_instance_id,
            encrypted_secret_json: json!({ "masked": true }),
            secret_version: input.secret_version,
            updated_at: OffsetDateTime::now_utc(),
        };
        self.secrets.write().await.insert(
            input.provider_instance_id,
            (record.clone(), input.plaintext_secret_json.clone()),
        );
        Ok(record)
    }

    async fn upsert_main_instance(
        &self,
        input: &UpsertModelProviderMainInstanceInput,
    ) -> Result<ModelProviderMainInstanceRecord> {
        let now = OffsetDateTime::now_utc();
        let mut main_instances = self.main_instances.write().await;
        let key = Self::main_instance_key(input.workspace_id, &input.provider_code);
        let existing = main_instances.get(&key).cloned();
        let model_distribution_rules = input
            .model_distribution_rules
            .as_ref()
            .map(|rules| {
                rules
                    .iter()
                    .map(|rule| {
                        let existing_rule = existing.as_ref().and_then(|record| {
                            record
                                .model_distribution_rules
                                .iter()
                                .find(|existing_rule| existing_rule.model_id == rule.model_id)
                        });
                        domain::ModelProviderMainModelDistributionRuleRecord {
                            workspace_id: input.workspace_id,
                            provider_code: input.provider_code.clone(),
                            model_id: rule.model_id.clone(),
                            distribution_rule: rule.distribution_rule,
                            created_by: existing_rule
                                .map(|record| record.created_by)
                                .unwrap_or(input.updated_by),
                            updated_by: input.updated_by,
                            created_at: existing_rule
                                .map(|record| record.created_at)
                                .unwrap_or(now),
                            updated_at: now,
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                existing
                    .as_ref()
                    .map(|record| record.model_distribution_rules.clone())
                    .unwrap_or_default()
            });
        let record = ModelProviderMainInstanceRecord {
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            auto_include_new_instances: input.auto_include_new_instances,
            model_distribution_rules,
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
    ) -> Result<Option<ModelProviderMainInstanceRecord>> {
        Ok(self
            .main_instances
            .read()
            .await
            .get(&Self::main_instance_key(workspace_id, provider_code))
            .cloned())
    }

    async fn create_preview_session(
        &self,
        input: &CreateModelProviderPreviewSessionInput,
    ) -> Result<ModelProviderPreviewSessionRecord> {
        let record = ModelProviderPreviewSessionRecord {
            id: input.session_id,
            workspace_id: input.workspace_id,
            actor_user_id: input.actor_user_id,
            installation_id: input.installation_id,
            instance_id: input.instance_id,
            config_fingerprint: input.config_fingerprint.clone(),
            models_json: input.models_json.clone(),
            expires_at: input.expires_at,
            created_at: OffsetDateTime::now_utc(),
        };
        self.preview_sessions
            .write()
            .await
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn get_preview_session(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<ModelProviderPreviewSessionRecord>> {
        Ok(self
            .preview_sessions
            .read()
            .await
            .get(&session_id)
            .filter(|record| record.workspace_id == workspace_id)
            .cloned())
    }

    async fn delete_preview_session(&self, workspace_id: Uuid, session_id: Uuid) -> Result<()> {
        let mut sessions = self.preview_sessions.write().await;
        if sessions
            .get(&session_id)
            .map(|record| record.workspace_id == workspace_id)
            .unwrap_or(false)
        {
            sessions.remove(&session_id);
        }
        Ok(())
    }

    async fn get_secret_json(
        &self,
        provider_instance_id: Uuid,
        _master_key: &str,
    ) -> Result<Option<Value>> {
        Ok(self
            .secrets
            .read()
            .await
            .get(&provider_instance_id)
            .map(|(_, value)| value.clone()))
    }

    async fn get_secret_record(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<ModelProviderSecretRecord>> {
        Ok(self
            .secrets
            .read()
            .await
            .get(&provider_instance_id)
            .map(|(record, _)| record.clone()))
    }

    async fn delete_instance(&self, workspace_id: Uuid, instance_id: Uuid) -> Result<()> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get(&instance_id)
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        if instance.workspace_id != workspace_id {
            return Err(ControlPlaneError::NotFound("model_provider_instance").into());
        }
        instances.remove(&instance_id);
        self.caches.write().await.remove(&instance_id);
        self.secrets.write().await.remove(&instance_id);
        Ok(())
    }

    async fn count_instance_references(
        &self,
        _workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<u64> {
        Ok(*self.references.read().await.get(&instance_id).unwrap_or(&0))
    }
}

#[derive(Clone, Default)]
struct MemoryProviderRuntime {
    validate_calls: Arc<RwLock<Vec<Uuid>>>,
    list_model_calls: Arc<RwLock<Vec<Uuid>>>,
    list_models_error: Arc<RwLock<Option<String>>>,
}

impl MemoryProviderRuntime {
    async fn list_model_call_count(&self) -> usize {
        self.list_model_calls.read().await.len()
    }

    async fn set_list_models_error(&self, message: Option<&str>) {
        *self.list_models_error.write().await = message.map(str::to_string);
    }
}

#[async_trait]
impl ProviderRuntimePort for MemoryProviderRuntime {
    async fn ensure_loaded(&self, _installation: &PluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn validate_provider(
        &self,
        installation: &PluginInstallationRecord,
        provider_config: Value,
    ) -> Result<Value> {
        self.validate_calls.write().await.push(installation.id);
        Ok(json!({
            "sanitized": {
                "base_url": provider_config["base_url"],
                "api_key": "***"
            }
        }))
    }

    async fn list_models(
        &self,
        installation: &PluginInstallationRecord,
        _provider_config: Value,
    ) -> Result<Vec<ProviderModelDescriptor>> {
        if let Some(message) = self.list_models_error.read().await.clone() {
            anyhow::bail!(message);
        }
        self.list_model_calls.write().await.push(installation.id);
        Ok(vec![ProviderModelDescriptor {
            model_id: "fixture_chat".to_string(),
            display_name: "Fixture Chat".to_string(),
            source: ProviderModelSource::Dynamic,
            supports_streaming: true,
            supports_tool_call: false,
            supports_multimodal: false,
            context_window: Some(128000),
            max_output_tokens: Some(4096),
            provider_metadata: json!({}),
        }])
    }

    async fn invoke_stream(
        &self,
        _installation: &PluginInstallationRecord,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderRuntimeInvocationOutput> {
        Ok(ProviderRuntimeInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult::default(),
        })
    }
}

mod access_main_instance;
mod instance_lifecycle;
mod model_options;
mod refresh_catalog;
