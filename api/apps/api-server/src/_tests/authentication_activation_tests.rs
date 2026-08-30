use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, InterfaceExtensionTier, PluginIdentity, PrincipalProfile,
    UserPrincipal,
};

use crate::extension_bus::{
    AuthenticationAdapterFactoryBinding, AuthenticationAdapterFactoryRegistry,
    HostExtensionAuthenticationFactoryCatalog,
};

use super::support::test_api_state_with_database_url;

struct HostCredential(&'static str);
struct WrongCredential;

fn host_activation() -> ActivatedAuthenticationAdapter {
    ActivatedAuthenticationAdapter::new(
        PluginIdentity::new("acme.authentication-host").unwrap(),
        InterfaceExtensionTier::HostExtension,
        AuthenticationAdapterReference::new("acme.authentication-host.api-key").unwrap(),
        AuthenticationActivationIdentity::new("acme.authentication-host.activation.v1").unwrap(),
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
        "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/manifest.yaml"
    ))
    .unwrap();
    let contribution = plugin_framework::parse_host_extension_contribution_manifest(include_str!(
        "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/host-extension.yaml"
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
                move || Ok(vec![host_binding(Arc::clone(&calls))])
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
