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
