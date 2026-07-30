use super::*;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

#[derive(Debug, Clone)]
struct FixtureResponse {
    path: &'static str,
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    extra_headers: Vec<(&'static str, &'static str)>,
}

#[derive(Debug, Clone)]
struct CapturedHttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

async fn spawn_http_fixture(
    responses: Vec<FixtureResponse>,
) -> (String, Arc<Mutex<Vec<CapturedHttpRequest>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_task = Arc::clone(&captured);
    let responses = Arc::new(responses);
    let responses_for_task = Arc::clone(&responses);
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let captured = Arc::clone(&captured_for_task);
            let responses = Arc::clone(&responses_for_task);
            tokio::spawn(async move {
                let mut buffer = vec![0_u8; 4096];
                let mut bytes = Vec::new();
                let header_end = loop {
                    let read = stream.read(&mut buffer).await.unwrap_or(0);
                    if read == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                    if let Some(index) = find_header_end(&bytes) {
                        break index;
                    }
                };
                let header_text = String::from_utf8_lossy(&bytes[..header_end]);
                let mut lines = header_text.split("\r\n");
                let request_line = lines.next().unwrap_or_default();
                let mut request_parts = request_line.split_whitespace();
                let method = request_parts.next().unwrap_or_default().to_string();
                let path = request_parts.next().unwrap_or_default().to_string();
                let mut headers = BTreeMap::new();

                for line in lines {
                    if let Some((name, value)) = line.split_once(':') {
                        headers.insert(name.trim().to_lowercase(), value.trim().to_string());
                    }
                }

                let content_length = headers
                    .get("content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let body_start = header_end + 4;
                while bytes.len() < body_start + content_length {
                    let read = stream.read(&mut buffer).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                }
                let body = bytes
                    .get(body_start..body_start + content_length)
                    .unwrap_or_default()
                    .to_vec();

                captured
                    .lock()
                    .expect("captured requests mutex poisoned")
                    .push(CapturedHttpRequest {
                        method: method.clone(),
                        path: path.clone(),
                        headers: headers.clone(),
                        body: body.clone(),
                    });

                let response = responses
                    .iter()
                    .find(|candidate| path.starts_with(candidate.path))
                    .unwrap_or_else(|| responses.first().expect("fixture response required"));
                let reason = match response.status {
                    200 => "OK",
                    201 => "Created",
                    404 => "Not Found",
                    500 => "Internal Server Error",
                    _ => "OK",
                };
                let mut response_text = format!(
                    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n",
                    response.status,
                    reason,
                    response.body.len(),
                    response.content_type
                );
                for (name, value) in &response.extra_headers {
                    response_text.push_str(name);
                    response_text.push_str(": ");
                    response_text.push_str(value);
                    response_text.push_str("\r\n");
                }
                response_text.push_str("\r\n");
                stream.write_all(response_text.as_bytes()).await.unwrap();
                stream.write_all(&response.body).await.unwrap();
            });
        }
    });

    (format!("http://{addr}"), captured, handle)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn response(
    path: &'static str,
    status: u16,
    content_type: &'static str,
    body: impl Into<Vec<u8>>,
) -> FixtureResponse {
    FixtureResponse {
        path,
        status,
        content_type,
        body: body.into(),
        extra_headers: Vec::new(),
    }
}

fn http_output_contract() -> Vec<CompiledOutput> {
    vec![
        CompiledOutput {
            key: "body".to_string(),
            title: "响应内容".to_string(),
            value_type: "string".to_string(),
            selector: Vec::new(),
            json_schema: None,
        },
        CompiledOutput {
            key: "status_code".to_string(),
            title: "响应状态码".to_string(),
            value_type: "number".to_string(),
            selector: Vec::new(),
            json_schema: None,
        },
        CompiledOutput {
            key: "headers".to_string(),
            title: "响应头列表 JSON".to_string(),
            value_type: "object".to_string(),
            selector: Vec::new(),
            json_schema: None,
        },
        CompiledOutput {
            key: "files".to_string(),
            title: "文件列表".to_string(),
            value_type: "Array[File]".to_string(),
            selector: Vec::new(),
            json_schema: None,
        },
    ]
}

