use std::collections::BTreeMap;

use axum::{
    body::{to_bytes, Body},
    http::{header::COOKIE, HeaderMap, HeaderValue, Request, StatusCode},
};
use control_plane::ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput};
use orchestration_runtime::{
    compiled_plan::CompiledNode, execution_engine::RuntimeInternalToolInvoker,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::_tests::support::{
    create_member, login_and_capture_cookie, seed_workspace, test_api_state_with_database_url,
    test_app,
};
use crate::{
    middleware::require_session::require_session,
    routes::mcp_protocol::virtual_ui::ApiMcpRuntimeToolInvoker,
};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_api_key(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/user-api-keys")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"mcp client","expiration_policy":"never"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_mcp_instance(app: &axum::Router, cookie: &str, csrf: &str) {
    create_named_mcp_instance(app, cookie, csrf, "taichuy").await;
}

async fn create_named_mcp_instance(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    instance_id: &str,
) {
    let response = app.clone().oneshot(Request::builder()
        .method("POST").uri("/api/console/mcp/instances")
        .header("cookie", cookie).header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"instance_id":instance_id,"name":instance_id,"description_short":null,"status":"enabled","default_entry_path":"/"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        response_json(response).await
    );
}

#[tokio::test]
async fn ac_003_protocol_uses_api_key_actor_workspace_and_rejects_cross_workspace_instance() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let secondary_workspace_id = seed_workspace(&database_url, "MCP credential workspace").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_named_mcp_instance(&app, &cookie, &csrf, "bootstrap-only").await;

    let switch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"workspace_id":secondary_workspace_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switch_response.status(), StatusCode::OK);
    let secondary_csrf = response_json(switch_response).await["data"]["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    create_mcp_instance(&app, &cookie, &secondary_csrf).await;
    let secondary_token = create_api_key(&app, &cookie, &secondary_csrf).await;

    let scoped = call_mcp(
        &app,
        &secondary_token,
        json!({"jsonrpc":"2.0","id":101,"method":"initialize","params":{}}),
    )
    .await;
    assert_eq!(scoped["result"]["serverInfo"]["name"], json!("taichuy"));

    let mismatch = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mcp/bootstrap-only")
                .header("authorization", format!("Bearer {secondary_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":102,"method":"initialize","params":{}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatch.status(), StatusCode::NOT_FOUND);
}

async fn create_interface_tool_and_binding(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    tool_id: &str,
    interface_id: &str,
    input_mapping: Value,
) {
    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": tool_id,
                        "des_id": format!("{tool_id}_description"),
                        "name": tool_id,
                        "short_description": format!("Execute {tool_id}."),
                        "full_description": "",
                        "execution_target": {
                            "kind": "interface_wrapper",
                            "interface_id": interface_id
                        },
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": input_mapping,
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
    let status = create_tool_response.status();
    let payload = response_json(create_tool_response).await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/runtime",
                        "tool_id": tool_id,
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);
}

async fn call_mcp(app: &axum::Router, token: &str, request: Value) -> Value {
    call_mcp_instance(app, token, "taichuy", request).await
}

