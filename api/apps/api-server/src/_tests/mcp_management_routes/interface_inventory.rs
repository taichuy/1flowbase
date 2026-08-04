use std::collections::BTreeMap;

use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn route_identity(method: &str, path: &str) -> (String, String) {
    (
        method.to_ascii_uppercase(),
        path.split('/')
            .map(|segment| {
                if segment.starts_with(':') || (segment.starts_with('{') && segment.ends_with('}'))
                {
                    "{}"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

#[tokio::test]
async fn ac_001_ac_005_ac_010_interface_catalog_covers_compiled_routes_and_media_boundaries() {
    let inventory = crate::app_state::compile_core_console_operation_inventory_snapshot()
        .unwrap()
        .compiled_inventory;
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/interface-capabilities")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    let mut catalog_by_route = BTreeMap::new();
    for entry in payload["data"].as_array().unwrap() {
        let key = route_identity(
            entry["method"].as_str().unwrap(),
            entry["path"].as_str().unwrap(),
        );
        assert!(
            catalog_by_route.insert(key, entry).is_none(),
            "interface catalog must expose one entry per method and route template"
        );
    }

    for interface in inventory.interfaces {
        let key = route_identity(&interface.route.method, &interface.route.path);
        assert!(
            catalog_by_route.contains_key(&key),
            "compiled console interface is missing from catalog: {} {} ({})",
            interface.route.method,
            interface.route.path,
            interface.interface_id
        );
    }

    let missing_openapi = catalog_by_route
        .get(&route_identity(
            "DELETE",
            "/api/console/mcp/instances/{instance_id}/client-credential",
        ))
        .expect("compiled route without OpenAPI must remain visible");
    assert_eq!(missing_openapi["bindable"], json!(false));
    assert_eq!(
        missing_openapi["disabled_reason"],
        json!("missing_openapi_contract")
    );
    assert_eq!(missing_openapi["result_schema"], Value::Bool(false));

    let event_stream = catalog_by_route
        .get(&route_identity(
            "POST",
            "/api/console/applications/{id}/orchestration/debug-runs/stream",
        ))
        .expect("SSE operation must remain visible");
    assert_eq!(event_stream["bindable"], json!(false));
    assert_eq!(
        event_stream["disabled_reason"],
        json!("unsupported_response_media_type")
    );

    for (method, path, operation_id) in [
        (
            "GET",
            "/api/console/mcp/bundles/library",
            "list_mcp_bundle_library",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/sync",
            "sync_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/preview",
            "preview_mcp_bundle_library_release",
        ),
        (
            "POST",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/import",
            "import_mcp_bundle_library_release",
        ),
    ] {
        let entry = catalog_by_route
            .get(&route_identity(method, path))
            .unwrap_or_else(|| panic!("missing Bundle catalog entry {method} {path}"));
        assert_eq!(entry["interface_id"], json!(operation_id));
        assert_eq!(entry["bindable"], json!(true));
        assert_eq!(entry["disabled_reason"], Value::Null);
    }
}
