use interface_runtime::{
    BindingId, InterfaceExecutionMode, InterfaceExtensionPoint, InterfaceProtocol, PrincipalProfile,
};

use super::support::test_api_state_with_database_url;

#[tokio::test]
async fn issue_1944_boot_catalog_contains_the_four_typed_vertical_slices() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .expect("boot must publish the interface catalog")
        .snapshot();
    let expected = [
        (
            "http.public.auth.login-entries.v1",
            InterfaceProtocol::Http,
            PrincipalProfile::Public,
            InterfaceExecutionMode::Unary,
        ),
        (
            crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID,
            InterfaceProtocol::Http,
            PrincipalProfile::User,
            InterfaceExecutionMode::Unary,
        ),
        (
            crate::routes::application_public_api::native_interface::STREAM_BINDING_ID,
            InterfaceProtocol::Http,
            PrincipalProfile::Application,
            InterfaceExecutionMode::ServerStream,
        ),
        (
            "mcp.user-api-key.invoke.v1",
            InterfaceProtocol::Mcp,
            PrincipalProfile::User,
            InterfaceExecutionMode::Unary,
        ),
    ];
    for (binding_id, protocol, principal, mode) in expected {
        let plan = registry
            .plan(&BindingId::new(binding_id).unwrap())
            .unwrap_or_else(|| panic!("missing published binding {binding_id}"));
        assert_eq!(plan.binding().projection().protocol(), protocol);
        assert_eq!(plan.definition().principal_profile(), principal);
        assert_eq!(plan.definition().execution_mode(), mode);
        assert_eq!(plan.authentication().principal_profile(), principal);
        assert_eq!(
            plan.authentication().adapter(),
            plan.adapter_plan().authentication()
        );
        assert!(!plan.authentication().activation().as_str().is_empty());
        assert!(plan.fingerprint().as_str().starts_with("sha256:"));
    }
}

#[tokio::test]
async fn issue_1944_providers_http_and_mcp_resolve_distinct_binding_plans() {
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
    let http = registry
        .plan(&BindingId::new(
            crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID,
        ).unwrap())
        .unwrap();
    let mcp = registry
        .plan(&BindingId::new(
            crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_MCP_BINDING_ID,
        ).unwrap())
        .unwrap();
    assert_eq!(
        http.definition().interface_id(),
        mcp.definition().interface_id()
    );
    assert_eq!(
        http.binding().projection().protocol(),
        InterfaceProtocol::Http
    );
    assert_eq!(
        mcp.binding().projection().protocol(),
        InterfaceProtocol::Mcp
    );
    assert_ne!(http.binding_fingerprint(), mcp.binding_fingerprint());
    assert_eq!(http.extension_plan(), mcp.extension_plan());
    assert!(!http.extension_plan().registrations().is_empty());
    let points = http
        .extension_plan()
        .registrations()
        .iter()
        .map(|entry| entry.registration().point())
        .collect::<Vec<_>>();
    for required in [
        InterfaceExtensionPoint::Definition,
        InterfaceExtensionPoint::AuthenticationAdapter,
        InterfaceExtensionPoint::Authorization,
        InterfaceExtensionPoint::Admission,
    ] {
        assert!(
            points.contains(&required),
            "missing executable point {required:?}"
        );
    }
}