async fn call_mcp_instance(
    app: &axum::Router,
    token: &str,
    instance_id: &str,
    request: Value,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/mcp/{instance_id}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

#[tokio::test]
async fn ac_005_builtin_frontstage_source_tools_are_discoverable_and_callable() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, HeaderValue::from_str(&cookie).unwrap());
    let context = require_session(&state, &headers).await.unwrap();
    let workspace_id = context.actor.current_workspace_id;
    let interface_catalog =
        crate::openapi_interface::build_openapi_capability_catalog(&state, workspace_id)
            .await
            .unwrap()
            .into_iter()
            .map(crate::routes::mcp_management::mcp_interface_entry_from_capability)
            .collect();
    ExtensionInstallationRepository::upsert_extension_installation(
        &state.store,
        &UpsertExtensionInstallationInput {
            installation_id: uuid::Uuid::now_v7(),
            identity: domain::ExtensionInstallationIdentity {
                category: domain::ExtensionCategory::Mcp,
                organization: "1flowbase".into(),
                artifact_id: "frontstage_assistant".into(),
                version: "1.1.0".into(),
            },
            node_id: "test-node".into(),
            source_kind: "builtin".into(),
            trust_level: "verified_official".into(),
            local_path: "/tmp/frontstage-assistant.tar.gz".into(),
            expected_checksum: Some("sha256:test".into()),
            local_checksum: "sha256:test".into(),
            signature_status: domain::ExtensionSignatureStatus::Verified,
            signature_algorithm: Some("builtin-code-shipped".into()),
            signing_key_id: Some("1flowbase-builtin".into()),
            warnings: Vec::new(),
            receipt: json!({"kind": "builtin"}),
            application_action: domain::ExtensionApplicationAction::ImportMcp,
            status: domain::ExtensionInstallationStatus::Installed,
            is_current: true,
            created_by: context.actor.user_id,
        },
    )
    .await
    .unwrap();
    control_plane::mcp_management::McpManagementService::new(state.store.clone())
        .seed_builtin_bundle_once(control_plane::mcp_bundle::SeedBuiltinMcpBundleCommand {
            actor_user_id: context.actor.user_id,
            workspace_id,
            package: crate::official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package().unwrap(),
            interface_catalog,
        })
        .await
        .unwrap();
    let workspace_id = workspace_id.to_string();
    let create_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/frontstage/{workspace_id}/pages"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "MCP source page",
                        "rank": "a",
                        "placement": "topbar",
                        "slug": "mcp-source-page"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_page.status(), StatusCode::CREATED);
    let page = response_json(create_page).await;
    let page_id = page["data"]["page"]["id"].as_str().unwrap();
    let tab_id = page["data"]["default_tab"]["id"].as_str().unwrap();
    let create_block = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tab_id": tab_id,
                        "title": "MCP source block",
                        "description": null,
                        "presentation": "inline",
                        "parent_block_id": null,
                        "before_block_id": null,
                        "after_block_id": null,
                        "source_code": "alpha\nbeta\ngamma",
                        "runtime_descriptor": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_block.status(), StatusCode::CREATED);
    let block = response_json(create_block).await;
    let block_id = block["data"]["block_id"].as_str().unwrap();
    let token = create_api_key(&app, &cookie, &csrf).await;

    let list = call_mcp_instance(
        &app,
        &token,
        "frontstage_browser",
        json!({
            "jsonrpc":"2.0",
            "id":201,
            "method":"tools/call",
            "params":{"name":"mcp_list","arguments":{"path":"/frontstage","keywords":["源码"],"path_regex":"","depth":1,"limit":20}}
        }),
    )
    .await;
    let listed = list["result"]["structuredContent"].as_array().unwrap();
    assert!(listed
        .iter()
        .any(|item| item["id"] == json!("frontstage_read_block_source_fragment")));
    assert!(listed
        .iter()
        .any(|item| item["id"] == json!("frontstage_patch_block_source")));

    let get = call_mcp_instance(
        &app,
        &token,
        "frontstage_browser",
        json!({
            "jsonrpc":"2.0",
            "id":202,
            "method":"tools/call",
            "params":{"name":"mcp_get","arguments":{"tool_id":"frontstage_read_block_source_fragment"}}
        }),
    )
    .await;
    let input_schema = &get["result"]["structuredContent"]["input_schema"];
    assert!(input_schema["properties"].get("page_id").is_some());
    assert!(input_schema["properties"].get("block_id").is_some());
    assert!(input_schema["properties"].get("workspace_id").is_none());

    let read = call_mcp_instance(
        &app,
        &token,
        "frontstage_browser",
        json!({
            "jsonrpc":"2.0",
            "id":203,
            "method":"tools/call",
            "params":{"name":"mcp_call","arguments":{"tool_id":"frontstage_read_block_source_fragment","arguments":{"page_id":page_id,"block_id":block_id,"start_line":2,"line_count":1,"max_chars":20}}}
        }),
    )
    .await;
    assert_eq!(read["result"]["isError"], json!(false), "{read}");
    let fragment = &read["result"]["structuredContent"];
    assert_eq!(fragment["source_fragment"], json!("beta\n"));
    let revision = fragment["source_revision"].as_str().unwrap();

    let patch = call_mcp_instance(
        &app,
        &token,
        "frontstage_browser",
        json!({
            "jsonrpc":"2.0",
            "id":204,
            "method":"tools/call",
            "params":{"name":"mcp_call","arguments":{"tool_id":"frontstage_patch_block_source","max_inline_chars":12000,"arguments":{"page_id":page_id,"block_id":block_id,"expected_source_revision":revision,"edits":[{"start_line":2,"start_column":1,"end_line":2,"end_column":5,"replacement":"changed"}]}}}
        }),
    )
    .await;
    assert_eq!(patch["result"]["isError"], json!(false), "{patch}");
    assert_eq!(
        patch["result"]["structuredContent"]["source_code"],
        json!("alpha\nchanged\ngamma"),
        "{patch}"
    );
}

fn runtime_mcp_test_node() -> CompiledNode {
    CompiledNode {
        node_id: "node-llm".to_string(),
        node_type: "llm".to_string(),
        alias: "LLM".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

async fn runtime_mcp_invoker(
    state: std::sync::Arc<crate::app_state::ApiState>,
    cookie: &str,
) -> ApiMcpRuntimeToolInvoker {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, HeaderValue::from_str(cookie).unwrap());
    let context = require_session(&state, &headers).await.unwrap();
    ApiMcpRuntimeToolInvoker::new(state, headers, context.actor, vec!["taichuy".to_string()])
        .await
        .unwrap()
}

async fn invoke_runtime_create_model(
    invoker: &ApiMcpRuntimeToolInvoker,
    model_code: &str,
) -> orchestration_runtime::execution_engine::RuntimeInternalToolOutput {
    let node = runtime_mcp_test_node();
    let registration = invoker
        .registrations_for_node(&node)
        .into_iter()
        .find(|registration| registration.provider_name == "taichuy_mcp_call")
        .expect("runtime registration should expose the selected MCP call tool");

    invoker
        .invoke_runtime_internal_tool(
            &node,
            &registration,
            json!({
                "tool_id": "create_model_probe",
                "max_inline_chars": 12000,
                "arguments": {
                    "body": {
                        "code": model_code,
                        "title": "Delegated model",
                        "scope_kind": "workspace",
                        "template_provider": "core",
                        "template_code": "general",
                        "template_version": "v1"
                    }
                }
            }),
        )
        .await
        .unwrap()
}

async fn create_model_probe_tool(app: &axum::Router, cookie: &str, csrf: &str) {
    create_interface_tool_and_binding(
        app,
        cookie,
        csrf,
        "create_model_probe",
        "create_model",
        json!({
            "mappings": [
                {
                    "interface_param": "code",
                    "mcp_param": "body.code",
                    "required": true
                },
                {
                    "interface_param": "title",
                    "mcp_param": "body.title",
                    "required": true
                },
                {
                    "interface_param": "scope_kind",
                    "mcp_param": "body.scope_kind",
                    "required": true
                },
                {
                    "interface_param": "template_provider",
                    "mcp_param": "body.template_provider",
                    "required": true
                },
                {
                    "interface_param": "template_code",
                    "mcp_param": "body.template_code",
                    "required": true
                },
                {
                    "interface_param": "template_version",
                    "mcp_param": "body.template_version",
                    "required": true
                }
            ]
        }),
    )
    .await;
}

#[tokio::test]
async fn ac_001_runtime_mcp_write_uses_server_delegation_without_browser_csrf() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &root_cookie, &root_csrf).await;
    create_model_probe_tool(&app, &root_cookie, &root_csrf).await;

    let invoker = runtime_mcp_invoker(state, &root_cookie).await;
    let output = invoke_runtime_create_model(&invoker, "runtime_delegated_model_root").await;

    assert!(!output.is_error, "{}", output.content);
    assert_eq!(
        output.content["structuredContent"]["code"],
        json!("runtime_delegated_model_root")
    );
}

