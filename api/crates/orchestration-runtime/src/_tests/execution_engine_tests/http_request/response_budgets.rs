use super::*;

#[tokio::test]
async fn http_request_node_default_response_budget_allows_six_mib() {
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/large-default",
        200,
        "application/octet-stream",
        vec![b'x'; 6 * 1024 * 1024],
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/large-default"),
            "body_type": "none",
            "timeout_ms": 30000,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-http"]["status_code"],
        json!(200)
    );
    server.abort();
}

#[tokio::test]
async fn http_request_node_keeps_binary_response_file_descriptor_when_inline_body_is_truncated() {
    let body = vec![0, 1, 2, 3, 4, 5];
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/large-download",
        200,
        "application/octet-stream",
        body,
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/large-download"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 4,
            "verify_ssl": true,
            "store_response_as_file": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-http"]["body"],
        json!(String::from_utf8_lossy(&[0, 1, 2, 3]).to_string())
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["files"][0]["filename"],
        "response.bin"
    );
    assert_eq!(outcome.variable_pool["node-http"]["files"][0]["size"], 6);
    assert_eq!(
        outcome.node_traces[1].metrics_payload["body_truncated"],
        json!(true)
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["stored_body_bytes"],
        json!(4)
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["response_bytes_observed"],
        json!(6)
    );
    server.abort();
}

#[tokio::test]
async fn http_request_node_truncates_response_body_at_configured_budget() {
    let (base_url, _captured, server) =
        spawn_http_fixture(vec![response("/large-text", 200, "text/plain", "abcdef")]).await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/large-text"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 4,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(outcome.variable_pool["node-http"]["body"], json!("abcd"));
    assert_eq!(outcome.variable_pool["node-http"]["files"], json!([]));
    assert_eq!(
        outcome.node_traces[1].metrics_payload["body_truncated"],
        json!(true)
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["response_bytes_observed"],
        json!(6)
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["stored_body_bytes"],
        json!(4)
    );
    server.abort();
}

#[tokio::test]
async fn http_request_node_caps_configured_response_budget_at_ten_mib() {
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/too-large",
        200,
        "application/octet-stream",
        vec![b'x'; 10 * 1024 * 1024 + 1],
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/too-large"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 20 * 1024 * 1024,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-http"]["body"]
            .as_str()
            .unwrap()
            .len(),
        10 * 1024 * 1024
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["body_truncated"],
        json!(true)
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["response_bytes_observed"],
        json!(10 * 1024 * 1024 + 1)
    );
    server.abort();
}

#[tokio::test]
async fn http_request_node_treats_error_status_as_normal_output() {
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/missing",
        500,
        "text/plain",
        "server error",
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/missing"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(
        outcome.variable_pool["node-http"]["status_code"],
        json!(500)
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["body"],
        json!("server error")
    );
    server.abort();
}

#[tokio::test]
async fn http_request_node_fails_on_network_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    drop(listener);
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("http://{addr}/unreachable"),
            "body_type": "none",
            "timeout_ms": 1000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-http");
            assert_eq!(failure.error_payload["kind"], json!("http_request_error"));
        }
        other => panic!("expected failed http_request, got {other:?}"),
    }
}

#[tokio::test]
async fn http_request_node_reads_file_url_for_binary_and_form_data_file() {
    let (base_url, captured, server) = spawn_http_fixture(vec![
        response("/file", 200, "application/octet-stream", "binary-input"),
        response("/binary", 200, "text/plain", "binary-ok"),
        response("/multipart-file", 200, "text/plain", "multipart-ok"),
    ])
    .await;
    let file_descriptor = json!({
        "filename": "demo.txt",
        "size": 12,
        "mimetype": "text/plain",
        "path": "attachments/demo.txt",
        "url": format!("{base_url}/file"),
        "storage_id": "storage-1",
        "meta": {}
    });

    let binary_plan = http_request_plan(
        json!({
            "method": "POST",
            "url": format!("{base_url}/binary"),
            "body_type": "binary",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::from([(
            "binary".to_string(),
            selector_binding(vec!["node-start", "files"]),
        )]),
    );
    let form_data_plan = http_request_plan(
        json!({
            "method": "POST",
            "url": format!("{base_url}/multipart-file"),
            "body_type": "form-data",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::from([(
            "form_data".to_string(),
            named_bindings(json!([
                {
                    "name": "attachment",
                    "valueType": "file",
                    "value": { "kind": "selector", "selector": ["node-start", "files"] }
                }
            ])),
        )]),
    );

    for plan in [&binary_plan, &form_data_plan] {
        let outcome = start_flow_debug_run(
            plan,
            &json!({ "node-start": { "files": [file_descriptor.clone()] } }),
            &successful_invoker(),
        )
        .await
        .unwrap();

        assert!(matches!(
            outcome.stop_reason,
            ExecutionStopReason::Completed
        ));
        assert_eq!(
            outcome.variable_pool["node-http"]["status_code"],
            json!(200)
        );
    }

    let requests = captured.lock().expect("captured requests mutex poisoned");
    let binary_request = requests
        .iter()
        .find(|request| request.path == "/binary")
        .expect("binary request should be captured");
    let multipart_request = requests
        .iter()
        .find(|request| request.path == "/multipart-file")
        .expect("multipart file request should be captured");
    assert_eq!(
        String::from_utf8_lossy(&binary_request.body),
        "binary-input"
    );
    assert!(String::from_utf8_lossy(&multipart_request.body).contains("binary-input"));
    assert!(String::from_utf8_lossy(&multipart_request.body).contains("demo.txt"));
    server.abort();
}
