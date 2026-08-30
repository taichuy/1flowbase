use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use control_plane::ports::SessionStore;
use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, InterfaceExtensionTier, PluginIdentity, PrincipalProfile,
    UserPrincipal,
};
use tower::ServiceExt;

use crate::extension_bus::{
    AuthenticationAdapterFactoryBinding, AuthenticationAdapterFactoryRegistry,
    HostExtensionAuthenticationFactoryCatalog,
};

use super::support::{login_and_capture_cookie, test_api_state_with_database_url};

struct CountingSessionStore {
    inner: Arc<dyn SessionStore>,
    gets: Arc<AtomicUsize>,
}

#[async_trait]
impl SessionStore for CountingSessionStore {
    async fn put(&self, session: domain::SessionRecord) -> anyhow::Result<()> {
        self.inner.put(session).await
    }

    async fn get(&self, session_id: &str) -> anyhow::Result<Option<domain::SessionRecord>> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.inner.get(session_id).await
    }

    async fn delete(&self, session_id: &str) -> anyhow::Result<()> {
        self.inner.delete(session_id).await
    }

    async fn touch(&self, session_id: &str, expires_at_unix: i64) -> anyhow::Result<()> {
        self.inner.touch(session_id, expires_at_unix).await
    }
}

struct HostCredential(&'static str);
struct WrongCredential;

fn host_activation() -> ActivatedAuthenticationAdapter {
    ActivatedAuthenticationAdapter::new(
        PluginIdentity::new("acme.authentication-host").unwrap(),
        InterfaceExtensionTier::HostExtension,
        AuthenticationAdapterReference::new("acme.authentication-host.api-key").unwrap(),
        AuthenticationActivationIdentity::new(
            "acme.authentication-host.console-session.activation.v1",
        )
        .unwrap(),
        PrincipalProfile::User,
    )
}

fn host_binding(calls: Arc<AtomicUsize>) -> AuthenticationAdapterFactoryBinding {
    AuthenticationAdapterFactoryBinding::typed(
        host_activation(),
        move |credential: HostCredential| {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                if credential.0 != "accepted" {
                    anyhow::bail!("host authentication rejected")
                }
                Ok(UserPrincipal::server_delegation(
                    domain::ActorContext::root(uuid::Uuid::now_v7(), uuid::Uuid::now_v7(), "root"),
                ))
            }
        },
    )
    .unwrap()
}

#[tokio::test]
async fn rr14_compiled_authentication_registration_without_factory_fails_catalog_publish() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();

    let error = AuthenticationAdapterFactoryRegistry::default()
        .validate_registry(registry.as_ref())
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("authentication activation is not bound to a factory"));
}