#[tokio::test]
async fn ac_002_runtime_mcp_write_keeps_console_operation_authorization() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &root_cookie, &root_csrf).await;
    create_model_probe_tool(&app, &root_cookie, &root_csrf).await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "runtime-mcp-member",
        "temp-pass",
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "runtime-mcp-member", "temp-pass").await;

    let invoker = runtime_mcp_invoker(state, &member_cookie).await;
    let output = invoke_runtime_create_model(&invoker, "runtime_delegated_model_forbidden").await;

    assert!(output.is_error, "{}", output.content);
    assert_eq!(output.content["data"]["http_status"], json!(403));
    assert_eq!(
        output.content["data"]["category"],
        json!("target_authorization")
    );
    assert_eq!(
        output.content["data"]["target_code"],
        json!("console_operation_permission_denied")
    );
}

#[tokio::test]
async fn ac_003_runtime_mcp_write_rejects_revoked_session_delegation() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &root_cookie, &root_csrf).await;
    create_model_probe_tool(&app, &root_cookie, &root_csrf).await;
    let invoker = runtime_mcp_invoker(state, &root_cookie).await;

    let revoked = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/revoke-all")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);

    let output = invoke_runtime_create_model(&invoker, "runtime_delegated_model_revoked").await;

    assert!(output.is_error, "{}", output.content);
    assert_eq!(output.content["data"]["http_status"], json!(401));
    assert_eq!(
        output.content["data"]["category"],
        json!("target_authentication")
    );
    assert_eq!(
        output.content["data"]["target_code"],
        json!("not_authenticated")
    );
}

fn assert_meta_tools(payload: &Value) {
    let tools = payload["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["mcp_list", "mcp_get", "mcp_result", "mcp_call"]
    );
    assert!(tools
        .iter()
        .all(|tool| tool["inputSchema"]["type"] == json!("object")));
}

#[tokio::test]
async fn mcp_initialize_requires_api_key_and_returns_protocol_capabilities() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    let request_body = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}});

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mcp/taichuy")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mcp/taichuy")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["jsonrpc"], json!("2.0"));
    assert_eq!(payload["id"], json!(1));
    assert_eq!(payload["result"]["protocolVersion"], json!("2025-03-26"));
    assert!(payload["result"]["capabilities"]["tools"].is_object());
}

