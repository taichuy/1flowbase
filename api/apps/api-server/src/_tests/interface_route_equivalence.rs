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
            .matches("boot_snapshot.authenticate(")
            .count(),
        1
    );
    assert!(!workflow_extension.contains("require_session(&state"));
    assert!(!workflow_extension.contains("require_csrf(&headers"));
}
