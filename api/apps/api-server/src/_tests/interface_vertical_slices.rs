use interface_runtime::{BindingId, InterfaceExecutionMode, InterfaceProtocol, PrincipalProfile};

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
            "http.console.host-infrastructure.memory.overview.get.v1",
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
async fn retired_provider_bindings_are_absent_from_published_catalog() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    for id in [
        "http.host_infrastructure.providers.view.v1",
        "mcp.host_infrastructure.providers.view.v1",
        "internal.host_infrastructure.providers.view.v1",
        "http.console.host-infrastructure.providers.configure.v1",
    ] {
        assert!(
            registry.plan(&BindingId::new(id).unwrap()).is_none(),
            "retired binding: {id}"
        );
    }
}