#[tokio::test]
async fn mcp_tools_list_always_returns_four_meta_tools() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;

    let empty_instance_payload = call_mcp(
        &app,
        &token,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await;
    assert_meta_tools(&empty_instance_payload);

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "runtime_profile",
                        "des_id": "des12345",
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
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/",
                        "tool_id": "runtime_profile",
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    let bound_instance_payload = call_mcp(
        &app,
        &token,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
    )
    .await;
    assert_meta_tools(&bound_instance_payload);
    assert!(bound_instance_payload["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["name"] != json!("runtime_profile")));
}

#[tokio::test]
async fn mcp_meta_tools_progressively_disclose_only_visible_instance_tools() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "runtime_profile",
                        "des_id": "des12345",
                        "name": "Runtime profile",
                        "short_description": "Runtime topology summary",
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
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);

    let create_group_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/groups")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/runtime",
                        "display_name": "Runtime",
                        "description_short": "Runtime topology tools",
                        "enabled": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_group_response.status(), StatusCode::OK);

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/runtime",
                        "tool_id": "runtime_profile",
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    let list_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":4,
            "method":"tools/call",
            "params":{
                "name":"mcp_list",
                "arguments":{"path":"/","keywords":["runtime","topology"],"path_regex":"","depth":1,"limit":10}
            }
        }),
    )
    .await;
    let listed_items = list_payload["result"]["structuredContent"]
        .as_array()
        .unwrap();
    assert_eq!(listed_items.len(), 2);
    let group = listed_items
        .iter()
        .find(|item| item["item_kind"] == json!("group"))
        .unwrap();
    assert_eq!(group["path"], json!("/runtime"));
    assert_eq!(group["children_count"], json!(1));
    let listed_tool = listed_items
        .iter()
        .find(|item| item["item_kind"] == json!("tool"))
        .unwrap();
    assert_eq!(listed_tool["id"], json!("runtime_profile"));
    assert_eq!(listed_tool["children_count"], json!(0));
    assert!(listed_tool.get("full_description").is_none());

    let get_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":5,
            "method":"tools/call",
            "params":{"name":"mcp_get","arguments":{"tool_id":"runtime_profile"}}
        }),
    )
    .await;
    let tool = &get_payload["result"]["structuredContent"];
    assert_eq!(tool["tool_id"], json!("runtime_profile"));
    assert_eq!(tool["des_id"], json!("des12345"));
    assert_eq!(
        tool["full_description"],
        json!("Read system runtime topology and locale profile.")
    );
    assert!(tool["input_schema"].is_object());

    let direct_call_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":6,
            "method":"tools/call",
            "params":{"name":"runtime_profile","arguments":{}}
        }),
    )
    .await;
    assert_eq!(direct_call_payload["error"]["code"], json!(-32601));

    let legacy_dotted_name_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":61,
            "method":"tools/call",
            "params":{"name":"mcp.list","arguments":{}}
        }),
    )
    .await;
    assert_eq!(legacy_dotted_name_payload["error"]["code"], json!(-32601));

    let call_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":7,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{"tool_id":"runtime_profile","arguments":{}}
            }
        }),
    )
    .await;
    assert_eq!(call_payload["result"]["isError"], json!(false));
    assert!(call_payload["result"]["structuredContent"].is_object());
    assert_eq!(
        serde_json::from_str::<Value>(
            call_payload["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        call_payload["result"]["structuredContent"]
    );

    let missing_tool_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":8,
            "method":"tools/call",
            "params":{"name":"mcp_get","arguments":{"tool_id":"not_visible"}}
        }),
    )
    .await;
    assert_eq!(missing_tool_payload["error"]["code"], json!(-32602));
}

#[tokio::test]
async fn mcp_get_projects_input_mapping_into_agent_schema() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "mapped_interface_catalog",
                        "des_id": "mapped123",
                        "name": "Mapped interface catalog",
                        "short_description": "List bindable interfaces.",
                        "full_description": "",
                        "execution_target": {
                            "kind": "interface_wrapper",
                            "interface_id": "list_mcp_interface_capabilities"
                        },
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {
                            "interface_parameters": [{
                                "name": "bindable_only",
                                "field_type": "boolean",
                                "parameter_type": "url",
                                "description": "Backend interface description",
                                "required": false
                            }],
                            "mappings": [{
                                "interface_param": "bindable_only",
                                "mcp_param": "filters.only_bindable",
                                "description": "Only include interfaces that can be bound.",
                                "required": true
                            }]
                        },
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
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/",
                        "tool_id": "mapped_interface_catalog",
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    let get_payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":9,
            "method":"tools/call",
            "params":{
                "name":"mcp_get",
                "arguments":{"tool_id":"mapped_interface_catalog"}
            }
        }),
    )
    .await;

    assert_eq!(
        get_payload["result"]["structuredContent"]["input_schema"],
        json!({
            "type": "object",
            "required": ["filters"],
            "properties": {
                "filters": {
                    "type": "object",
                    "required": ["only_bindable"],
                    "properties": {
                        "only_bindable": {
                            "type": ["boolean", "null"],
                            "description": "Only include interfaces that can be bound."
                        }
                    },
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        })
    );
}

#[tokio::test]
async fn mcp_call_routes_large_interface_catalog_with_boolean_schemas_to_continuation() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "interface_catalog",
                        "des_id": "catalog123",
                        "name": "Interface catalog",
                        "short_description": "List bindable interfaces.",
                        "full_description": "Lists the current MCP interface catalog.",
                        "execution_target": {
                            "kind": "interface_wrapper",
                            "interface_id": "list_mcp_interface_capabilities"
                        },
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": { "mappings": [] },
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
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/runtime",
                        "tool_id": "interface_catalog",
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    let payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "mcp_call",
                "arguments": { "tool_id": "interface_catalog", "arguments": {} }
            }
        }),
    )
    .await;

    assert_eq!(payload["result"]["isError"], json!(false), "{payload}");
    let compact = &payload["result"]["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"), "{payload}");
    assert_eq!(
        compact["detail"]["status"],
        json!("continuation_available"),
        "{payload}"
    );
}

#[tokio::test]
async fn mcp_call_rejects_stale_or_missing_required_des_id() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;

    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": "runtime_profile_guarded",
                        "des_id": "guard123",
                        "name": "Guarded runtime profile",
                        "short_description": "Guarded runtime profile",
                        "full_description": "Read runtime profile after description confirmation.",
                        "execution_target": {"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": {
                            "interface_parameters": [{
                                "name": "des_id",
                                "field_type": "string",
                                "parameter_type": "json_body",
                                "required": true
                            }]
                        },
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "high",
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_tool_response.status(), StatusCode::CREATED);
    let tool_payload = response_json(create_tool_response).await;
    assert_eq!(tool_payload["data"]["des_id_required"], json!(true));

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/runtime",
                        "tool_id": "runtime_profile_guarded",
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);

    for (id, des_id) in [(9, None), (10, Some("stale123"))] {
        let payload = call_mcp(
            &app,
            &token,
            json!({
                "jsonrpc":"2.0",
                "id":id,
                "method":"tools/call",
                "params":{
                    "name":"mcp_call",
                    "arguments":{
                        "tool_id":"runtime_profile_guarded",
                        "des_id":des_id,
                        "arguments":{}
                    }
                }
            }),
        )
        .await;
        assert_eq!(payload["error"]["code"], json!(-32602));
        assert_eq!(payload["error"]["message"], json!("Invalid des_id"));
    }
}

