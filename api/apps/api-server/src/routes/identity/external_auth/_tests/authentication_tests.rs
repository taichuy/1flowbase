//! The actual callback binding must resolve its credential factory and record a
//! rejected authentication attempt before canonical business dispatch.
use super::*;
use crate::extension_bus::{AuthenticationAdapterFactoryRegistry, PublicAuthenticationCredential};
use interface_runtime::{BindingId, InterfaceProtocol, PublicPrincipal};
use std::io::Write;
use tracing::instrument::WithSubscriber;

#[derive(Clone)]
struct Logs(Arc<Mutex<Vec<u8>>>);
impl Write for Logs {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn external_callback_uses_its_activated_factory_and_rejects_public_credentials() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let _app = crate::app_with_state(state.clone());
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let registry = boot.interface_registry().unwrap().snapshot();
    let binding = BindingId::new(interface::CALLBACK_BINDING).unwrap();
    let activation = registry.authentication(&binding).unwrap();
    assert_eq!(
        activation.plugin().as_str(),
        interface::CALLBACK_AUTH_PLUGIN
    );
    assert_eq!(
        activation.adapter().as_str(),
        interface::CALLBACK_AUTH_ADAPTER
    );
    assert_eq!(
        activation.activation().as_str(),
        interface::CALLBACK_AUTH_ACTIVATION
    );
    assert_eq!(
        activation.tier(),
        interface_runtime::InterfaceExtensionTier::BuiltIn
    );
    assert_eq!(
        activation.principal_profile(),
        interface_runtime::PrincipalProfile::Public
    );
    AuthenticationAdapterFactoryRegistry::built_in()
        .unwrap()
        .validate_registry(&registry)
        .unwrap();
    let rejected = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            registry.clone(),
            &binding,
            InterfaceProtocol::Http,
            PublicAuthenticationCredential,
        )
        .await;
    assert!(rejected
        .err()
        .unwrap()
        .to_string()
        .contains("credential contract mismatch"));
}

#[tokio::test]
async fn external_callback_factory_failure_publishes_safe_correlated_attempt() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let logs = Logs(Arc::new(Mutex::new(Vec::new())));
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || writer.clone())
        .finish();
    use tower::ServiceExt;
    let response = app.oneshot(axum::http::Request::builder()
        .uri("/api/public/auth/external/callback?state=private-invalid-state&code=private-oauth-code")
        .header("cookie", "private-cookie-name=private-cookie-value")
        .body(axum::body::Body::empty()).unwrap()).with_subscriber(subscriber).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()["location"],
        "/sign-in?auth_error=invalid_state"
    );
    let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    for expected in [
        "interface authentication rejected",
        interface::CALLBACK_BINDING,
        interface::CALLBACK_AUTH_ACTIVATION,
        "invocation_id",
        "registry_fingerprint",
        "plan_fingerprint",
        "unestablished",
        "credential-rejected",
        "Rejected",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert_eq!(text.matches("interface authentication rejected").count(), 1);
    for secret in [
        "private-invalid-state",
        "private-oauth-code",
        "private-cookie-name",
        "private-cookie-value",
    ] {
        assert!(!text.contains(secret));
    }
    assert!(!response
        .headers()
        .get_all("set-cookie")
        .iter()
        .any(|value| value
            .to_str()
            .unwrap()
            .starts_with(&format!("{}=", state.cookie_name))));
}
