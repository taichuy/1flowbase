use crate::_tests::support::{login_and_capture_cookie, test_app, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[path = "mcp_management_routes/interface_inventory.rs"]
mod interface_inventory;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn ac_002_ac_006_bundle_source_is_projected_without_blocking_mutations() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "frontstage_browser",
                        "name": "Frontstage Browser",
                        "description_short": null,
                        "status": "enabled",
                        "default_entry_path": "/frontstage"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"
        update mcp_instances
        set managed_bundle_organization='1flowbase',
            managed_bundle_id='frontstage_assistant',
            managed_bundle_version='1.0.2'
        where instance_id='frontstage_browser'
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/instances")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let payload = response_json(list_response).await;
    assert_eq!(
        payload["data"][0]["managed_by"],
        json!({
            "organization": "1flowbase",
            "bundle_id": "frontstage_assistant",
            "bundle_version": "1.0.2"
        })
    );

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/mcp/instances/frontstage_browser")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn mcp_group_path_is_machine_safe_and_display_name_has_a_path_leaf_default() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

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
                        "instance_id": "group_validation",
                        "name": "Group validation",
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

    for invalid_path in [
        "",
        "/",
        "backend_settings",
        "/后台设置",
        "/backend settings",
        "/backend/settings/",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/console/mcp/instances/group_validation/groups")
                    .header("cookie", &root_cookie)
                    .header("x-csrf-token", &root_csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "path": invalid_path,
                            "display_name": "后台设置",
                            "description_short": null,
                            "enabled": true,
                            "sort_order": 0
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response_json(response).await["code"], json!("path"));
    }

    let valid_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/group_validation/groups")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/backend_settings",
                        "display_name": "后台设置",
                        "description_short": null,
                        "enabled": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(valid_response.status(), StatusCode::OK);
    let valid_payload = response_json(valid_response).await;
    assert_eq!(valid_payload["data"]["path"], json!("/backend_settings"));
    assert_eq!(valid_payload["data"]["display_name"], json!("后台设置"));

    let default_name_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/group_validation/groups")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/user_pages",
                        "description_short": null,
                        "enabled": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(default_name_response.status(), StatusCode::OK);
    let default_name_payload = response_json(default_name_response).await;
    assert_eq!(
        default_name_payload["data"]["display_name"],
        json!("user_pages")
    );

    let blank_name_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/group_validation/groups")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/audit_logs",
                        "display_name": "   ",
                        "description_short": null,
                        "enabled": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blank_name_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(blank_name_response).await["data"]["display_name"],
        json!("audit_logs")
    );

    let catalog_response = app
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
    assert!(catalog_payload["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["path"] == json!("/backend_settings")
            && group["display_name"] == json!("后台设置")));
    assert!(catalog_payload["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["path"] == json!("/user_pages")
            && group["display_name"] == json!("user_pages")));
}

#[tokio::test]
async fn mcp_tool_contract_accepts_boolean_json_result_schemas() {
    let response = crate::app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let openapi = response_json(response).await;

    for schema_name in ["McpToolResponse", "CreateMcpToolBody", "UpdateMcpToolBody"] {
        assert_eq!(
            openapi["components"]["schemas"][schema_name]["properties"]["result_schema"],
            json!({}),
            "{schema_name}.result_schema must accept object and boolean JSON Schema forms"
        );
    }
}

#[tokio::test]
async fn mcp_tool_create_and_update_allow_omitting_full_description() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
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
                        "tool_id": "optional_full_description",
                        "des_id": null,
                        "name": "Optional full description",
                        "short_description": "Tool with an optional full description",
                        "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
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
    assert_eq!(create_response.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(create_response).await["data"]["full_description"],
        json!("")
    );

    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/tools/optional_full_description")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "des_id": null,
                        "name": "Optional full description updated",
                        "short_description": "Updated without full description",
                        "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
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
    assert_eq!(update_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(update_response).await["data"]["full_description"],
        json!("")
    );
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
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "template_provider": "core",
                        "template_code": "general",
                        "template_version": "v1",
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
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
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

    let expose_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}"
                ))
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
        Some("settings_feature.access.system.system-runtime")
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
                        "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
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
        Some("settings_feature.access.system.system-runtime")
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
                        "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
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

#[path = "mcp_management_routes/instance_discovery.rs"]
mod instance_discovery;
mod runtime_interfaces;
