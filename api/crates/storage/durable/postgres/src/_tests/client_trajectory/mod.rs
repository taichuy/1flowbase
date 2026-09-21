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
        namespace: None,
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
        ClientTrajectoryFact::Step {
            step: Box::new(root.clone()),
        },
    )
    .await;
    let call = Uuid::now_v7();
    append(
        &store,
        flow,
        Some(node),
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(step(
                flow,
                Some(node),
                request,
                call,
                "emitted",
                "tool_call",
            )),
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
    assert_eq!(first.integrity, "pending");
    let stable = first.items[0].sequence;
    let mut updated = root;
    updated.preview = "model-x · reasoning.effort=high".into();
    append(
        &store,
        flow,
        Some(node),
        request,
        ClientTrajectoryFact::Step {
            step: Box::new(updated),
        },
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
            step: Box::new(step(
                flow,
                Some(node),
                followup,
                result,
                "submitted",
                "tool_result",
            )),
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

#[tokio::test]
async fn client_trajectory_cross_capture_namespace_is_exact_or_inherited_from_real_call() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    let mut ids = Vec::new();
    for namespace in [Some("github"), Some("docs"), None] {
        let id = Uuid::now_v7();
        ids.push(id);
        let mut call = step(flow, None, request, id, "emitted", "tool_call");
        call.namespace = namespace.map(str::to_owned);
        if namespace.is_none() {
            call.call_id = Some("legacy".into());
        }
        append(
            &store,
            flow,
            None,
            request,
            ClientTrajectoryFact::Step {
                step: Box::new(call),
            },
        )
        .await;
    }
    // Old metadata remains nullable without a migration or raw-body backfill.
    sqlx::query("update client_trajectory_steps set metadata=metadata-'namespace' where id=$1")
        .bind(ids[2])
        .execute(store.pool())
        .await
        .unwrap();
    let next = Uuid::now_v7();
    begin(&store, flow, None, next).await;
    let mut results = Vec::new();
    for (namespace, call_id) in [
        (Some("github"), "actual-call-id"),
        (Some("unknown"), "actual-call-id"),
        (None, "actual-call-id"),
        (None, "legacy"),
    ] {
        let id = Uuid::now_v7();
        results.push(id);
        let mut result = step(flow, None, next, id, "submitted", "tool_result");
        result.namespace = namespace.map(str::to_owned);
        result.call_id = Some(call_id.into());
        append(
            &store,
            flow,
            None,
            next,
            ClientTrajectoryFact::Step {
                step: Box::new(result),
            },
        )
        .await;
    }
    let page = store
        .client_trajectory_page(flow, None, None, 50)
        .await
        .unwrap();
    let find = |id| page.items.iter().find(|step| step.id == id).unwrap();
    assert_eq!(find(results[0]).related_step_id, Some(ids[0]));
    assert_eq!(find(results[0]).namespace.as_deref(), Some("github"));
    assert!(find(results[1]).related_step_id.is_none());
    assert_eq!(find(results[1]).namespace.as_deref(), Some("unknown"));
    assert_eq!(find(results[2]).related_step_id, Some(ids[1]));
    assert_eq!(find(results[2]).namespace.as_deref(), Some("docs"));
    assert_eq!(find(results[3]).related_step_id, Some(ids[2]));
    assert!(find(results[3]).namespace.is_none());
    for id in results {
        assert_eq!(find(id).origin, "submitted");
    }
}

#[tokio::test]
async fn client_trajectory_integrity_preserves_pending_and_prioritizes_real_loss() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    begin(&store, flow, None, first).await;
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .integrity,
        "pending"
    );
    append(
        &store,
        flow,
        None,
        first,
        ClientTrajectoryFact::Integrity {
            status: "complete".into(),
            dropped_count: 0,
            persist_failed_count: 0,
        },
    )
    .await;
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .integrity,
        "complete"
    );
    begin(&store, flow, None, second).await;
    assert_eq!(
        store
            .client_trajectory_page(flow, None, None, 50)
            .await
            .unwrap()
            .integrity,
        "pending"
    );
    for (status, dropped_count, persist_failed_count) in
        [("incomplete", 0, 0), ("complete", 1, 0), ("complete", 0, 1)]
    {
        append(
            &store,
            flow,
            None,
            first,
            ClientTrajectoryFact::Integrity {
                status: status.into(),
                dropped_count,
                persist_failed_count,
            },
        )
        .await;
        assert_eq!(
            store
                .client_trajectory_page(flow, None, None, 50)
                .await
                .unwrap()
                .integrity,
            "incomplete"
        );
    }
}