fn http_request_plan(config: Value, bindings: BTreeMap<String, CompiledBinding>) -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-http".to_string()],
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-http".to_string(),
        CompiledNode {
            node_id: "node-http".to_string(),
            node_type: "http_request".to_string(),
            alias: "HTTP Request".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: Vec::new(),
            bindings,
            outputs: http_output_contract(),
            config,
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec!["node-start".to_string(), "node-http".to_string()],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

fn templated_binding(value: impl Into<String>) -> CompiledBinding {
    CompiledBinding {
        i18n_text_ref: None,
        kind: "templated_text".to_string(),
        raw_value: json!(value.into()),
        selector_paths: Vec::new(),
    }
}

fn selector_binding(selector: Vec<&str>) -> CompiledBinding {
    CompiledBinding {
        i18n_text_ref: None,
        kind: "selector".to_string(),
        raw_value: json!(selector),
        selector_paths: vec![selector.into_iter().map(str::to_string).collect()],
    }
}

fn named_bindings(entries: Value) -> CompiledBinding {
    CompiledBinding {
        i18n_text_ref: None,
        kind: "named_bindings".to_string(),
        raw_value: entries,
        selector_paths: Vec::new(),
    }
}

#[tokio::test]
async fn http_request_node_executes_get_and_outputs_contract() {
    let (base_url, captured, server) = spawn_http_fixture(vec![FixtureResponse {
        extra_headers: vec![("X-Request-Id", "fixture-1")],
        ..response("/echo", 200, "application/json", r#"{"ok":true}"#)
    }])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/echo"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::from([
            (
                "params".to_string(),
                named_bindings(json!([
                    {
                        "name": "query",
                        "value": { "kind": "templated_text", "value": "{{node-start.query}}" }
                    }
                ])),
            ),
            (
                "headers".to_string(),
                named_bindings(json!([
                    {
                        "name": "X-Trace",
                        "value": { "kind": "templated_text", "value": "run-{{node-start.query}}" }
                    }
                ])),
            ),
        ]),
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
        json!(r#"{"ok":true}"#)
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["status_code"],
        json!(200)
    );
    assert_eq!(outcome.variable_pool["node-http"]["files"], json!([]));
    assert_eq!(
        outcome.variable_pool["node-http"]["headers"]["x-request-id"],
        json!(["fixture-1"])
    );
    let requests = captured.lock().expect("captured requests mutex poisoned");
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/echo?query=refund");
    assert_eq!(requests[0].headers["x-trace"], "run-refund");
    server.abort();
}

