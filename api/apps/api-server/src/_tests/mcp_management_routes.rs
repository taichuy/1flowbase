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

async fn create_exposed_published_model(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
) -> String {
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": code,
                        "title": code,
                        "status": "published"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_payload = response_json(create_response).await;
    let model_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/fields"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "order_title",
                        "title": "order_title",
                        "field_kind": "string",
                        "is_required": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_response.status(), StatusCode::CREATED);

    let grant_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "system",
                        "scope_id": domain::SYSTEM_SCOPE_ID,
                        "enabled": true,
                        "permission_profile": "scope_all"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grant_response.status(), StatusCode::CREATED);

    let expose_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "published" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(expose_response.status(), StatusCode::OK);

    model_id
}

#[tokio::test]
async fn mcp_interface_capabilities_include_bindable_runtime_data_model_crud_operations() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id =
        create_exposed_published_model(&app, &root_cookie, &root_csrf, "mcp_ready_orders").await;
    let hidden_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "mcp_hidden_orders",
                        "title": "mcp_hidden_orders",
                        "status": "draft"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hidden_model_response.status(), StatusCode::CREATED);

    let interface_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload = response_json(interface_response).await;
    let entries = interface_payload["data"].as_array().unwrap();

    for (method, path, suffix, risk_level) in [
        (
            "GET",
            "/api/runtime/models/mcp_ready_orders/list",
            "list_records",
            "low",
        ),
        (
            "POST",
            "/api/runtime/models/mcp_ready_orders/create",
            "create_record",
            "high",
        ),
        (
            "GET",
            "/api/runtime/models/mcp_ready_orders/get/{id}",
            "get_record",
            "low",
        ),
        (
            "PATCH",
            "/api/runtime/models/mcp_ready_orders/update/{id}",
            "update_record",
            "high",
        ),
        (
            "DELETE",
            "/api/runtime/models/mcp_ready_orders/delete/{id}",
            "delete_record",
            "critical",
        ),
    ] {
        let entry = entries
            .iter()
            .find(|entry| entry["method"] == json!(method) && entry["path"] == json!(path))
            .unwrap_or_else(|| panic!("missing bindable runtime interface {method} {path}"));
        assert_eq!(
            entry["interface_id"],
            json!(format!("data_model__{model_id}__{suffix}"))
        );
        assert_eq!(entry["bindable"], json!(true));
        assert_eq!(entry["risk_level"], json!(risk_level));
        assert_ne!(
            entry["path"],
            json!("/api/runtime/models/{model_code}/list")
        );
        assert_ne!(
            entry["path"],
            json!("/api/runtime/models/{model_code}/get/{id}")
        );
    }
    let list_entry = entries
        .iter()
        .find(|entry| {
            entry["method"] == json!("GET")
                && entry["path"] == json!("/api/runtime/models/mcp_ready_orders/list")
        })
        .expect("missing bindable runtime list interface");
    assert!(
        list_entry["result_schema"]["properties"]
            .get("items")
            .is_some(),
        "runtime list result_schema should expose RuntimeListResponse.items"
    );
    assert!(
        list_entry["result_schema"]["properties"]
            .get("data")
            .is_none(),
        "runtime list result_schema must not rename RuntimeListResponse.items to data"
    );
    assert!(list_entry["result_schema"]["properties"]
        .get("total")
        .is_some());
    assert!(!entries.iter().any(|entry| entry["path"]
        .as_str()
        .is_some_and(|path| path.contains("mcp_hidden_orders"))));
}

#[tokio::test]
async fn mcp_interface_capabilities_include_system_table_create_operation() {
    let app = test_app().await;
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let interface_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload = response_json(interface_response).await;
    let entries = interface_payload["data"].as_array().unwrap();
    let roles_create_interface = entries
        .iter()
        .find(|entry| {
            entry["method"] == json!("POST")
                && entry["path"] == json!("/api/runtime/models/roles/create")
        })
        .expect("roles create should be exposed as a bindable runtime interface");
    let descriptor_names = roles_create_interface["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|descriptor| descriptor["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(descriptor_names.is_empty());
}

#[tokio::test]
async fn mcp_tool_create_and_update_accept_runtime_data_model_crud_interfaces() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let model_id =
        create_exposed_published_model(&app, &root_cookie, &root_csrf, "mcp_tool_orders").await;
    let create_interface_id = format!("data_model__{model_id}__create_record");
    let update_interface_id = format!("data_model__{model_id}__update_record");

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
                        "tool_id": "mcp_tool_orders_create",
                        "des_id": "des-runtime-create",
                        "name": "Create order",
                        "short_description": "Create order",
                        "full_description": "Create a runtime order record through a concrete Data Model interface.",
                        "interface_id": create_interface_id,
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "medium",
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
    assert_eq!(
        create_tool_payload["data"]["interface_id"].as_str(),
        Some(create_interface_id.as_str())
    );
    assert_eq!(
        create_tool_payload["data"]["operation"].as_str(),
        Some("POST /api/runtime/models/mcp_tool_orders/create")
    );
    assert!(create_tool_payload["data"]
        .get("usage_description")
        .is_none());
    assert!(create_tool_payload["data"].get("audit_policy").is_none());

    let update_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/tools/mcp_tool_orders_create")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Update order",
                        "des_id": "des-runtime-update",
                        "short_description": "Update order",
                        "full_description": "Update a runtime order record through a concrete Data Model interface.",
                        "interface_id": update_interface_id,
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "medium",
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_tool_response.status(), StatusCode::OK);
    let update_tool_payload = response_json(update_tool_response).await;
    assert_eq!(
        update_tool_payload["data"]["interface_id"].as_str(),
        Some(update_interface_id.as_str())
    );
    assert_eq!(
        update_tool_payload["data"]["operation"].as_str(),
        Some("PATCH /api/runtime/models/mcp_tool_orders/update/{id}")
    );
    assert!(update_tool_payload["data"]
        .get("usage_description")
        .is_none());
    assert!(update_tool_payload["data"].get("audit_policy").is_none());

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
    let catalog_tool = catalog_payload["data"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["tool_id"].as_str() == Some("mcp_tool_orders_create"))
        .expect("updated tool should be returned in MCP catalog");
    assert_eq!(
        catalog_tool["operation"].as_str(),
        Some("PATCH /api/runtime/models/mcp_tool_orders/update/{id}")
    );
}

