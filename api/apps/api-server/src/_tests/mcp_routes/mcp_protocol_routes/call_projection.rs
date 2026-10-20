use super::*;

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
async fn mcp_call_rejects_invalid_explicit_des_id_without_requiring_it_by_default() {
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
    assert_eq!(tool_payload["data"]["des_id_required"], json!(false));

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

    let payload = call_mcp(
        &app,
        &token,
        json!({
            "jsonrpc":"2.0",
            "id":9,
            "method":"tools/call",
            "params":{
                "name":"mcp_call",
                "arguments":{
                    "tool_id":"runtime_profile_guarded",
                    "des_id":null,
                    "arguments":{}
                }
            }
        }),
    )
    .await;
    assert_eq!(payload["error"]["code"], json!(-32602));
    assert_eq!(payload["error"]["message"], json!("Invalid des_id"));
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
        "../../../../../../crates/storage/durable/postgres/migrations/20260806120000_bind_frontstage_mcp_tools_to_instance_workspace.sql"
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