#[tokio::test]
async fn http_request_node_preview_returns_debug_payload_and_outputs() {
    let (base_url, _captured, server) =
        spawn_http_fixture(vec![response("/preview", 200, "text/plain", "preview-ok")]).await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/preview"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
            "verify_ssl": true
        }),
        BTreeMap::new(),
    );

    let outcome = crate::preview_executor::run_node_preview(
        &plan,
        "node-http",
        &json!({ "node-start": { "query": "refund" } }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    assert!(!outcome.is_failed());
    assert_eq!(outcome.node_output["body"], json!("preview-ok"));
    assert_eq!(outcome.node_output["status_code"], json!(200));
    assert_eq!(outcome.debug_payload["request"]["method"], json!("GET"));
    assert_eq!(outcome.debug_payload["response"]["status_code"], json!(200));
    server.abort();
}

#[tokio::test]
async fn http_request_node_posts_json_form_urlencoded_form_data_and_raw() {
    let (base_url, captured, server) = spawn_http_fixture(vec![
        response("/json", 200, "text/plain", "json-ok"),
        response("/form", 200, "text/plain", "form-ok"),
        response("/multipart", 200, "text/plain", "multipart-ok"),
        response("/raw", 200, "text/plain", "raw-ok"),
    ])
    .await;

    for (path, body_type, bindings) in [
        (
            "/json",
            "json",
            BTreeMap::from([(
                "body".to_string(),
                templated_binding(r#"{"query":"{{node-start.query}}"}"#),
            )]),
        ),
        (
            "/form",
            "x-www-form-urlencoded",
            BTreeMap::from([(
                "urlencoded".to_string(),
                named_bindings(json!([
                    {
                        "name": "query",
                        "value": { "kind": "templated_text", "value": "{{node-start.query}}" }
                    }
                ])),
            )]),
        ),
        (
            "/multipart",
            "form-data",
            BTreeMap::from([(
                "form_data".to_string(),
                named_bindings(json!([
                    {
                        "name": "query",
                        "valueType": "text",
                        "value": { "kind": "templated_text", "value": "{{node-start.query}}" }
                    }
                ])),
            )]),
        ),
        (
            "/raw",
            "raw",
            BTreeMap::from([(
                "body".to_string(),
                templated_binding("raw {{node-start.query}}"),
            )]),
        ),
    ] {
        let plan = http_request_plan(
            json!({
                "method": "POST",
                "url": format!("{base_url}{path}"),
                "body_type": body_type,
                "timeout_ms": 30000,
                "max_response_bytes": 1048576,
                "verify_ssl": true
            }),
            bindings,
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
    }

    let requests = captured.lock().expect("captured requests mutex poisoned");
    assert!(String::from_utf8_lossy(&requests[0].body).contains(r#""query":"refund""#));
    assert_eq!(String::from_utf8_lossy(&requests[1].body), "query=refund");
    assert!(String::from_utf8_lossy(&requests[2].body).contains("refund"));
    assert_eq!(String::from_utf8_lossy(&requests[3].body), "raw refund");
    server.abort();
}

#[tokio::test]
async fn http_request_node_keeps_application_javascript_response_inline() {
    let body = r#"jQuery1123({"data":{"total":5}});"#;
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/jsonp",
        200,
        "application/javascript; charset=UTF-8",
        body,
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/jsonp"),
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
    assert_eq!(outcome.variable_pool["node-http"]["body"], json!(body));
    assert_eq!(outcome.variable_pool["node-http"]["files"], json!([]));
    server.abort();
}

#[tokio::test]
async fn http_request_node_keeps_text_response_inline_when_file_storage_enabled() {
    let body = r#"jQuery1123({"data":{"total":5}});"#;
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/jsonp-file",
        200,
        "application/javascript; charset=UTF-8",
        body,
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/jsonp-file"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
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
    assert_eq!(outcome.variable_pool["node-http"]["body"], json!(body));
    assert_eq!(outcome.variable_pool["node-http"]["files"], json!([]));
    server.abort();
}

#[tokio::test]
async fn http_request_node_keeps_binary_response_inline_when_file_storage_disabled() {
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/download",
        200,
        "application/octet-stream",
        vec![0, 1, 2, 3],
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/download"),
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
        outcome.variable_pool["node-http"]["body"],
        json!(String::from_utf8_lossy(&[0, 1, 2, 3]).to_string())
    );
    assert_eq!(outcome.variable_pool["node-http"]["files"], json!([]));
    server.abort();
}

#[tokio::test]
async fn http_request_node_projects_binary_response_file_descriptor_when_enabled() {
    let (base_url, _captured, server) = spawn_http_fixture(vec![response(
        "/download",
        200,
        "application/octet-stream",
        vec![0, 1, 2, 3],
    )])
    .await;
    let plan = http_request_plan(
        json!({
            "method": "GET",
            "url": format!("{base_url}/download"),
            "body_type": "none",
            "timeout_ms": 30000,
            "max_response_bytes": 1048576,
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
        outcome.variable_pool["node-http"]["files"][0]["filename"],
        "response.bin"
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["files"][0]["mimetype"],
        "application/octet-stream"
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["files"][0]["storage_id"],
        "runtime-inline"
    );
    assert_eq!(
        outcome.variable_pool["node-http"]["files"][0]["meta"]["persisted"],
        json!(false)
    );
    server.abort();
}

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
