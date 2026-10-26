use super::projection_cost::{event, setup};
use super::*;

async fn persisted_events(store: &PgControlPlaneStore, run: Uuid) -> Vec<(Uuid, i64, Value)> {
    sqlx::query_as("select id,sequence,runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where flow_run_id=$1 order by sequence")
        .bind(run).fetch_all(store.pool()).await.unwrap()
}

async fn projection_snapshot(store: &PgControlPlaneStore, run: Uuid) -> Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'messages',(select jsonb_agg(to_jsonb(m) order by display_sequence) from application_run_conversation_message_items m where flow_run_id=$1),
        'progress',(select to_jsonb(p) from application_run_native_projection_progress p where flow_run_id=$1),
        'summary',(select to_jsonb(s) from application_run_log_summaries s where flow_run_id=$1),
        'run',(select to_jsonb(f) from flow_runs f where id=$1))"#)
        .bind(run).fetch_one(store.pool()).await.unwrap()
}

// The archive ownership change must preserve both borrowed ordinary Values and
// transformed owned Values. Existing append_regressions remain the full-fold,
// metadata and controlled-defect oracle; this test adds exact returned bodies.
#[tokio::test]
async fn archive_ownership_preserves_exact_returns_and_projection_failure_rollback() {
    let (store, run) = setup().await;
    let mut inputs = vec![
        event(
            run.id,
            "lossless-a".into(),
            "actual\0 literal \\u0000 中文 🦀".into(),
        ),
        event(run.id, "lossless-b".into(), "full body".repeat(4096)),
    ];
    inputs[0].payload["item"]["extra"] = json!({
        "nul\0key":[null,true,false,0,-1,u64::MAX,1.25e100,{"nested":"\0\\u0000"}],
        "unknown":{"array":[],"object":{}},
    });
    for payload in [
        json!({"source":"external","missing_body":[1,null]}),
        json!({"body_ref":"already-referenced","body":{"keep":"inline"}}),
        json!({"nul\0key":true,"body":{"keep":"exceptional"}}),
        json!({"source":"external","kind":"observation","body":{"text":"archive exact\0 body","array":[null,42]}}),
    ] {
        let mut input = event(run.id, "unused".into(), String::new());
        input.event_type = "provider_protocol_observation".into();
        input.payload = payload;
        inputs.push(input);
    }
    let records = store.append_runtime_events(&inputs).await.unwrap();
    assert_eq!(records.len(), inputs.len());
    for (input, record) in inputs.iter().zip(&records) {
        assert_eq!(record.payload, input.payload);
        assert_eq!(record.flow_run_id, input.flow_run_id);
        assert_eq!(record.node_run_id, input.node_run_id);
        assert_eq!(record.span_id, input.span_id);
        assert_eq!(record.parent_span_id, input.parent_span_id);
        assert_eq!(record.event_type, input.event_type);
        assert_eq!(record.layer, input.layer);
        assert_eq!(record.source, input.source);
        assert_eq!(record.trust_level, input.trust_level);
        assert_eq!(record.item_id, input.item_id);
        assert_eq!(record.ledger_ref, input.ledger_ref);
        assert_eq!(record.visibility, input.visibility);
        assert_eq!(record.durability, input.durability);
    }
    let persisted = persisted_events(&store, run.id).await;
    assert_eq!(persisted.len(), records.len());
    for ((id, sequence, payload), record) in persisted.iter().zip(&records) {
        assert_eq!(*id, record.id);
        assert_eq!(*sequence, record.sequence);
        assert_eq!(*payload, record.payload);
    }
    for pair in records.windows(2) {
        assert!(pair[0].sequence < pair[1].sequence);
        assert_ne!(pair[0].id, pair[1].id);
    }
    let bodies: Vec<Value> = sqlx::query_scalar("select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence")
        .bind(run.id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(bodies.len(), 2);
    for (index, body) in bodies.iter().enumerate() {
        assert_eq!(body["_source_item"], inputs[index].payload["item"]);
        assert_eq!(body["content"], inputs[index].payload["item"]["content"]);
        assert_eq!(body["_log_source_sequence"], records[index].sequence);
        assert_eq!(body["_log_conflicting"], false);
        assert!(body["_log_source_revision"]
            .as_str()
            .unwrap()
            .starts_with("v7:"));
    }
    let before = projection_snapshot(&store, run.id).await;
    // This is a SAME-RUN batch. The mixed-run fallback retains its existing
    // independent per-event transactions; it is not a whole-batch guarantee.
    // Fail after runtime event insertion/RETURNING, at the projection write.
    // All materialization and projection failures must still precede commit.
    sqlx::raw_sql(
        r#"
        create function history_memory_reject_progress() returns trigger language plpgsql as $$
        begin raise exception 'history-memory controlled projection failure'; end $$;
        create trigger history_memory_reject_progress before insert or update
            on application_run_native_projection_progress for each row
            execute function history_memory_reject_progress();
    "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    let failed = [event(
        run.id,
        "must-rollback".into(),
        "rollback body\0".into(),
    )];
    assert!(store.append_runtime_events(&failed).await.is_err());
    assert_eq!(persisted_events(&store, run.id).await, persisted);
    assert_eq!(projection_snapshot(&store, run.id).await, before);
    sqlx::raw_sql("drop trigger history_memory_reject_progress on application_run_native_projection_progress; drop function history_memory_reject_progress()")
        .execute(store.pool()).await.unwrap();
    let returned = store.append_runtime_events(&failed).await.unwrap();
    assert_eq!(returned[0].payload, failed[0].payload);
    assert_eq!(returned[0].sequence, records.last().unwrap().sequence + 1);
}

// Mixed-run batches retain independent per-event transactions: an error leaves
// the committed prefix visible and must not execute the remaining inputs.
#[tokio::test]
async fn mixed_run_append_failure_keeps_committed_prefix_and_skips_tail() {
    let (store, run) = setup().await;
    assert!(persisted_events(&store, run.id).await.is_empty());
    let missing_run = Uuid::now_v7();
    let exists: bool = sqlx::query_scalar("select exists(select 1 from flow_runs where id=$1)")
        .bind(missing_run)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(!exists);
    let inputs = [
        event(run.id, "mixed-prefix".into(), "committed exact\0".into()),
        event(missing_run, "mixed-failure".into(), "missing run".into()),
        event(run.id, "mixed-tail".into(), "later literal \\u0000".into()),
    ];

    assert!(store.append_runtime_events(&inputs).await.is_err());
    let committed = persisted_events(&store, run.id).await;
    assert_eq!(committed.len(), 1, "only the successful prefix may persist");
    assert_eq!(committed[0].2, inputs[0].payload);
    let bodies: Vec<Value> = sqlx::query_scalar(
        "select runtime_original_json(native_message,raw_json_payloads,'native_message') from application_run_conversation_message_items where flow_run_id=$1 and source_occurrence_sequence is not null order by display_sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "the prefix projection must commit with its event"
    );
    assert_eq!(bodies[0]["_source_item"], inputs[0].payload["item"]);

    let tail = store.append_runtime_events(&inputs[2..]).await.unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].payload, inputs[2].payload);
    assert_eq!(tail[0].sequence, committed[0].1 + 1);
    let after = persisted_events(&store, run.id).await;
    assert_eq!(after.len(), 2);
    assert_eq!(after[0], committed[0]);
    assert_eq!(after[1].2, inputs[2].payload);
}
