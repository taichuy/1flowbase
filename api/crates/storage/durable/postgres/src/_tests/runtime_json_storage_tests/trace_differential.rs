use super::*;

fn node(id: Uuid, locator: &str, parent: Option<Uuid>) -> ApplicationRunTraceNodeProjectionInput {
    let mut node = super::trace_shared_payload::trace_node(id);
    node.stable_locator = locator.into();
    node.parent_trace_node_id = parent;
    node.metrics_payload = exceptional();
    node.content_ref = Some("ref\0with\\escape".into());
    node
}

fn content(id: Uuid, payload: Value) -> ApplicationRunTraceNodeContentProjectionInput {
    ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id: id,
        content_kind: "tool_callback".into(),
        payload: payload.into(),
        source_refs: exceptional(),
    }
}

fn projection(run: Uuid, ids: &[Uuid]) -> ReplaceApplicationRunTraceProjectionInput {
    ReplaceApplicationRunTraceProjectionInput {
        flow_run_id: run,
        projection_version: 15,
        source_watermark: "before".into(),
        nodes: ids
            .iter()
            .enumerate()
            .map(|(index, id)| node(*id, &format!("node:{index}"), None))
            .collect(),
        contents: ids.iter().map(|id| content(*id, exceptional())).collect(),
    }
}

async fn physical_contents(store: &PgControlPlaneStore, run: Uuid) -> Vec<(Uuid, Uuid, String)> {
    sqlx::query_as("select trace_node_id, id, ctid::text from application_run_trace_node_contents where flow_run_id=$1 order by trace_node_id")
        .bind(run).fetch_all(store.pool()).await.unwrap()
}

#[tokio::test]
async fn trace_projection_batches_keep_unchanged_contents_and_public_generation_times() {
    let (store, run, _, _) = seed().await;
    let ids: Vec<_> = (0..256).map(|_| Uuid::now_v7()).collect();
    let mut input = projection(run, &ids);
    for item in &mut input.contents {
        item.payload = json!({"body": "界😀\n\\\"\0".repeat(128)}).into();
    }
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    let before = physical_contents(&store, run).await;
    assert_eq!(before.len(), ids.len());
    input.source_watermark = "after".into();
    input.projection_version += 1;
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert_eq!(
        physical_contents(&store, run).await,
        before,
        "unchanged bodies must not be rewritten"
    );
    for (index, id) in ids.iter().enumerate() {
        let saved = store
            .get_application_run_trace_node(run, *id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.projection_version, input.projection_version);
        assert_eq!(saved.source_watermark, input.source_watermark);
        assert_eq!(saved.metrics_payload, input.nodes[index].metrics_payload);
        assert_eq!(saved.content_ref, input.nodes[index].content_ref);
        let body = store
            .get_application_run_trace_node_content(run, *id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            body.payload,
            serde_json::to_value(&input.contents[index].payload).unwrap()
        );
        assert_eq!(body.source_refs, input.contents[index].source_refs);
        assert_eq!(body.created_at, saved.created_at);
        assert_eq!(body.updated_at, saved.updated_at);
    }
    // Same query projection, different original: equality must include the raw
    // sidecar rather than comparing only the JSONB business column.
    input.contents[0].payload = json!("before\0after").into();
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    let nul = physical_contents(&store, run).await;
    input.contents[0].payload = json!("before\\u0000after").into();
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    let literal = physical_contents(&store, run).await;
    assert_ne!(
        nul.iter().find(|row| row.0 == ids[0]).unwrap().2,
        literal.iter().find(|row| row.0 == ids[0]).unwrap().2
    );
    assert_eq!(
        nul.iter().filter(|row| row.0 != ids[0]).collect::<Vec<_>>(),
        literal
            .iter()
            .filter(|row| row.0 != ids[0])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        store
            .get_application_run_trace_node_content(run, ids[0])
            .await
            .unwrap()
            .unwrap()
            .payload,
        json!("before\\u0000after")
    );
}

#[tokio::test]
async fn trace_projection_reparent_locator_changes_content_removal_and_empty_are_atomic() {
    let (store, run, _, _) = seed().await;
    let [a, b, c] = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let mut input = projection(run, &[a, b, c]);
    input.nodes[1].parent_trace_node_id = Some(a);
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    let initial = physical_contents(&store, run).await;
    input.nodes.remove(0);
    input.contents.remove(0);
    input.nodes[0].parent_trace_node_id = Some(c);
    input.source_watermark = "reparent".into();
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert_eq!(
        physical_contents(&store, run).await,
        initial
            .into_iter()
            .filter(|row| row.0 != a)
            .collect::<Vec<_>>(),
        "deleting the old parent must preserve retained child bodies"
    );
    assert_eq!(
        store
            .get_application_run_trace_node(run, b)
            .await
            .unwrap()
            .unwrap()
            .parent_trace_node_id,
        Some(c)
    );
    let first_locator = input.nodes[0].stable_locator.clone();
    input.nodes[0].stable_locator = input.nodes[1].stable_locator.clone();
    input.nodes[1].stable_locator = first_locator;
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert_eq!(
        store
            .get_application_run_trace_node_by_locator(run, &input.nodes[0].stable_locator)
            .await
            .unwrap()
            .unwrap()
            .trace_node_id,
        b
    );
    let replacement = Uuid::now_v7();
    input.nodes[0].trace_node_id = replacement;
    input.contents[0].trace_node_id = replacement;
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert!(store
        .get_application_run_trace_node(run, b)
        .await
        .unwrap()
        .is_none());
    input.nodes[0].has_content = false;
    input.contents.remove(0);
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert!(store
        .get_application_run_trace_node_content(run, replacement)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get_application_run_trace_node_content(run, c)
        .await
        .unwrap()
        .is_some());
    input.nodes.clear();
    input.contents.clear();
    input.source_watermark = "empty".into();
    store
        .replace_application_run_trace_projection(&input)
        .await
        .unwrap();
    assert!(store
        .list_application_run_trace_roots(run)
        .await
        .unwrap()
        .is_empty());
    assert!(physical_contents(&store, run).await.is_empty());
    assert_eq!(
        store
            .get_application_run_trace_projection_status(run, 15)
            .await
            .unwrap()
            .unwrap()
            .source_watermark,
        "empty"
    );
}

