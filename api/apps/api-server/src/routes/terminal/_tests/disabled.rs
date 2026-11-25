use super::*;

const CODE: &str = "local_terminal_disabled";
const MESSAGE: &str =
    "Local command and terminal interfaces are disabled because API_LOCAL_TERMINAL_DISABLED=true.";
const INSTANCE: &str = "terminal_disabled";
const RESOURCE_TOOL: &str = "read_terminal_fixture";
const DES_ID: &str = "terminaldisabledv1";

fn write_marker(path: &std::path::Path) -> String {
    let path = path.display().to_string();
    if cfg!(windows) {
        format!(r#"echo ran > "{path}""#)
    } else {
        format!("printf ran > '{}'", path.replace('\'', "'\\''"))
    }
}

#[tokio::test]
async fn disabled_local_terminal_blocks_root_execution_but_preserves_mcp_discovery() {
    let (state, _) = test_api_state_with_database_url().await;
    assert!(!state.local_terminal_disabled);
    let mut config = crate::_tests::support::test_config();
    config.local_terminal_disabled = true;
    let app = crate::app_with_state_and_config(state.clone(), &config);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let temp = Temp::new();
    let command_marker = temp.0.join("command-ran");
    let job_marker = temp.0.join("job-ran");
    let download_target = temp.0.join("downloaded");
    let file = temp.0.join("fixture.txt");
    std::fs::write(&file, "fixture").unwrap();
    let command_args = json!({"command":write_marker(&command_marker)});
    let resource_args =
        json!({"path":file,"timeout_ms":1000,"max_file_bytes":32,"link_only":false});
    let id = Uuid::now_v7();
    for (path, body) in [
        ("local-commands/execute", command_args.clone()),
        ("local-commands/execute", json!({"command":""})),
        ("terminal/info", json!({})),
        (
            "terminal/files/list",
            json!({"path":temp.0,"timeout_ms":1000,"cursor":0,"limit":10}),
        ),
        (
            "terminal/files/stat",
            json!({"path":file,"timeout_ms":1000}),
        ),
        (
            "terminal/files/read-text",
            json!({"path":file,"timeout_ms":1000,"offset":0,"max_bytes":32}),
        ),
        (
            "terminal/files/read-range",
            json!({"path":file,"timeout_ms":1000,"offset":0,"length":3}),
        ),
        (
            "terminal/files/search",
            json!({"path":temp.0,"timeout_ms":1000,"pattern":"fixture","max_scan_bytes":32,"max_matches":1,"max_depth":1,"max_line_bytes":32}),
        ),
        (
            "terminal/files/read-image",
            json!({"path":file,"timeout_ms":1000,"max_file_bytes":32}),
        ),
        (
            "terminal/files/read-audio",
            json!({"path":file,"timeout_ms":1000,"max_file_bytes":32}),
        ),
        ("terminal/files/read-resource", resource_args.clone()),
        (
            "terminal/files/import-resource",
            json!({"resource_uri":"fixture-resource","file_table_id":id}),
        ),
        (
            "terminal/files/download-file",
            json!({"file_table_id":id,"record_id":id,"target_path":download_target,"timeout_ms":1000}),
        ),
        (
            "terminal/jobs/start",
            json!({"request_id":id,"command":write_marker(&job_marker),"timeout_ms":1000,"retention_ms":1000,"max_output_bytes":32}),
        ),
        ("terminal/jobs/get", json!({"execution_id":id})),
        (
            "terminal/jobs/read",
            json!({"execution_id":id,"stream":"stdout","offset":0,"max_bytes":32}),
        ),
        ("terminal/jobs/cancel", json!({"execution_id":id})),
    ] {
        let (status, response) = request(
            &app,
            "POST",
            &format!("/api/console/{path}"),
            &cookie,
            &csrf,
            body,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {response}");
        assert_eq!(response["code"], CODE, "{path}: {response}");
        assert_eq!(response["message"], MESSAGE, "{path}: {response}");
    }

    assert_catalog_bindable(&app, &cookie, &csrf).await;
    let token = create_mcp_instance(&app, &cookie, &csrf).await;
    assert_mcp_failure_after_discovery(
        &app,
        &cookie,
        &csrf,
        &token,
        command_args,
        resource_args.clone(),
    )
    .await;

    // Build a valid locator for this actor, as if obtained before disabling the terminal.
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    let session = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .unwrap();
    let locator = extension_contracts::ResourceReference {
        v: 1,
        user_id: session.actor.user_id,
        workspace_id: session.actor.current_workspace_id,
        instance_id: INSTANCE.into(),
        tool_id: RESOURCE_TOOL.into(),
        des_id: DES_ID.into(),
        arguments: resource_args,
        resource_uri: url::Url::from_file_path(&file).unwrap().to_string(),
    }
    .encode()
    .unwrap();
    let replay = protocol(&app, &token, "resources/read", json!({"uri":locator})).await;
    assert_terminal_disabled_error(&replay);

    for path in [command_marker, job_marker, download_target] {
        assert!(!path.exists(), "unexpected side effect: {}", path.display());
    }
    let starts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_logs WHERE event_code = 'local_command.started'",
    )
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(starts, 0);
}

async fn assert_catalog_bindable(app: &Router, cookie: &str, csrf: &str) {
    let (status, catalog) = request(
        app,
        "GET",
        "/api/console/mcp/interface-capabilities",
        cookie,
        csrf,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let entries = catalog["data"].as_array().unwrap();
    assert_eq!(INTERFACE_IDS.len(), 15);
    for id in std::iter::once("execute_local_command").chain(INTERFACE_IDS.iter().copied()) {
        let entry = entries
            .iter()
            .find(|entry| entry["interface_id"] == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(entry["bindable"], true, "{id}: {entry}");
        assert_eq!(
            entry.get("disabled_reason"),
            Some(&Value::Null),
            "{id}: {entry}"
        );
    }
}

async fn create_mcp_instance(app: &Router, cookie: &str, csrf: &str) -> String {
    for (path, expected, body) in [
        (
            "/api/console/mcp/instances",
            StatusCode::CREATED,
            json!({"instance_id":INSTANCE,"name":"Disabled terminal","status":"enabled","default_entry_path":"/"}),
        ),
        (
            "/api/console/mcp/instances/terminal_disabled/groups",
            StatusCode::OK,
            json!({"path":"/terminal","display_name":"Terminal","description_short":"Local terminal fixtures","enabled":true,"sort_order":0}),
        ),
        (
            "/api/console/user-api-keys",
            StatusCode::CREATED,
            json!({"name":"terminal disabled test client","expiration_policy":"never"}),
        ),
    ] {
        let (status, response) = request(app, "POST", path, cookie, csrf, body).await;
        assert_eq!(status, expected, "{path}: {response}");
        if let Some(token) = response["data"]["token"].as_str() {
            return token.to_owned();
        }
    }
    panic!("user API key response did not contain a token");
}

async fn assert_mcp_failure_after_discovery(
    app: &Router,
    cookie: &str,
    csrf: &str,
    token: &str,
    command_args: Value,
    resource_args: Value,
) {
    let tools = [
        (
            "execute_terminal_fixture",
            "execute_local_command",
            command_args,
            json!({}),
        ),
        (
            RESOURCE_TOOL,
            "terminal.files.read_resource",
            resource_args,
            json!({
                "resource_read":true,
                "resource_read_arguments":{"link_only":false},
                "structured_content_path":"/metadata",
                "mcp_content":[
                    {"type":"resource_link","uri_path":"/uri","name_path":"/name","mime_type_path":"/mime_type"},
                    {"type":"resource","uri_path":"/uri","blob_path":"/data","mime_type_path":"/mime_type","skip_if_null":true}
                ]
            }),
        ),
    ];
    for (tool_id, interface_id, args, output_mapping) in &tools {
        let mappings = args.as_object().unwrap().keys().map(|name|json!({"interface_param":name,"source":{"kind":"mcp_argument","path":name},"required":true})).collect::<Vec<_>>();
        let (status, created) = request(app, "POST", "/api/console/mcp/tools", cookie, csrf, json!({
            "tool_id":tool_id,"des_id":DES_ID,"name":tool_id,
            "short_description":"Run a terminal fixture.","full_description":"Invoke a local terminal fixture through its canonical interface.",
            "execution_target":{"kind":"interface_wrapper","interface_id":interface_id},
            "parameter_schema":{},"result_schema":{},"input_mapping":{"mappings":mappings},"output_mapping":output_mapping,
            "permission_code":null,"risk_level":"low","status":"enabled"
        })).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let (status, bound) = request(
            app,
            "POST",
            "/api/console/mcp/instances/terminal_disabled/tool-bindings",
            cookie,
            csrf,
            json!({"group_path":"/terminal","tool_id":tool_id,"visible":true,"sort_order":0}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{bound}");
    }

    let listed = protocol(
        app,
        token,
        "tools/call",
        json!({"name":"mcp_list","arguments":{"path":"/","depth":1,"limit":10}}),
    )
    .await;
    let items = listed["result"]["structuredContent"]["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"));
    assert!(
        items
            .iter()
            .any(|item| item["item_kind"] == "group" && item["path"] == "/terminal"),
        "{listed}"
    );
    for (tool_id, _, args, _) in tools {
        assert!(
            items
                .iter()
                .any(|item| item["item_kind"] == "tool" && item["id"] == tool_id),
            "{listed}"
        );
        let get = protocol(
            app,
            token,
            "tools/call",
            json!({"name":"mcp_get","arguments":{"tool_id":tool_id}}),
        )
        .await;
        assert_eq!(
            get["result"]["structuredContent"]["tool_id"], tool_id,
            "{get}"
        );
        let call = protocol(
            app,
            token,
            "tools/call",
            json!({"name":"mcp_call","arguments":{"tool_id":tool_id,"arguments":args}}),
        )
        .await;
        assert_terminal_disabled_error(&call);
    }
}

async fn protocol(app: &Router, token: &str, method: &str, params: Value) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/mcp/{INSTANCE}"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn assert_terminal_disabled_error(response: &Value) {
    assert_eq!(response["error"]["message"], MESSAGE, "{response}");
    assert_eq!(response["error"]["data"]["reason"], MESSAGE, "{response}");
    assert_eq!(response["error"]["data"]["target_code"], CODE, "{response}");
}
