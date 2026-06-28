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

#[tokio::test]
async fn mcp_management_routes_read_empty_catalog_without_seeding_default_instance() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/catalog")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog_payload = response_json(catalog_response).await;
    assert!(catalog_payload["data"].get("default_instance").is_none());
    assert_eq!(
        catalog_payload["data"]["instances"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        catalog_payload["data"]["meta_tool_config"]["list_default_limit"].as_i64(),
        Some(50)
    );

    let create_instance_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "workspace_ops",
                        "name": "Workspace Ops",
                        "description_short": "Workspace MCP instance",
                        "status": "enabled",
                        "default_entry_path": "/"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_instance_response.status(), StatusCode::CREATED);

    let interface_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload = response_json(interface_response).await;
    let runtime_profile_interface = interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["interface_id"].as_str() == Some("get_runtime_profile"))
        .expect("MCP interface catalog should expose real OpenAPI operations");
    assert_eq!(runtime_profile_interface["method"].as_str(), Some("GET"));
    assert_eq!(
        runtime_profile_interface["path"].as_str(),
        Some("/api/console/system/runtime-profile")
    );
    assert_eq!(
        runtime_profile_interface["permission_code"].as_str(),
        Some("system_runtime.view.all")
    );
    assert_eq!(runtime_profile_interface["bindable"].as_bool(), Some(true));
    assert!(
        runtime_profile_interface["parameter_schema"]["properties"]["query"]["properties"]
            .get("locale")
            .is_some()
    );
    assert!(runtime_profile_interface["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|descriptor| descriptor["name"].as_str() == Some("locale")
            && descriptor["parameter_type"].as_str() == Some("url")
            && descriptor["required"].as_bool() == Some(false)
            && descriptor["field_type"].as_str().is_some()
            && descriptor["schema"].is_object()));
    assert!(runtime_profile_interface["result_schema"]["properties"]
        .get("topology")
        .is_some());
    let application_api_docs_interface = interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["interface_id"].as_str() == Some("get_application_api_docs_catalog"))
        .expect("MCP interface catalog should expose application API docs catalog");
    assert_eq!(
        application_api_docs_interface["permission_code"].as_str(),
        Some("application.view.all")
    );

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "runtime_profile",
                        "des_id": "des12345",
                        "name": "Runtime profile",
                        "short_description": "Runtime profile",
                        "usage_description": "Read runtime profile",
                        "full_description": "Read system runtime topology and locale profile.",
                        "interface_id": "get_runtime_profile",
                        "parameter_schema": { "type": "object", "properties": { "fake": { "type": "string" } } },
                        "result_schema": { "type": "string" },
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": "file_storage.manage.all",
                        "risk_level": "low",
                        "audit_policy": { "enabled": true },
                        "des_id_required": true,
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);
    let create_tool_payload = response_json(create_tool_response).await;
    let tool_id = create_tool_payload["data"]["tool_id"].as_str().unwrap();
    assert_eq!(tool_id, "runtime_profile");
    let first_des_id = create_tool_payload["data"]["des_id"].as_str().unwrap();
    assert_eq!(first_des_id, "des12345");
    assert_eq!(
        create_tool_payload["data"]["permission_code"].as_str(),
        Some("system_runtime.view.all")
    );
    assert_eq!(
        create_tool_payload["data"]["risk_level"].as_str(),
        Some("low")
    );
    assert!(
        create_tool_payload["data"]["parameter_schema"]["properties"]["query"]["properties"]
            .get("locale")
            .is_some()
    );
    assert!(
        create_tool_payload["data"]["parameter_schema"]["properties"]
            .get("fake")
            .is_none()
    );

    let upsert_group_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/workspace_ops/groups")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/system",
                        "display_name": "System",
                        "description_short": "System tools",
                        "enabled": true,
                        "sort_order": 1
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upsert_group_response.status(), StatusCode::OK);

    let get_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/mcp/tools/{tool_id}"))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_tool_response.status(), StatusCode::OK);
    let get_tool_payload = response_json(get_tool_response).await;
    assert_eq!(get_tool_payload["data"]["tool_id"].as_str(), Some(tool_id));

    let refresh_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/mcp/tools/{tool_id}/description/refresh"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_payload = response_json(refresh_response).await;
    assert_ne!(
        refresh_payload["data"]["des_id"].as_str().unwrap(),
        first_des_id
    );

    let directory_export_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/instances/export")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(directory_export_response.status(), StatusCode::OK);
    let directory_export_payload = response_json(directory_export_response).await;
    assert!(directory_export_payload["data"].get("tools").is_none());
    assert!(directory_export_payload["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["path"].as_str() == Some("/system")));

    let delete_group_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/mcp/instances/workspace_ops/groups?path=%2Fsystem")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_group_response.status(), StatusCode::NO_CONTENT);

    let delete_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/mcp/tools/{tool_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_tool_response.status(), StatusCode::NO_CONTENT);

    let missing_get_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/mcp/tools/{tool_id}"))
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_get_tool_response.status(), StatusCode::NOT_FOUND);

    let missing_description_check_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/mcp/tools/{tool_id}/description-check"
                ))
                .header("cookie", &root_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "des_id": first_des_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        missing_description_check_response.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn mcp_tool_create_requires_tool_id() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Runtime profile",
                        "short_description": "Runtime profile",
                        "usage_description": "Read runtime profile",
                        "full_description": "Read system runtime topology and locale profile.",
                        "interface_id": "get_runtime_profile",
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "low",
                        "audit_policy": { "enabled": true },
                        "des_id_required": true,
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        create_tool_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn mcp_meta_tool_config_updates_validate_and_shape_list_defaults() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/catalog")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);

    let create_instance_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "workspace_ops",
                        "name": "Workspace Ops",
                        "description_short": null,
                        "status": "enabled",
                        "default_entry_path": "/"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_instance_response.status(), StatusCode::CREATED);

    for (path, display_name, sort_order) in [
        ("/system", "System", 1),
        ("/system/runtime", "Runtime", 2),
        ("/ops", "Operations", 3),
    ] {
        let upsert_group_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/console/mcp/instances/workspace_ops/groups")
                    .header("cookie", &root_cookie)
                    .header("x-csrf-token", &root_csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "path": path,
                            "display_name": display_name,
                            "description_short": null,
                            "enabled": true,
                            "sort_order": sort_order
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upsert_group_response.status(), StatusCode::OK);
    }

    let invalid_return_fields_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/meta-tool-config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "secret"],
                        "get_include_mapping_summary": true,
                        "get_include_interface_summary": true,
                        "call_default_des_id_policy": "required",
                        "call_high_risk_requires_des_id": true,
                        "call_validation_error_format": "field_errors"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        invalid_return_fields_response.status(),
        StatusCode::BAD_REQUEST
    );
    let invalid_return_fields_payload = response_json(invalid_return_fields_response).await;
    assert_eq!(
        invalid_return_fields_payload["code"].as_str(),
        Some("list_return_fields")
    );

    let invalid_des_policy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/meta-tool-config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "name"],
                        "get_include_mapping_summary": true,
                        "get_include_interface_summary": true,
                        "call_default_des_id_policy": "frontend_only",
                        "call_high_risk_requires_des_id": true,
                        "call_validation_error_format": "field_errors"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        invalid_des_policy_response.status(),
        StatusCode::BAD_REQUEST
    );
    let invalid_des_policy_payload = response_json(invalid_des_policy_response).await;
    assert_eq!(
        invalid_des_policy_payload["code"].as_str(),
        Some("call_default_des_id_policy")
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/meta-tool-config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "name"],
                        "get_include_mapping_summary": true,
                        "get_include_interface_summary": true,
                        "call_default_des_id_policy": "required",
                        "call_high_risk_requires_des_id": true,
                        "call_validation_error_format": "field_errors"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload = response_json(update_response).await;
    assert_eq!(
        update_payload["data"]["call_default_des_id_policy"].as_str(),
        Some("required")
    );
    assert_eq!(
        update_payload["data"]["call_validation_error_format"].as_str(),
        Some("field_errors")
    );

    let instance_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(instance_list_response.status(), StatusCode::OK);
    let instance_list_payload = response_json(instance_list_response).await;
    let instance_items = instance_list_payload["data"].as_array().unwrap();
    assert_eq!(instance_items.len(), 1);
    assert!(instance_items[0].get("id").is_some());
    assert!(instance_items[0].get("name").is_some());
    assert!(instance_items[0].get("path").is_none());
    assert!(instance_items[0].get("children_count").is_none());

    let regex_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops&limit=10&path_regex=%5E%2Fsystem")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(regex_list_response.status(), StatusCode::OK);
    let regex_list_payload = response_json(regex_list_response).await;
    let regex_items = regex_list_payload["data"].as_array().unwrap();
    assert_eq!(regex_items.len(), 1);
    assert_eq!(regex_items[0]["name"].as_str(), Some("System"));

    let long_regex_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops&path_regex=%5E%2Fsystem%2Fruntime-long")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(long_regex_response.status(), StatusCode::BAD_REQUEST);
    let long_regex_payload = response_json(long_regex_response).await;
    assert_eq!(long_regex_payload["code"].as_str(), Some("path_regex"));
}
