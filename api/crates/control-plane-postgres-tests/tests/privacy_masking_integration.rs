//! Real-Postgres integration tests for Privacy Masking runtime policies, atomic lifecycle fencing,
//! workspace isolation, keyset log pagination, and zero-plaintext audit guarantees (AC-001 ~ AC-004).
use std::sync::Arc;

use control_plane::{
    audit::audit_log,
    plugin_management::ManagedPrivacyMaskingService,
    ports::{
        CommitPluginFamilyUninstallInput, GrantContributionAuthorizationInput,
        ListPrivacyMaskingLogsQuery, PluginContributionAuthorityRepository, PluginRepository,
        PrivacyMaskingInvalidationObserver, PrivacyMaskingRepository,
        RevokeContributionAuthorizationInput, UpdatePluginDesiredStateInput,
        UpsertPluginArtifactInstanceInput, UpsertPluginInstallationInput,
    },
    privacy_masking::{
        list_builtin_rule_descriptors, PrivacyInvocationContext, PrivacyMaskingKernel,
        BUILTIN_EMAIL,
    },
};
use domain::{
    ContributionResourceScope, ExtensionCategory, ExtensionSignatureStatus,
    PluginArtifactInstanceStatus, PluginAvailabilityStatus, PluginDesiredState,
    PluginRuntimeStatus, PluginVerificationStatus,
};
use extension_contracts::provider_contract::{
    ProviderInvocationInput, ProviderMessage, ProviderMessageRole,
};
use extension_contracts::{
    PrivacyMaskingBinding, PrivacyMaskingCallerDto, PrivacyMaskingPort, PrivacyMaskingRequest,
    PrivacyMaskingResponse, PrivacyMaskingRuleDto, PrivacyMaskingRuleKind, PrivacyMaskingRuleMode,
    PrivacyMaskingSubjectBindingDto, PrivacyMaskingSubjectKind, PrivacyMaskingWorkspacePolicyDto,
    PRIVACY_MASKING_CONTRACT_ID, PRIVACY_POLICY_SYNC_PERMISSION,
};
use serde_json::json;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
use uuid::Uuid;

#[path = "privacy_masking_integration/debug_fixture.rs"]
mod debug_fixture;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn setup_store_and_workspaces() -> (
    PgControlPlaneStore,
    Arc<PrivacyMaskingKernel>,
    Uuid,
    Uuid,
    Uuid,
) {
    let schema = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .expect("isolated schema should be available");
    let pool = schema.connect().await.expect("schema should connect");
    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    let base_store = PgControlPlaneStore::new(pool);
    let kernel = Arc::new(
        PrivacyMaskingKernel::new(Arc::new(base_store.clone()), "pg-integration-master-key-v1")
            .expect("kernel init"),
    );
    let store = base_store.with_privacy_masking_invalidation_observer(
        kernel.clone() as Arc<dyn PrivacyMaskingInvalidationObserver>
    );

    let tenant = store.upsert_root_tenant().await.expect("tenant");
    let ws_a = store
        .upsert_workspace(tenant.id, "privacy-ws-a")
        .await
        .expect("ws_a");
    let ws_b = store
        .upsert_workspace(tenant.id, "privacy-ws-b")
        .await
        .expect("ws_b");
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: json!({}),
        })
        .await
        .expect("login entry");
    let actor = store
        .upsert_root_user(
            ws_a.id,
            "privacy-root",
            "privacy-root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Privacy",
            "Root",
        )
        .await
        .expect("root user");

    (store, kernel, actor.id, ws_a.id, ws_b.id)
}

async fn seed_active_privacy_plugin(
    store: &PgControlPlaneStore,
    actor_id: Uuid,
    version: &str,
) -> (Uuid, domain::PluginContributionAuthoritySnapshot) {
    seed_privacy_plugin(
        store,
        actor_id,
        version,
        PluginDesiredState::ActiveRequested,
    )
    .await
}

async fn seed_privacy_plugin(
    store: &PgControlPlaneStore,
    actor_id: Uuid,
    version: &str,
    desired_state: PluginDesiredState,
) -> (Uuid, domain::PluginContributionAuthoritySnapshot) {
    let installation_id = Uuid::now_v7();
    let installed = PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: ExtensionCategory::RuntimeExtensions,
            organization: "1flowbase".to_string(),
            provider_code: "privacy-masking".to_string(),
            plugin_id: format!("privacy-masking@{version}"),
            plugin_version: version.to_string(),
            contract_version: "1flowbase.extension-bus/v1".to_string(),
            protocol: "stdio_json_worker".to_string(),
            display_name: "Privacy Masking".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "verified_official".to_string(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({
                "binding_targets": ["system"],
                "managed_service": { "scope": "system" },
                "managed": {
                    "module": {
                        "bus_version": "v1",
                        "module_id": "privacy-masking",
                        "module_version": version,
                        "module_kind": "runtime",
                        "contributions": [{
                            "contribution_id": "privacy-masking.service",
                            "contributor_module_id": "privacy-masking",
                            "point_id": plugin_framework::MANAGED_SERVICE_POINT,
                            "contract_version": "1",
                            "mode": "append",
                            "required_permissions": [PRIVACY_POLICY_SYNC_PERMISSION]
                        }]
                    },
                    "execution_bindings": []
                }
            }),
            is_system_reserved: false,
            actor_user_id: actor_id,
        },
    )
    .await
    .expect("installation upsert");

    let snapshot = PluginContributionAuthorityRepository::grant_contribution_authorization(
        store,
        &GrantContributionAuthorizationInput {
            candidate_node_id: None,
            expected_installation_updated_at: installed.updated_at,
            installation_id,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            contribution_id: "privacy-masking.service".to_string(),
            point_id: plugin_framework::MANAGED_SERVICE_POINT.to_string(),
            permission: PRIVACY_POLICY_SYNC_PERMISSION.to_string(),
            resource_scope: ContributionResourceScope::System,
            permission_contract_id: PRIVACY_MASKING_CONTRACT_ID.to_string(),
            permission_contract_version: "1".to_string(),
            actor_user_id: actor_id,
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_contribution_authorization",
                Some(installation_id),
                "grant",
                json!({}),
            ),
        },
    )
    .await
    .expect("grant privacy.policy.sync");

    (installation_id, snapshot)
}