#[tokio::test]
async fn client_trajectory_node_links_filter_real_shared_captures_without_relabeling_steps() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let nodes = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    for node in nodes {
        sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,$1::text,'llm','LLM','succeeded' from flow_runs where id=$2")
            .bind(node).bind(flow).execute(store.pool()).await.unwrap();
    }
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: step(flow, None, request, request, "submitted", "request"),
        },
    )
    .await;
    section(
        &store,
        flow,
        None,
        request,
        request,
        "raw",
        json!({"body":"actual client request"}),
    )
    .await;
    for node_run_id in [nodes[0], nodes[1], nodes[0]] {
        append(
            &store,
            flow,
            None,
            request,
            ClientTrajectoryFact::NodeLink { node_run_id },
        )
        .await;
    }
    for node in &nodes[..2] {
        let page = store
            .client_trajectory_page(flow, Some(*node), None, 10)
            .await
            .unwrap();
        assert_eq!(page.integrity, "pending");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].request_id, request);
        assert!(
            page.items[0].node_run_id.is_none(),
            "client capture remains shared, never relabeled as supplier content"
        );
        let raw = store
            .client_trajectory_section(flow, Some(*node), request, "raw", None, 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(raw.items[0].value["body"], "actual client request");
    }
    let unrelated = store
        .client_trajectory_page(flow, Some(nodes[2]), None, 10)
        .await
        .unwrap();
    assert!(unrelated.items.is_empty());
    assert_eq!(unrelated.integrity, "not_recorded");
    assert!(store
        .client_trajectory_section(flow, Some(nodes[2]), request, "raw", None, 10)
        .await
        .unwrap()
        .is_none());
    let (_, other_flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    assert!(store
        .append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: flow,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::NodeLink {
                node_run_id: other_flow
            }
        })
        .await
        .is_err());
    let count: i64 =
        sqlx::query_scalar("select count(*) from client_trajectory_node_links where request_id=$1")
            .bind(request)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn client_trajectory_append_allows_fk_readers_and_serializes_event_sequences() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let mut fk_reader = store.pool().begin().await.unwrap();
    sqlx::query("select id from flow_runs where id=$1 for key share")
        .bind(flow)
        .fetch_one(&mut *fk_reader)
        .await
        .unwrap();
    let request = Uuid::now_v7();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store.append_client_trajectory(&AppendClientTrajectoryInput {
            flow_run_id: flow,
            node_run_id: None,
            request_id: request,
            observed_at: AT.into(),
            fact: ClientTrajectoryFact::Integrity {
                status: "pending".into(),
                dropped_count: 0,
                persist_failed_count: 0,
            },
        }),
    )
    .await;
    fk_reader.rollback().await.unwrap();
    result
        .expect("non-key observation append must coexist with FK reader")
        .unwrap();
    let mut writers = Vec::new();
    for _ in 0..4 {
        let store = store.clone();
        writers.push(tokio::spawn(async move {
            for _ in 0..8 {
                append(
                    &store,
                    flow,
                    None,
                    request,
                    ClientTrajectoryFact::Section {
                        step_id: request,
                        section: "raw".into(),
                        value: json!({"body":"fragment"}),
                    },
                )
                .await;
            }
        }));
    }
    for writer in writers {
        writer.await.unwrap();
    }
    let (count,unique):(i64,i64)=sqlx::query_as("select count(*),count(distinct sequence) from runtime_events where flow_run_id=$1 and event_type='client_protocol_trajectory'").bind(flow).fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 33);
    assert_eq!(count, unique);
}

#[tokio::test]
async fn client_trajectory_full_request_value_budget_excludes_fixed_observation_envelope() {
    let (pool, flow) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Step {
            step: step(flow, None, request, request, "submitted", "request"),
        },
    )
    .await;
    let mut value = json!({"role":"user","content":""});
    let overhead = serde_json::to_vec(&value).unwrap().len();
    value["content"] = json!("x".repeat(2 * 1024 * 1024 - overhead - 32));
    assert!(
        serde_json::to_vec(&json!({"input":[&value]}))
            .unwrap()
            .len()
            <= 2 * 1024 * 1024
    );
    let input = AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact: ClientTrajectoryFact::Section {
            step_id: request,
            section: "overview".into(),
            value: value.clone(),
        },
    };
    assert!(serde_json::to_vec(&input).unwrap().len() > 2 * 1024 * 1024);
    store.append_client_trajectory(&input).await.unwrap();
    let actual = store
        .client_trajectory_section(flow, None, request, "overview", None, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual.items[0].value, value);
    let oversized = AppendClientTrajectoryInput {
        fact: ClientTrajectoryFact::Section {
            step_id: request,
            section: "overview".into(),
            value: json!("x".repeat(2 * 1024 * 1024 + 8192)),
        },
        ..input
    };
    assert!(store.append_client_trajectory(&oversized).await.is_err());
}
