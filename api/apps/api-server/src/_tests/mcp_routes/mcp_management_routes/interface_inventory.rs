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

// This task owns a bounded inventory, not a new hand-maintained global catalog.
#[tokio::test]
async fn mcp_role_permission_inventory_preserves_identities_and_exposes_real_contracts() {
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
    let entries = payload["data"].as_array().unwrap();
    let inventory = [
        ("GET", "/api/console/settings/roles", "list_roles"),
        ("POST", "/api/console/settings/roles", "create_role"),
        ("PATCH", "/api/console/settings/roles/{id}", "update_role"),
        ("DELETE", "/api/console/settings/roles/{id}", "delete_role"),
        (
            "GET",
            "/api/console/settings/roles/permission-options",
            "list_permissions",
        ),
        (
            "GET",
            "/api/console/settings/roles/data-model-options",
            "list_data_model_options",
        ),
        (
            "GET",
            "/api/console/settings/roles/console-policy-catalog",
            "get_console_policy_catalog",
        ),
        (
            "PUT",
            "/api/console/settings/roles/console-policy-catalog/order",
            "replace_console_settings_order",
        ),
        (
            "GET",
            "/api/console/settings/roles/{id}/permissions",
            "get_role_permissions",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{id}/permissions",
            "replace_role_permissions",
        ),
        (
            "GET",
            "/api/console/settings/roles/{id}/data-policy",
            "get_role_data_policy",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{id}/data-policy",
            "replace_role_data_policy",
        ),
        (
            "GET",
            "/api/console/settings/roles/{id}/console-policy",
            "roles.console_policy.view",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{id}/console-policy",
            "roles.console_policy.replace",
        ),
        (
            "GET",
            "/api/console/settings/roles/{id}/frontstage-routes",
            "roles.frontstage_routes.view",
        ),
        (
            "PUT",
            "/api/console/settings/roles/{id}/frontstage-routes",
            "roles.frontstage_routes.replace",
        ),
        (
            "GET",
            "/api/console/settings/members/role-options",
            "members.role_options.list",
        ),
        (
            "PUT",
            "/api/console/settings/members/{id}/roles",
            "replace_member_roles",
        ),
    ];
    assert_eq!(inventory.len(), 18);
    let mut by_id = BTreeMap::new();
    for (method, path, id) in inventory {
        let matches = entries
            .iter()
            .filter(|entry| {
                route_identity(
                    entry["method"].as_str().unwrap(),
                    entry["path"].as_str().unwrap(),
                ) == route_identity(method, path)
            })
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "{method} {path}");
        let entry = matches[0];
        assert_eq!(entry["interface_id"], id, "{method} {path}");
        assert_eq!(entry["bindable"], true, "{id}: {entry}");
        assert_eq!(entry["disabled_reason"], Value::Null, "{id}");
        assert_eq!(entry["parameter_schema"]["type"], "object", "{id}");
        assert_ne!(entry["result_schema"], json!(false), "{id}");
        assert!(by_id.insert(id, entry).is_none());
    }
    for id in [
        "roles.console_policy.view",
        "roles.console_policy.replace",
        "roles.frontstage_routes.view",
        "roles.frontstage_routes.replace",
    ] {
        let entry = by_id[id];
        let descriptor = entry["parameter_descriptors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "id")
            .expect("role code path descriptor");
        assert_eq!(descriptor["location"], "path", "{id}");
        assert_eq!(descriptor["required"], true, "{id}");
        assert_eq!(descriptor["schema"]["type"], "string", "{id}");
        assert_eq!(
            entry["security"],
            if entry["method"] == "PUT" {
                json!([{"sessionCookie":[],"csrfHeader":[]},{"patBearer":[]}])
            } else {
                json!([{"sessionCookie":[]},{"patBearer":[]}])
            }
        );
    }
    let policy = by_id["roles.console_policy.view"];
    assert_eq!(
        policy["result_schema"]["properties"]["role_code"]["type"],
        "string"
    );
    assert_eq!(
        policy["result_schema"]["properties"]["groups"]["type"],
        "array"
    );
    let replace = by_id["roles.console_policy.replace"];
    let groups = &replace["parameter_schema"]["properties"]["body"]["properties"]["groups"];
    assert_eq!(groups["type"], "array");
    assert_eq!(groups["items"]["properties"]["group_id"]["type"], "string");
    assert_eq!(groups["items"]["properties"]["enabled"]["type"], "boolean");
    assert_eq!(
        groups["items"]["properties"]["strategy"]["enum"],
        json!(["full", "custom"])
    );
    assert_eq!(groups["items"]["properties"]["operations"]["type"], "array");
    assert!(replace["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| {
            item["name"] == "groups" && item["location"] == "json_body" && item["required"] == true
        }));
    // A 204 has no response media/schema, rather than a fabricated JSON object.
    assert_eq!(replace["result_schema"], json!(true));
    let frontstage = by_id["roles.frontstage_routes.view"];
    for field in ["checked_page_ids", "checked_tab_ids", "tree"] {
        assert_eq!(
            frontstage["result_schema"]["properties"][field]["type"], "array",
            "{field}"
        );
    }
    let replace = by_id["roles.frontstage_routes.replace"];
    for field in ["page_ids", "tab_ids"] {
        let schema = &replace["parameter_schema"]["properties"]["body"]["properties"][field];
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["format"], "uuid");
        assert!(replace["parameter_descriptors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item["name"] == field && item["location"] == "json_body" && item["required"] == true
            }));
    }
    assert_eq!(replace["result_schema"], json!(true));
    let options = by_id["members.role_options.list"];
    assert_eq!(options["result_schema"]["type"], "array");
    for field in ["code", "name"] {
        assert_eq!(
            options["result_schema"]["items"]["properties"][field]["type"],
            "string"
        );
    }
    assert_eq!(
        options["security"],
        json!([{"sessionCookie":[]},{"patBearer":[]}])
    );
}
