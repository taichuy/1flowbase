use super::*;
use orchestration_runtime::execution_engine::ProviderInvoker;

type SessionTestInvoker = RuntimeProviderInvoker<
    test_support::InMemoryOrchestrationRuntimeRepository,
    test_support::InMemoryProviderRuntime,
>;

fn session_invoker() -> SessionTestInvoker {
    RuntimeProviderInvoker {
        repository: test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]),
        runtime: test_support::InMemoryProviderRuntime::default(),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".into(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        provider_session_scope: Some("session-fixture".into()),
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".into()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        transport_connection_scope_override: None,
        observation_context: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        response_round_id: None,
        native_user_messages_digest: None,
        native_history: None,
        model_pricing_cache_store: None,
    }
}

fn logical_runtime() -> CompiledLlmRuntime {
    compiled_llm_runtime("", "fixture_provider")
}

#[tokio::test]
async fn provider_session_bound_main_instance_status_and_exclusion_errors_are_not_filtered_into_another_owner(
) {
    let invoker = session_invoker();
    let (owner, backup) = invoker.repository.seed_included_provider_instances();
    let runtime = logical_runtime();
    invoker
        .bind_session_llm_provider(&runtime, "node-llm", &owner.to_string())
        .await
        .unwrap();
    invoker
        .repository
        .set_instance_status(owner, domain::ModelProviderInstanceStatus::Disabled);
    let error = invoker
        .resolve_main_llm_routing(&runtime, "node-llm")
        .await
        .err()
        .unwrap();
    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_instance_not_ready"),
    );
    invoker
        .repository
        .set_instance_status(owner, domain::ModelProviderInstanceStatus::Ready);
    invoker.repository.set_main_model_routing_policy(
        "fixture_provider",
        "gpt-5.4-mini",
        domain::ModelProviderDistributionRule::None,
        vec![backup, owner],
        vec![owner],
    );
    let error = invoker
        .resolve_main_llm_routing(&runtime, "node-llm")
        .await
        .err()
        .unwrap();
    assert_control_plane_error(
        error,
        ControlPlaneError::Conflict("provider_session_owner_ineligible"),
    );
    assert_eq!(
        invoker
            .get_session_llm_provider(&runtime, "node-llm")
            .await
            .unwrap(),
        Some(owner.to_string())
    );
}

#[tokio::test]
async fn provider_session_explicit_main_distribution_ignores_original_binding_and_none_restores_it()
{
    let invoker = session_invoker();
    let (owner, backup) = invoker.repository.seed_included_provider_instances();
    let runtime = logical_runtime();
    invoker
        .bind_session_llm_provider(&runtime, "node-llm", &owner.to_string())
        .await
        .unwrap();
    invoker.repository.set_main_model_routing_policy(
        "fixture_provider",
        "gpt-5.4-mini",
        domain::ModelProviderDistributionRule::RetryRoundRobin,
        vec![backup, owner],
        vec![],
    );
    let explicit = invoker
        .resolve_main_llm_routing(&runtime, "node-llm")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        explicit.candidates[0].runtime.provider_instance_id,
        backup.to_string()
    );
    assert_eq!(explicit.candidates.len(), 2);
    invoker
        .bind_session_llm_provider(&runtime, "node-llm", &backup.to_string())
        .await
        .unwrap();
    invoker.repository.set_main_model_routing_policy(
        "fixture_provider",
        "gpt-5.4-mini",
        domain::ModelProviderDistributionRule::None,
        vec![backup, owner],
        vec![],
    );
    let default = invoker
        .resolve_main_llm_routing(&runtime, "node-llm")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(default.candidates.len(), 1);
    assert_eq!(
        default.candidates[0].runtime.provider_instance_id,
        owner.to_string()
    );
}

