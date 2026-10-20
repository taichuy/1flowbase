use super::*;

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
            revision: existing.as_ref().map_or(1, |record| record.revision + 1),
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
            model_routing_policies: input
                .model_routing_policies
                .as_ref()
                .map(|policies| {
                    policies
                        .iter()
                        .map(|policy| domain::ModelProviderMainModelRoutingPolicyRecord {
                            workspace_id: input.workspace_id,
                            provider_code: input.provider_code.clone(),
                            model_id: policy.model_id.clone(),
                            distribution_rule: policy.distribution_rule.clone(),
                            provider_instance_ids: policy.provider_instance_ids.clone(),
                            excluded_provider_instance_ids: policy
                                .excluded_provider_instance_ids
                                .clone(),
                            created_by: input.updated_by,
                            updated_by: input.updated_by,
                            created_at: now,
                            updated_at: now,
                        })
                        .collect()
                })
                .or_else(|| {
                    existing
                        .as_ref()
                        .map(|record| record.model_routing_policies.clone())
                })
                .unwrap_or_default(),
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
