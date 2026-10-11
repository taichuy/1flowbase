//! Real host generation compilation and HTTP ingress; runtime execution has separate fixtures.
use super::*;
use crate::{
    _tests::support::{login_and_capture_cookie, test_api_state_with_database_url, test_config},
    managed_publication::ManagedGenerationPublisher,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use std::{collections::BTreeMap, sync::Arc};
use tower::ServiceExt;

async fn request(app: &Router, path: &str, cookie: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    let value =
        serde_json::from_slice(&body).unwrap_or_else(|_| json!(String::from_utf8_lossy(&body)));
    (status, value)
}

async fn projection(app: &Router, path: &str, cookie: &str) -> serde_json::Value {
    let (status, value) = request(app, path, cookie).await;
    assert_eq!(status, StatusCode::OK, "{path}: {value}");
    value
}

async fn assert_unregistered(app: &Router, path: &str, cookie: &str) {
    let (status, body) = request(app, path, cookie).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "console_route_unregistered");
}

// Navigation exposes only durably enabled services with an applied published template.
// Seed that existing contract through the same template repository used by activation.
async fn apply_service_fixture(
    state: &crate::app_state::ApiState,
    service: &ManagedServiceRegistration,
) {
    use control_plane_contracts::ports::PluginRepository;
    let actor: uuid::Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    sqlx::query("insert into extension_installations(id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values($1,'runtime-extensions','acme',$2,$3,$4,'1flowbase.extension-bus/v1','stdio_json','Fixture','uploaded','unverified','valid','active_requested','missing',$5,$6)")
        .bind(service.installation_id).bind(&service.plugin_code).bind(&service.plugin_version)
        .bind(format!("{}@{}", service.plugin_code, service.plugin_version))
        .bind(json!({"managed_service":{"scope":"system"}})).bind(actor)
        .execute(state.store.pool()).await.unwrap();
    sqlx::query("insert into plugin_settings_template_defaults (installation_id,contribution_code,feature_id,source,language) values ($1,'settings','fixture.settings','export default function Fixture() { return <div>Fixture</div>; }','tsx')")
        .bind(service.installation_id).execute(state.store.pool()).await.unwrap();
    state
        .store
        .apply_managed_plugin_settings_templates(service.installation_id)
        .await
        .unwrap();
}

fn contains_string(value: &serde_json::Value, expected: &str) -> bool {
    match value {
        serde_json::Value::String(value) => value == expected,
        serde_json::Value::Array(values) => values.iter().any(|v| contains_string(v, expected)),
        serde_json::Value::Object(values) => values.values().any(|v| contains_string(v, expected)),
        _ => false,
    }
}

