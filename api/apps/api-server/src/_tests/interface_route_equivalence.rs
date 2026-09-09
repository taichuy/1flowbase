use interface_runtime::{BindingId, InterfaceExecutionMode, ProtocolProjection};

use super::support::test_api_state_with_database_url;

#[tokio::test]
async fn issue_1944_native_response_modes_are_distinct_published_contracts() {
    let (state, _) = test_api_state_with_database_url().await;
    state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let cases = [
        (
            crate::routes::application_public_api::native_interface::ASYNC_BINDING_ID,
            "async",
            InterfaceExecutionMode::Unary,
        ),
        (
            crate::routes::application_public_api::native_interface::BLOCKING_BINDING_ID,
            "blocking",
            InterfaceExecutionMode::Unary,
        ),
        (
            crate::routes::application_public_api::native_interface::STREAM_BINDING_ID,
            "streaming",
            InterfaceExecutionMode::ServerStream,
        ),
    ];
    for (binding_id, selector, mode) in cases {
        let plan = registry
            .plan(&BindingId::new(binding_id).unwrap())
            .unwrap_or_else(|| panic!("missing Native binding {binding_id}"));
        assert_eq!(plan.definition().execution_mode(), mode);
        let route = plan.binding().projection().http_route().unwrap();
        assert_eq!(route.method(), "POST");
        assert_eq!(route.path(), "/api/agent/v1/runs");
        match (selector, plan.binding().projection()) {
            ("blocking", ProtocolProjection::Http(_)) => {}
            (expected, ProtocolProjection::HttpVariant { variant, .. }) => {
                assert_eq!(variant.as_ref(), expected)
            }
            _ => panic!("Native binding selector does not match {selector}"),
        }
        assert_eq!(
            plan.adapter_plan().authentication().as_str(),
            "api-server.application-api-key"
        );
    }
}

#[test]
fn issue_1958_migrated_routes_have_no_production_compatibility_bypass() {
    let compatibility_stream = include_str!("../routes/application_public_api/compat_sse.rs");
    let openai = include_str!("../routes/application_public_api/openai.rs");
    let anthropic = include_str!("../routes/application_public_api/anthropic.rs");
    let workflow_extension = include_str!("../routes/application_public_api/ex.rs");

    for legacy in [
        "PreparedCompatibleTurn",
        "start_compatible_turn_stream",
        "start_openai_run_stream",
        "start_openai_response_stream",
        "start_anthropic_run_stream",
        "authenticate_openai_response_credential",
        "execute_openai_tool_resume",
        "execute_anthropic_tool_resume",
    ] {
        assert!(
            !compatibility_stream.contains(legacy)
                && !openai.contains(legacy)
                && !anthropic.contains(legacy),
            "legacy production compatibility owner remains: {legacy}"
        );
    }
    assert!(!compatibility_stream.contains("public_mcp_runtime_invoker(&state"));
    assert!(openai.contains("compatibility_interface::invoke_blocking"));
    assert!(openai.contains("compatibility_interface::invoke_stream"));
    assert!(anthropic.contains("compatibility_interface::invoke_blocking"));
    assert!(anthropic.contains("compatibility_interface::invoke_stream"));
    assert_eq!(
        workflow_extension
            .matches(".authenticate_invocation")
            .count(),
        1
    );
    assert!(!workflow_extension.contains("require_session(&state"));
    assert!(!workflow_extension.contains("require_csrf(&headers"));
}


// E00 metadata-only diagnostic; never invoked as a product acceptance test.
#[tokio::test]
async fn root_2014_e00_export_compiled_snapshot() {
    use serde_json::json;
    let (state, _) = test_api_state_with_database_url().await;
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    boot.publish_complete_catalog(&state).unwrap();
    let registry = boot.interface_registry().unwrap().snapshot();
    let contract = |c: &interface_runtime::ContractIdentity| json!({"id": c.contract_id(), "version": c.version()});
    let definitions: Vec<_> = registry.definitions().map(|d| json!({
        "interface_id": d.interface_id().as_str(), "version": d.version().as_str(),
        "owner": d.owner().as_str(), "mode": format!("{:?}", d.execution_mode()),
        "registration_lifecycle": format!("{:?}", d.lifecycle()),
        "principal_profile": format!("{:?}", d.principal_profile()),
        "authentication_policy": format!("{:?}", d.authentication()),
        "scope": format!("{:?}", d.scope()), "authorization_operation": d.authorization_operation().as_str(),
        "input": contract(d.input_contract()), "output": contract(d.output_contract()),
        "error": contract(d.target_error_contract()), "stream_event": d.stream_event_contract().map(contract),
        "definition_debug": format!("{:?}", d)
    })).collect();
    let bindings: Vec<_> = registry.bindings().map(|b| {
        let p = registry.plan(b.binding_id()).expect("every binding has a compiled plan");
        json!({"binding_id": b.binding_id().as_str(), "interface_id": b.interface_identity().interface_id().as_str(),
            "interface_version": b.interface_identity().version().as_str(),
            "projection": format!("{:?}", b.projection()),
            "binding_fingerprint": p.binding_fingerprint().as_str(), "plan_fingerprint": p.fingerprint().as_str(),
            "authentication_activation": format!("{:?}",p.authentication()),
            "adapter_plan": format!("{:?}",p.adapter_plan()),
            "effective_handler": format!("{:?}",p.effective_handler()),
            "extension_plan": format!("{:?}",p.extension_plan()),
            "has_executable_extensions": p.has_executable_extensions()})
    }).collect();
    let output = json!({"schema": "root-2014-e00/v1", "source_sha": "ce355a56a4a3ec91acc627b5691daf06814c1225",
        "harness_sha": std::env::var("GITHUB_SHA").ok(),
        "configuration": "default_test_config + DEFAULT_PLUGIN_SET_PATH; no filesystem dropins; isolated PostgreSQL schema",
        "graph_fingerprint": boot.fingerprint(), "registry_fingerprint": registry.fingerprint().as_str(),
        "effective_extension_plan": serde_json::from_str::<serde_json::Value>(&boot.render_effective_plan().unwrap()).unwrap(),
        "definition_count": definitions.len(), "binding_count": bindings.len(),
        "definitions": definitions,"bindings": bindings});
    let output_path = std::env::var("E00_OUTPUT").unwrap();
    std::fs::write(output_path, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
    drop(registry);
    drop(state); // Existing TestResources drops isolated schema and temporary filesystem roots.
}