#[test]
fn provider_session_binding_scope_ignores_queue_candidate_order_but_separates_node_model_and_pool()
{
    let invoker = session_invoker();
    let mut runtime = logical_runtime();
    runtime.provider_instance_id = "first-a".into();
    runtime.routing = Some(orchestration_runtime::compiled_plan::CompiledLlmRouting {
        routing_mode: orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue,
        fixed_model_target: None,
        queue_template_id: Some("queue-a".into()),
        queue_snapshot_id: None,
        queue_targets: vec![],
        distribution_rule: Default::default(),
        distribution_key: None,
        context_policy: json!({}),
        stream_policy: json!({}),
    });
    let scope = invoker.session_provider_scope(&runtime, "node-a").unwrap();
    runtime.provider_instance_id = "first-b".into();
    runtime.routing.as_mut().unwrap().distribution_rule =
        orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin;
    assert_eq!(
        invoker.session_provider_scope(&runtime, "node-a").unwrap(),
        scope
    );
    assert_ne!(
        invoker.session_provider_scope(&runtime, "node-b").unwrap(),
        scope
    );
    runtime.model = "different-model".into();
    assert_ne!(
        invoker.session_provider_scope(&runtime, "node-a").unwrap(),
        scope
    );
    runtime.model = "gpt-5.4-mini".into();
    runtime.routing.as_mut().unwrap().queue_template_id = Some("queue-b".into());
    assert_ne!(
        invoker.session_provider_scope(&runtime, "node-a").unwrap(),
        scope
    );
    assert!(invoker.session_provider_scope(&runtime, "").is_none());
}

fn session_run() -> domain::FlowRunRecord {
    let now = OffsetDateTime::now_utc();
    domain::FlowRunRecord {
        id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: String::new(),
        flow_schema_version: "1flowbase.flow/v2".into(),
        document_hash: String::new(),
        run_mode: domain::FlowRunMode::PublishedApiRun,
        target_node_id: None,
        title: String::new(),
        status: domain::FlowRunStatus::Running,
        input_payload: json!({"sys":{"provider_session_identity":"sealed-session"}}),
        output_payload: json!({}),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: None,
        api_key_id: Some(Uuid::now_v7()),
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: now,
        finished_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn provider_session_scope_is_authenticated_isolated_and_absent_without_explicit_identity() {
    let invoker = session_invoker();
    let run = session_run();
    let runtime = logical_runtime();
    let scope = invoker
        .with_provider_session_scope(&run)
        .session_provider_scope(&runtime, "node-a")
        .unwrap();
    let mut another_round = run.clone();
    another_round.id = Uuid::now_v7();
    another_round.draft_id = Uuid::now_v7();
    assert_eq!(
        invoker
            .with_provider_session_scope(&another_round)
            .session_provider_scope(&runtime, "node-a")
            .unwrap(),
        scope
    );
    for field in ["application_id", "flow_id", "created_by", "api_key_id"] {
        let mut altered = serde_json::to_value(&run).unwrap();
        altered[field] = json!(Uuid::now_v7());
        let altered: domain::FlowRunRecord = serde_json::from_value(altered).unwrap();
        assert_ne!(
            invoker
                .with_provider_session_scope(&altered)
                .session_provider_scope(&runtime, "node-a")
                .unwrap(),
            scope,
            "{field}"
        );
    }
    let mut other_workspace = invoker.clone();
    other_workspace.workspace_id = Uuid::now_v7();
    assert_ne!(
        other_workspace
            .with_provider_session_scope(&run)
            .session_provider_scope(&runtime, "node-a")
            .unwrap(),
        scope
    );
    let mut no_identity = run.clone();
    no_identity.input_payload = json!({});
    assert!(invoker
        .with_provider_session_scope(&no_identity)
        .session_provider_scope(&runtime, "node-a")
        .is_none());
    no_identity.external_conversation_id = Some("existing-conversation".into());
    assert!(invoker
        .with_provider_session_scope(&no_identity)
        .session_provider_scope(&runtime, "node-a")
        .is_some());
}