fn sample_policy_for_instance(
    instance_id: Uuid,
    restore_output: bool,
) -> PrivacyMaskingWorkspacePolicyDto {
    let email_desc = list_builtin_rule_descriptors()
        .into_iter()
        .find(|d| d.builtin_code == BUILTIN_EMAIL)
        .expect("email builtin exists");
    PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![PrivacyMaskingRuleDto {
            rule_id: email_desc.builtin_code.clone(),
            name: email_desc.display_name_zh,
            kind: PrivacyMaskingRuleKind::Builtin,
            builtin_code: Some(email_desc.builtin_code),
            placeholder_kind: email_desc.placeholder_kind,
            pattern: None,
            enabled: true,
            priority: 1,
        }],
        subject_bindings: vec![PrivacyMaskingSubjectBindingDto {
            binding_id: format!("bind-{instance_id}"),
            subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
            subject_id: instance_id.to_string(),
            subject_name: "Prod Model".to_string(),
            enabled: true,
            restore_output,
            rule_mode: PrivacyMaskingRuleMode::AllEnabled,
            rule_ids: vec![],
        }],
    }
}

#[tokio::test]
async fn workspace_isolation_log_pagination_and_zero_plaintext() {
    let (store, kernel, actor_id, ws_a, ws_b) = setup_store_and_workspaces().await;
    let (installation_id, _) = seed_active_privacy_plugin(&store, actor_id, "0.1.0").await;
    let instance_id = Uuid::now_v7();

    let summary = kernel
        .sync_workspace_policy(
            installation_id,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync policy for ws_a");
    assert_eq!(summary.policy_version, 1);
    assert_eq!(summary.enabled_rule_count, 1);
    assert_eq!(summary.active_subject_count, 1);

    // Workspace B must NOT see Workspace A's policy
    let ws_b_policies = store
        .list_active_privacy_masking_policies(ws_b)
        .await
        .expect("list ws_b");
    assert!(ws_b_policies.is_empty());

    // Execute two masking invocations in Workspace A and write logs
    let flow_run_1 = Uuid::now_v7();
    for (email, run_id) in [
        ("secret1@acme.org", flow_run_1),
        ("secret2@acme.org", Uuid::now_v7()),
    ] {
        let mut input = ProviderInvocationInput {
            model: "gpt-4o".into(),
            protocol: "openai_chat".into(),
            provider_instance_id: instance_id.to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: format!("Please email {email} immediately"),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..Default::default()
        };
        let ctx = PrivacyInvocationContext {
            workspace_id: ws_a,
            flow_run_id: Some(run_id),
            application_id: None,
            provider_instance_id: instance_id,
            provider_instance_name: "Prod Model".into(),
            model_id: "gpt-4o".into(),
            protocol: "openai_chat".into(),
        };
        let prepared = kernel
            .prepare_invocation(&ctx, &mut input)
            .await
            .expect("prepare")
            .expect("matched");
        assert!(!input.messages[0].content.contains(email));
        let restored = prepared
            .restoration_stack
            .restore_str(&input.messages[0].content);
        assert!(restored.contains(email));
        kernel.finalize_invocation(&prepared, None).await;
    }

    // Keyset pagination with limit = 1
    let page1 = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            limit: 1,
            ..Default::default()
        })
        .await
        .expect("page1");
    assert_eq!(page1.items.len(), 1);
    assert!(page1.next_cursor.is_some());

    let page2 = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            limit: 10,
            cursor: page1.next_cursor,
            ..Default::default()
        })
        .await
        .expect("page2");
    assert_eq!(page2.items.len(), 1);
    assert!(page2.next_cursor.is_none());

    // Zero plaintext in every persisted log record
    for entry in page1.items.iter().chain(page2.items.iter()) {
        let raw = serde_json::to_string(entry).unwrap();
        assert!(!raw.contains("secret1@acme.org"), "leaked plaintext: {raw}");
        assert!(!raw.contains("secret2@acme.org"), "leaked plaintext: {raw}");
        assert_eq!(entry.restored_count, 1);
    }

    // Workspace B sees zero logs
    let ws_b_logs = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_b,
            limit: 10,
            ..Default::default()
        })
        .await
        .expect("ws_b logs");
    assert!(ws_b_logs.items.is_empty());

    // Filter by flow_run_id & placeholder_kind
    let filtered = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            flow_run_id: Some(flow_run_1),
            placeholder_kind: Some("EMAIL".into()),
            limit: 10,
            ..Default::default()
        })
        .await
        .expect("filtered");
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(
        filtered.items[0].flow_run_id.as_deref(),
        Some(flow_run_1.to_string().as_str())
    );
}

