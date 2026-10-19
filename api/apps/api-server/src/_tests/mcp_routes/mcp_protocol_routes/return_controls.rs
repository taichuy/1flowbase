use super::*;

#[tokio::test]
async fn issue_2176_mcp_return_controls_persist_defaults_and_read_cached_views() {
    use crate::routes::mcp_protocol::result_delivery::{
        deliver_result, read_result, CompletedOperation, ResultSelection,
    };
    use control_plane::ports::McpManagementRepository;

    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "return_controls",
        "get_mcp_catalog",
        json!({"mappings": []}),
    )
    .await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/tools/return_controls")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response_json(response).await["data"].clone();
    assert_eq!(body["max_inline_chars"], Value::Null);
    assert_eq!(body["response_fields"], Value::Null);
    body["max_inline_chars"] = json!(1600);
    body["response_fields"] = json!(["/title"]);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/tools/return_controls")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated = response_json(response).await;
    assert_eq!(updated["data"]["max_inline_chars"], 1600);
    assert_eq!(updated["data"]["response_fields"], json!(["/title"]));
    let tool = state
        .store
        .get_mcp_tool(state.bootstrap_workspace_id, "return_controls")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tool.max_inline_chars, Some(1600));
    assert_eq!(tool.response_fields, Some(vec!["/title".to_owned()]));
    let actor =
        domain::ActorContext::root(uuid::Uuid::now_v7(), state.bootstrap_workspace_id, "root");
    let dependencies = mcp_result_delivery_dependencies(state.as_ref());
    let source = "甲😀乙丙丁".repeat(5000);
    let detail =
        json!({"title": "summary", "body": source, "items": [{"name": "one", "other": "two"}]});
    let selection = ResultSelection::parse(&json!({}), tool.response_fields.as_deref()).unwrap();
    let delivered = deliver_result(
        &dependencies,
        &actor,
        CompletedOperation::Read {
            operation_id: "read",
        },
        detail,
        1600,
        &selection,
        &tool,
    )
    .await;
    let first = &delivered["structuredContent"];
    assert!(serde_json::to_string(first).unwrap().chars().count() <= 1600);
    assert_eq!(
        first["entries"],
        json!([{"path": "/title", "value": "summary"}])
    );
    assert_eq!(first["next_cursor"], Value::Null);
    let result_ref =
        uuid::Uuid::parse_str(first["detail"]["result_ref"].as_str().unwrap()).unwrap();
    let request = json!({"response_fields": ["/body"], "string_ranges": {"/body": {"offset": 3, "length": 5000}}});
    let mut page = read_result(&dependencies, &actor, result_ref, &request).await;
    let cursor = page["structuredContent"]["next_cursor"]
        .as_str()
        .unwrap()
        .to_owned();
    let changed = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"cursor": cursor, "response_fields": ["/title"]}),
    )
    .await;
    assert_eq!(
        changed["structuredContent"]["detail_status"],
        "invalid_cursor"
    );
    let mut reconstructed = String::new();
    loop {
        let content = &page["structuredContent"];
        assert!(serde_json::to_string(content).unwrap().chars().count() <= 1600);
        assert_eq!(content["detail_status"], "available");
        for entry in content["entries"].as_array().unwrap() {
            assert_eq!(entry["path"], "/body");
            assert_eq!(entry["char_offset"], 3 + reconstructed.chars().count());
            assert_eq!(entry["total_chars"], 25000);
            reconstructed.push_str(entry["value"].as_str().unwrap());
            assert_eq!(entry["next_offset"], 3 + reconstructed.chars().count());
        }
        let Some(cursor) = content["next_cursor"].as_str() else {
            break;
        };
        page = read_result(
            &dependencies,
            &actor,
            result_ref,
            &json!({"cursor": cursor}),
        )
        .await;
    }
    assert_eq!(
        reconstructed,
        source.chars().skip(3).take(5000).collect::<String>()
    );
    let empty = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": []}),
    )
    .await;
    assert_eq!(empty["structuredContent"]["entries"], json!([]));
    let large = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": ["/body"], "max_inline_chars": 80000}),
    )
    .await;
    assert_eq!(large["structuredContent"]["entries"][0]["value"], source);
    let nested = read_result(
        &dependencies,
        &actor,
        result_ref,
        &json!({"response_fields": ["/items/0/name"]}),
    )
    .await;
    assert_eq!(
        nested["structuredContent"]["entries"],
        json!([{"path": "/items/0/name", "value": "one"}])
    );
}