#[tokio::test]
async fn mcp_call_get_catalog_preserves_discovery_policy_fields_via_continuation() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "catalog_snapshot",
        "get_mcp_catalog",
        json!({ "mappings": [] }),
    )
    .await;

    let payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":20,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{"tool_id":"catalog_snapshot","arguments":{}}
            }
        }),
    )
    .await;

    assert!(payload["error"].is_null(), "{payload}");
    assert_eq!(payload["result"]["isError"], json!(false));
    let result_ref = payload["result"]["structuredContent"]["detail"]["result_ref"]
        .as_str()
        .expect("large catalog should provide a continuation reference");
    let mut cursor = payload["result"]["structuredContent"]["detail"]["next_cursor"]
        .as_str()
        .map(str::to_owned);
    let mut found_list_return_field = false;
    for id in 21..53 {
        let Some(current_cursor) = cursor.as_deref() else {
            break;
        };
        let continuation_arguments = json!({"result_ref": result_ref, "cursor": current_cursor});
        let continuation = call_mcp(
            &app,
            &token,
            json!({
                "jsonrpc":"2.0",
                "id":id,
                "method":"tools/call",
                "params":{
                    "name":"mcp_result",
                    "arguments":continuation_arguments
                }
            }),
        )
        .await;
        let page = &continuation["result"]["structuredContent"];
        assert_eq!(page["detail_status"], json!("available"), "{page}");
        found_list_return_field |= page["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|entry| {
                entry["path"].as_str().is_some_and(|path| {
                    path.starts_with("/discovery_policies/0/list_return_fields/")
                }) && entry["value"].is_string()
            });
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if found_list_return_field || cursor.is_none() {
            break;
        }
    }
    assert!(found_list_return_field, "{payload}");
}

#[tokio::test]
async fn mcp_call_classifies_interface_argument_and_target_failures() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "publish_probe",
        "publish_application_api",
        json!({
            "mappings": [
                {
                    "interface_param": "application_id",
                    "mcp_param": "application_id",
                    "required": true
                },
                {
                    "interface_param": "api_enabled",
                    "mcp_param": "api_enabled",
                    "required": true
                },
                {
                    "interface_param": "mapping.input.query_target",
                    "mcp_param": "query_target",
                    "required": true
                }
            ]
        }),
    )
    .await;

    let invalid_arguments = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":21,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"publish_probe",
                    "arguments":{
                        "application_id":null,
                        "api_enabled":true,
                        "query_target":"node-start.query"
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(invalid_arguments["error"]["code"], json!(-32602));
    assert_eq!(
        invalid_arguments["error"]["data"],
        json!({
            "category":"invalid_tool_arguments",
            "field":"request_schema",
            "outcome":"not_started",
            "retry_original":false
        })
    );

    let target_failure = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":22,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"publish_probe",
                    "arguments":{
                        "application_id":"00000000-0000-0000-0000-000000000000",
                        "api_enabled":true,
                        "query_target":"node-start.query"
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(target_failure["error"]["code"], json!(-32603));
    assert_eq!(
        target_failure["error"]["data"],
        json!({
            "category":"target_interface",
            "http_status":404,
            "outcome":"failed",
            "retry_original":false
        })
    );
    let serialized = target_failure.to_string();
    assert!(!serialized.contains("change-me"));
    assert!(!serialized.contains("resource not found"));
}

#[tokio::test]
async fn ac_004_frontstage_tool_migration_binds_all_six_workspace_parameters_to_the_server() {
    const TOOL_IDS: [&str; 6] = [
        "frontstage_update_page_metadata",
        "frontstage_list_pages",
        "frontstage_create_tab",
        "frontstage_list_tabs",
        "frontstage_get_page_detail",
        "frontstage_create_page",
    ];
    const MIGRATION_SQL: &str = include_str!(
        "../../../../crates/storage-durable/postgres/migrations/20260806120000_bind_frontstage_mcp_tools_to_instance_workspace.sql"
    );
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let secondary_workspace_id = seed_workspace(&database_url, "MCP migration workspace").await;
    let switch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"workspace_id":secondary_workspace_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switch_response.status(), StatusCode::OK);
    let csrf = response_json(switch_response).await["data"]["csrf_token"]
        .as_str()
        .unwrap()
        .to_owned();
    create_mcp_instance(&app, &cookie, &csrf).await;

    for tool_id in TOOL_IDS {
        create_interface_tool_and_binding(
            &app,
            &cookie,
            &csrf,
            tool_id,
            "list_frontstage_pages",
            json!({"mappings":[{
                "interface_param":"workspace_id",
                "mcp_param":"workspace_id",
                "required":true
            }]}),
        )
        .await;
    }

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::raw_sql(MIGRATION_SQL).execute(&pool).await.unwrap();
    let mappings = sqlx::query_as::<_, (String, Value)>(
        "select tool_id, input_mapping from mcp_tools where tool_id = any($1) order by tool_id",
    )
    .bind(TOOL_IDS.as_slice())
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(mappings.len(), TOOL_IDS.len());
    for (tool_id, input_mapping) in mappings {
        let workspace_mapping = &input_mapping["mappings"][0];
        assert_eq!(
            workspace_mapping["source"],
            json!({"kind":"server_binding","binding":"workspace_id"}),
            "{tool_id}"
        );
        assert!(workspace_mapping.get("mcp_param").is_none(), "{tool_id}");
    }
}

