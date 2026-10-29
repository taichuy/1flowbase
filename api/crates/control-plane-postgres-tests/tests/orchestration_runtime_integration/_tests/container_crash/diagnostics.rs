use super::*;

pub(super) async fn capture(
    store: &PgControlPlaneStore,
    run: &domain::FlowRunRecord,
    phase: &str,
    logs: &std::path::Path,
    filename: &str,
    ledger: &harness::ReceiptLedger,
    process: Value,
) {
    let flow = match store.get_flow_run(run.application_id, run.id).await {
        Ok(Some(flow)) => {
            json!({"status":flow.status.as_str(),"error_payload":flow.error_payload,"output_payload":flow.output_payload})
        }
        other => json!({"read_error":format!("{other:?}")}),
    };
    let plan = sqlx::query_scalar::<_, Value>("select plan from flow_compiled_plans where id=$1")
        .bind(run.compiled_plan_id)
        .fetch_optional(store.pool())
        .await;
    let compile_issues = match plan {
        Ok(Some(plan)) => plan["compile_issues"].clone(),
        other => json!({"read_error":format!("{other:?}")}),
    };
    let checkpoint = match store.load_container_checkpoint(run.id).await {
        Ok(value) => json!(value),
        Err(error) => json!({"read_error":format!("{error:#}")}),
    };
    let nodes = sqlx::query_scalar::<_,Value>("select coalesce(jsonb_agg(jsonb_build_object('id',id,'node_id',node_id,'status',status,'error_payload',error_payload) order by started_at,id),'[]'::jsonb) from node_runs where flow_run_id=$1")
        .bind(run.id).fetch_one(store.pool()).await;
    let evidence = json!({"phase":phase,"run_id":run.id,"process":process,"flow":flow,
        "compile_issues":compile_issues,"checkpoint":checkpoint,
        "node_records":nodes.unwrap_or_else(|error|json!({"read_error":error.to_string()})),
        "external_receipts":ledger.receipts.lock().unwrap().clone()});
    std::fs::write(
        logs.join(filename),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
