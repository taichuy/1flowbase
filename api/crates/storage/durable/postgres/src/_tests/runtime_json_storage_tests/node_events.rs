use super::*;

fn event(run: Uuid, kind: &str, payload: Value) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id: run,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: kind.into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
    }
}

#[tokio::test]
async fn node_transition_snapshots_share_bytes_but_keep_identity_version_and_original_dto() {
    let (store, run, _, _) = seed().await;
    let body = exceptional();
    let one = event(
        run,
        "node_started",
        json!({"type":"node_started","node_id":"a",
        "started_at":"first","input_payload":body}),
    );
    let two = event(
        run,
        "node_started",
        json!({"type":"node_started","node_id":"b",
        "started_at":"second","input_payload":body}),
    );
    let three = event(
        run,
        "node_finished",
        json!({"type":"node_finished","node_id":"a",
        "output_payload":{"new":"version\0"},"error_payload":null,"metrics_payload":{"n":9007199254740993u64}}),
    );
    let records = store
        .append_runtime_events(&[one.clone(), two.clone(), three.clone()])
        .await
        .unwrap();
    for (record, input) in records.iter().zip([one, two, three]) {
        assert_eq!(record.payload, input.payload);
        let physical: Value = sqlx::query_scalar("select payload from runtime_events where id=$1")
            .bind(record.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        for field in [
            "input_payload",
            "output_payload",
            "error_payload",
            "metrics_payload",
        ] {
            assert!(physical.get(field).is_none());
        }
        assert_eq!(physical["_node_payload_ref"]["version"], 1);
    }
    assert_ne!(
        records[0].id, records[1].id,
        "byte sharing cannot merge different facts"
    );
    let references:Vec<Uuid> = sqlx::query_scalar("select observation_body_content_id from runtime_events where flow_run_id=$1 order by sequence")
        .bind(run).fetch_all(store.pool()).await.unwrap();
    assert_eq!(references[0], references[1]);
    assert_ne!(references[0], references[2]);
    let reread = store.list_runtime_events(run, 0).await.unwrap();
    assert_eq!(
        reread.iter().map(|e| &e.payload).collect::<Vec<_>>(),
        records.iter().map(|e| &e.payload).collect::<Vec<_>>()
    );
    let wrong:Result<Value,_> = sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,$2) from runtime_events where id=$1")
        .bind(records[0].id).bind(Uuid::now_v7()).fetch_one(store.pool()).await;
    assert!(wrong.is_err());
}

#[tokio::test]
async fn failed_node_append_rolls_back_immutable_body_and_unknown_ref_version_fails_closed() {
    let (store, run, _, _) = seed().await;
    let before: i64 = sqlx::query_scalar("select count(*) from runtime_canonical_contents")
        .fetch_one(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql("create function reject_node_event() returns trigger language plpgsql as $$ begin if new.event_type='node_started' then raise exception 'controlled node append failure'; end if; return new; end $$; create trigger reject_node_event before insert on runtime_events for each row execute function reject_node_event()")
        .execute(store.pool()).await.unwrap();
    let input = event(run, "node_started", json!({"input_payload":exceptional()}));
    assert!(store.append_runtime_event(&input).await.is_err());
    let after: i64 = sqlx::query_scalar("select count(*) from runtime_canonical_contents")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        after, before,
        "event and immutable snapshot share the transaction"
    );
    sqlx::query("drop trigger reject_node_event on runtime_events")
        .execute(store.pool())
        .await
        .unwrap();
    let saved = store.append_runtime_event(&input).await.unwrap();
    sqlx::query("update runtime_events set payload=jsonb_set(payload,'{_node_payload_ref,version}','99') where id=$1")
        .bind(saved.id).execute(store.pool()).await.unwrap();
    assert!(store.list_runtime_events(run, 0).await.is_err());
}
