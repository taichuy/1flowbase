//! Root #1998 A4: real Console authentication owner, error projection and safe publication.
use super::*;
use axum::{http::HeaderMap, response::IntoResponse};
use interface_runtime::{InterfaceContract, InterfaceProtocol, PrincipalProfile};
use std::{io::Write, sync::Mutex};
use tracing::instrument::WithSubscriber;

#[derive(Clone)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);
impl Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct Probe;
impl InterfaceContract for Probe {
    const CONTRACT_ID: &'static str = "root-1998-auth-probe";
    const CONTRACT_VERSION: &'static str = "1";
}

#[tokio::test]
async fn root_1998_console_unary_and_stream_rejection_preserve_http_and_publish_safe_record() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        "unknown-session=root-1998-private-credential"
            .parse()
            .unwrap(),
    );
    let expected = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .err()
        .unwrap()
        .into_response();
    let expected_status = expected.status();
    let expected_body = axum::body::to_bytes(expected.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(expected_status, StatusCode::UNAUTHORIZED);
    for (stream, binding) in [
        (false, "http.console.assistant.settings.get.v1"),
        (true, "http.console.assistant.runs.stream.v1"),
    ] {
        let logs = LogBuffer(Arc::new(Mutex::new(Vec::new())));
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .with_writer(move || writer.clone())
            .finish();
        let credential = crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers: headers.clone(),
        };
        let error = async {
            if stream {
                crate::routes::console_interface::invoke_server_stream::<Probe, Probe, Probe>(
                    state.clone(),
                    binding,
                    credential,
                    Probe,
                )
                .await
                .err()
                .unwrap()
            } else {
                crate::routes::console_interface::invoke::<Probe, Probe>(
                    state.clone(),
                    binding,
                    credential,
                    Probe,
                )
                .await
                .err()
                .unwrap()
            }
        }
        .with_subscriber(subscriber)
        .await;
        let response = error.into_response();
        assert_eq!(response.status(), expected_status);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
            expected_body
        );
        let published = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        for expected in [
            "interface authentication rejected",
            binding,
            "invocation_id",
            "interface_id",
            "plan_fingerprint",
            "graph_fingerprint",
            "registry_fingerprint",
            "authentication_activation",
            "unestablished",
            "credential-rejected",
            "Rejected",
            "observers",
        ] {
            assert!(
                published.contains(expected),
                "missing {expected}: {published}"
            );
        }
        assert_eq!(
            published
                .matches("interface authentication rejected")
                .count(),
            1
        );
        assert!(!published.contains("root-1998-private-credential"));
        assert!(!published.contains("unknown-session"));
    }
}

#[tokio::test]
async fn root_1998_success_reuses_attempt_lineage_and_unknown_binding_is_ingress_only() {
    let (state, _) = test_api_state_with_database_url().await;
    let _router = crate::app_with_state(state.clone());
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let snapshot = boot.interface_registry().unwrap().snapshot();
    let binding = snapshot
        .bindings()
        .find(|binding| {
            snapshot
                .authentication(binding.binding_id())
                .unwrap()
                .principal_profile()
                == PrincipalProfile::Public
        })
        .unwrap()
        .binding_id()
        .clone();
    let authenticated = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            snapshot.clone(),
            &binding,
            InterfaceProtocol::Http,
            PublicAuthenticationCredential,
        )
        .await
        .unwrap();
    let id = authenticated.lineage().invocation_id();
    assert_eq!(
        authenticated.into_envelope(Probe).lineage().invocation_id(),
        id
    );
    let logs = LogBuffer(Arc::new(Mutex::new(Vec::new())));
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    assert!(boot
        .authenticate_invocation::<_, PublicPrincipal>(
            snapshot.clone(),
            &interface_runtime::BindingId::new("unknown.binding").unwrap(),
            InterfaceProtocol::Http,
            PublicAuthenticationCredential
        )
        .with_subscriber(subscriber)
        .await
        .is_err());
    let published = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(published.contains("unresolved-authentication-binding"));
    assert!(!published.contains("interface authentication rejected"));
    assert!(!published.contains("plan_fingerprint"));
    let logs = LogBuffer(Arc::new(Mutex::new(Vec::new())));
    let writer = logs.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let error = AuthenticationAdapterFactoryRegistry::default()
        .authenticate_invocation::<_, PublicPrincipal>(
            snapshot,
            &binding,
            InterfaceProtocol::Http,
            PublicAuthenticationCredential,
        )
        .with_subscriber(subscriber)
        .await
        .err()
        .unwrap();
    assert!(error
        .to_string()
        .contains("authentication activation is not bound to a factory"));
    let published = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(published.contains("unresolved-authentication-activation"));
    assert!(!published.contains("interface authentication rejected"));
    assert!(!published.contains("plan_fingerprint"));
}

#[tokio::test]
async fn root_1998_mcp_identity_native_webmcp_http_rejections_publish_correlated_safe_terminal() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    for (method, path, body, binding) in [
        (
            "POST",
            "/mcp/root-1998",
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}",
            "mcp.user-api-key.invoke.v1",
        ),
        (
            "GET",
            "/api/console/session",
            "",
            "http.console.identity.session.get.v1",
        ),
        (
            "GET",
            "/api/agent/v1/runs/00000000-0000-0000-0000-000000000001",
            "",
            "http.application.native.runs.get.v1",
        ),
        (
            "GET",
            "/api/webmcp/registrations",
            "",
            "http.webmcp.registrations.v1",
        ),
    ] {
        let logs = LogBuffer(Arc::new(Mutex::new(Vec::new())));
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_max_level(tracing::Level::WARN)
            .with_writer(move || writer.clone())
            .finish();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", "Bearer root-1998-private-invalid-token")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .with_subscriber(subscriber)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let response: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(response["code"], "not_authenticated", "{path}: {response}");
        let published = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            published
                .matches("interface authentication rejected")
                .count(),
            1,
            "{path}: {published}"
        );
        assert!(published.contains(binding), "{path}: {published}");
        for field in [
            "invocation_id",
            "plan_fingerprint",
            "registry_fingerprint",
            "authentication_activation",
            "unestablished",
            "Rejected",
        ] {
            assert!(published.contains(field), "{path}: {field}");
        }
        assert!(!published.contains("root-1998-private-invalid-token"));
    }
}
