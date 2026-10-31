use super::*;

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
        self.operation_trace
            .write()
            .await
            .push(match input.refresh_status {
                ModelProviderCatalogRefreshStatus::Ready => "cache_ready",
                _ => "cache_failed",
            });
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
        self.operation_trace.write().await.push("get_catalog_cache");
        if *self.catalog_read_error.read().await {
            anyhow::bail!("catalog read failed for test");
        }
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

    async fn patch_secret(
        &self,
        input: &PatchModelProviderSecretInput,
    ) -> Result<ModelProviderSecretRecord> {
        let mut secrets = self.secrets.write().await;
        let next_version = match (
            input.expected_secret_version,
            secrets.get(&input.provider_instance_id),
        ) {
            (None, None) => 1,
            (Some(expected), Some((record, _))) if record.secret_version == expected => {
                record.secret_version + 1
            }
            _ => {
                return Err(
                    ControlPlaneError::Conflict("model_provider_secret_version_conflict").into(),
                );
            }
        };
        let record = ModelProviderSecretRecord {
            provider_instance_id: input.provider_instance_id,
            encrypted_secret_json: json!({ "masked": true }),
            secret_version: next_version,
            updated_at: OffsetDateTime::now_utc(),
        };
        secrets.insert(
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
        if existing
            .as_ref()
            .is_some_and(|record| record.revision != input.expected_revision)
        {
            return Err(crate::errors::ControlPlaneError::Conflict(
                "model_provider_main_instance_revision",
            )
            .into());
        }
        let model_routing_policies = input
            .model_routing_policies
            .as_ref()
            .map(|policies| {
                policies
                    .iter()
                    .map(|policy| {
                        let existing_policy = existing.as_ref().and_then(|record| {
                            record
                                .model_routing_policies
                                .iter()
                                .find(|existing_policy| existing_policy.model_id == policy.model_id)
                        });
                        domain::ModelProviderMainModelRoutingPolicyRecord {
                            workspace_id: input.workspace_id,
                            provider_code: input.provider_code.clone(),
                            model_id: policy.model_id.clone(),
                            distribution_rule: policy.distribution_rule.clone(),
                            provider_instance_ids: policy.provider_instance_ids.clone(),
                            excluded_provider_instance_ids: policy
                                .excluded_provider_instance_ids
                                .clone(),
                            created_by: existing_policy
                                .map(|record| record.created_by)
                                .unwrap_or(input.updated_by),
                            updated_by: input.updated_by,
                            created_at: existing_policy
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
                    .map(|record| record.model_routing_policies.clone())
                    .unwrap_or_default()
            });
        let record = ModelProviderMainInstanceRecord {
            workspace_id: input.workspace_id,
            provider_code: input.provider_code.clone(),
            auto_include_new_instances: input.auto_include_new_instances,
            revision: existing.as_ref().map_or(1, |record| record.revision + 1),
            model_routing_policies,
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
        self.operation_trace.write().await.push("get_secret_json");
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
