use super::*;
use crate::agent_logs_protocol as protocol;
use control_plane_contracts::ports::{AgentLogsBatch, OrchestrationRuntimeRepository};
use opentelemetry_proto::tonic::{
    collector::trace::v1::ExportTraceServiceResponse,
    common::v1::{any_value, AnyValue},
};
use prost::Message;

const PATH: &str = "/api/logs/otlp/v1/traces";

async fn console(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn key(app: &Router, cookie: &str, csrf: &str, application_id: &str) -> (Uuid, String) {
    let response = console(
        app,
        "POST",
        &format!("/api/console/applications/{application_id}/api-keys"),
        cookie,
        csrf,
        json!({"name":"OTLP fixture"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let value = response_json(response).await;
    (
        Uuid::parse_str(value["data"]["id"].as_str().unwrap()).unwrap(),
        value["data"]["token"].as_str().unwrap().into(),
    )
}
async fn post(
    app: &Router,
    token: Option<&str>,
    mime: &str,
    encoding: Option<&str>,
    body: Vec<u8>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(PATH)
        .header("content-type", mime);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if let Some(encoding) = encoding {
        request = request.header("content-encoding", encoding);
    }
    app.clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}
async fn ack(response: axum::response::Response, mime: &str) -> ExportTraceServiceResponse {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], mime);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    if mime == "application/json" {
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(value.get("data").is_none() && value.get("meta").is_none());
        serde_json::from_value(value).unwrap()
    } else {
        ExportTraceServiceResponse::decode(bytes.as_ref()).unwrap()
    }
}
async fn error(response: axum::response::Response, status: StatusCode, mime: &str, message: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["content-type"], mime);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let actual = if mime == "application/x-protobuf" {
        tonic_types::pb::Status::decode(bytes.as_ref())
            .unwrap()
            .message
    } else {
        serde_json::from_slice::<Value>(&bytes).unwrap()["message"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(actual, message);
}
async fn receipt_count(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from application_log_upload_receipts where application_id=$1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .fetch_one(pool)
    .await
    .unwrap()
}
fn batch(source: &str, event: &str, text: &str) -> AgentLogsBatch {
    serde_json::from_value(json!({"schema_version":"1flowbase.agent-logs/v1","source_id":source,"source_client":"fixture","events":[{"event_id":event,"source_session_id":"source-session","source_task_id":"source-task","sequence":1,"occurred_at":"2026-10-07T08:00:00.123456789Z","kind":"user","content":text,"raw":{"complete":"raw source\u{0}value"}}]})).unwrap()
}

#[tokio::test]
async fn otlp_route_json_protobuf_gzip_full_ack_and_duplicate_replay_keep_scope_and_raw() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let id = create_application_with_type(&app, &cookie, &csrf, "OTLP traces", "agent_logs").await;
    let other =
        create_application_with_type(&app, &cookie, &csrf, "Foreign target", "agent_logs").await;
    let (key_id, token) = key(&app, &cookie, &csrf, &id).await;
    let mut request = protocol::decode(
        "application/json",
        None,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/api/kernel/src/contracts/application_runtime_shared/agent_logs_protocol/_tests/fixtures/openlit.json"
        )),
    )
    .unwrap();
    // Payload/resource claims cannot override the authenticated application's scope.
    request.resource_spans[0]
        .resource
        .as_mut()
        .unwrap()
        .attributes
        .push(opentelemetry_proto::tonic::common::v1::KeyValue {
            key: "application_id".into(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(other.clone())),
            }),
            ..Default::default()
        });
    request.resource_spans[0]
        .resource
        .as_mut()
        .unwrap()
        .attributes
        .push(opentelemetry_proto::tonic::common::v1::KeyValue {
            key: "scope_id".into(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(Uuid::now_v7().to_string())),
            }),
            ..Default::default()
        });
    let expected = protocol::traces_to_batches(&request).unwrap();
    let expected_count = expected
        .iter()
        .map(|batch| batch.events.len())
        .sum::<usize>();
    for (mime, body) in [
        ("application/json", protocol::encode_json(&request).unwrap()),
        (
            "application/x-protobuf",
            protocol::encode_protobuf(&request),
        ),
    ] {
        for encoding in [None, Some("gzip")] {
            let body = if encoding.is_some() {
                protocol::encode_gzip(&body).unwrap()
            } else {
                body.clone()
            };
            assert!(
                ack(post(&app, Some(&token), mime, encoding, body).await, mime)
                    .await
                    .partial_success
                    .is_none()
            );
            assert_eq!(receipt_count(&pool, &id).await, expected_count as i64);
        }
    }
    assert_eq!(receipt_count(&pool, &other).await, 0);
    let application_id = Uuid::parse_str(&id).unwrap();
    let scope: Uuid = sqlx::query_scalar("select scope_id from applications where id=$1")
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let ids: Vec<Uuid> = sqlx::query_scalar("select id from application_run_log_tasks where application_id=$1 and scope_id=$2 and api_key_id=$3 order by id").bind(application_id).bind(scope).bind(key_id).fetch_all(&pool).await.unwrap();
    assert_eq!(ids.len(), 4);
    let store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone());
    let stored = store
        .read_imported_agent_logs(application_id, scope, &ids)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(stored).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let runs: i64 = sqlx::query_scalar("select count(*) from flow_runs where application_id=$1")
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[tokio::test]
async fn otlp_route_sealed_bearer_reception_type_and_invalid_wire_have_standard_errors() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let id = create_application_with_type(&app, &cookie, &csrf, "OTLP gates", "agent_logs").await;
    let (_, token) = key(&app, &cookie, &csrf, &id).await;
    let request =
        protocol::batches_to_traces(&[batch("collector", "gate-event", "source body")]).unwrap();
    let json = protocol::encode_json(&request).unwrap();
    error(
        post(&app, None, "application/json", None, json.clone()).await,
        StatusCode::UNAUTHORIZED,
        "application/json",
        "not_authenticated",
    )
    .await;
    error(
        post(
            &app,
            Some("sk-invalid-key"),
            "application/x-protobuf",
            None,
            protocol::encode_protobuf(&request),
        )
        .await,
        StatusCode::UNAUTHORIZED,
        "application/x-protobuf",
        "not_authenticated",
    )
    .await;
    error(
        post(&app, Some(&token), "image/png", None, json.clone()).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "application/json",
        "agent_logs_otlp_content_type",
    )
    .await;
    error(
        post(
            &app,
            Some(&token),
            "application/json",
            Some("br"),
            json.clone(),
        )
        .await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "application/json",
        "agent_logs_otlp_content_encoding",
    )
    .await;
    error(
        post(
            &app,
            Some(&token),
            "application/json",
            Some("gzip"),
            b"not gzip".to_vec(),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "application/json",
        "agent_logs_otlp_payload",
    )
    .await;
    error(
        post(
            &app,
            Some(&token),
            "application/json",
            None,
            b"{not json".to_vec(),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "application/json",
        "agent_logs_otlp_payload",
    )
    .await;
    error(
        post(
            &app,
            Some(&token),
            "application/x-protobuf",
            None,
            vec![255],
        )
        .await,
        StatusCode::BAD_REQUEST,
        "application/x-protobuf",
        "agent_logs_otlp_payload",
    )
    .await;
    let disabled = console(
        &app,
        "PATCH",
        &format!("/api/console/applications/{id}/api-status"),
        &cookie,
        &csrf,
        json!({"api_enabled":false}),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    error(
        post(&app, Some(&token), "application/json", None, json.clone()).await,
        StatusCode::FORBIDDEN,
        "application/json",
        "agent_logs_ingestion_disabled",
    )
    .await;
    assert_eq!(receipt_count(&pool, &id).await, 0);
    let flow = create_application(&app, &cookie, &csrf, "Unsupported flow source").await;
    let (_, flow_token) = key(&app, &cookie, &csrf, &flow).await;
    error(
        post(&app, Some(&flow_token), "application/json", None, json).await,
        StatusCode::FORBIDDEN,
        "application/json",
        "agent_logs_application_type",
    )
    .await;
    assert_eq!(receipt_count(&pool, &flow).await, 0);
}

#[tokio::test]
async fn otlp_route_partial_ack_counts_duplicate_input_spans_and_stops_unattempted_sources() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let id = create_application_with_type(&app, &cookie, &csrf, "OTLP partial", "agent_logs").await;
    let (_, token) = key(&app, &cookie, &csrf, &id).await;
    let prior =
        protocol::batches_to_traces(&[batch("b-source", "conflict-event", "old immutable body")])
            .unwrap();
    assert!(ack(
        post(
            &app,
            Some(&token),
            "application/json",
            None,
            protocol::encode_json(&prior).unwrap()
        )
        .await,
        "application/json"
    )
    .await
    .partial_success
    .is_none());
    let changed =
        protocol::batches_to_traces(&[batch("b-source", "conflict-event", "changed body")])
            .unwrap();
    error(
        post(
            &app,
            Some(&token),
            "application/x-protobuf",
            None,
            protocol::encode_protobuf(&changed),
        )
        .await,
        StatusCode::CONFLICT,
        "application/x-protobuf",
        "agent_logs_event_conflict",
    )
    .await;
    let mut request = protocol::batches_to_traces(&[
        batch("a-source", "accepted-event", "accepted body"),
        batch("b-source", "conflict-event", "changed body"),
        batch("c-source", "unattempted-event", "unattempted body"),
    ])
    .unwrap();
    let duplicate = request.resource_spans[1].scope_spans[0].spans[0].clone();
    request.resource_spans[1].scope_spans[0]
        .spans
        .push(duplicate);
    let response = ack(
        post(
            &app,
            Some(&token),
            "application/json",
            None,
            protocol::encode_json(&request).unwrap(),
        )
        .await,
        "application/json",
    )
    .await;
    let partial = response.partial_success.unwrap();
    assert_eq!(
        partial.rejected_spans, 3,
        "two duplicate input spans in rejected source plus one unattempted span"
    );
    assert!(partial.error_message.contains("Do not retry"));
    assert!(
        !partial.error_message.contains(&token) && !partial.error_message.contains("changed body")
    );
    let sources: Vec<String> = sqlx::query_scalar("select source_id from application_log_upload_receipts where application_id=$1 order by source_id").bind(Uuid::parse_str(&id).unwrap()).fetch_all(&pool).await.unwrap();
    assert_eq!(sources, ["a-source", "b-source"]);
}

#[tokio::test]
async fn otlp_route_validates_all_source_facts_before_any_batch_commit() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let id =
        create_application_with_type(&app, &cookie, &csrf, "OTLP validation", "agent_logs").await;
    let (_, token) = key(&app, &cookie, &csrf, &id).await;
    let mut request = protocol::batches_to_traces(&[
        batch("a-source", "valid", "valid source"),
        batch("z-source", "invalid", "invalid source"),
    ])
    .unwrap();
    let attribute = request.resource_spans[1].scope_spans[0].spans[0]
        .attributes
        .iter_mut()
        .find(|attribute| attribute.key == "1flowbase.agent_log.source_fact")
        .unwrap();
    attribute.value = Some(AnyValue {
        value: Some(any_value::Value::StringValue(
            "malformed-source-fact".into(),
        )),
    });
    error(
        post(
            &app,
            Some(&token),
            "application/json",
            None,
            protocol::encode_json(&request).unwrap(),
        )
        .await,
        StatusCode::BAD_REQUEST,
        "application/json",
        "agent_logs_otlp_payload",
    )
    .await;
    assert_eq!(receipt_count(&pool, &id).await, 0);
}
