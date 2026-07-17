use super::fixtures::{write_test_capability_package, write_test_provider_package};
use super::*;

#[derive(Default)]
struct InMemoryOrchestrationRuntimeState {
    console_policies: Vec<domain::RoleConsolePolicy>,
    actor_is_root: bool,
    actor_workspace_id: Uuid,
    compiled_plans_by_id: HashMap<Uuid, domain::CompiledPlanRecord>,
    pub(super) flow_runs_by_id: HashMap<Uuid, domain::FlowRunRecord>,
    node_runs_by_id: HashMap<Uuid, domain::NodeRunRecord>,
    checkpoints_by_id: HashMap<Uuid, domain::CheckpointRecord>,
    callback_tasks_by_id: HashMap<Uuid, domain::CallbackTaskRecord>,
    debug_variable_cache_entries_by_key:
        HashMap<(Uuid, Uuid, Uuid, String, String), DebugVariableCacheEntry>,
    events_by_flow_run_id: HashMap<Uuid, Vec<domain::RunEventRecord>>,
    runtime_spans_by_flow_run_id: HashMap<Uuid, Vec<domain::RuntimeSpanRecord>>,
    runtime_events_by_flow_run_id: HashMap<Uuid, Vec<domain::RuntimeEventRecord>>,
    runtime_items_by_flow_run_id: HashMap<Uuid, Vec<domain::RuntimeItemRecord>>,
    context_projections_by_flow_run_id: HashMap<Uuid, Vec<domain::ContextProjectionRecord>>,
    usage_ledger_by_flow_run_id: HashMap<Uuid, Vec<domain::UsageLedgerRecord>>,
    cost_ledger_by_flow_run_id: HashMap<Uuid, Vec<domain::CostLedgerRecord>>,
    credit_ledger_by_idempotency: HashMap<(Uuid, String), domain::CreditLedgerRecord>,
    billing_sessions_by_idempotency: HashMap<(Uuid, String), domain::BillingSessionRecord>,
    data_model_side_effect_receipts_by_idempotency:
        HashMap<(Uuid, String), domain::DataModelSideEffectReceiptRecord>,
    audit_hashes_by_flow_run_id: HashMap<Uuid, Vec<domain::AuditHashRecord>>,
    capability_invocations_by_flow_run_id: HashMap<Uuid, Vec<domain::CapabilityInvocationRecord>>,
    application_environment_variables: HashMap<Uuid, Vec<domain::ApplicationEnvironmentVariable>>,
    application_js_dependency_selections:
        HashMap<(Uuid, String, String), domain::ApplicationJsDependencySelection>,
    installations_by_id: HashMap<Uuid, domain::PluginInstallationRecord>,
    artifact_instances_by_key: HashMap<(String, Uuid), domain::PluginArtifactInstanceRecord>,
    assignments_by_workspace: HashMap<Uuid, Vec<domain::PluginAssignmentRecord>>,
    node_contributions_by_workspace: HashMap<Uuid, Vec<domain::NodeContributionRegistryEntry>>,
    instances_by_id: HashMap<Uuid, domain::ModelProviderInstanceRecord>,
    caches_by_instance_id: HashMap<Uuid, domain::ModelProviderCatalogCacheRecord>,
    catalog_entries_by_instance_id: HashMap<Uuid, Vec<domain::ModelProviderCatalogEntryRecord>>,
    model_failover_attempts_by_flow_run_id:
        HashMap<Uuid, Vec<domain::ModelFailoverAttemptLedgerRecord>>,
    secret_json_by_instance_id: HashMap<Uuid, Value>,
    main_instances_by_provider: HashMap<(Uuid, String), domain::ModelProviderMainInstanceRecord>,
    scope_data_model_grants: Vec<domain::ScopeDataModelGrantRecord>,
    file_storages_by_id: HashMap<Uuid, domain::FileStorageRecord>,
    file_tables_by_id: HashMap<Uuid, domain::FileTableRecord>,
    status_after_next_get: Option<(Uuid, domain::FlowRunStatus)>,
    status_before_next_flow_update: Option<(Uuid, domain::FlowRunStatus)>,
    application_run_detail_read_count: usize,
}

