use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(crate) fn seed_provider_instance(
        &self,
        provider_code: &str,
        display_name: &str,
        included_in_main: bool,
        status: domain::ModelProviderInstanceStatus,
        enabled_model_ids: Vec<&str>,
    ) -> Uuid {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let installation_id = Self::fixture_provider_installation_id(&inner, provider_code);
        let instance_id = Uuid::now_v7();
        let model_ids = enabled_model_ids
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let configured_models = model_ids
            .iter()
            .cloned()
            .map(|model_id| domain::ModelProviderConfiguredModel {
                model_id,
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            })
            .collect::<Vec<_>>();
        let models_json = model_ids
            .iter()
            .map(|model_id| {
                json!({
                    "model_id": model_id,
                    "display_name": model_id,
                    "source": "dynamic",
                    "supports_streaming": true,
                    "supports_tool_call": true,
                    "supports_multimodal": false,
                    "context_window": 128000,
                    "max_output_tokens": 4096,
                    "provider_metadata": {}
                })
            })
            .collect::<Vec<_>>();

        inner.instances_by_id.insert(
            instance_id,
            domain::ModelProviderInstanceRecord {
                id: instance_id,
                workspace_id: Uuid::nil(),
                installation_id,
                provider_code: provider_code.to_string(),
                protocol: "openai_compatible".to_string(),
                display_name: display_name.to_string(),
                status,
                config_json: json!({
                    "base_url": format!("https://{}.example.com/v1", provider_code),
                }),
                configured_models,
                enabled_model_ids: model_ids.clone(),
                included_in_main,
                created_by: Uuid::nil(),
                updated_by: Uuid::nil(),
                created_at: now,
                updated_at: now,
            },
        );
        inner.caches_by_instance_id.insert(
            instance_id,
            domain::ModelProviderCatalogCacheRecord {
                provider_instance_id: instance_id,
                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                source: domain::ModelProviderCatalogSource::Hybrid,
                models_json: Value::Array(models_json),
                last_error_message: None,
                refreshed_at: Some(now),
                updated_at: now,
            },
        );
        inner
            .secret_json_by_instance_id
            .insert(instance_id, json!({ "api_key": "test-secret" }));

        instance_id
    }

    pub(crate) fn set_instance_included_in_main(&self, instance_id: Uuid, included_in_main: bool) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .instances_by_id
            .get_mut(&instance_id)
            .expect("provider instance should exist")
            .included_in_main = included_in_main;
    }

    pub(crate) fn set_main_model_routing_policy(
        &self,
        provider_code: &str,
        model_id: &str,
        distribution_rule: domain::ModelProviderDistributionRule,
        provider_instance_ids: Vec<Uuid>,
        excluded_provider_instance_ids: Vec<Uuid>,
    ) {
        let now = OffsetDateTime::now_utc();
        let record = domain::ModelProviderMainInstanceRecord {
            workspace_id: Uuid::nil(),
            provider_code: provider_code.to_string(),
            auto_include_new_instances: true,
            revision: 1,
            model_routing_policies: vec![domain::ModelProviderMainModelRoutingPolicyRecord {
                workspace_id: Uuid::nil(),
                provider_code: provider_code.to_string(),
                model_id: model_id.to_string(),
                distribution_rule,
                provider_instance_ids,
                excluded_provider_instance_ids,
                created_by: Uuid::nil(),
                updated_by: Uuid::nil(),
                created_at: now,
                updated_at: now,
            }],
            created_by: Uuid::nil(),
            updated_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .main_instances_by_provider
            .insert((Uuid::nil(), provider_code.to_string()), record);
    }

    pub(crate) fn seed_catalog_entries_for_instance(
        &self,
        instance_id: Uuid,
        model_ids: Vec<&str>,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let entries = model_ids
            .into_iter()
            .map(|model_id| domain::ModelProviderCatalogEntryRecord {
                id: Uuid::now_v7(),
                provider_instance_id: Some(instance_id),
                catalog_source_id: Uuid::now_v7(),
                upstream_model_id: model_id.to_string(),
                display_label: model_id.to_string(),
                protocol: "openai_compatible".to_string(),
                capability_snapshot: json!({}),
                parameter_schema_ref: None,
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                pricing_ref: None,
                fetched_at: now,
                status: "active".to_string(),
            })
            .collect::<Vec<_>>();
        inner
            .catalog_entries_by_instance_id
            .insert(instance_id, entries);
    }

    pub(crate) fn set_instance_status(
        &self,
        instance_id: Uuid,
        status: domain::ModelProviderInstanceStatus,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let instance = inner
            .instances_by_id
            .get_mut(&instance_id)
            .expect("provider instance should exist");
        instance.status = status;
        instance.updated_at = OffsetDateTime::now_utc();
    }

    pub(crate) fn set_instance_enabled_models(
        &self,
        instance_id: Uuid,
        enabled_model_ids: Vec<&str>,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let model_ids = enabled_model_ids
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let instance = inner
            .instances_by_id
            .get_mut(&instance_id)
            .expect("provider instance should exist");
        instance.enabled_model_ids = model_ids.clone();
        instance.configured_models = model_ids
            .iter()
            .cloned()
            .map(|model_id| domain::ModelProviderConfiguredModel {
                model_id,
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            })
            .collect();
        instance.updated_at = OffsetDateTime::now_utc();
        let updated_at = instance.updated_at;
        inner.caches_by_instance_id.insert(
            instance_id,
            domain::ModelProviderCatalogCacheRecord {
                provider_instance_id: instance_id,
                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                source: domain::ModelProviderCatalogSource::Hybrid,
                models_json: Value::Array(
                    model_ids
                        .iter()
                        .map(|model_id| {
                            json!({
                                "model_id": model_id,
                                "display_name": model_id,
                                "source": "dynamic",
                                "supports_streaming": true,
                                "supports_tool_call": true,
                                "supports_multimodal": false,
                                "context_window": 128000,
                                "max_output_tokens": 4096,
                                "provider_metadata": {}
                            })
                        })
                        .collect(),
                ),
                last_error_message: None,
                refreshed_at: Some(updated_at),
                updated_at,
            },
        );
    }

    pub(crate) fn set_configured_model_supports_multimodal(
        &self,
        instance_id: Uuid,
        model_id: &str,
        supports_multimodal: bool,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let instance = inner
            .instances_by_id
            .get_mut(&instance_id)
            .expect("provider instance should exist");
        let configured_model = instance
            .configured_models
            .iter_mut()
            .find(|model| model.model_id == model_id)
            .expect("configured model should exist");
        configured_model.supports_multimodal = Some(supports_multimodal);
        instance.updated_at = OffsetDateTime::now_utc();
    }

    pub(crate) fn set_instance_catalog_models(
        &self,
        instance_id: Uuid,
        catalog_model_ids: Vec<&str>,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        inner.caches_by_instance_id.insert(
            instance_id,
            domain::ModelProviderCatalogCacheRecord {
                provider_instance_id: instance_id,
                model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
                refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                source: domain::ModelProviderCatalogSource::Hybrid,
                models_json: Value::Array(
                    catalog_model_ids
                        .into_iter()
                        .map(|model_id| {
                            json!({
                                "model_id": model_id,
                                "display_name": model_id,
                                "source": "dynamic",
                                "supports_streaming": true,
                                "supports_tool_call": true,
                                "supports_multimodal": false,
                                "context_window": 128000,
                                "max_output_tokens": 4096,
                                "provider_metadata": {}
                            })
                        })
                        .collect(),
                ),
                last_error_message: None,
                refreshed_at: Some(now),
                updated_at: now,
            },
        );
    }

    pub(crate) fn remove_assignment_for_installation(
        &self,
        workspace_id: Uuid,
        installation_id: Uuid,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let assignments = inner
            .assignments_by_workspace
            .entry(workspace_id)
            .or_default();
        assignments.retain(|assignment| assignment.installation_id != installation_id);
    }

    pub(crate) fn set_installation_state(
        &self,
        installation_id: Uuid,
        desired_state: domain::PluginDesiredState,
        availability_status: domain::PluginAvailabilityStatus,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let installation = inner
            .installations_by_id
            .get_mut(&installation_id)
            .expect("installation should exist");
        installation.desired_state = desired_state;
        for artifact in inner.artifact_instances_by_key.values_mut() {
            if artifact.installation_id == installation_id {
                artifact.availability_status = availability_status;
            }
        }
    }

    pub(crate) async fn mark_provider_manifest_legacy_missing_publisher_namespace(
        &self,
        installation_id: Uuid,
    ) {
        let manifest_path = {
            let inner = self.inner.lock().expect("runtime repo mutex poisoned");
            let local_path = inner
                .artifact_instances_by_key
                .values()
                .find(|artifact| artifact.installation_id == installation_id)
                .and_then(|artifact| artifact.local_path.clone())
                .expect("provider artifact path should exist");
            std::path::Path::new(&local_path).join("manifest.yaml")
        };
        let raw = std::fs::read_to_string(&manifest_path)
            .expect("provider fixture manifest should be readable");
        let legacy = raw
            .lines()
            .filter(|line| !line.starts_with("publisher_namespace:"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(&manifest_path, legacy)
            .expect("provider fixture manifest should become legacy");
        let fingerprint = plugin_framework::compute_manifest_fingerprint(&manifest_path)
            .await
            .expect("legacy provider fingerprint should be canonical");
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let installation = inner
            .installations_by_id
            .get_mut(&installation_id)
            .expect("provider installation should exist");
        installation.organization = "1flowbase-tests".to_string();
        installation.legacy_manifest_compatibility =
            Some("missing_publisher_namespace_v1".to_string());
        inner
            .artifact_instances_by_key
            .values_mut()
            .find(|artifact| artifact.installation_id == installation_id)
            .expect("provider artifact should exist")
            .manifest_fingerprint = Some(fingerprint);
    }

    pub(crate) fn seed_included_provider_instances(&self) -> (Uuid, Uuid) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let installation_id = Self::fixture_provider_installation_id(&inner, "fixture_provider");

        let alpha_instance_id = Uuid::now_v7();
        let backup_instance_id = self.default_provider_instance_id;
        let alpha_now = now - time::Duration::minutes(5);

        let alpha_instance = domain::ModelProviderInstanceRecord {
            id: alpha_instance_id,
            workspace_id: Uuid::nil(),
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Fixture Alpha".to_string(),
            status: domain::ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://alpha.example.com/v1",
            }),
            configured_models: vec![domain::ModelProviderConfiguredModel {
                model_id: "gpt-5.4-mini".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
                pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
                pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
            }],
            enabled_model_ids: vec!["gpt-5.4-mini".to_string()],
            included_in_main: true,
            created_by: Uuid::nil(),
            updated_by: Uuid::nil(),
            created_at: alpha_now,
            updated_at: alpha_now,
        };
        let backup_instance = inner
            .instances_by_id
            .get_mut(&backup_instance_id)
            .expect("default provider instance should exist");
        backup_instance.installation_id = installation_id;
        backup_instance.provider_code = "fixture_provider".to_string();
        backup_instance.protocol = "openai_compatible".to_string();
        backup_instance.display_name = "Fixture Backup".to_string();
        backup_instance.status = domain::ModelProviderInstanceStatus::Ready;
        backup_instance.config_json = json!({
            "base_url": "https://backup.example.com/v1",
        });
        backup_instance.configured_models = vec![domain::ModelProviderConfiguredModel {
            model_id: "gpt-5.4-mini".to_string(),
            enabled: true,
            context_window_override_tokens: None,
            supports_multimodal: None,
            pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.to_string(),
            pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.to_string(),
        }];
        backup_instance.enabled_model_ids = vec!["gpt-5.4-mini".to_string()];
        backup_instance.created_by = Uuid::nil();
        backup_instance.updated_by = Uuid::nil();
        backup_instance.created_at = now;
        backup_instance.updated_at = now;

        let alpha_cache = domain::ModelProviderCatalogCacheRecord {
            provider_instance_id: alpha_instance_id,
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
            source: domain::ModelProviderCatalogSource::Hybrid,
            models_json: json!([
                {
                    "model_id": "gpt-5.4-mini",
                    "display_name": "GPT-5.4 Mini",
                    "source": "dynamic",
                    "supports_streaming": true,
                    "supports_tool_call": true,
                    "supports_multimodal": false,
                    "context_window": 128000,
                    "max_output_tokens": 4096,
                    "provider_metadata": {}
                }
            ]),
            last_error_message: None,
            refreshed_at: Some(now),
            updated_at: now,
        };
        let backup_cache = domain::ModelProviderCatalogCacheRecord {
            provider_instance_id: backup_instance_id,
            model_discovery_mode: domain::ModelProviderDiscoveryMode::Hybrid,
            refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
            source: domain::ModelProviderCatalogSource::Hybrid,
            models_json: json!([
                {
                    "model_id": "gpt-5.4-mini",
                    "display_name": "GPT-5.4 Mini",
                    "source": "dynamic",
                    "supports_streaming": true,
                    "supports_tool_call": true,
                    "supports_multimodal": false,
                    "context_window": 128000,
                    "max_output_tokens": 4096,
                    "provider_metadata": {}
                }
            ]),
            last_error_message: None,
            refreshed_at: Some(now),
            updated_at: now,
        };

        inner
            .instances_by_id
            .insert(alpha_instance_id, alpha_instance);
        inner
            .caches_by_instance_id
            .insert(alpha_instance_id, alpha_cache);
        inner
            .caches_by_instance_id
            .insert(backup_instance_id, backup_cache);
        inner
            .secret_json_by_instance_id
            .insert(alpha_instance_id, json!({ "api_key": "alpha-secret" }));
        inner
            .secret_json_by_instance_id
            .insert(backup_instance_id, json!({ "api_key": "backup-secret" }));

        (alpha_instance_id, backup_instance_id)
    }
}