#[test]
fn root_1569_ac_006_inline_budget_counts_unicode_and_valid_json_without_truncation() {
    use crate::routes::mcp_protocol::result_delivery::{
        exceeds_inline_limit, inline_limit, DEFAULT_INLINE_CHARS, MAX_INLINE_CHARS,
    };

    let exact = json!("界".repeat(DEFAULT_INLINE_CHARS - 2));
    let over = json!("界".repeat(DEFAULT_INLINE_CHARS - 1));
    assert!(!exceeds_inline_limit(&exact, DEFAULT_INLINE_CHARS));
    assert!(exceeds_inline_limit(&over, DEFAULT_INLINE_CHARS));
    assert!(!exceeds_inline_limit(
        &json!({"nested": [{"value": "界"}, [1, 2, 3]]}),
        DEFAULT_INLINE_CHARS
    ));
    assert_eq!(inline_limit(&json!({})), Ok(DEFAULT_INLINE_CHARS));
    assert_eq!(
        inline_limit(&json!({"max_inline_chars": MAX_INLINE_CHARS})),
        Ok(MAX_INLINE_CHARS)
    );
    assert!(inline_limit(&json!({"max_inline_chars": MAX_INLINE_CHARS + 1})).is_err());
    assert!(inline_limit(&json!({"max_inline_chars": 0})).is_err());
}

#[tokio::test]
async fn root_1569_ac_006_ac_008_oversized_read_uses_read_only_paged_continuation() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "catalog_continuation",
        "get_mcp_catalog",
        json!({ "mappings": [] }),
    )
    .await;

    let oversized = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":30,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"catalog_continuation",
                    "arguments":{},
                    "max_inline_chars":1
                }
            }
        }),
    )
    .await;
    let compact = &oversized["result"]["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["operation_id"], json!("get_mcp_catalog"));
    assert_eq!(compact["detail"]["status"], json!("continuation_available"));
    assert_eq!(compact["retry_original"], json!(false));
    assert!(compact.get("receipt_id").is_none());
    let result_ref = compact["detail"]["result_ref"].as_str().unwrap();
    let initial_cursor = compact["detail"]["next_cursor"]
        .as_str()
        .expect("continuation receipt should expose the first opaque cursor");

    let missing_cursor = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":301,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":result_ref}
            }
        }),
    )
    .await;
    assert_eq!(missing_cursor["error"]["code"], json!(-32602));
    assert_eq!(missing_cursor["error"]["message"], json!("Invalid cursor"));

    let first_page = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":31,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{
                    "result_ref":result_ref,
                    "cursor":initial_cursor,
                    "max_inline_chars":1000
                }
            }
        }),
    )
    .await;
    let page = &first_page["result"]["structuredContent"];
    assert_eq!(page["detail_status"], json!("available"));
    assert_eq!(page["retry_original"], json!(false));
    assert!(page["entries"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry["path"].is_string() && entry.get("value").is_some()));
    let next_cursor = page["next_cursor"]
        .as_str()
        .expect("the finite fixture should require more than one continuation page");
    let second_page = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":311,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{
                    "result_ref":result_ref,
                    "cursor":next_cursor,
                    "max_inline_chars":1000
                }
            }
        }),
    )
    .await;
    assert_eq!(
        second_page["result"]["structuredContent"]["detail_status"],
        json!("available")
    );

    let cache_key = format!("mcp-result:{}:{}", state.bootstrap_workspace_id, result_ref);
    state
        .infrastructure
        .cache_store()
        .delete(&cache_key)
        .await
        .unwrap();
    let expired = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":32,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":result_ref,"cursor":initial_cursor}
            }
        }),
    )
    .await;
    assert_eq!(
        expired["result"]["structuredContent"]["detail_status"],
        json!("detail_unavailable")
    );
    assert_eq!(
        expired["result"]["structuredContent"]["retry_original"],
        json!(false)
    );
}