#[tokio::test]
async fn rr14_trusted_host_extension_factory_is_assembled_and_executes_success_reject_and_contract_mismatch(
) {
    let manifest = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/manifest.yaml"
    ))
    .unwrap();
    let contribution = plugin_framework::parse_host_extension_contribution_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/host-extension.yaml"
    ))
    .unwrap();
    let library = contribution.native.library.clone();
    let entry_symbol = contribution.native.entry_symbol.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut catalog = HostExtensionAuthenticationFactoryCatalog::default();
    catalog
        .register(
            library,
            entry_symbol,
            Arc::new({
                let calls = Arc::clone(&calls);
                move |_contribution| Ok(vec![host_binding(Arc::clone(&calls))])
            }),
        )
        .unwrap();
    let bindings = catalog.activate(&[(manifest, contribution)]).unwrap();
    assert_eq!(bindings.len(), 1);

    let mut registry = AuthenticationAdapterFactoryRegistry::default();
    registry.extend(bindings).unwrap();
    let principal: UserPrincipal = registry
        .authenticate(&host_activation(), HostCredential("accepted"))
        .await
        .unwrap();
    assert_eq!(
        principal.credential_kind(),
        interface_runtime::UserCredentialKind::ServerDelegation
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    assert!(registry
        .authenticate::<_, UserPrincipal>(&host_activation(), HostCredential("rejected"))
        .await
        .unwrap_err()
        .to_string()
        .contains("host authentication rejected"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    assert!(registry
        .authenticate::<_, UserPrincipal>(&host_activation(), WrongCredential)
        .await
        .unwrap_err()
        .to_string()
        .contains("credential contract mismatch"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn rr14_host_extension_authentication_manifest_without_native_factory_fails_activation() {
    let manifest = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/manifest.yaml"
    ))
    .unwrap();
    let contribution = plugin_framework::parse_host_extension_contribution_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/host-extension.yaml"
    ))
    .unwrap();

    let error = HostExtensionAuthenticationFactoryCatalog::default()
        .activate(&[(manifest, contribution)])
        .err()
        .expect("missing native factory must fail activation");
    assert!(error.to_string().contains("no activation factory"));
}

#[tokio::test]
async fn rr14_real_host_extension_route_authenticates_once_through_frozen_factory() {
    let (mut state, _) = test_api_state_with_database_url().await;

    let manifest = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/manifest.yaml"
    ))
    .unwrap();
    let contribution = plugin_framework::parse_host_extension_contribution_manifest(include_str!(
        "../../../../plugins/fixtures/acme.authentication-host/host-extension.yaml"
    ))
    .unwrap();
    let mut assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )
    .unwrap();
    assembly
        .extend_active_host_extensions(&[(manifest, contribution)])
        .unwrap();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let host_factories = crate::extension_bus::production_host_extension_authentication_factories()
        .activate(assembly.host_extension_manifests())
        .unwrap();
    let snapshot = Arc::new(
        crate::extension_bus::ExtensionBootSnapshot::compile(
            graph,
            assembly.interface_operations(),
            assembly.host_extension_manifests(),
            Arc::new(
                crate::extension_bus::DurableHostInfrastructureProvidersViewQuery::new(
                    state.store.clone(),
                    state.api_node_id.clone(),
                ),
            ),
            host_factories,
        )
        .unwrap(),
    );
    let compiled = snapshot.interface_registry().unwrap().snapshot();
    let binding_id = interface_runtime::BindingId::new(
        crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID,
    )
    .unwrap();
    let activation = compiled.authentication(&binding_id).unwrap();
    assert_eq!(activation.plugin().as_str(), "acme.authentication-host");
    assert_eq!(activation.tier(), InterfaceExtensionTier::HostExtension);

    let console_boot_plan = crate::app_state::compile_console_boot_plan_with_interface_operations(
        Vec::new(),
        Some(compiled.as_ref()),
    )
    .unwrap();
    let gets = Arc::new(AtomicUsize::new(0));
    let mutable = Arc::get_mut(&mut state).unwrap();
    mutable.extension_boot_snapshot = Some(snapshot);
    mutable.settings_feature_registry = console_boot_plan.settings_feature_registry;
    mutable.console_operation_registry = console_boot_plan.console_operation_registry;
    mutable.console_surface_registry = console_boot_plan.console_surface_registry;
    mutable.session_store = Arc::new(CountingSessionStore {
        inner: Arc::clone(&mutable.session_store),
        gets: Arc::clone(&gets),
    });

    let app = crate::app_with_state(Arc::clone(&state));
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    gets.store(0, Ordering::SeqCst);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(
                    crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH,
                )
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(gets.load(Ordering::SeqCst), 1);

    let invalid_cookie = format!(
        "{}=invalid",
        cookie
            .split_once('=')
            .map(|(name, _)| name)
            .expect("login response must set a named session cookie")
    );
    let rejected = app
        .oneshot(
            Request::builder()
                .uri(
                    crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH,
                )
                .header("cookie", invalid_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(gets.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn rr14_factory_without_compiled_registration_fails_catalog_publish() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let compiled = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let mut registry = AuthenticationAdapterFactoryRegistry::built_in().unwrap();
    registry
        .extend([host_binding(Arc::new(AtomicUsize::new(0)))])
        .unwrap();
    let error = registry.validate_registry(compiled.as_ref()).unwrap_err();
    assert!(error
        .to_string()
        .contains("authentication factory has no compiled registration"));
}
