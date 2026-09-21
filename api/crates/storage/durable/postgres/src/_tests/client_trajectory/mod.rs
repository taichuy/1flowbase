use control_plane_contracts::ports::*;
use serde_json::{json, Value};
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;
const AT: &str = "2026-09-22T00:00:00Z";
async fn append(
    store: &PgControlPlaneStore,
    flow: Uuid,
    node: Option<Uuid>,
    request: Uuid,
    fact: ClientTrajectoryFact,
) {
    store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: flow,
            node_run_id: node,
            request_id: request,
            observed_at: AT.into(),
            fact,
        })
        .await
        .unwrap();
}
async fn begin(store: &PgControlPlaneStore, flow: Uuid, node: Option<Uuid>, request: Uuid) {
    append(
        store,
        flow,
        node,
        request,
        ClientTrajectoryFact::Integrity {
            status: "pending".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
    )
    .await;
}
fn step(
    flow: Uuid,
    node: Option<Uuid>,
    request: Uuid,
    id: Uuid,
    origin: &str,
    category: &str,
) -> ClientTrajectoryStep {
    ClientTrajectoryStep {
        id,
        request_id: request,
        sequence: 0,
        created_at: AT.into(),
        category: category.into(),
        name: "weather".into(),
        preview: "北京".into(),
        parameters_preview: Some("city: 北京".into()),
        result_preview: None,
        status: if origin == "submitted" {
            "submitted"
        } else {
            "recorded"
        }
        .into(),
        origin: origin.into(),
        protocol: "responses".into(),
        transport: ClientTrajectoryTransport::Http,
        flow_run_id: flow,
        node_run_id: node,
        parent_id: if id == request { None } else { Some(request) },
        call_id: Some("actual-call-id".into()),
        item_id: None,
        response_id: None,
        turn_id: None,
        related_step_id: None,
        available_sections: vec![
            "overview".into(),
            "parameters".into(),
            "schema".into(),
            "result".into(),
            "timing".into(),
            "raw".into(),
        ],
    }
}
async fn section(
    store: &PgControlPlaneStore,
    flow: Uuid,
    node: Option<Uuid>,
    request: Uuid,
    id: Uuid,
    section: &str,
    value: Value,
) {
    append(
        store,
        flow,
        node,
        request,
        ClientTrajectoryFact::Section {
            step_id: id,
            section: section.into(),
            value,
        },
    )
    .await;
}

#[tokio::test]
async fn client_trajectory_scoped_pages_sections_originals_terminal_append_and_capture_links() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .integrity,
        "not_recorded"
    );
    let node = Uuid::now_v7();
    sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','succeeded' from flow_runs where id=$2")
        .bind(node).bind(flow).execute(store.pool()).await.unwrap();
    sqlx::query("update flow_runs set status='succeeded',finished_at=now() where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    let request = Uuid::now_v7();
    begin(&store, flow, Some(node), request).await;
    let root = step(flow, Some(node), request, request, "submitted", "request");
    append(
        &store,
        flow,
        Some(node),
        request,
        ClientTrajectoryFact::Step { step: root.clone() },
    )
    .await;
    let call = Uuid::now_v7();
    append(
        &store,
        flow,
        Some(node),
        request,
        ClientTrajectoryFact::Step {
            step: step(flow, Some(node), request, call, "emitted", "tool_call"),
        },
    )
    .await;
    let exact=" { \"city\": \"北京\", \"token\": \"user-token\", \"password\": \"user-password\", \"nul\": \"\\u0000\" } \n";
    section(
        &store,
        flow,
        Some(node),
        request,
        call,
        "parameters",
        Value::String(exact.into()),
    )
    .await;
    section(
        &store,
        flow,
        Some(node),
        request,
        call,
        "schema",
        json!({"name":"weather","parameters":{"properties":{"password":{"type":"string"}}}}),
    )
    .await;
    section(
        &store,
        flow,
        Some(node),
        request,
        call,
        "result",
        json!({"text":"not selected"}),
    )
    .await;
    for text in ["first ", "北京 \0", " last\n"] {
        section(&store,flow,Some(node),request,request,"raw",json!({"direction":"emitted","encoding":"utf8","body":text,"frame_kind":"response_json"})).await;
    }
    let first = store
        .client_trajectory_page(flow, Some(node), None, 1)
        .await
        .unwrap();
    assert_eq!(first.items[0].id, request);
    assert_eq!(first.integrity, "incomplete");
    let stable = first.items[0].sequence;
    let mut updated = root;
    updated.preview = "model-x · reasoning.effort=high".into();
    append(
        &store,
        flow,
        Some(node),
        request,
        ClientTrajectoryFact::Step { step: updated },
    )
    .await;
    let page = store
        .client_trajectory_page(flow, Some(node), None, 1)
        .await
        .unwrap();
    assert_eq!(page.items[0].sequence, stable);
    assert_eq!(page.items[0].created_at, AT);
    let next = store
        .client_trajectory_page(flow, Some(node), first.next_cursor, 1)
        .await
        .unwrap();
    assert_eq!(next.items[0].id, call);
    assert!(next.next_cursor.is_none());
    // Poison the unrelated body: a body-free list and selected parameters/schema must still work.
    sqlx::query("update runtime_events e set raw_json_payloads=jsonb_build_object('payload','invalid JSON') from client_trajectory_sections p where p.event_id=e.id and p.step_id=$1 and p.section='result'")
        .bind(call).execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    let parameters = store
        .client_trajectory_section(flow, Some(node), call, "parameters", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parameters.items[0].value, exact);
    assert_eq!(parameters.evidence_scope, "step");
    let schema = store
        .client_trajectory_section(flow, None, call, "schema", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        schema.items[0].value["parameters"]["properties"]["password"]["type"],
        "string"
    );
    let raw = store
        .client_trajectory_section(flow, None, call, "raw", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.evidence_scope, "capture");
    assert_eq!(raw.request_id, request);
    assert_eq!(raw.items[0].value["body"], "first ");
    let raw2 = store
        .client_trajectory_section(flow, None, call, "raw", raw.next_cursor, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw2.items[0].value["body"], "北京 \0");
    assert!(store
        .client_trajectory_section(Uuid::now_v7(), None, call, "parameters", None, 10)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .client_trajectory_section(flow, Some(Uuid::now_v7()), call, "parameters", None, 10)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .client_trajectory_section(flow, None, call, "not_a_section", None, 10)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .client_trajectory_page(flow, Some(Uuid::now_v7()), None, 10)
        .await
        .unwrap()
        .items
        .is_empty());
    // A later capture submits a tool result; associate by real call_id without recasting history as execution.
    let followup = Uuid::now_v7();
    begin(&store, flow, Some(node), followup).await;
    let result = Uuid::now_v7();
    append(
        &store,
        flow,
        Some(node),
        followup,
        ClientTrajectoryFact::Step {
            step: step(
                flow,
                Some(node),
                followup,
                result,
                "submitted",
                "tool_result",
            ),
        },
    )
    .await;
    section(&store,flow,Some(node),followup,followup,"raw",json!({"direction":"submitted","encoding":"utf8","body":"second request","frame_kind":"request"})).await;
    let page = store
        .client_trajectory_page(flow, None, None, 50)
        .await
        .unwrap();
    let result = page.items.iter().find(|s| s.id == result).unwrap();
    assert_eq!(result.related_step_id, Some(call));
    assert_eq!(result.status, "submitted");
    let original_raw = store
        .client_trajectory_section(flow, None, call, "raw", None, 32)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original_raw.items.len(), 3);
    for capture in [request, followup] {
        append(
            &store,
            flow,
            Some(node),
            capture,
            ClientTrajectoryFact::Integrity {
                status: "complete".into(),
                dropped_count: 0,
                persist_failed_count: 0,
            },
        )
        .await;
    }
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .integrity,
        "complete"
    );
    let forged = AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Integrity {
            status: "complete".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
    };
    assert!(store.append_client_trajectory(&forged).await.is_err());
    let status: String = sqlx::query_scalar("select status from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "succeeded");
}
