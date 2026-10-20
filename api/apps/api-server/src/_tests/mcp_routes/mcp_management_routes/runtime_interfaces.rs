use super::*;

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
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "template_provider": "core",
                        "template_code": "general",
                        "template_version": "v1",
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
    assert!(!entries.iter().any(|entry| {
        entry["path"]
            .as_str()
            .is_some_and(|path| path.contains("mcp_hidden_orders"))
    }));
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
    assert!(!descriptor_names.is_empty());
    assert!(roles_create_interface["parameter_schema"]["properties"]
        .get("body")
        .is_some());
}

#[tokio::test]
async fn mcp_interface_capabilities_share_complete_runtime_model_crud() {
    // Root AC-003: MCP and Studio consume the same dynamic OpenAPI inventory.
    let app = test_app().await;
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
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
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    let entries = payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.contains("/application_conversations/"))
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 5, "{payload}");
    assert!(entries.iter().any(|entry| entry["method"] == json!("POST")));
    assert!(entries
        .iter()
        .any(|entry| entry["method"] == json!("PATCH")));
    assert!(entries
        .iter()
        .any(|entry| entry["method"] == json!("DELETE")));
    let list = entries
        .iter()
        .find(|entry| entry["path"].as_str().unwrap().ends_with("/list"))
        .unwrap();
    assert_eq!(
        list["parameter_schema"]["properties"]["query"]["properties"]["filter"]["type"],
        json!("string")
    );
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
                        "execution_target": {"kind":"interface_wrapper","interface_id":create_interface_id},
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
        create_tool_payload["data"]["execution_target"]["interface_id"].as_str(),
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
                        "execution_target": {"kind":"interface_wrapper","interface_id":update_interface_id},
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
        update_tool_payload["data"]["execution_target"]["interface_id"].as_str(),
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
                        "execution_target": {"kind":"interface_wrapper","interface_id":agent_interface_id},
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