#[tokio::test]
async fn mcp_tool_create_rejects_non_bindable_agent_interface() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

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
    let agent_interface = interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("/api/agent/"))
        })
        .expect("MCP interface catalog should expose /api/agent entries as non-bindable");
    assert_eq!(agent_interface["bindable"], json!(false));
    let agent_interface_id = agent_interface["interface_id"].as_str().unwrap();

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
                        "tool_id": "agent_run_proxy",
                        "des_id": "des-agent",
                        "name": "Agent run proxy",
                        "short_description": "Agent run proxy",
                        "full_description": "This should remain unavailable for MCP binding.",
                        "interface_id": agent_interface_id,
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "low",
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_tool_response.status(), StatusCode::BAD_REQUEST);
    let create_tool_payload = response_json(create_tool_response).await;
    assert_eq!(create_tool_payload["code"].as_str(), Some("interface_id"));
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
        catalog_payload["data"]["discovery_policies"]
            .as_array()
            .unwrap()
            .len(),
        0
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
    let publish_application_api_interface = interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["interface_id"].as_str() == Some("publish_application_api"))
        .expect("MCP interface catalog should expose publish application API");
    assert!(publish_application_api_interface["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |descriptor| descriptor["name"].as_str() == Some("application_id")
                && descriptor["parameter_type"].as_str() == Some("url")
                && descriptor["required"].as_bool() == Some(true)
        ));
    assert!(publish_application_api_interface["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |descriptor| descriptor["name"].as_str() == Some("mapping.input.query_target")
                && descriptor["parameter_type"].as_str() == Some("json_body")
                && descriptor["required"].as_bool() == Some(true)
        ));
    assert!(publish_application_api_interface["parameter_descriptors"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |descriptor| descriptor["name"].as_str() == Some("mapping.output.answer_selector")
                && descriptor["parameter_type"].as_str() == Some("json_body")
                && descriptor["required"].as_bool() == Some(false)
        ));

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
                        "full_description": "Read system runtime topology and locale profile.",
                        "interface_id": "get_runtime_profile",
                        "parameter_schema": { "type": "object", "properties": { "fake": { "type": "string" } } },
                        "result_schema": { "type": "string" },
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": "file_storage.manage.all",
                        "risk_level": "low",
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
    assert!(create_tool_payload["data"]
        .get("usage_description")
        .is_none());
    let first_des_id = create_tool_payload["data"]["des_id"].as_str().unwrap();
    assert_eq!(first_des_id, "des12345");
    assert_eq!(
        create_tool_payload["data"]["des_id_required"].as_bool(),
        Some(false)
    );
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
    assert!(get_tool_payload["data"].get("usage_description").is_none());

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
                        "full_description": "Read system runtime topology and locale profile.",
                        "interface_id": "get_runtime_profile",
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {},
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "low",
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
async fn mcp_instance_discovery_policy_updates_validate_and_isolate_list_behavior() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    for (instance_id, name) in [
        ("workspace_ops", "Workspace Ops"),
        ("workspace_data", "Workspace Data"),
    ] {
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
                            "instance_id": instance_id,
                            "name": name,
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
                        .uri(format!("/api/console/mcp/instances/{instance_id}/groups"))
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
    }

    let invalid_return_fields_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/workspace_ops/discovery-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "secret"]
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

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/workspace_ops/discovery-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "name"]
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
        update_payload["data"]["instance_id"],
        json!("workspace_ops")
    );
    assert_eq!(update_payload["data"]["list_default_limit"], json!(1));
    assert!(update_payload["data"]
        .get("get_include_mapping_summary")
        .is_none());
    assert!(update_payload["data"]
        .get("call_default_des_id_policy")
        .is_none());

    let data_policy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/instances/workspace_data/discovery-policy")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(data_policy_response.status(), StatusCode::OK);
    let data_policy_payload = response_json(data_policy_response).await;
    assert_eq!(
        data_policy_payload["data"]["instance_id"],
        json!("workspace_data")
    );
    assert_eq!(data_policy_payload["data"]["list_default_limit"], json!(50));

    let ops_list_response = app
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
    assert_eq!(ops_list_response.status(), StatusCode::OK);
    let ops_list_payload = response_json(ops_list_response).await;
    let ops_items = ops_list_payload["data"].as_array().unwrap();
    assert_eq!(ops_items.len(), 1);
    assert!(ops_items[0].get("id").is_some());
    assert!(ops_items[0].get("name").is_some());
    assert!(ops_items[0].get("path").is_none());
    assert!(ops_items[0].get("children_count").is_none());

    let data_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_data")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(data_list_response.status(), StatusCode::OK);
    let data_list_payload = response_json(data_list_response).await;
    let data_items = data_list_payload["data"].as_array().unwrap();
    assert_eq!(data_items.len(), 3);
    assert!(data_items[0].get("path").is_some());
    assert!(data_items[0].get("children_count").is_some());

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

    let legacy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/meta-tool-config")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_response.status(), StatusCode::NOT_FOUND);
}
