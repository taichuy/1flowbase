use crate::_tests::support::{login_and_capture_cookie, test_app};
use api_server::app;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Map, Value};
use tower::ServiceExt;

async fn openapi_paths() -> Map<String, Value> {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    payload["paths"].as_object().cloned().unwrap_or_default()
}

async fn create_member(app: &axum::Router, cookie: &str, csrf: &str, account: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": "temp-pass",
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    payload["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn openapi_contains_runtime_and_model_detail_routes() {
    let paths = openapi_paths().await;

    for route in [
        "/api/console/models/{id}",
        "/api/console/models/agent-flow-options",
        "/api/console/models/{id}/fields",
        "/api/console/models/{id}/advisor-findings",
        "/api/console/models/{id}/scope-grants",
        "/api/console/models/{id}/scope-grants/{grant_id}",
        "/api/console/docs/data-models/{model_id}/openapi.json",
        "/api/console/model-providers/catalog",
        "/api/console/model-providers/options",
        "/api/console/system/runtime-profile",
        "/api/console/user-api-keys",
        "/api/console/user-api-keys/role-options",
        "/api/console/user-api-keys/{api_key_id}/revoke",
        "/api/public/auth/login-instances",
        "/api/console/settings/auth-center/authenticators",
        "/api/console/settings/auth-center/authenticators/{id}/copy",
        "/api/console/settings/auth-center/authenticators/{id}",
        "/api/console/settings/auth-center/authenticators/order",
        "/api/runtime/models/{model_code}/list",
        "/api/runtime/models/{model_code}/get/{id}",
        "/api/runtime/models/{model_code}/create",
        "/api/runtime/models/{model_code}/update/{id}",
        "/api/runtime/models/{model_code}/delete/{id}",
        "/api/console/session/actions/revoke-all",
        "/api/console/me/actions/change-password",
        "/api/console/data-sources/instances/{instance_id}/secret/rotate",
        "/api/console/data-sources/instances/{instance_id}/resources/map-to-model",
        "/api/console/applications/{id}/orchestration/debug-artifacts/{artifact_id}",
        "/v1/messages/count_tokens",
    ] {
        assert!(
            paths.contains_key(route),
            "expected openapi to contain path {route}, got: {:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn openapi_contains_auth_center_lifecycle_schemas() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let components = payload["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    for schema in [
        "CreateAuthCenterAuthenticatorBody",
        "CopyAuthCenterAuthenticatorBody",
        "ReorderAuthCenterAuthenticatorsBody",
        "PublicLoginInstanceResponse",
        "PublicLoginInstancesResponse",
    ] {
        assert!(components.contains_key(schema), "missing schema {schema}");
    }
}

#[tokio::test]
async fn openapi_contains_advisor_and_dynamic_data_model_doc_schemas() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let components = payload["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    for schema in [
        "DataModelAdvisorFindingResponse",
        "DataModelOpenApiDocumentResponse",
    ] {
        assert!(components.contains_key(schema), "missing schema {schema}");
    }

    assert_eq!(
        payload["paths"]["/api/console/models/{id}/advisor-findings"]["get"]["responses"]["200"]
            ["content"]["application/json"]["schema"]["items"]["$ref"]
            .as_str(),
        Some("#/components/schemas/DataModelAdvisorFindingResponse")
    );
    assert!(
        payload["paths"]["/api/console/docs/data-models/{model_id}/openapi.json"]["get"]
            ["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .is_some()
    );
}

#[tokio::test]
async fn openapi_documents_model_mutation_bad_request_responses() {
    let paths = openapi_paths().await;

    for (route, method) in [
        ("/api/console/models", "post"),
        ("/api/console/models/{id}/fields", "post"),
        ("/api/console/models/{id}/fields/{field_id}", "patch"),
    ] {
        assert_eq!(
            paths[route][method]["responses"]["400"]["content"]["application/json"]["schema"]
                ["$ref"]
                .as_str(),
            Some("#/components/schemas/ErrorBody"),
            "expected {method} {route} to document 400 ErrorBody"
        );
    }
}

#[tokio::test]
async fn openapi_omits_legacy_data_model_api_key_create_schemas() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let components = payload["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    for schema in [
        "CreateApiKeyRequest",
        "CreateApiKeyResponse",
        "ApiKeyDataModelPermissionRequest",
        "ApiKeyDataModelPermissionResponse",
    ] {
        assert!(
            !components.contains_key(schema),
            "legacy schema should not be exposed: {schema}"
        );
    }
}

#[tokio::test]
async fn openapi_contains_file_management_routes() {
    let paths = openapi_paths().await;

    for route in [
        "/api/console/settings/files/storages",
        "/api/console/settings/files/tables",
        "/api/console/settings/files/tables/{id}/binding",
        "/api/console/files/upload",
        "/api/console/files/{file_table_id}/records/{record_id}/content",
    ] {
        assert!(paths.contains_key(route), "missing path {route}");
    }
}

#[tokio::test]
async fn openapi_contains_session_csrf_and_patch_me_routes() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let paths = payload["paths"].as_object().cloned().unwrap_or_default();
    let components = payload["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    assert!(paths.contains_key("/api/console/session"));
    assert_eq!(
        paths["/api/console/me"]["patch"]["operationId"].as_str(),
        Some("patch_me")
    );
    assert_eq!(
        paths["/api/console/me/meta"]["patch"]["operationId"].as_str(),
        Some("patch_me_meta")
    );
    assert!(components.contains_key("PatchMeBody"));
    assert!(components.contains_key("PatchMeMetaBody"));
    assert_eq!(
        components["SessionResponse"]["properties"]["csrf_token"]["type"].as_str(),
        Some("string")
    );
}

#[tokio::test]
async fn openapi_contains_workspace_switch_routes() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let paths = payload["paths"].as_object().cloned().unwrap_or_default();
    let components = payload["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    assert!(paths.contains_key("/api/console/workspaces"));
    assert!(paths.contains_key("/api/console/session/actions/switch-workspace"));
    assert!(components.contains_key("WorkspaceSummaryResponse"));
    assert!(components.contains_key("SwitchWorkspaceBody"));
}

#[tokio::test]
async fn openapi_contains_workspace_detail_path_and_omits_team_path() {
    let paths = openapi_paths().await;
    let legacy_path = format!("/api/console/{}", ["te", "am"].concat());

    assert!(paths.contains_key("/api/console/workspace"));
    assert!(!paths.contains_key(&legacy_path));
}

#[tokio::test]
async fn openapi_contains_frontstage_pages_route_and_error_responses() {
    let paths = openapi_paths().await;
    let frontstage_route = paths.get("/api/console/frontstage/{workspace_id}/pages");
    let frontstage_groups_route = paths.get("/api/console/frontstage/{workspace_id}/pages/groups");
    let frontstage_node_route = paths.get("/api/console/frontstage/{workspace_id}/pages/{page_id}");
    let frontstage_move_route =
        paths.get("/api/console/frontstage/{workspace_id}/pages/{page_id}/move");
    let frontstage_content_route =
        paths.get("/api/console/frontstage/{workspace_id}/pages/{page_id}/content");
    let frontstage_block_code_route =
        paths.get("/api/console/frontstage/{workspace_id}/pages/{page_id}/block-codes/{code_ref}");

    assert!(
        frontstage_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages"
    );
    assert!(
        frontstage_groups_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages/groups"
    );
    assert!(
        frontstage_node_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages/{{page_id}}"
    );
    assert!(
        frontstage_move_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages/{{page_id}}/move"
    );
    assert!(
        frontstage_content_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages/{{page_id}}/content"
    );
    assert!(
        frontstage_block_code_route.is_some(),
        "missing path /api/console/frontstage/{{workspace_id}}/pages/{{page_id}}/block-codes/{{code_ref}}"
    );

    let get_op = &frontstage_route.unwrap()["get"];
    let post_page_op = &frontstage_route.unwrap()["post"];
    let post_group_op = &frontstage_groups_route.unwrap()["post"];
    let detail_get_op = &frontstage_node_route.unwrap()["get"];
    let patch_op = &frontstage_node_route.unwrap()["patch"];
    let delete_op = &frontstage_node_route.unwrap()["delete"];
    let move_op = &frontstage_move_route.unwrap()["post"];
    let content_put_op = &frontstage_content_route.unwrap()["put"];
    let block_get_op = &frontstage_block_code_route.unwrap()["get"];
    let block_put_op = &frontstage_block_code_route.unwrap()["put"];

    assert!(get_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(post_page_op["responses"]["201"]["content"]["application/json"]["schema"].is_object());
    assert!(post_group_op["responses"]["201"]["content"]["application/json"]["schema"].is_object());
    assert!(detail_get_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(patch_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(move_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(
        content_put_op["responses"]["200"]["content"]["application/json"]["schema"].is_object()
    );
    assert_eq!(
        content_put_op["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        json!("#/components/schemas/SaveFrontstagePageContentBody")
    );
    assert!(block_get_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(block_put_op["responses"]["200"]["content"]["application/json"]["schema"].is_object());
    assert!(delete_op["responses"]["204"].is_object());
    assert_eq!(
        get_op["responses"]["400"]["content"]["application/json"]["schema"]["$ref"],
        json!("#/components/schemas/ErrorBody")
    );
    assert_eq!(
        get_op["responses"]["401"]["content"]["application/json"]["schema"]["$ref"],
        json!("#/components/schemas/ErrorBody")
    );
    assert_eq!(
        get_op["responses"]["403"]["content"]["application/json"]["schema"]["$ref"],
        json!("#/components/schemas/ErrorBody")
    );
}

#[tokio::test]
async fn openapi_contains_application_console_routes() {
    let paths = openapi_paths().await;

    for route in [
        "/api/console/applications",
        "/api/console/applications/{id}",
        "/api/console/applications/{id}/environment-variables",
        "/api/console/applications/{id}/orchestration",
        "/api/console/applications/{id}/orchestration/draft",
        "/api/console/applications/{id}/orchestration/versions/{version_id}",
        "/api/console/applications/{id}/orchestration/versions/{version_id}/restore",
        "/api/console/applications/{id}/orchestration/nodes/{node_id}/debug-runs",
        "/api/console/applications/{id}/orchestration/nodes/{node_id}/last-run",
        "/api/console/applications/{id}/logs/runs",
        "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree",
        "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes",
        "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/content",
        "/api/console/applications/{id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/details/{detail_ref_id}",
        "/api/console/applications/{id}/logs/runs/{run_id}/resume-timeline",
        "/api/console/applications/{id}/logs/runs/{run_id}/nodes/{node_id}",
    ] {
        assert!(paths.contains_key(route), "missing path {route}");
    }
}

#[tokio::test]
async fn openapi_plugin_descriptions_drop_compatibility_wording() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let serialized = serde_json::to_string(&payload).unwrap();

    assert!(!serialized.contains("compatible with future generic plugin kinds"));
    assert!(!serialized.contains("for compatibility"));
    assert!(!serialized.contains("provider-only plugin packages"));
}

#[tokio::test]
async fn openapi_excludes_legacy_member_mutation_routes() {
    let paths = openapi_paths().await;
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let action_member_id = create_member(&app, &cookie, &csrf, "action-member").await;
    let legacy_member_id = create_member(&app, &cookie, &csrf, "legacy-member").await;

    for route in [
        "/api/console/settings/members/{id}/disable",
        "/api/console/settings/members/{id}/reset-password",
    ] {
        assert!(
            !paths.contains_key(route),
            "expected openapi to exclude legacy path {route}"
        );
    }

    let member_mutation_paths = paths
        .keys()
        .filter(|route| {
            route.starts_with("/api/console/settings/members/{id}/")
                && (route.contains("disable") || route.contains("reset-password"))
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        member_mutation_paths,
        vec![
            "/api/console/settings/members/{id}/disable".to_string(),
            "/api/console/settings/members/{id}/reset-password".to_string(),
        ]
    );

    let action_reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{action_member_id}/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "new_password": "next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action_reset_response.status(), StatusCode::NO_CONTENT);

    let action_disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{action_member_id}/disable"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action_disable_response.status(), StatusCode::NO_CONTENT);

    let legacy_reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{legacy_member_id}/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "new_password": "legacy-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_reset_response.status(), StatusCode::NOT_FOUND);

    let legacy_disable_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{legacy_member_id}/disable"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_disable_response.status(), StatusCode::NOT_FOUND);
}
