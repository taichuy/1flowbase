use super::*;

#[tokio::test]
async fn ledger_totals_preserve_all_calls_when_one_node_snapshot_is_replaced() {
    let (store, run, app, scope) = seed().await;
    assert_eq!(store.get_flow_run_usage_totals(run).await.unwrap(), None);
    let node = store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run,
            node_id: "reused-llm".into(),
            node_type: "llm".into(),
            node_alias: "LLM".into(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    for (input, output, cached) in [(43757_i64, 104_i64, 43264_i64), (46884, 152, 43648)] {
        sqlx::query("insert into runtime_usage_ledger(id,scope_id,flow_run_id,node_run_id,input_tokens,output_tokens,cached_input_tokens,usage_status) values($1,$2,$3,$4,$5,$6,$7,'recorded')")
            .bind(Uuid::now_v7()).bind(scope).bind(run).bind(node.id)
            .bind(input).bind(output).bind(cached).execute(store.pool()).await.unwrap();
        let latest = json!({"input_tokens":input,"output_tokens":output});
        store
            .update_node_run_payloads(&UpdateNodeRunPayloadsInput {
                node_run_id: node.id,
                input_payload: json!({}),
                output_payload: json!({"usage":latest}),
                error_payload: None,
                metrics_payload: json!({"usage":latest}),
                debug_payload: json!({}),
            })
            .await
            .unwrap();
    }
    let totals = store.get_flow_run_usage_totals(run).await.unwrap().unwrap();
    assert_eq!(totals["prompt_tokens"], 90641);
    assert_eq!(totals["completion_tokens"], 256);
    assert_eq!(totals["total_tokens"], 90897);
    assert_eq!(totals["cache_read_tokens"], 86912);
    assert!(totals["reasoning_tokens"].is_null());
    let nodes = store.get_flow_run_node_usages(run).await.unwrap();
    assert_eq!(
        nodes[0].metrics_usage.as_ref().unwrap()["input_tokens"],
        46884
    );
    let stream = store
        .get_published_run_stream_state(app, run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stream.usage_totals, Some(totals));
    assert_eq!(
        store
            .get_flow_run_usage_totals(Uuid::now_v7())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn unmeasured_ledger_entries_remain_unknown() {
    let (store, run, _, scope) = seed().await;
    sqlx::query("insert into runtime_usage_ledger(id,scope_id,flow_run_id,usage_status) values($1,$2,$3,'unavailable_error')")
        .bind(Uuid::now_v7()).bind(scope).bind(run).execute(store.pool()).await.unwrap();
    assert_eq!(store.get_flow_run_usage_totals(run).await.unwrap(), None);
}
