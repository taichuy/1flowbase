use super::*;

#[tokio::test]
async fn node_details_keep_identity_sections_and_callback_output_lossless() {
    let (store, run, app, _) = seed().await;
    let original = exceptional();
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "detail-node".into(),
            node_type: "fixture".into(),
            node_alias: "Detail node".into(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: original.clone(),
            debug_payload: original.clone(),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let checkpoint = store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: run,
            node_run_id: Some(node.id),
            status: "waiting".into(),
            reason: "callback".into(),
            locator_payload: json!({"node_id":"detail-node"}),
            variable_snapshot: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let callback = store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id: run,
            node_run_id: node.id,
            callback_kind: "fixture".into(),
            request_payload: json!({}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    let waiting = store
        .get_callback_resume_context(app, callback.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(waiting.checkpoint.id, checkpoint.id);
    assert_eq!(waiting.waiting_node.id, node.id);
    let saved = store
        .update_node_run(&UpdateNodeRunInput {
            node_run_id: node.id,
            status: domain::NodeRunStatus::Running,
            output_payload: original.clone(),
            error_payload: Some(Value::Null),
            metrics_payload: json!({"usage":{"input_tokens":7}}),
            debug_payload: original.clone(),
            finished_at: None,
        })
        .await
        .unwrap();
    assert_eq!(saved.id, node.id);
    assert_eq!(saved.input_payload, original);
    assert_eq!(saved.output_payload, original);
    assert_eq!(saved.error_payload, Some(Value::Null));
    assert_eq!(
        store
            .get_callback_resume_context(app, callback.id)
            .await
            .unwrap()
            .unwrap()
            .waiting_node
            .output_payload,
        original
    );
    let stream = store
        .get_published_run_stream_state(app, run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stream.node_usages.len(), 1);
    assert_eq!(
        stream.node_usages[0].metrics_usage,
        Some(json!({"input_tokens":7}))
    );
    let sections: Vec<String> = sqlx::query_scalar(
        "select section from node_run_details where node_run_id=$1 order by section",
    )
    .bind(node.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        sections,
        vec![
            "debug_payload",
            "error_payload",
            "input_payload",
            "metrics_payload",
            "output_payload"
        ]
    );
    let body_columns: i64 = sqlx::query_scalar(
        "select count(*) from information_schema.columns where table_schema=current_schema() and table_name='node_runs' and column_name in ('input_payload','output_payload','error_payload','metrics_payload','debug_payload','raw_json_payloads')"
    ).fetch_one(store.pool()).await.unwrap();
    assert_eq!(body_columns, 0);
    let input = store
        .list_application_run_trace_node_run_sections(run, vec![node.id], "input_payload")
        .await
        .unwrap();
    assert_eq!(input[0].input_payload, original);
    assert_eq!(input[0].debug_payload, json!({}));
    assert_eq!(input[0].output_payload, json!({}));
    // Corrupt an unrequested original: lazy input reads must neither hydrate nor
    // validate it, while the unrestricted operational reader must detect it.
    sqlx::query("update node_run_details set raw_json_payloads=jsonb_build_object('debug_payload','invalid JSON') where node_run_id=$1 and section='debug_payload'")
        .bind(node.id).execute(store.pool()).await.unwrap();
    assert_eq!(
        store
            .list_application_run_trace_node_run_sections(run, vec![node.id], "input_payload")
            .await
            .unwrap()[0]
            .input_payload,
        original
    );
    assert!(store
        .list_application_run_trace_node_run_sections(run, vec![node.id], "debug_payload")
        .await
        .is_err());
    assert!(store
        .list_application_run_trace_node_run_details(run, vec![node.id])
        .await
        .is_err());
    // Usage reads stay narrow even with a corrupt unrequested context body,
    // and remain available to debug callback persistence.
    sqlx::query("update flow_runs set run_mode='debug_flow_run' where id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let usages = store.get_flow_run_node_usages(run).await.unwrap();
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].metrics_usage, stream.node_usages[0].metrics_usage);
    assert_eq!(usages[0].output_usage, stream.node_usages[0].output_usage);

    let queued: bool = sqlx::query_scalar(
        "select exists(select 1 from application_run_trace_refresh_queue where flow_run_id=$1)",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(queued);
}

#[tokio::test]
async fn node_detail_write_failure_rolls_back_metadata_and_sections() {
    let (store, run, _, _) = seed().await;
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "atomic-node".into(),
            node_type: "fixture".into(),
            node_alias: "Atomic".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: exceptional(),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let revision: i64 = sqlx::query_scalar(
        "select revision from application_run_trace_refresh_queue where flow_run_id=$1",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    sqlx::raw_sql("create function reject_node_detail_fixture() returns trigger language plpgsql as $$ begin raise exception 'fixture rejected detail'; end; $$; create trigger reject_node_detail_fixture before update on node_run_details for each row execute function reject_node_detail_fixture();")
        .execute(store.pool()).await.unwrap();
    assert!(store
        .update_node_run(&UpdateNodeRunInput {
            node_run_id: node.id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: json!({"done":true}),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await
        .is_err());
    let records = store
        .list_application_run_trace_node_run_details(run, vec![node.id])
        .await
        .unwrap();
    assert_eq!(records[0].status, domain::NodeRunStatus::Running);
    assert_eq!(records[0].input_payload, exceptional());
    assert_eq!(records[0].output_payload, json!({}));
    assert_eq!(records[0].finished_at, None);
    let unchanged_revision: i64 = sqlx::query_scalar(
        "select revision from application_run_trace_refresh_queue where flow_run_id=$1",
    )
    .bind(run)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(unchanged_revision, revision);
}

#[tokio::test]
async fn node_details_official_upgrade_preserves_historical_payloads_and_sidecars() {
    let (store, run, _, _) = seed().await;
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "historical-node".into(),
            node_type: "fixture".into(),
            node_alias: "Historical".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: exceptional(),
            debug_payload: exceptional(),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    // Reconstitute only the pre-migration physical node payload columns using
    // the current official schema fixture; apply the actual migration blob.
    sqlx::raw_sql(r#"
        create temporary table historical_node_payloads as
        select id,input_payload,output_payload,error_payload,metrics_payload,debug_payload,raw_json_payloads
        from node_run_records;
        drop view node_run_records;
        drop function write_node_run_record();
        drop table node_run_details;
        alter table node_runs
            add column input_payload jsonb not null default '{}',
            add column output_payload jsonb not null default '{}',
            add column error_payload jsonb,
            add column metrics_payload jsonb not null default '{}',
            add column debug_payload jsonb not null default '{}',
            add column raw_json_payloads jsonb not null default '{}';
        update node_runs n set input_payload=h.input_payload,output_payload=h.output_payload,
            error_payload=h.error_payload,metrics_payload=h.metrics_payload,debug_payload=h.debug_payload,
            raw_json_payloads=h.raw_json_payloads || '{"historical_extra":"retained"}'::jsonb
        from historical_node_payloads h where h.id=n.id;
    "#).execute(store.pool()).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../migrations/20260928190000_node_run_details.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();
    let loaded = store
        .list_application_run_trace_node_run_details(run, vec![node.id])
        .await
        .unwrap();
    assert_eq!(loaded[0].id, node.id);
    assert_eq!(loaded[0].input_payload, exceptional());
    assert_eq!(loaded[0].debug_payload, exceptional());
    assert_eq!(loaded[0].output_payload, json!({}));
    assert_eq!(loaded[0].error_payload, None);
    let extra: Value = sqlx::query_scalar(
        "select raw_json_payloads->'historical_extra' from node_run_records where id=$1",
    )
    .bind(node.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(extra, json!("retained"));
}