#[tokio::test]
async fn trace_projection_failed_refresh_rolls_back_detach_delete_payload_and_status() {
    let (store, run, _, _) = seed().await;
    let [a, b] = [Uuid::now_v7(), Uuid::now_v7()];
    let mut original = projection(run, &[a, b]);
    original.nodes[1].parent_trace_node_id = Some(a);
    store
        .replace_application_run_trace_projection(&original)
        .await
        .unwrap();
    let before = physical_contents(&store, run).await;
    let mut invalid = original.clone();
    invalid.nodes.remove(0);
    invalid.contents.remove(0);
    invalid.nodes[0].parent_trace_node_id = Some(Uuid::now_v7());
    invalid.contents[0].payload = json!({"must": "roll back"}).into();
    invalid.source_watermark = "failed".into();
    assert!(store
        .replace_application_run_trace_projection(&invalid)
        .await
        .is_err());
    assert_eq!(physical_contents(&store, run).await, before);
    assert_eq!(
        store
            .get_application_run_trace_node(run, b)
            .await
            .unwrap()
            .unwrap()
            .parent_trace_node_id,
        Some(a)
    );
    assert_eq!(
        store
            .get_application_run_trace_projection_status(run, 15)
            .await
            .unwrap()
            .unwrap()
            .source_watermark,
        "before"
    );
    let mut duplicate = original.clone();
    duplicate.nodes.push(duplicate.nodes[0].clone());
    assert!(store
        .replace_application_run_trace_projection(&duplicate)
        .await
        .is_err());
    duplicate = original.clone();
    duplicate.nodes[1].stable_locator = duplicate.nodes[0].stable_locator.clone();
    assert!(store
        .replace_application_run_trace_projection(&duplicate)
        .await
        .is_err());
    duplicate = original.clone();
    duplicate.contents.push(duplicate.contents[0].clone());
    assert!(store
        .replace_application_run_trace_projection(&duplicate)
        .await
        .is_err());
    assert_eq!(physical_contents(&store, run).await, before);

    let other = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(other).bind(run).execute(store.pool()).await.unwrap();
    let mut collision = original.clone();
    collision.flow_run_id = other;
    assert!(store
        .replace_application_run_trace_projection(&collision)
        .await
        .is_err());
    assert_eq!(physical_contents(&store, run).await, before);
    assert!(physical_contents(&store, other).await.is_empty());
}

#[tokio::test]
async fn trace_projection_revision_and_lease_recovery_keep_new_work_after_old_finish() {
    let (store, run, _, _) = seed().await;
    // Seed one owned queue item after formal migrations; no polling or wall-clock
    // sleeps are needed to exercise the production claim/finish state machine.
    sqlx::query("delete from application_run_trace_refresh_queue")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("insert into application_run_trace_refresh_queue(flow_run_id) values($1)")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let old = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    assert!(store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .is_none());
    sqlx::query(
        "update application_run_trace_refresh_queue set revision=revision+1 where flow_run_id=$1",
    )
    .bind(run)
    .execute(store.pool())
    .await
    .unwrap();
    store
        .finish_application_run_trace_refresh(&old, true)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar(
        "select revision from application_run_trace_refresh_queue where flow_run_id=$1",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(remaining, old.revision + 1);
    sqlx::query(
        "update application_run_trace_refresh_queue set available_at=now() where flow_run_id=$1",
    )
    .bind(run)
    .execute(store.pool())
    .await
    .unwrap();
    let failed = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    store
        .finish_application_run_trace_refresh(&failed, false)
        .await
        .unwrap();
    let retry: (Option<OffsetDateTime>, bool) = sqlx::query_as("select lease_until, available_at>now() from application_run_trace_refresh_queue where flow_run_id=$1")
        .bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(retry, (None, true));
    sqlx::query("update application_run_trace_refresh_queue set available_at=now(), lease_until=now()-interval '1 second' where flow_run_id=$1")
        .bind(run).execute(store.pool()).await.unwrap();
    let recovered = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    store
        .replace_application_run_trace_projection(&projection(run, &[Uuid::now_v7()]))
        .await
        .unwrap();
    // Publication triggers may enqueue a new revision; finish must honor it.
    store
        .finish_application_run_trace_refresh(&recovered, true)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("select count(*) from application_run_trace_refresh_queue where flow_run_id=$1 and revision=$2")
        .bind(run).bind(recovered.revision).fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 0);
}
