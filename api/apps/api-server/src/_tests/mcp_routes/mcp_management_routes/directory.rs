use super::*;
use crate::_tests::support::{create_member, create_role, replace_member_roles};
use axum::Router;
use utoipa::OpenApi;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn create_instance(app: &Router, cookie: &str, csrf: &str) {
    let (status, payload) = request(app, "POST", "/api/console/mcp/instances", cookie, csrf,
        json!({"instance_id":"directory","name":"Directory","description_short":null,"status":"draft","default_entry_path":"/"})).await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
}

fn body(snapshot: Value) -> Value {
    json!({"expected_snapshot":snapshot, "groups":[{
        "path":"/ops", "display_name":"Operations", "description_short":null, "enabled":true, "sort_order":4
    }], "bindings":[]})
}

#[test]
fn directory_typed_openapi_contract_exposes_exact_snapshot_and_replace_fields() {
    let document = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let path = &document["paths"]["/api/console/mcp/instances/{instance_id}/directory"];
    for method in ["get", "put"] {
        assert!(path[method]["summary"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(path[method]["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(path[method]["responses"]["200"].is_object());
    }
    assert!(path["put"]["responses"]["409"].is_object());
    let schemas = &document["components"]["schemas"];
    let mut response_fields = schemas["McpInstanceDirectoryResponse"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    response_fields.sort_unstable();
    assert_eq!(
        response_fields,
        ["bindings", "groups", "instance_record_id"]
    );
    let mut request_fields = schemas["ReplaceMcpInstanceDirectoryBody"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    request_fields.sort_unstable();
    assert_eq!(request_fields, ["bindings", "expected_snapshot", "groups"]);
}

#[tokio::test]
async fn directory_routes_return_committed_snapshot_conflict_error_and_preserve_state_on_failure() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_instance(&app, &cookie, &csrf).await;
    let route = "/api/console/mcp/instances/directory/directory";
    let (status, initial) = request(&app, "GET", route, &cookie, "", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{initial}");
    assert!(uuid::Uuid::parse_str(initial["data"]["instance_record_id"].as_str().unwrap()).is_ok());
    assert_eq!(initial["data"]["groups"], json!([]));
    assert_eq!(initial["data"]["bindings"], json!([]));
    let replacement = body(initial["data"].clone());
    let (status, saved) = request(&app, "PUT", route, &cookie, &csrf, replacement.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"]["groups"][0]["path"], "/ops");
    assert_eq!(saved["data"]["groups"][0]["sort_order"], 4);
    assert_eq!(
        saved["data"]["groups"][0]["instance_record_id"],
        initial["data"]["instance_record_id"]
    );
    assert!(uuid::Uuid::parse_str(saved["data"]["groups"][0]["id"].as_str().unwrap()).is_ok());
    let (status, error) = request(&app, "PUT", route, &cookie, &csrf, replacement).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(error["status"], 409);
    assert_eq!(error["code"], "mcp_directory_snapshot");
    assert!(error["message"].as_str().is_some());
    let mut invalid = body(saved["data"].clone());
    invalid["groups"][0]["id"] = saved["data"]["groups"][0]["id"].clone();
    invalid["groups"][0]["path"] = json!("/missing/child");
    let (status, error) = request(&app, "PUT", route, &cookie, &csrf, invalid).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert_eq!(error["code"], "group_parent");
    assert_eq!(
        request(&app, "GET", route, &cookie, "", Value::Null)
            .await
            .1["data"],
        saved["data"]
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/console/mcp/instances/missing/directory",
            &cookie,
            "",
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, "PUT", route, &cookie, "", body(saved["data"].clone()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(&app, "GET", route, "", "", Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn directory_operations_authorize_view_replace_independently_and_fail_closed() {
    let app = test_app().await;
    let (root, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_instance(&app, &root, &root_csrf).await;
    create_role(&app, &root, &root_csrf, "directory_editor").await;
    let member = create_member(&app, &root, &root_csrf, "directory-editor", "temp-pass").await;
    replace_member_roles(&app, &root, &root_csrf, &member, &["directory_editor"]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "directory-editor", "temp-pass").await;
    let route = "/api/console/mcp/instances/directory/directory";
    let initial = request(&app, "GET", route, &root, "", Value::Null).await.1["data"].clone();
    assert_eq!(
        request(&app, "GET", route, &member_cookie, "", Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let policy_route = "/api/console/settings/roles/directory_editor/console-policy";
    for operation in [
        "get_mcp_catalog",
        "list_mcp_interface_capabilities",
        "list_mcp_items",
        "mcp.directory.view",
        "mcp.directory.replace",
    ] {
        let (status, error) = request(&app, "PUT", policy_route, &root, &root_csrf,
            json!({"groups":[{"kind":"settings_feature","group_id":"system.mcp-management","enabled":true,"strategy":"custom",
                "operations":[{"kind":"simple","operation_id":operation,"enabled":true}]}]})).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{error}");
        let get = request(&app, "GET", route, &member_cookie, "", Value::Null)
            .await
            .0;
        assert_eq!(
            get,
            if operation == "mcp.directory.view" {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        let (put, response) = request(
            &app,
            "PUT",
            route,
            &member_cookie,
            &member_csrf,
            body(initial.clone()),
        )
        .await;
        assert_eq!(
            put,
            if operation == "mcp.directory.replace" {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{response}"
        );
    }
    let (status, error) = request(&app, "PUT", policy_route, &root, &root_csrf,
        json!({"groups":[{"kind":"settings_feature","group_id":"system.mcp-management","enabled":false,"strategy":"full","operations":[]}]})).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{error}");
    assert_eq!(
        request(&app, "GET", route, &member_cookie, "", Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            route,
            &member_cookie,
            &member_csrf,
            body(initial)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}