#[derive(Clone)]
pub(crate) struct InMemoryOrchestrationRuntimeRepository {
    pub(super) flow: InMemoryFlowRepository,
    inner: Arc<Mutex<InMemoryOrchestrationRuntimeState>>,
    default_provider_instance_id: Uuid,
}

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn reset_application_run_detail_read_count(&self) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .application_run_detail_read_count = 0;
    }

    pub(super) fn application_run_detail_read_count(&self) -> usize {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .application_run_detail_read_count
    }

    fn main_instance_key(workspace_id: Uuid, provider_code: &str) -> (Uuid, String) {
        (workspace_id, provider_code.to_string())
    }

    fn fixture_provider_installation_id(
        inner: &InMemoryOrchestrationRuntimeState,
        provider_code: &str,
    ) -> Uuid {
        inner
            .installations_by_id
            .values()
            .find(|record| record.provider_code == provider_code)
            .map(|record| record.id)
            .or_else(|| {
                inner
                    .installations_by_id
                    .values()
                    .find(|record| record.provider_code == "fixture_provider")
                    .map(|record| record.id)
            })
            .expect("fixture provider installation should exist")
    }

    pub(crate) fn with_permissions(permissions: Vec<&str>) -> Self {
        Self::with_permissions_and_console_policies(
            permissions,
            Self::full_application_console_policies(),
        )
    }

    fn full_application_console_policies() -> Vec<domain::RoleConsolePolicy> {
        vec![domain::RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![domain::RoleConsoleGroupPolicy::full(
                domain::ConsolePolicyGroup::settings_feature(
                    access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
                )
                .expect("applications settings feature id must be valid"),
            )],
        )]
    }

    pub(crate) fn with_permissions_and_console_policies(
        permissions: Vec<&str>,
        console_policies: Vec<domain::RoleConsolePolicy>,
    ) -> Self {
        Self::with_permissions_console_policies_and_data_model_scope_grant(
            permissions,
            console_policies,
            true,
        )
    }

    pub(crate) fn with_permissions_without_data_model_scope_grant(permissions: Vec<&str>) -> Self {
        Self::with_permissions_console_policies_and_data_model_scope_grant(
            permissions,
            Self::full_application_console_policies(),
            false,
        )
    }

    fn with_permissions_console_policies_and_data_model_scope_grant(
        permissions: Vec<&str>,
        console_policies: Vec<domain::RoleConsolePolicy>,
        include_data_model_scope_grant: bool,
    ) -> Self {
        let flow = InMemoryFlowRepository::with_permissions(permissions);
        let installation_id = Uuid::now_v7();
        let capability_installation_id = Uuid::now_v7();
        let provider_instance_id = Uuid::now_v7();
        let workspace_id = Uuid::nil();
        let install_path = write_test_provider_package();
        let capability_install_path = write_test_capability_package();
        let now = OffsetDateTime::now_utc();
        let installation = domain::PluginInstallationRecord {
            id: installation_id,
            provider_code: "fixture_provider".to_string(),
            plugin_id: "fixture_provider@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contract_version: "1flowbase.provider/v1".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Fixture Provider".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "unverified".to_string(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            artifact_status: domain::PluginArtifactStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: install_path.clone(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            created_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        let capability_installation = domain::PluginInstallationRecord {
            id: capability_installation_id,
            provider_code: "fixture_capability".to_string(),
            plugin_id: "fixture_capability@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contract_version: "1flowbase.capability/v1".to_string(),
            protocol: "stdio_json".to_string(),
            display_name: "Fixture Capability".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "unverified".to_string(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            artifact_status: domain::PluginArtifactStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: capability_install_path.clone(),
            checksum: None,
            manifest_fingerprint: None,
            signature_status: None,
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            created_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        let assignment = domain::PluginAssignmentRecord {
            id: Uuid::now_v7(),
            installation_id,
            workspace_id,
            provider_code: "fixture_provider".to_string(),
            assigned_by: Uuid::nil(),
            created_at: now,
        };
        let capability_assignment = domain::PluginAssignmentRecord {
            id: Uuid::now_v7(),
            installation_id: capability_installation_id,
            workspace_id,
            provider_code: "fixture_capability".to_string(),
            assigned_by: Uuid::nil(),
            created_at: now,
        };
        let capability_node_contribution = domain::NodeContributionRegistryEntry {
            installation_id: capability_installation_id,
            provider_code: "fixture_capability".to_string(),
            plugin_unique_identifier: "fixture_capability".to_string(),
            package_id: "fixture_capability@0.1.0".to_string(),
            plugin_id: "fixture_capability@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contribution_code: "fixture_action".to_string(),
            node_shell: "action".to_string(),
            category: "automation".to_string(),
            title: "Fixture Action".to_string(),
            description: "Fixture capability node".to_string(),
            icon: "puzzle".to_string(),
            schema_ui: json!({}),
            schema_version: "1flowbase.node-contribution/v2".to_string(),
            output_schema: json!({
                "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
            }),
            contribution_checksum: "sha256:contribution".to_string(),
            compiled_contribution_hash: "sha256:compiled".to_string(),
            output_schema_snapshot: json!({
                "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
            }),
            side_effect_policy: "external_read".to_string(),
            infra_contracts: vec![],
            required_auth: vec!["provider_instance".to_string()],
            visibility: "public".to_string(),
            experimental: false,
            dependency_installation_kind: "optional".to_string(),
            dependency_plugin_version_range: ">=0.1.0".to_string(),
            dependency_status: domain::NodeContributionDependencyStatus::Ready,
        };
        let instance = domain::ModelProviderInstanceRecord {
            id: provider_instance_id,
            workspace_id,
            installation_id,
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            display_name: "Fixture".to_string(),
            status: domain::ModelProviderInstanceStatus::Ready,
            config_json: json!({
                "base_url": "https://api.example.com",
            }),
            configured_models: vec![domain::ModelProviderConfiguredModel {
                model_id: "gpt-5.4-mini".to_string(),
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            }],
            enabled_model_ids: vec!["gpt-5.4-mini".to_string()],
            included_in_main: true,
            created_by: Uuid::nil(),
            updated_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        let cache = domain::ModelProviderCatalogCacheRecord {
            provider_instance_id,
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
        let scope_data_model_grants = if include_data_model_scope_grant {
            vec![domain::ScopeDataModelGrantRecord {
                id: Uuid::now_v7(),
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: workspace_id,
                data_model_id: Uuid::nil(),
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                created_by: None,
                created_at: now,
                updated_at: now,
            }]
        } else {
            Vec::new()
        };

        Self {
            flow,
            inner: Arc::new(Mutex::new(InMemoryOrchestrationRuntimeState {
                console_policies,
                actor_is_root: false,
                actor_workspace_id: workspace_id,
                installations_by_id: HashMap::from([
                    (installation_id, installation),
                    (capability_installation_id, capability_installation),
                ]),
                artifact_instances_by_key: HashMap::from([
                    (
                        ("local:test".to_string(), installation_id),
                        domain::PluginArtifactInstanceRecord {
                            node_id: "local:test".to_string(),
                            installation_id,
                            local_version: Some("0.1.0".to_string()),
                            local_checksum: None,
                            installed_path: Some(install_path),
                            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Active,
                            checked_at: now,
                            last_error: None,
                        },
                    ),
                    (
                        ("local:test".to_string(), capability_installation_id),
                        domain::PluginArtifactInstanceRecord {
                            node_id: "local:test".to_string(),
                            installation_id: capability_installation_id,
                            local_version: Some("0.1.0".to_string()),
                            local_checksum: None,
                            installed_path: Some(capability_install_path),
                            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                            runtime_status: domain::PluginRuntimeStatus::Active,
                            checked_at: now,
                            last_error: None,
                        },
                    ),
                ]),
                assignments_by_workspace: HashMap::from([(
                    workspace_id,
                    vec![assignment, capability_assignment],
                )]),
                node_contributions_by_workspace: HashMap::from([(
                    workspace_id,
                    vec![capability_node_contribution],
                )]),
                instances_by_id: HashMap::from([(provider_instance_id, instance)]),
                caches_by_instance_id: HashMap::from([(provider_instance_id, cache)]),
                secret_json_by_instance_id: HashMap::from([(
                    provider_instance_id,
                    json!({ "api_key": "test-secret" }),
                )]),
                scope_data_model_grants,
                ..InMemoryOrchestrationRuntimeState::default()
            })),
            default_provider_instance_id: provider_instance_id,
        }
    }

    pub(super) async fn seed_application_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
    ) -> Result<domain::ApplicationRecord> {
        self.flow
            .seed_application_for_actor(actor_user_id, name)
            .await
    }

    pub(super) async fn seed_application_with_type_for_actor(
        &self,
        actor_user_id: Uuid,
        name: &str,
        application_type: domain::ApplicationType,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::create_application(
            &self.flow,
            &CreateApplicationInput {
                actor_user_id,
                workspace_id: Uuid::nil(),
                application_type,
                workflow_trigger_type: match application_type {
                    domain::ApplicationType::AgentFlow => None,
                    domain::ApplicationType::Workflow => {
                        Some(domain::WorkflowTriggerType::Extension)
                    }
                },
                workflow_trigger_config: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        )
        .await
    }

    pub(super) async fn seed_application_in_workspace_for_actor(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        name: &str,
    ) -> Result<domain::ApplicationRecord> {
        ApplicationRepository::create_application(
            &self.flow,
            &CreateApplicationInput {
                actor_user_id,
                workspace_id,
                application_type: domain::ApplicationType::AgentFlow,
                workflow_trigger_type: None,
                workflow_trigger_config: None,
                name: name.to_string(),
                description: String::new(),
                icon: None,
                icon_type: None,
                icon_background: None,
            },
        )
        .await
    }

    pub(crate) fn replace_console_policies(
        &self,
        console_policies: Vec<domain::RoleConsolePolicy>,
    ) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .console_policies = console_policies;
    }

    pub(crate) fn configure_root_actor(&self, workspace_id: Uuid) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        inner.actor_is_root = true;
        inner.actor_workspace_id = workspace_id;
    }

    pub(crate) fn default_provider_instance_id(&self) -> Uuid {
        self.default_provider_instance_id
    }

    pub(crate) fn seed_file_storage(
        &self,
        storage: domain::FileStorageRecord,
        file_table: domain::FileTableRecord,
    ) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        inner.file_storages_by_id.insert(storage.id, storage);
        inner.file_tables_by_id.insert(file_table.id, file_table);
    }

    pub(crate) fn default_file_storage_id(&self) -> Uuid {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .file_storages_by_id
            .values()
            .find(|storage| storage.is_default)
            .map(|storage| storage.id)
            .expect("default file storage should exist")
    }

    pub(crate) fn events_for_flow_run(&self, flow_run_id: Uuid) -> Vec<domain::RunEventRecord> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .events_by_flow_run_id
            .get(&flow_run_id)
            .cloned()
            .unwrap_or_default()
    }

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
        installation.availability_status = availability_status;
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

    pub(super) fn force_flow_run_status(&self, flow_run_id: Uuid, status: domain::FlowRunStatus) {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let flow_run = inner
            .flow_runs_by_id
            .get_mut(&flow_run_id)
            .expect("flow run should exist for test");
        flow_run.status = status;
    }

    pub(super) fn force_flow_run_status_after_next_get(
        &self,
        flow_run_id: Uuid,
        status: domain::FlowRunStatus,
    ) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .status_after_next_get = Some((flow_run_id, status));
    }

    pub(super) fn force_flow_run_status_before_next_flow_update(
        &self,
        flow_run_id: Uuid,
        status: domain::FlowRunStatus,
    ) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .status_before_next_flow_update = Some((flow_run_id, status));
    }
}

mod flow_ports;
mod provider_runtime;
mod runtime_repository;
mod runtime_repository_helpers;
mod runtime_repository_status_tests;

pub(super) use flow_ports::test_data_model_definition;
pub(crate) use provider_runtime::InMemoryProviderRuntime;