#[tokio::test]
async fn hot_generation_same_ingress_swaps_routes_navigation_docs_and_mcp_atomically() {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let initial = crate::app_with_state_and_config(state.clone(), &config);
    let publisher = ManagedGenerationPublisher::new(&state, config, vec![], initial).unwrap();
    // This is the one router retained by the host; never reconstruct it after publication.
    let ingress = publisher.ingress();
    let (cookie, _) = login_and_capture_cookie(&ingress, "root", "change-me").await;
    let original_pid = std::process::id();
    let mut service = fixture();
    service.installation_id = uuid::Uuid::now_v7();
    apply_service_fixture(&state, &service).await;
    let old_route = service.declaration.operations[0].path.clone();
    assert_unregistered(&ingress, &old_route, &cookie).await;
    let candidate = publisher
        .prepare(vec![service.clone()], BTreeMap::new())
        .await
        .unwrap();
    // Merely preparing a complete candidate must leave every serving projection unchanged.
    assert_unregistered(&ingress, &old_route, &cookie).await;
    publisher.publish(candidate);
    assert_eq!(
        request(&ingress, &old_route, "").await.0,
        StatusCode::UNAUTHORIZED
    );
    for (endpoint, expected) in [
        ("/api/console/navigation", "/settings/fixture"),
        (
            "/api/console/docs/operations/fixture.list/openapi.json",
            "fixture.list",
        ),
        ("/api/console/mcp/interface-capabilities", "fixture.list"),
    ] {
        assert!(
            contains_string(&projection(&ingress, endpoint, &cookie).await, expected),
            "missing {expected} in {endpoint}"
        );
    }
    let mut upgraded = service;
    upgraded.installation_id = uuid::Uuid::now_v7();
    upgraded.plugin_version = "2.0.0".into();
    upgraded.declaration.feature.path = "/settings/fixture-v2".into();
    upgraded.declaration.operations[0].interface_id = "fixture.list-v2".into();
    upgraded.declaration.operations[0].path =
        "/api/console/managed-services/fixture/items-v2".into();
    apply_service_fixture(&state, &upgraded).await;
    publisher.publish(
        publisher
            .prepare(vec![upgraded.clone()], BTreeMap::new())
            .await
            .unwrap(),
    );
    assert_unregistered(&ingress, &old_route, &cookie).await;
    assert_eq!(
        request(&ingress, &upgraded.declaration.operations[0].path, "")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for (endpoint, removed, added) in [
        (
            "/api/console/navigation",
            "/settings/fixture",
            "/settings/fixture-v2",
        ),
        (
            "/api/console/mcp/interface-capabilities",
            "fixture.list",
            "fixture.list-v2",
        ),
    ] {
        let value = projection(&ingress, endpoint, &cookie).await;
        assert!(
            !contains_string(&value, removed),
            "stale {removed} in {endpoint}"
        );
        assert!(
            contains_string(&value, added),
            "missing {added} in {endpoint}"
        );
    }
    assert_eq!(
        request(
            &ingress,
            "/api/console/docs/operations/fixture.list/openapi.json",
            &cookie
        )
        .await
        .0,
        StatusCode::NOT_FOUND,
    );
    let updated_spec = projection(
        &ingress,
        "/api/console/docs/operations/fixture.list-v2/openapi.json",
        &cookie,
    )
    .await;
    assert!(contains_string(&updated_spec, "fixture.list-v2"));
    assert!(!contains_string(&updated_spec, "fixture.list"));
    // Invalid schemas and route ownership collisions fail before publication, preserving G2.
    let mut invalid = upgraded.clone();
    invalid.declaration.operations[0].output_schema = json!({"type":"not-a-json-schema-type"});
    assert!(publisher
        .prepare(vec![invalid], BTreeMap::new())
        .await
        .is_err());
    assert!(publisher
        .prepare(vec![upgraded.clone(), upgraded.clone()], BTreeMap::new())
        .await
        .is_err());
    assert_eq!(
        request(&ingress, &upgraded.declaration.operations[0].path, "")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let catalog = projection(
        &ingress,
        "/api/console/docs/operations/fixture.list-v2/openapi.json",
        &cookie,
    )
    .await;
    assert!(contains_string(&catalog, "fixture.list-v2"));
    assert!(!contains_string(&catalog, "fixture.list"));
    assert_eq!(std::process::id(), original_pid);
}

#[tokio::test]
async fn hot_generation_admitted_http_request_keeps_selected_router_until_completion() {
    let (state, _) = test_api_state_with_database_url().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let initial = Router::new().route(
        "/held-generation",
        get({
            let entered = entered.clone();
            let release = release.clone();
            move || {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    axum::Json(json!({"generation":1}))
                }
            }
        }),
    );
    let publisher =
        ManagedGenerationPublisher::new(&state, test_config(), vec![], initial).unwrap();
    let ingress = publisher.ingress();
    let old_request = tokio::spawn({
        let ingress = ingress.clone();
        async move { request(&ingress, "/held-generation", "").await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    assert_eq!(
        request(&ingress, "/held-generation", "").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(request(&ingress, "/health", "").await.0, StatusCode::OK);
    release.notify_one();
    let (status, value) = tokio::time::timeout(std::time::Duration::from_secs(5), old_request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value, json!({"generation":1}));
}