#[tokio::test]
async fn atomic_lifecycle_fencing_purges_policies_on_disable_revoke_and_uninstall() {
    let (store, kernel, actor_id, ws_a, _ws_b) = setup_store_and_workspaces().await;
    let instance_id = Uuid::now_v7();

    // 1. Disable purges runtime policy and invalidates in-process cache immediately
    let (inst_disable, _) = seed_active_privacy_plugin(&store, actor_id, "0.1.0").await;
    kernel
        .sync_workspace_policy(
            inst_disable,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    PluginRepository::update_desired_state(
        &store,
        &UpdatePluginDesiredStateInput {
            installation_id: inst_disable,
            desired_state: PluginDesiredState::Disabled,
            actor_user_id: actor_id,
        },
    )
    .await
    .expect("disable installation");

    assert!(
        store
            .list_active_privacy_masking_policies(ws_a)
            .await
            .unwrap()
            .is_empty(),
        "disabling plugin must atomically remove active policies"
    );
    assert!(
        !kernel.has_active_policies(ws_a).await.unwrap(),
        "in-process kernel cache must be invalidated immediately on disable"
    );

    // 2. Revoking `privacy.policy.sync` purges runtime policy and invalidates cache
    let (inst_revoke, snapshot) = seed_active_privacy_plugin(&store, actor_id, "0.2.0").await;
    kernel
        .sync_workspace_policy(
            inst_revoke,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    let auth_id = snapshot
        .authorizations
        .iter()
        .find(|a| a.permission == PRIVACY_POLICY_SYNC_PERMISSION)
        .expect("grant exists")
        .id;
    PluginContributionAuthorityRepository::revoke_contribution_authorization(
        &store,
        &RevokeContributionAuthorizationInput {
            installation_id: inst_revoke,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            authorization_id: auth_id,
            expected_revision: snapshot.revision,
            actor_user_id: actor_id,
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_contribution_authorization",
                Some(auth_id),
                "revoke",
                json!({}),
            ),
        },
    )
    .await
    .expect("revoke");

    assert!(store
        .list_active_privacy_masking_policies(ws_a)
        .await
        .unwrap()
        .is_empty());
    assert!(!kernel.has_active_policies(ws_a).await.unwrap());

    // 3. Uninstall purges runtime policy and invalidates cache
    let (inst_uninstall, _) = seed_active_privacy_plugin(&store, actor_id, "0.3.0").await;
    kernel
        .sync_workspace_policy(
            inst_uninstall,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    PluginRepository::commit_plugin_family_uninstall(
        &store,
        &CommitPluginFamilyUninstallInput {
            artifact_instances: vec![UpsertPluginArtifactInstanceInput {
                node_id: "test-node".to_string(),
                installation_id: inst_uninstall,
                local_version: None,
                local_checksum: None,
                local_path: None,
                package_path: None,
                manifest_fingerprint: None,
                artifact_status: PluginArtifactInstanceStatus::Missing,
                runtime_status: PluginRuntimeStatus::Inactive,
                availability_status: PluginAvailabilityStatus::ArtifactMissing,
                checked_at: OffsetDateTime::now_utc(),
                last_error: None,
                is_current: false,
            }],
            artifact_cleanups: vec![],
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_installation",
                Some(inst_uninstall),
                "uninstall",
                json!({}),
            ),
        },
    )
    .await
    .expect("commit uninstall");

    assert!(store
        .list_active_privacy_masking_policies(ws_a)
        .await
        .unwrap()
        .is_empty());
    assert!(!kernel.has_active_policies(ws_a).await.unwrap());
}

#[tokio::test]
async fn restore_policy_fences_persisted_run_and_conversation_payloads() {
    use control_plane::ports::{
        AppendRuntimeEventInput, ApplicationRunTraceNodeContentProjectionInput,
        ApplicationRunTraceNodeProjectionInput, CreateFlowRunInput, CreateNodeRunInput,
        ListApplicationRunConversationMessageItemsPageInput, OrchestrationRuntimeRepository,
        ReplaceApplicationRunTraceProjectionInput, UpdateFlowRunInput, UpdateNodeRunInput,
        UpsertCompiledPlanInput,
    };
    use control_plane_contracts::flow_run_title::build_flow_run_title;
    use domain::{
        FlowRunMode, FlowRunStatus, NodeRunStatus, RuntimeEventDurability, RuntimeEventLayer,
        RuntimeEventSource, RuntimeEventVisibility, RuntimeTrustLevel,
    };
    use extension_contracts::provider_contract::{ProviderInvocationResult, ProviderStreamEvent};
    use serde_json::Value;

    let (store, kernel, actor, workspace, _) = setup_store_and_workspaces().await;
    let (installation, _) = seed_active_privacy_plugin(&store, actor, "0.1.0").await;
    let app = create_privacy_fixture_app(&store, workspace, actor).await;
    let editor = store
        .get_or_create_editor_state(workspace, app.id, actor)
        .await
        .unwrap();
    let plan = store
        .upsert_compiled_plan(&UpsertCompiledPlanInput {
            actor_user_id: actor,
            flow_id: editor.flow.id,
            flow_draft_id: editor.draft.id,
            schema_version: "1flowbase.flow/v2".into(),
            document_hash: "privacy-fixture".into(),
            document_updated_at: editor.draft.updated_at,
            plan: json!({"topological_order":["node-start","node-llm"]}),
        })
        .await
        .unwrap();
    let instance = Uuid::new_v4();
    let started = OffsetDateTime::now_utc();
    let email = "pg-fixture-secret@acme.org";
    let long = format!("PG_LONG_{}_END", "Z".repeat(320));
    let secret_input = format!("{long} {email}\0tail");

    for restore_output in [false, true] {
        let mut policy = sample_policy_for_instance(instance, restore_output);
        policy.rules.push(PrivacyMaskingRuleDto {
            rule_id: "long".into(),
            name: "Long fixture".into(),
            kind: PrivacyMaskingRuleKind::CustomRegex,
            builtin_code: None,
            placeholder_kind: "LONG".into(),
            pattern: Some("PG_LONG_Z+_END".into()),
            enabled: true,
            priority: 10,
        });
        kernel
            .sync_workspace_policy(installation, workspace, Some(actor), &policy)
            .await
            .unwrap();
        let run = OrchestrationRuntimeRepository::create_flow_run(&store, &CreateFlowRunInput {
            application_run_log_context: None, actor_user_id: actor, application_id: app.id,
            flow_id: editor.flow.id, flow_draft_id: editor.draft.id, compiled_plan_id: plan.id,
            debug_session_id: String::new(), flow_schema_version: plan.schema_version.clone(),
            document_hash: plan.document_hash.clone(), run_mode: FlowRunMode::AssistantExecution,
            target_node_id: None, title: build_flow_run_title(None, &secret_input), status: FlowRunStatus::Running,
            input_payload: json!({
                "node-start":{"query":secret_input,"history":[{"role":"user","content":format!("previous {email}")}]},
                "__embedded_assistant_user_message":{"content":secret_input,"page_references":[]}
            }),
            started_at: started, api_key_id: None, publication_version_id: None,
            assistant_conversation_id: None, external_user: None, external_conversation_id: None,
            external_trace_id: None, compatibility_mode: None, idempotency_key: None,
        }).await.unwrap();
        let mut nodes = Vec::new();
        for (name, kind) in [("node-start", "start"), ("node-llm", "llm")] {
            nodes.push(
                store
                    .create_node_run(&CreateNodeRunInput {
                        flow_run_id: run.id,
                        node_id: name.into(),
                        node_type: kind.into(),
                        node_alias: name.into(),
                        status: NodeRunStatus::Running,
                        input_payload: json!({"id":secret_input}),
                        debug_payload: json!({"preexisting_snapshot":secret_input}),
                        started_at: started,
                    })
                    .await
                    .unwrap(),
            );
        }
        let trace_node_id = Uuid::new_v4();
        store
            .replace_application_run_trace_projection(&ReplaceApplicationRunTraceProjectionInput {
                flow_run_id: run.id,
                projection_version: 1,
                source_watermark: "privacy-fixture".into(),
                nodes: vec![ApplicationRunTraceNodeProjectionInput {
                    trace_node_id,
                    parent_trace_node_id: None,
                    stable_locator: "node-start".into(),
                    node_kind: "node_run".into(),
                    owner_kind: Some("node_run".into()),
                    owner_id: Some(nodes[0].id.to_string()),
                    order_key: "000001".into(),
                    node_id: Some("node-start".into()),
                    node_type: Some("start".into()),
                    node_mode: None,
                    node_alias: "Start".into(),
                    status: "running".into(),
                    started_at: started,
                    finished_at: None,
                    duration_ms: None,
                    metrics_payload: json!({}),
                    has_children: false,
                    child_count: 0,
                    has_content: true,
                    content_ref: None,
                    source_flow_run_id: None,
                    source_trace_node_id: None,
                    parent_callback_task_id: None,
                    parent_tool_call_id: None,
                    trace_relation_kind: None,
                }],
                contents: vec![ApplicationRunTraceNodeContentProjectionInput {
                    trace_node_id,
                    content_kind: "node_run".into(),
                    payload: json!({"input_payload":{"id":secret_input}}).into(),
                    source_refs: json!([]),
                }],
            })
            .await
            .unwrap();
        let start_event = store
            .append_runtime_event(&AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: Some(nodes[1].id),
                span_id: None,
                parent_span_id: None,
                event_type: "node_started".into(),
                layer: RuntimeEventLayer::RuntimeItem,
                source: RuntimeEventSource::Host,
                trust_level: RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({"input_payload":{"id":secret_input}}),
                visibility: RuntimeEventVisibility::Workspace,
                durability: RuntimeEventDurability::Durable,
            })
            .await
            .unwrap();
        let preview_event = store
            .append_runtime_event(&AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: Some(nodes[1].id),
                span_id: None,
                parent_span_id: None,
                event_type: "node_preview_completed".into(),
                layer: RuntimeEventLayer::Diagnostic,
                source: RuntimeEventSource::Host,
                trust_level: RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({"target_node_id":"node-llm", "resolved_inputs":{"prompt":secret_input}}),
                visibility: RuntimeEventVisibility::Workspace,
                durability: RuntimeEventDurability::Durable,
            })
            .await
            .unwrap();
        let original_event_body: Uuid = sqlx::query_scalar(
            "select observation_body_content_id from runtime_events where id=$1",
        )
        .bind(start_event.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        let context = PrivacyInvocationContext {
            workspace_id: workspace,
            flow_run_id: Some(run.id),
            application_id: Some(app.id),
            provider_instance_id: instance,
            provider_instance_name: "fixture".into(),
            model_id: "fixture".into(),
            protocol: "openai_chat".into(),
        };
        let mut input = ProviderInvocationInput {
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: secret_input.clone(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..Default::default()
        };
        let prepared = kernel
            .prepare_invocation(&context, &mut input)
            .await
            .unwrap()
            .unwrap();
        let outbound = &input.messages[0].content;
        assert!(!outbound.contains(email));
        assert!(!outbound.contains("PG_LONG_"));
        let check = |text: &str| {
            assert_eq!(
                text.contains(email),
                restore_output,
                "email in persisted copy: {text}"
            );
            assert_eq!(
                text.contains("PG_LONG_Z"),
                restore_output,
                "long secret in persisted copy: {text}"
            );
        };
        // The actual base columns and the NUL-triggered raw JSON sidecars must
        // have changed before there is an upstream response to persist.
        let row = sqlx::query_as::<_, (Value, Value, String)>(
            "select input_payload,raw_json_payloads,title from flow_runs where id=$1",
        )
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        check(&row.0.to_string());
        check(row.1["input_payload"].as_str().expect("lossless sidecar"));
        assert_eq!(row.2.contains("PG_LONG_Z"), restore_output);
        let sections = sqlx::query_as::<_, (Value, Value)>("select d.payload,d.raw_json_payloads from node_run_details d join node_runs n on n.id=d.node_run_id where n.flow_run_id=$1 and d.section in ('input_payload','debug_payload')")
            .bind(run.id).fetch_all(store.pool()).await.unwrap();
        assert_eq!(sections.len(), 4);
        for (body, raw) in sections {
            check(&body.to_string());
            check(&raw.to_string());
        }
        let (event, archived_body) = sqlx::query_as::<_, (Value, Uuid)>("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id),observation_body_content_id from runtime_events where id=$1")
            .bind(start_event.id).fetch_one(store.pool()).await.unwrap();
        check(&event.to_string());
        let preview = sqlx::query_as::<_, (Value, Value)>(
            "select payload,raw_json_payloads from runtime_events where id=$1",
        )
        .bind(preview_event.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(preview.0["target_node_id"], "node-llm");
        check(&preview.0.to_string());
        check(preview.1["payload"].as_str().expect("preview sidecar"));
        assert!(event["input_payload"]["id"]
            .as_str()
            .unwrap()
            .ends_with("\0tail"));
        assert_eq!(archived_body == original_event_body, restore_output);
        let old_body_exists: bool = sqlx::query_scalar(
            "select exists(select 1 from runtime_canonical_contents where id=$1)",
        )
        .bind(original_event_body)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(old_body_exists, restore_output);
        let trace = sqlx::query_as::<_, (Value, Value)>("select payload,raw_json_payloads from application_run_trace_node_contents where trace_node_id=$1")
            .bind(trace_node_id).fetch_one(store.pool()).await.unwrap();
        check(&trace.0.to_string());
        check(trace.1["payload"].as_str().unwrap());
        let before_answer = store
            .get_application_run_conversation_current_item(app.id, run.id)
            .await
            .unwrap()
            .unwrap();
        check(before_answer.query.as_deref().unwrap());

        // Simulate a provider echo, using the same completed-result restoration
        // and durable prompt projection that the real invoker consumes.
        let mut events = vec![ProviderStreamEvent::TextDelta {
            delta: outbound.clone(),
        }];
        let mut result = ProviderInvocationResult {
            final_content: Some(outbound.clone()),
            ..Default::default()
        };
        prepared.restore_completed_output_counting(&mut events, &mut result);
        let answer = result.final_content.unwrap();
        if restore_output {
            assert_eq!(answer, secret_input);
        }
        let prompt = prepared
            .persisted_input_override
            .as_ref()
            .map(|v| &v.messages[0].content)
            .unwrap_or(&secret_input);
        store
            .update_node_run(&UpdateNodeRunInput {
                node_run_id: nodes[1].id,
                status: NodeRunStatus::Succeeded,
                output_payload: json!({"answer":answer}),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({"llm":{"prompt_snapshot":prompt}}),
                finished_at: Some(started),
            })
            .await
            .unwrap();
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status: FlowRunStatus::Succeeded,
                output_payload: json!({"answer":answer}),
                error_payload: None,
                finished_at: Some(started),
            })
            .await
            .unwrap();
        kernel.finalize_invocation(&prepared, None).await;

        let detail = store
            .get_application_run_detail(app.id, run.id)
            .await
            .unwrap()
            .unwrap();
        check(&detail.flow_run.output_payload.to_string());
        let (debug, output) = sqlx::query_as::<_, (Value, Value)>(
            "select debug_payload,output_payload from node_run_records where id=$1",
        )
        .bind(nodes[1].id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        check(&debug.to_string());
        check(&output.to_string());
        let current = store
            .get_application_run_conversation_current_item(app.id, run.id)
            .await
            .unwrap()
            .unwrap();
        check(current.query.as_deref().unwrap());
        check(current.answer.as_deref().unwrap());
        let page = store
            .list_application_run_conversation_message_items_page(
                app.id,
                run.id,
                ListApplicationRunConversationMessageItemsPageInput {
                    before_sequence: None,
                    after_sequence: None,
                    limit: 20,
                },
            )
            .await
            .unwrap();
        // The task page intentionally pages turns, not the individual imported
        // history rows. Check both the task consumer and the retained history projection.
        if !restore_output {
            assert!(!format!("{page:?}").contains(email));
        }
        let history: String = sqlx::query_scalar(
            "select content from application_run_conversation_message_items where flow_run_id=$1 and role='user' and not is_current and content like 'previous %'",
        ).bind(run.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(history.contains(email), restore_output);
        let summaries: Value = sqlx::query_scalar(
            "select to_jsonb(s) from application_run_log_summaries s where flow_run_id=$1",
        )
        .bind(run.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(summaries.to_string().contains("PG_LONG_Z"), restore_output);
    }
}

async fn create_privacy_fixture_app(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
) -> domain::ApplicationRecord {
    use control_plane::ports::CreateApplicationInput;
    store
        .create_application(&CreateApplicationInput {
            actor_user_id,
            workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Privacy fixture".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn sync_policy_rejects_foreign_workspace_subjects_without_writing() {
    use control_plane::ports::{
        CreateModelProviderInstanceInput, CreatePluginAssignmentInput, ModelProviderRepository,
    };
    use extension_contracts::{
        ContributionId, ManagedContributionSubject, ManagedInstallationId, ManagedWorkspaceId,
    };

    let (store, kernel, actor, workspace, foreign_workspace) = setup_store_and_workspaces().await;
    let (installation, _) = seed_active_privacy_plugin(&store, actor, "0.1.0").await;
    let mut subjects_by_workspace = Vec::new();
    for scope in [workspace, foreign_workspace] {
        store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: installation,
                workspace_id: scope,
                provider_code: "privacy-masking".into(),
                actor_user_id: actor,
            })
            .await
            .unwrap();
        let instance = store
            .create_instance(&CreateModelProviderInstanceInput {
                instance_id: Uuid::now_v7(),
                workspace_id: scope,
                installation_id: installation,
                provider_code: "privacy-masking".into(),
                protocol: "openai_chat".into(),
                display_name: "Fixture provider".into(),
                status: domain::ModelProviderInstanceStatus::Ready,
                config_json: json!({}),
                configured_models: vec![],
                enabled_model_ids: vec![],
                included_in_main: None,
                created_by: actor,
            })
            .await
            .unwrap();
        let app = create_privacy_fixture_app(&store, scope, actor).await;
        subjects_by_workspace.push((instance.id, app.id));
    }
    let [(instance, app), (foreign_instance, foreign_app)] =
        subjects_by_workspace.try_into().unwrap();
    let store = Arc::new(store);
    let service = ManagedPrivacyMaskingService::new(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        kernel,
    );
    let binding = PrivacyMaskingBinding {
        subject: ManagedContributionSubject::new(
            ManagedInstallationId::new(installation.to_string()).unwrap(),
            ManagedWorkspaceId::new(domain::SYSTEM_SCOPE_ID.to_string()).unwrap(),
            ContributionId::new("privacy-masking.service").unwrap(),
        ),
        publisher_namespace: "1flowbase".into(),
        plugin_code: "privacy-masking".into(),
        plugin_version: "0.1.0".into(),
        can_read_subjects: false,
        can_sync_policy: true,
        can_read_logs: false,
        deadline_unix_ms: (OffsetDateTime::now_utc().unix_timestamp() + 60) * 1000,
        caller: Some(PrivacyMaskingCallerDto {
            user_id: actor.to_string(),
            tenant_id: store.upsert_root_tenant().await.unwrap().id.to_string(),
            current_workspace_id: workspace.to_string(),
            is_root: true,
            effective_display_role: "root".into(),
        }),
    };
    let mut local = sample_policy_for_instance(instance, true);
    let mut flow = local.subject_bindings[0].clone();
    flow.binding_id = "flow-fixture".into();
    flow.subject_kind = PrivacyMaskingSubjectKind::AgentFlow;
    flow.subject_id = app.to_string();
    local.subject_bindings.push(flow);

    for (index, id) in [(0, foreign_instance), (1, foreign_app)] {
        let mut policy = local.clone();
        policy.subject_bindings[index].subject_id = id.to_string();
        // Disabled entries still must not bind subjects owned by another workspace.
        policy.subject_bindings[index].enabled = index == 0;
        let error = service
            .execute(&binding, &PrivacyMaskingRequest::SyncPolicy { policy })
            .await
            .expect_err("foreign subject must be rejected");
        assert_eq!(error.code, "privacy_subject_not_in_workspace");
        assert_eq!(error.status, Some(400));
        let count: i64 =
            sqlx::query_scalar("select count(*) from privacy_masking_runtime_policies")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(count, 0, "must not partially persist the valid subject");
    }

    let result = service
        .execute(
            &binding,
            &PrivacyMaskingRequest::SyncPolicy {
                policy: local.clone(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        result,
        PrivacyMaskingResponse::PolicySynced {
            policy_version: 1,
            active_subject_count: 2,
            ..
        }
    ));
    let persisted = store
        .list_active_privacy_masking_policies(workspace)
        .await
        .unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].policy, local);
}

#[tokio::test]
async fn managed_version_switch_atomically_purges_policies_and_invalidates_cache() {
    use control_plane::ports::ManagedInstallationSwitch;
    use control_plane_test_support::{
        managed_manifest_fixture, validate_installed_managed_manifest,
    };

    let (store, kernel, actor, workspace, other_workspace) = setup_store_and_workspaces().await;
    let (current, _) = seed_active_privacy_plugin(&store, actor, "0.1.0").await;
    let (target, _) =
        seed_privacy_plugin(&store, actor, "0.2.0", PluginDesiredState::Disabled).await;
    let target = store.get_installation(target).await.unwrap().unwrap();
    let raw = managed_manifest_fixture("privacy-masking", &target.plugin_version);
    let fingerprint =
        validate_installed_managed_manifest(&target, &raw).expect("valid fixture manifest");
    let root = std::env::temp_dir().join(format!("privacy-version-switch-{}", Uuid::now_v7()));
    tokio::fs::create_dir_all(&root).await.unwrap();
    tokio::fs::write(root.join("manifest.yaml"), raw)
        .await
        .unwrap();
    tokio::fs::write(
        root.join(".1flowbase-artifact.json"),
        json!({
            "plugin_id": target.plugin_id, "version": target.plugin_version,
            "checksum": target.expected_checksum, "manifest_fingerprint": fingerprint,
        })
        .to_string(),
    )
    .await
    .unwrap();
    store
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "test-node".into(),
            installation_id: target.id,
            local_version: Some(target.plugin_version),
            local_checksum: None,
            local_path: Some(root.to_string_lossy().into_owned()),
            package_path: None,
            manifest_fingerprint: Some(fingerprint),
            artifact_status: PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Inactive,
            availability_status: PluginAvailabilityStatus::Disabled,
            checked_at: OffsetDateTime::now_utc(),
            last_error: None,
            is_current: false,
        })
        .await
        .unwrap();
    for scope in [workspace, other_workspace] {
        kernel
            .sync_workspace_policy(
                current,
                scope,
                Some(actor),
                &sample_policy_for_instance(Uuid::now_v7(), true),
            )
            .await
            .unwrap();
        assert!(kernel.has_active_policies(scope).await.unwrap());
    }
    let switch = ManagedInstallationSwitch {
        validate_manifest: validate_installed_managed_manifest,
        workspace_id: domain::SYSTEM_SCOPE_ID,
        current_installation_id: current,
        target_installation_id: target.id,
        node_id: "test-node".into(),
        installation_ids: vec![current, target.id],
    };
    let event = || {
        audit_log(
            None,
            Some(actor),
            "plugin_installation",
            Some(target.id),
            "plugin.version_switched",
            json!({}),
        )
    };
    let policy_count = || {
        sqlx::query_scalar::<_, i64>(
            "select count(*) from privacy_masking_runtime_policies where installation_id=$1",
        )
        .bind(current)
        .fetch_one(store.pool())
    };
    // A duplicate audit ID fails after the policy DELETE and exercises transaction rollback.
    let duplicate = event();
    sqlx::query("insert into audit_logs (id,target_type,target_id,event_code,payload) values ($1,$2,$3,$4,$5)")
        .bind(duplicate.id).bind(&duplicate.target_type).bind(duplicate.target_id).bind(&duplicate.event_code).bind(&duplicate.payload)
        .execute(store.pool()).await.unwrap();
    let error = store
        .lock_managed_installation_switch(&switch)
        .await
        .unwrap()
        .commit(duplicate)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<sqlx::Error>()
            .and_then(sqlx::Error::as_database_error)
            .and_then(|error| error.code())
            .as_deref(),
        Some("23505")
    );
    assert_eq!(policy_count().await.unwrap(), 2);
    assert_eq!(
        store
            .get_installation(current)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
    assert_eq!(
        store
            .get_installation(target.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::Disabled
    );
    for scope in [workspace, other_workspace] {
        assert!(kernel.has_active_policies(scope).await.unwrap());
    }

    store
        .lock_managed_installation_switch(&switch)
        .await
        .unwrap()
        .commit(event())
        .await
        .unwrap();
    tokio::fs::remove_dir_all(&root).await.unwrap();
    assert_eq!(
        policy_count().await.unwrap(),
        0,
        "retirement must delete the rows, not merely make them inactive"
    );
    assert_eq!(
        store
            .get_installation(current)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::Disabled
    );
    assert_eq!(
        store
            .get_installation(target.id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::ActiveRequested
    );
    for scope in [workspace, other_workspace] {
        assert!(!kernel.has_active_policies(scope).await.unwrap());
    }
}

#[tokio::test]
async fn debug_run_snapshot_and_legacy_event_sidecars_follow_restore_output() {
    use control_plane::orchestration_runtime::{
        OrchestrationRuntimeService, StartFlowDebugRunCommand,
    };
    use control_plane::ports::{
        AppendRunEventInput, CreateNodeRunInput, OrchestrationRuntimeRepository, UpdateFlowRunInput,
    };
    use domain::{FlowRunMode, FlowRunStatus, NodeRunStatus};
    use serde_json::Value;

    let (store, kernel, actor, workspace, _) = setup_store_and_workspaces().await;
    let (installation, _) = seed_active_privacy_plugin(&store, actor, "0.1.0").await;
    let app = create_privacy_fixture_app(&store, workspace, actor).await;
    let editor = store
        .get_or_create_editor_state(workspace, app.id, actor)
        .await
        .unwrap();
    sqlx::query("insert into roles (id, scope_kind, scope_id, code, name) values ($1, 'system', $2, 'root', 'Root') on conflict do nothing")
        .bind(Uuid::now_v7()).bind(domain::SYSTEM_SCOPE_ID).execute(store.pool()).await.unwrap();
    sqlx::query("insert into user_role_bindings (id, user_id, role_id, scope_id) select $1, $2, id, $3 from roles where code='root' and scope_kind='system' and scope_id=$3 on conflict do nothing")
        .bind(Uuid::now_v7()).bind(actor).bind(domain::SYSTEM_SCOPE_ID).execute(store.pool()).await.unwrap();
    let runtime = OrchestrationRuntimeService::new(
        store.clone(),
        debug_fixture::NoProvider,
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "privacy-fixture-key",
        Arc::new(storage_ephemeral::MemoryProviderTransportStore::new(
            time::Duration::minutes(5),
        )),
        false,
    );
    let document = json!({
        "schemaVersion":"1flowbase.flow/v2",
        "meta":{"flowId":editor.flow.id,"name":"Debug privacy fixture","description":"","tags":[]},
        "graph":{
            "nodes":[
                {"id":"start","type":"start","alias":"Start","description":"","containerId":null,"position":{"x":0,"y":0},"configVersion":1,"config":{},"bindings":{},"outputs":[]},
                {"id":"answer","type":"answer","alias":"Answer","description":"","containerId":null,"position":{"x":240,"y":0},"configVersion":1,"config":{},"bindings":{"answer_template":{"kind":"templated_text","value":"fixture"}},"outputs":[{"key":"answer","title":"Answer","valueType":"string"}]}
            ],
            "edges":[{"id":"edge","source":"start","target":"answer","sourceHandle":null,"targetHandle":null,"containerId":null,"points":[]}]
        },
        "editor":{"viewport":{"x":0,"y":0,"zoom":1},"annotations":[],"activeContainerPath":[]}
    });
    let instance = Uuid::now_v7();
    let email = "private-debug-fixture@acme.org";
    let stage_one_secret = "DEBUG_FLOW_SECRET";
    let input = format!("{stage_one_secret} {email}\0tail"); // Force the production lossless JSON sidecar.
    for restore_output in [false, true] {
        let mut policy = sample_policy_for_instance(instance, restore_output);
        let mut flow = policy.subject_bindings[0].clone();
        flow.binding_id = "debug-flow".into();
        flow.subject_kind = PrivacyMaskingSubjectKind::AgentFlow;
        flow.subject_id = app.id.to_string();
        flow.rule_mode = PrivacyMaskingRuleMode::SelectedRules;
        flow.rule_ids = vec!["debug_secret".into()];
        policy.subject_bindings.push(flow);
        policy.rules.push(PrivacyMaskingRuleDto {
            rule_id: "debug_secret".into(),
            name: "Debug fixture".into(),
            kind: PrivacyMaskingRuleKind::CustomRegex,
            builtin_code: None,
            placeholder_kind: "DEBUG_SECRET".into(),
            pattern: Some(stage_one_secret.into()),
            enabled: true,
            priority: 10,
        });
        kernel
            .sync_workspace_policy(installation, workspace, Some(actor), &policy)
            .await
            .unwrap();
        let started = runtime
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: actor,
                application_id: app.id,
                input_payload: json!({"query": input}),
                document_snapshot: Some(document.clone()),
                debug_session_id: None,
            })
            .await
            .unwrap();
        let run = started.flow_run;
        assert_eq!(run.run_mode, FlowRunMode::DebugFlowRun);
        assert_eq!(started.events[0].event_type, "flow_run_started");
        let node = store
            .create_node_run(&CreateNodeRunInput {
                flow_run_id: run.id,
                node_id: "start".into(),
                node_type: "start".into(),
                node_alias: "Start".into(),
                status: NodeRunStatus::Running,
                input_payload: json!({"query": input}),
                debug_payload: json!({}),
                started_at: run.started_at,
            })
            .await
            .unwrap();
        // Debug startup wrote the first row. Seed legacy node events via the real
        // event writer; current node observation also has a runtime_events projection.
        let events = [
            (
                "flow_run_started",
                None,
                json!({"run_mode": "debug_flow_run", "input_payload": {"query": input}}),
                "input_payload",
                "query",
            ),
            (
                "node_started",
                Some(node.id),
                json!({"node_id": "start", "input_payload": {"prompt": input}}),
                "input_payload",
                "prompt",
            ),
            (
                "node_finished",
                Some(node.id),
                json!({"node_id": "start", "output_payload": {"answer": input}}),
                "output_payload",
                "answer",
            ),
            (
                "flow_run_resumed",
                None,
                json!({"response_payload": {"query": input}}),
                "response_payload",
                "query",
            ),
        ];
        for (event_type, node_run_id, payload, _, _) in &events[1..] {
            store
                .append_run_event(&AppendRunEventInput {
                    flow_run_id: run.id,
                    node_run_id: *node_run_id,
                    event_type: (*event_type).into(),
                    payload: payload.clone(),
                })
                .await
                .unwrap();
        }
        let read_events = || {
            sqlx::query_as::<_, (Uuid, i64, String, Value, Value)>(
            "select id, sequence, event_type, payload, raw_json_payloads from flow_run_events where flow_run_id=$1 and event_type <> 'gateway_billing_session_reserved' order by sequence",
        ).bind(run.id).fetch_all(store.pool())
        };
        let before = read_events().await.unwrap();
        assert_eq!(before.len(), events.len());
        for (_, _, _, payload, sidecar) in &before {
            assert!(payload.to_string().contains(email));
            assert!(sidecar["payload"].as_str().unwrap().contains(email));
        }

        let mut invocation = ProviderInvocationInput {
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                // NUL exercises event storage; the provider prompt only needs the two secrets.
                content: format!("{stage_one_secret} {email}"),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..Default::default()
        };
        let prepared = kernel
            .prepare_invocation(
                &PrivacyInvocationContext {
                    workspace_id: workspace,
                    flow_run_id: Some(run.id),
                    application_id: Some(app.id),
                    provider_instance_id: instance,
                    provider_instance_name: "fixture".into(),
                    model_id: "fixture".into(),
                    protocol: "openai_chat".into(),
                },
                &mut invocation,
            )
            .await
            .unwrap()
            .expect("policy must run");
        assert!(!invocation.messages[0].content.contains(email));
        assert!(!invocation.messages[0].content.contains(stage_one_secret));
        kernel.finalize_invocation(&prepared, None).await;
        let logs = store
            .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
                workspace_id: workspace,
                flow_run_id: Some(run.id),
                limit: 10,
                ..Default::default()
            })
            .await
            .unwrap()
            .items;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[1].subject_kind, PrivacyMaskingSubjectKind::AgentFlow);
        assert_eq!(
            logs[0].subject_kind,
            PrivacyMaskingSubjectKind::ModelProviderInstance
        );
        let audit = serde_json::to_string(&logs).unwrap();
        assert!(!audit.contains(email) && !audit.contains(stage_one_secret));
        store
            .update_flow_run(&UpdateFlowRunInput {
                flow_run_id: run.id,
                status: FlowRunStatus::Succeeded,
                output_payload: json!({}),
                error_payload: None,
                finished_at: Some(OffsetDateTime::now_utc()),
            })
            .await
            .unwrap();

        // The debug-snapshot handler uses this durable detail reader, which reconstructs lossless JSON.
        let detail = store
            .get_application_run_detail(app.id, run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
        let after = read_events().await.unwrap();
        assert_eq!(after.len(), events.len());
        assert_eq!(detail.events.len(), events.len() + 1);
        let billing = detail
            .events
            .iter()
            .find(|event| event.event_type == "gateway_billing_session_reserved")
            .unwrap();
        assert_eq!(billing.payload, started.events[1].payload);
        for (index, (kind, node_run_id, original, field, content_key)) in events.iter().enumerate()
        {
            let (id, sequence, event_type, payload, sidecar) = &after[index];
            assert_eq!((*id, *sequence), (before[index].0, before[index].1));
            assert_eq!(event_type, kind);
            assert_eq!(payload["run_mode"], original["run_mode"]);
            assert_eq!(payload["node_id"], original["node_id"]);
            assert_eq!(
                payload.to_string().contains(email),
                restore_output,
                "{kind} JSONB"
            );
            assert_eq!(
                sidecar["payload"].as_str().unwrap().contains(email),
                restore_output,
                "{kind} sidecar"
            );
            assert_eq!(
                payload.to_string().contains(stage_one_secret),
                restore_output
            );
            assert_eq!(
                sidecar["payload"]
                    .as_str()
                    .unwrap()
                    .contains(stage_one_secret),
                restore_output
            );
            let event = detail.events.iter().find(|event| event.id == *id).unwrap();
            assert_eq!((event.id, event.node_run_id), (*id, *node_run_id));
            assert_eq!(
                event.payload.to_string().contains(email),
                restore_output,
                "{kind} snapshot"
            );
            assert_eq!(
                event.payload.to_string().contains(stage_one_secret),
                restore_output
            );
            assert!(event.payload[*field][*content_key]
                .as_str()
                .unwrap()
                .ends_with("\0tail"));
            if restore_output {
                assert_eq!(&event.payload, original);
                assert_eq!((payload, sidecar), (&before[index].3, &before[index].4));
            }
        }
    }
}
