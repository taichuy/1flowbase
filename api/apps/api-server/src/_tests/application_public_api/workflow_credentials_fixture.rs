use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

pub(super) fn document(flow_id: &str, workflow: bool, subject: &str) -> Value {
    let start_id = if workflow {
        "node-workflow-start"
    } else {
        "node-start"
    };
    let end = if workflow {
        node(
            "node-end",
            "workflow_end",
            json!({}),
            json!({
                "identity":{"kind":"selector","value":["node-http","body"]},
                "status_code":{"kind":"selector","value":["node-http","status_code"]}
            }),
            json!([
                {"key":"identity","title":"Identity","valueType":"string"},
                {"key":"status_code","title":"HTTP status","valueType":"number"}
            ]),
        )
    } else {
        node(
            "node-end",
            "answer",
            json!({}),
            json!({"answer_template":{"kind":"templated_text","value":"{{ node-http.body }}"}}),
            json!([{"key":"answer","title":"Answer","valueType":"string"}]),
        )
    };
    json!({
        "schemaVersion":"1flowbase.flow/v2",
        "meta":{"flowId":flow_id,"name":"Workflow credential consumer","description":"","tags":[]},
        "graph":{
            "nodes":[
                node(start_id, if workflow {"workflow_start"} else {"start"},
                    json!({"input_fields":[],"sync_timeout_ms":30000}),json!({}),json!([])),
                node("node-http","http_request",json!({"method":"GET","url":"/api/console/me","body_type":"none",
                    "timeout_ms":30000,"max_response_bytes":1048576,"verify_ssl":true}),json!({
                    "params":{"kind":"named_bindings","value":[]},
                    "headers":{"kind":"named_bindings","value":[{"name":"Authorization","valueType":"string",
                        "value":{"kind":"templated_text","value":format!("Bearer {{{{ sys.{subject}_credential }}}}")}}]},
                    "body":{"kind":"templated_text","value":""},
                    "urlencoded":{"kind":"named_bindings","value":[]},
                    "form_data":{"kind":"named_bindings","value":[]}
                }),json!([
                    {"key":"body","title":"Body","valueType":"string"},
                    {"key":"status_code","title":"Status","valueType":"number"},
                    {"key":"headers","title":"Headers","valueType":"object"},
                    {"key":"files","title":"Files","valueType":"Array[File]"}
                ])),
                end
            ],
            "edges":[edge("edge-start-http",start_id,"node-http"),edge("edge-http-end","node-http","node-end")]
        },
        "editor":{"viewport":{"x":0,"y":0,"zoom":1},"annotations":[],"activeContainerPath":[]}
    })
}
fn node(id: &str, kind: &str, config: Value, bindings: Value, outputs: Value) -> Value {
    json!({"id":id,"type":kind,"alias":id,"description":"","containerId":null,"position":{"x":0,"y":0},
        "configVersion":1,"config":config,"bindings":bindings,"outputs":outputs})
}
fn edge(id: &str, source: &str, target: &str) -> Value {
    json!({"id":id,"source":source,"target":target,"sourceHandle":null,"targetHandle":null,"containerId":null,"points":[]})
}
pub(super) async fn save_document(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    workflow: bool,
    subject: &str,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let state: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let flow_id = state["data"]["draft"]["document"]["meta"]["flowId"]
        .as_str()
        .unwrap();
    let response = app.clone().oneshot(Request::builder().method("PUT")
        .uri(format!("/api/console/applications/{application_id}/orchestration/draft"))
        .header("cookie",cookie).header("x-csrf-token",csrf).header("content-type","application/json")
        .body(Body::from(json!({"document":document(flow_id,workflow,subject),"change_kind":"logical","summary":"Credential consumer"}).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
}