#[tokio::test]
async fn issue_1733_ac_001_through_ac_004_large_source_code_uses_string_continuation() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, read_continuation, CompletedOperation, ContinuationCursor,
        MAX_INLINE_CHARS,
    };
    use sha2::{Digest, Sha256};

    let (state, _) = test_api_state_with_database_url().await;
    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    let source_code = "界".repeat(17_416);
    let source_sha256 = format!("{:x}", Sha256::digest(source_code.as_bytes()));
    let delivered = deliver_oversized_result(
        state.as_ref(),
        &actor,
        CompletedOperation::Read {
            operation_id: "get_frontstage_block_node_code",
        },
        json!({
            "page_id": "019f51cf-4423-7ff0-93aa-6b0b1b8020bf",
            "code_ref": "frontstage-js-block-a3f0ffb2-ee70-4d48-88ac-23f0f810c0d7-code",
            "source_code": &source_code,
            "source_sha256": &source_sha256,
            "dependency_lock": [{"name": "react", "version": "19"}]
        }),
    )
    .await;
    let compact = &delivered["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["detail"]["status"], json!("continuation_available"));
    assert_eq!(compact["retry_original"], json!(false));
    let result_ref = uuid::Uuid::parse_str(
        compact["detail"]["result_ref"]
            .as_str()
            .expect("large source must expose result_ref"),
    )
    .unwrap();

    let mut cursor = ContinuationCursor::default();
    let mut reconstructed = String::new();
    for _ in 0..16 {
        let page =
            read_continuation(state.as_ref(), &actor, result_ref, cursor, MAX_INLINE_CHARS).await;
        let detail = &page["structuredContent"];
        assert_eq!(detail["detail_status"], json!("available"), "{detail}");
        assert_eq!(detail["retry_original"], json!(false));
        assert!(serde_json::to_string(detail).unwrap().chars().count() <= MAX_INLINE_CHARS);
        for entry in detail["entries"].as_array().unwrap() {
            if entry["path"] == json!("/source_code") {
                assert_eq!(entry["value_type"], json!("string_chunk"));
                assert_eq!(entry["char_offset"], json!(reconstructed.chars().count()));
                reconstructed.push_str(entry["value"].as_str().unwrap());
            }
        }
        let Some(next_cursor) = detail["next_cursor"].as_str() else {
            break;
        };
        cursor = ContinuationCursor::parse(next_cursor).expect("server cursor must be valid");
    }
    assert_eq!(reconstructed.chars().count(), 17_416);
    assert_eq!(reconstructed, source_code);
    assert_eq!(
        format!("{:x}", Sha256::digest(reconstructed.as_bytes())),
        source_sha256
    );

    let invalid = read_continuation(
        state.as_ref(),
        &actor,
        result_ref,
        ContinuationCursor::parse("v2:0:999999").unwrap(),
        MAX_INLINE_CHARS,
    )
    .await;
    assert_eq!(
        invalid["structuredContent"]["detail_status"],
        json!("invalid_cursor")
    );
    assert_eq!(invalid["structuredContent"]["retry_original"], json!(false));
}

#[tokio::test]
async fn root_1569_ac_007_ac_009_oversized_write_returns_durable_receipt_without_retry() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "create_instance_once",
        "create_mcp_instance",
        json!({
            "mappings": [
                {"interface_param":"instance_id","mcp_param":"instance_id","required":true},
                {"interface_param":"name","mcp_param":"name","required":true},
                {"interface_param":"description_short","mcp_param":"description_short","required":true},
                {"interface_param":"status","mcp_param":"status","required":true},
                {"interface_param":"default_entry_path","mcp_param":"default_entry_path","required":true}
            ]
        }),
    )
    .await;

    let invalid_budget = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":39,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"create_instance_once",
                    "arguments":{
                        "instance_id":"must_not_exist",
                        "name":"Must not exist",
                        "description_short":null,
                        "status":"draft",
                        "default_entry_path":"/"
                    },
                    "max_inline_chars":16001
                }
            }
        }),
    )
    .await;
    assert_eq!(invalid_budget["error"]["code"], json!(-32602));
    let invalid_write_count: i64 = sqlx::query_scalar(
        "select count(*) from mcp_instances where workspace_id = $1 and instance_id = 'must_not_exist'",
    )
    .bind(state.bootstrap_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(invalid_write_count, 0);

    let completed = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":40,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"create_instance_once",
                    "arguments":{
                        "instance_id":"created_once",
                        "name":"Created once",
                        "description_short":"Single dispatch fixture",
                        "status":"draft",
                        "default_entry_path":"/"
                    },
                    "max_inline_chars":1
                }
            }
        }),
    )
    .await;
    let compact = &completed["result"]["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(compact["operation_id"], json!("create_mcp_instance"));
    assert_eq!(compact["receipt_status"], json!("available"));
    assert_eq!(compact["retry_original"], json!(false));
    let receipt_id = compact["receipt_id"].as_str().unwrap();
    let initial_cursor = compact["detail"]["next_cursor"]
        .as_str()
        .expect("continuation receipt should expose the first opaque cursor");

    let created_count: i64 = sqlx::query_scalar(
        "select count(*) from mcp_instances where workspace_id = $1 and instance_id = 'created_once'",
    )
    .bind(state.bootstrap_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        created_count, 1,
        "the gateway must dispatch a write only once"
    );
    let receipt_count: i64 = sqlx::query_scalar("select count(*) from audit_logs where id = $1")
        .bind(uuid::Uuid::parse_str(receipt_id).unwrap())
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(receipt_count, 1);

    let cache_key = format!("mcp-result:{}:{}", state.bootstrap_workspace_id, receipt_id);
    state
        .infrastructure
        .cache_store()
        .delete(&cache_key)
        .await
        .unwrap();
    let expired = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":41,
            "method":"tools/call",
            "params":{
                "name":"mcp_result",
                "arguments":{"result_ref":receipt_id,"cursor":initial_cursor}
            }
        }),
    )
    .await;
    let expired = &expired["result"]["structuredContent"];
    assert_eq!(expired["detail_status"], json!("detail_unavailable"));
    assert_eq!(expired["receipt"]["outcome"], json!("succeeded"));
    assert_eq!(expired["retry_original"], json!(false));
}

