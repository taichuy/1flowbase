use super::*;
use crate::capability_plugin_runtime::CapabilityExecutionOutput;
use std::collections::BTreeSet;

fn credit_command_runtime(
    installation_id: Uuid,
) -> orchestration_runtime::compiled_plan::CompiledPluginRuntime {
    orchestration_runtime::compiled_plan::CompiledPluginRuntime {
        installation_id,
        plugin_unique_identifier: "fixture_capability".to_string(),
        package_id: "fixture_capability@0.1.0".to_string(),
        plugin_id: "fixture_capability@0.1.0".to_string(),
        plugin_version: "0.1.0".to_string(),
        contribution_code: "fixture_action".to_string(),
        node_shell: "action".to_string(),
        schema_version: "1flowbase.node-contribution/v2".to_string(),
        contribution_checksum: "sha256:contribution".to_string(),
        compiled_contribution_hash: "sha256:compiled".to_string(),
        output_schema_snapshot: Vec::new(),
        side_effect_policy: "external_write".to_string(),
    }
}

fn credit_command_request(idempotency_key: &str) -> crate::ports::PluginCreditCommandRequest {
    crate::ports::PluginCreditCommandRequest {
        command: "grant".to_string(),
        user_id: Uuid::now_v7(),
        amount: "12.50".to_string(),
        credit_unit: "USD".to_string(),
        reason: "fixture grant".to_string(),
        source_type: Some("test_batch".to_string()),
        source_id: Some("WP-1894-02G4d".to_string()),
        idempotency_key: idempotency_key.to_string(),
        billing_session_id: None,
        provider_invocation_id: None,
        pricing_rule_id: None,
        flow_run_id: None,
        reservation_expires_at: None,
        price_snapshot: Value::Null,
        usage_snapshot: Value::Null,
        metadata: json!({"acceptance": "AC-02G4d"}),
    }
}

fn credit_command_invoker(
    repository: test_support::InMemoryOrchestrationRuntimeRepository,
    output: CapabilityExecutionOutput,
) -> RuntimeProviderInvoker<
    test_support::InMemoryOrchestrationRuntimeRepository,
    test_support::InMemoryProviderRuntime,
> {
    RuntimeProviderInvoker {
        repository,
        runtime: test_support::InMemoryProviderRuntime::with_capability_output(output),
        workspace_id: Uuid::nil(),
        provider_secret_master_key: "test-master-key".to_string(),
        live_provider_events: None,
        runtime_event_stream: None,
        flow_run_id: None,
        active_node_id: None,
        active_node_run_id: None,
        api_node_id: Some("local:test".to_string()),
        provider_install_root: Some(std::env::temp_dir()),
        flow_execution_context: None,
        answer_presentation: None,
        provider_transport_payload: None,
        provider_transport_store: None,
        provider_continuation: None,
        model_pricing_cache_store: None,
    }
}

#[tokio::test]
async fn ac_02g4d_verified_plugin_credit_command_uses_application_service_and_is_idempotent() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let installation_id = repository.verify_fixture_capability();
    let request = credit_command_request("plugin-credit-idempotent");
    let output = CapabilityExecutionOutput {
        output_payload: json!({
            "answer": "preserved",
            "_1flowbase_credit_command": serde_json::to_value(&request).unwrap(),
        }),
        granted_credit_permissions: BTreeSet::from(["credit.grant".to_string()]),
    };
    let invoker = credit_command_invoker(repository, output);
    let runtime = credit_command_runtime(installation_id);

    let first = orchestration_runtime::execution_engine::CapabilityInvoker::invoke_capability_node(
        &invoker,
        &runtime,
        json!({}),
        json!({}),
    )
    .await
    .unwrap();
    let second =
        orchestration_runtime::execution_engine::CapabilityInvoker::invoke_capability_node(
            &invoker,
            &runtime,
            json!({}),
            json!({}),
        )
        .await
        .unwrap();

    assert_eq!(first.output_payload["answer"], json!("preserved"));
    assert!(first
        .output_payload
        .get("_1flowbase_credit_command")
        .is_none());
    let first_transaction = &first.output_payload["_1flowbase_credit_result"]["transaction"];
    let second_transaction = &second.output_payload["_1flowbase_credit_result"]["transaction"];
    assert_eq!(
        first_transaction["transaction_id"],
        second_transaction["transaction_id"]
    );
    assert_eq!(
        first_transaction["idempotency_key"],
        json!(request.idempotency_key)
    );
    assert_eq!(
        first_transaction["actor_plugin_id"],
        json!("fixture_capability@0.1.0")
    );
    assert!(first_transaction["actor_user_id"].is_null());
}

#[tokio::test]
async fn ac_02g4d_credit_permission_denial_records_rejection_audit() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let installation_id = repository.verify_fixture_capability();
    let request = credit_command_request("plugin-credit-denied");
    let output = CapabilityExecutionOutput {
        output_payload: json!({
            "_1flowbase_credit_command": serde_json::to_value(request).unwrap(),
        }),
        granted_credit_permissions: BTreeSet::new(),
    };
    let invoker = credit_command_invoker(repository.clone(), output);

    let error = orchestration_runtime::execution_engine::CapabilityInvoker::invoke_capability_node(
        &invoker,
        &credit_command_runtime(installation_id),
        json!({}),
        json!({}),
    )
    .await
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("credit_command_permission_denied"));
    assert_eq!(
        repository.plugin_credit_rejections(),
        vec![(
            Uuid::nil(),
            "fixture_capability@0.1.0".to_string(),
            "grant".to_string(),
            "credit_command_permission_denied".to_string(),
            "plugin-credit-denied".to_string(),
        )]
    );
}
