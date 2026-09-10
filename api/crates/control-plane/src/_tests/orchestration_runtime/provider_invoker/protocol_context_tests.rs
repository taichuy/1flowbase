use super::*;
use serde_json::json;

// #2021 AC-001: a generated workflow is business data, including its MCP IDs.
#[test]
fn ordinary_json_is_not_an_embedded_protocol_context() {
    for value in [
        json!(["1flowbase"]),
        json!(r#"["1flowbase"]"#),
        json!({"document": {"graph": {"nodes": [{
            "config": {"mcp_instance_ids": ["1flowbase"]}
        }]}}}),
        json!(["node-start", "protocol_context"]),
        json!([]),
        json!({"source_protocol": "business-label", "description": "ordinary data"}),
    ] {
        let mut contexts = Vec::new();
        collect_embedded_protocol_contexts(&value, &mut Vec::new(), &mut contexts);
        assert!(contexts.is_empty(), "ordinary JSON misclassified: {value}");
    }
}

// #2021 AC-001: both object and serialized-object protections remain active.
#[test]
fn embedded_protocol_context_objects_remain_identifiable() {
    let envelope = json!({"source_protocol": "openai_chat", "body": {"seed": 42}});
    for (value, serialized) in [
        (envelope.clone(), false),
        (json!(envelope.to_string()), true),
    ] {
        let payload = json!({"nested": [value]});
        let mut contexts = Vec::new();
        collect_embedded_protocol_contexts(&payload, &mut Vec::new(), &mut contexts);
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].serialized_string, serialized);
        assert_eq!(
            value_at_path(&payload, &contexts[0].path),
            payload.pointer("/nested/0")
        );
    }
}

// #2021 AC-001/002: exercise the actual service-to-invoker assembly and sealing
// path, not just the classifier. The same store must survive each projection.
#[tokio::test]
async fn service_preserves_business_json_and_protects_scoped_contexts() {
    use crate::orchestration_runtime::{
        provider_transport::tests::TestProviderTransportStore,
        test_support::{InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime},
        OrchestrationRuntimeService,
    };
    let service = OrchestrationRuntimeService::new(
        InMemoryOrchestrationRuntimeRepository::with_permissions(Vec::new()),
        InMemoryProviderRuntime::default(),
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "test-master-key",
        Arc::new(TestProviderTransportStore::default()),
    );
    let invoker = service
        .runtime_invoker(Uuid::nil())
        .for_flow_run(Uuid::now_v7());
    let business = json!({"graph": {"nodes": [{"config": {"mcp_instance_ids": ["1flowbase"]}}]}});
    let envelope = json!({"source_protocol": "openai_chat", "body": {"seed": 42}});
    let mut output = CodeInvocationOutput {
        output_payload: json!({"document": business, "context": envelope}),
        console_logs: vec![ConsoleLogEntry {
            level: "log".into(),
            message: envelope.to_string(),
            args: vec![envelope.clone()],
        }],
    };
    invoker
        .protect_code_protocol_context_output(&mut output, &[])
        .await
        .unwrap();
    assert_eq!(output.output_payload["document"], business);
    let locator_value = &output.output_payload["context"];
    assert!(ProviderProtocolContextLocator::parse(locator_value)
        .unwrap()
        .is_some());
    assert_eq!(
        invoker
            .open_protocol_context_locator_value(locator_value)
            .await
            .unwrap(),
        Some(envelope)
    );
    assert_eq!(
        output.console_logs[0].message,
        REDACTED_PROTOCOL_CONTEXT_LOG
    );
    assert!(
        ProviderProtocolContextLocator::parse(&output.console_logs[0].args[0])
            .unwrap()
            .is_some()
    );
    assert!(invoker
        .for_flow_run(Uuid::now_v7())
        .open_protocol_context_locator_value(locator_value)
        .await
        .is_err());

    let mut tampered = locator_value.clone();
    tampered["__1flowbase_ephemeral_protocol_context"]["digest"] =
        Value::String(format!("sha256:{}", "0".repeat(64)));
    assert!(invoker
        .open_protocol_context_locator_value(&tampered)
        .await
        .unwrap_err()
        .to_string()
        .contains("ephemeral_protocol_context_integrity_mismatch"));

    // Explicitly typed Code outputs remain protected even when Code transforms
    // their shape; embedded discovery must not replace that declared contract.
    output.output_payload = json!({"declared": {"custom": "selected output"}});
    invoker
        .protect_code_protocol_context_output(&mut output, &[vec!["declared".into()]])
        .await
        .unwrap();
    assert!(
        ProviderProtocolContextLocator::parse(&output.output_payload["declared"])
            .unwrap()
            .is_some()
    );
}