#[tokio::test]
async fn root_1569_ac_010_large_or_base64_like_detail_is_never_cached_or_inlined() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, CompletedOperation,
    };

    let (state, _) = test_api_state_with_database_url().await;
    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    for (detail, expected_reason) in [
        (
            // Beyond the chunked-continuation ceiling (16 chunks * 512 KiB):
            // each "界" serializes to 3 bytes, so this is ~9 MiB.
            json!({"rows": "界".repeat(3 * control_plane::ports::EPHEMERAL_VALUE_MAX_BYTES)}),
            "cache_capacity_exceeded",
        ),
        (
            json!({"content_base64": "A".repeat(4096)}),
            "binary_or_base64_content",
        ),
    ] {
        let delivered = deliver_oversized_result(
            state.as_ref(),
            &actor,
            CompletedOperation::Read {
                operation_id: "large_read_fixture",
            },
            detail,
        )
        .await;
        assert_eq!(
            delivered["structuredContent"]["detail"]["status"],
            json!("detail_unavailable")
        );
        assert_eq!(
            delivered["structuredContent"]["detail"]["reason"],
            json!(expected_reason)
        );
        assert_eq!(
            delivered["structuredContent"]["retry_original"],
            json!(false)
        );
        assert!(delivered.to_string().len() < 4_000);
    }
}

#[tokio::test]
async fn root_1569_ac_003_ac_007_ac_009_bundle_import_uses_domain_summary_and_durable_receipt() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_oversized_result, exceeds_inline_limit, read_continuation, CompletedOperation,
        DEFAULT_INLINE_CHARS,
    };

    let (state, _) = test_api_state_with_database_url().await;
    let actor_user_id: uuid::Uuid =
        sqlx::query_scalar("select id from users where account = 'root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let actor = domain::ActorContext::root(actor_user_id, state.bootstrap_workspace_id, "root");
    let item_reports = (0..300)
        .map(|index| {
            json!({
                "id": format!("bundle_tool_{index}"),
                "effect": "already_present",
                "result": "already_present",
                "reason": null
            })
        })
        .collect::<Vec<_>>();
    let report = json!({
        "manifest": {
            "schema_version": "1flowbase.mcp.bundle/v2",
            "organization": "1flowbase",
            "bundle_id": "1flowbase_zh_hans",
            "bundle_version": "1.2.3",
            "locale": "zh_Hans",
            "minimum_host_version": "0.1.0",
            "exported_from_system_version": "0.1.0",
            "exported_at": "2026-08-03T00:00:00Z",
            "files": []
        },
        "current_system_version": "0.1.0",
        "version_status": "same_system_version",
        "status": "applied",
        "effect_summary": {
            "changes": 7,
            "already_present": 3,
            "conflicts": 0,
            "unavailable": 0,
            "failed": 0
        },
        "tools": item_reports,
        "instances": [],
        "connections": []
    });
    assert!(exceeds_inline_limit(&report, DEFAULT_INLINE_CHARS));
    let mut already_applied_report = report.clone();
    already_applied_report["status"] = json!("already_applied");
    already_applied_report["effect_summary"] = json!({
        "changes": 0,
        "already_present": 310,
        "conflicts": 0,
        "unavailable": 0,
        "failed": 0
    });

    let delivered = deliver_oversized_result(
        state.as_ref(),
        &actor,
        CompletedOperation::Write {
            operation_id: "import_mcp_bundle_library_release",
        },
        report,
    )
    .await;
    let compact = &delivered["structuredContent"];
    assert_eq!(compact["outcome"], json!("succeeded"));
    assert_eq!(
        compact["operation_id"],
        json!("import_mcp_bundle_library_release")
    );
    assert_eq!(
        compact["summary"]["bundle"],
        json!({
            "organization":"1flowbase",
            "bundle_id":"1flowbase_zh_hans",
            "bundle_version":"1.2.3",
            "locale":"zh_Hans"
        })
    );
    assert_eq!(compact["summary"]["status"], json!("applied"));
    assert_eq!(compact["summary"]["effect_summary"]["changes"], json!(7));
    assert_eq!(compact["retry_original"], json!(false));
    let receipt_id = uuid::Uuid::parse_str(compact["receipt_id"].as_str().unwrap()).unwrap();

    state
        .infrastructure
        .cache_store()
        .delete(&format!(
            "mcp-result:{}:{}",
            state.bootstrap_workspace_id, receipt_id
        ))
        .await
        .unwrap();
    let expired = read_continuation(
        state.as_ref(),
        &actor,
        receipt_id,
        Default::default(),
        DEFAULT_INLINE_CHARS,
    )
    .await;
    assert_eq!(
        expired["structuredContent"]["detail_status"],
        json!("detail_unavailable")
    );
    assert_eq!(
        expired["structuredContent"]["receipt"]["summary"]["bundle"]["bundle_id"],
        json!("1flowbase_zh_hans")
    );
    assert_eq!(
        expired["structuredContent"]["receipt"]["outcome"],
        json!("succeeded")
    );
    assert_eq!(expired["structuredContent"]["retry_original"], json!(false));

    let already_applied = deliver_oversized_result(
        state.as_ref(),
        &actor,
        CompletedOperation::Write {
            operation_id: "import_mcp_bundle_library_release",
        },
        already_applied_report,
    )
    .await;
    assert_eq!(
        already_applied["structuredContent"]["summary"]["status"],
        json!("already_applied")
    );
    assert_eq!(
        already_applied["structuredContent"]["summary"]["effect_summary"]["changes"],
        json!(0)
    );
    assert_eq!(
        already_applied["structuredContent"]["retry_original"],
        json!(false)
    );
}
