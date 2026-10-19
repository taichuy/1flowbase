use super::*;

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
    let listed_items = list_payload["result"]["structuredContent"]["result"]
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

