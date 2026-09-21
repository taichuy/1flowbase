use super::*;
use base64::Engine;
use control_plane::ports::OrchestrationRuntimeRepository;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}
pub(crate) async fn seeded_flow_run() -> (sqlx::PgPool, Uuid) {
    let schema = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = schema.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, 'Capsule fixture')")
        .bind(workspace_id)
        .bind(tenant_id)
        .execute(store.pool())
        .await
        .unwrap();
    let user_id = Uuid::now_v7();
    let account = format!("capsule-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status,
            session_version
        ) values ($1, $2, $3, 'hash', $2, $2, '', 'member', true, false, 'active', 1)
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .execute(store.pool())
    .await
    .unwrap();
    let application_id = Uuid::now_v7();
    sqlx::query(
        "insert into applications (id, workspace_id, application_type, name, description, created_by, updated_by) values ($1, $2, 'agent_flow', 'Capsule fixture', '', $3, $3)",
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let flow_id = Uuid::now_v7();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(application_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let draft_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), $3, '{}'::jsonb, $4, $4)",
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let plan_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_compiled_plans (id, flow_id, flow_draft_id, schema_version, document_updated_at, plan, scope_id, created_by, updated_by) values ($1, $2, $3, $4, now(), '{}'::jsonb, (select scope_id from flows where id = $2), $5, $5)",
    )
    .bind(plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    let flow_run_id = Uuid::now_v7();
    sqlx::query(
        "insert into flow_runs (id, application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, status, created_by) values ($1, $2, $3, $4, $5, 'published_api_run', 'running', $6)",
    )
    .bind(flow_run_id)
    .bind(application_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(plan_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    (pool, flow_run_id)
}

pub(crate) async fn raw_bytes(
    store: &PgControlPlaneStore,
    flow: Uuid,
    request: Uuid,
    direction: &str,
) -> Vec<u8> {
    let section = store
        .client_trajectory_section(flow, None, request, "raw", None, 100)
        .await
        .unwrap()
        .unwrap();
    assert!(section.next_cursor.is_none());
    let mut bytes = Vec::new();
    for item in section.items {
        let value = item.value;
        if value["direction"] != direction {
            continue;
        }
        let body = value["body"].as_str().unwrap();
        if value["encoding"] == "base64" {
            bytes.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(body)
                    .unwrap(),
            );
        } else {
            bytes.extend(body.as_bytes());
        }
    }
    bytes
}

#[tokio::test]
async fn client_http_body_tee_preserves_json_and_sse_bytes_and_marks_drop_incomplete() {
    let (pool, flow) = seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = b" { \"input\": \"hello\", \"stream\": false } \n";
    for (content_type, output) in [
        ("application/json", " {\"id\":\"resp_1\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"你好  🌏\"}]}]}\n"),
        ("text/event-stream", "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"你好  🌏\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"output\":[]}}\n\n")
    ] {
        let recorder = ClientTrajectoryRecorder::new(Arc::new(store.clone()), ClientTrajectoryTransport::Http);
        recorder.record(ClientTrajectoryFrameKind::Request, request);
        assert!(recorder.bind_run(flow, None));
        // Split inside UTF-8 and JSON/SSE tokens; the transport must not reserialize.
        let chunks: Vec<Result<Bytes, std::io::Error>> = output.as_bytes().chunks(7).map(|chunk| Ok(Bytes::copy_from_slice(chunk))).collect();
        let response = Response::builder().header("content-type", content_type).body(Body::from_stream(futures_util::stream::iter(chunks))).unwrap();
        let response = observe_response(response, CaptureGuard::new(recorder.clone()));
        let delivered = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        recorder.wait_finished().await;
        assert_eq!(delivered.as_ref(), output.as_bytes());
        assert_eq!(raw_bytes(&store, flow, recorder.capture_id(), "submitted").await, request);
        assert_eq!(raw_bytes(&store, flow, recorder.capture_id(), "emitted").await, output.as_bytes());
        let null_nodes: bool = sqlx::query_scalar("select bool_and(node_run_id is null) from client_trajectory_steps where request_id=$1").bind(recorder.capture_id()).fetch_one(store.pool()).await.unwrap();
        assert!(null_nodes);
    }
    let recorder =
        ClientTrajectoryRecorder::new(Arc::new(store.clone()), ClientTrajectoryTransport::Http);
    recorder.record(ClientTrajectoryFrameKind::Request, request);
    recorder.bind_run(flow, None);
    let response = observe_response(
        Response::new(Body::from("not read")),
        CaptureGuard::new(recorder.clone()),
    );
    drop(response);
    recorder.wait_finished().await;
    let status: String =
        sqlx::query_scalar("select status from client_trajectory_captures where request_id=$1")
            .bind(recorder.capture_id())
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "incomplete");
    let recorder =
        ClientTrajectoryRecorder::new(Arc::new(store.clone()), ClientTrajectoryTransport::Http);
    recorder.record(ClientTrajectoryFrameKind::Request, request);
    recorder.bind_run(flow, None);
    let failed_stream =
        futures_util::stream::iter([Err::<Bytes, _>(std::io::Error::other("transport failed"))]);
    let response = observe_response(
        Response::new(Body::from_stream(failed_stream)),
        CaptureGuard::new(recorder.clone()),
    );
    assert!(axum::body::to_bytes(response.into_body(), 1024)
        .await
        .is_err());
    recorder.wait_finished().await;
    let status: String =
        sqlx::query_scalar("select status from client_trajectory_captures where request_id=$1")
            .bind(recorder.capture_id())
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "incomplete");
}

struct TrailersBody {
    frames: std::collections::VecDeque<Frame<Bytes>>,
    remaining: u64,
}
impl HttpBody for TrailersBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let frame = self.frames.pop_front();
        if let Some(bytes) = frame.as_ref().and_then(Frame::data_ref) {
            self.remaining -= bytes.len() as u64;
        }
        Poll::Ready(frame.map(Ok))
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.remaining)
    }
    fn is_end_stream(&self) -> bool {
        self.frames.is_empty()
    }
}

#[tokio::test]
async fn client_http_observer_preserves_trailers_and_size_hint() {
    let (pool, flow) = seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let recorder =
        ClientTrajectoryRecorder::new(Arc::new(store.clone()), ClientTrajectoryTransport::Http);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{\"input\":\"x\"}");
    recorder.bind_run(flow, None);
    let bytes = Bytes::from_static(b"{\"output\":[]}");
    let mut trailers = axum::http::HeaderMap::new();
    trailers.insert("x-response-checksum", "preserved".parse().unwrap());
    let body = Body::new(TrailersBody {
        remaining: bytes.len() as u64,
        frames: [
            Frame::data(bytes.clone()),
            Frame::trailers(trailers.clone()),
        ]
        .into(),
    });
    let mut observed =
        observe_response(Response::new(body), CaptureGuard::new(recorder.clone())).into_body();
    assert_eq!(observed.size_hint().exact(), Some(bytes.len() as u64));
    let first = futures_util::future::poll_fn(|cx| Pin::new(&mut observed).poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.into_data().unwrap(), bytes);
    let second = futures_util::future::poll_fn(|cx| Pin::new(&mut observed).poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.into_trailers().unwrap(), trailers);
    assert!(observed.is_end_stream());
    drop(observed); // No extra EOF poll is required for a complete capture.
    recorder.wait_finished().await;
    let status: String =
        sqlx::query_scalar("select status from client_trajectory_captures where request_id=$1")
            .bind(recorder.capture_id())
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "complete");
}
