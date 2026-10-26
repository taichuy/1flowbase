use super::projection_cost::{counters, event, install_counters, setup};
use super::*;

async fn messages(store: &PgControlPlaneStore, run: &domain::FlowRunRecord) -> Vec<Value> {
    let scope: Uuid = sqlx::query_scalar("select workspace_id from applications where id=$1")
        .bind(run.application_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    store
        .read_native_agent_log_messages(scope, run.application_id, run.id)
        .await
        .unwrap()
}

async fn tuples(store: &PgControlPlaneStore, run: Uuid) -> Vec<(Uuid, String, String)> {
    sqlx::query_as("select id,ctid::text,xmin::text from application_run_conversation_message_items where flow_run_id=$1 order by id")
        .bind(run).fetch_all(store.pool()).await.unwrap()
}

fn without_revision(mut units: Vec<Value>) -> Vec<Value> {
    for unit in &mut units {
        unit["native_message"]
            .as_object_mut()
            .unwrap()
            .remove("_log_source_revision");
    }
    units
}

async fn assert_published_revision(store: &PgControlPlaneStore, run: &domain::FlowRunRecord) {
    let revision: i64 = sqlx::query_scalar("select source_revision from application_run_native_projection_progress where flow_run_id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    for unit in messages(store, run).await {
        assert_eq!(
            unit["native_message"]["_log_source_revision"],
            format!("v7:{revision}")
        );
    }
}

async fn task_and_summary(store: &PgControlPlaneStore, run: Uuid) -> Value {
    // The oracle runs another source-writing transaction. Its update timestamps
    // and revision must advance; compare all other summary data exactly.
    sqlx::query_scalar(r#"select jsonb_build_object(
        'summary',to_jsonb(s)-array['updated_at','log_updated_at'],
        'task',jsonb_build_object('status',t.status,'outcome',t.outcome,'user_input',t.user_input,
            'final_output',t.final_output,'final_output_run_id',t.final_output_run_id,
            'member_run_ids',t.member_run_ids,'invocation_count',t.invocation_count,
            'total_tokens',t.total_tokens,'total_cost',t.total_cost))
        from application_run_log_summaries s join application_run_log_tasks t on t.id=coalesce(s.log_task_run_id,s.flow_run_id)
        where s.flow_run_id=$1"#).bind(run).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn tail_append_keeps_old_tuples_lossless_bodies_and_full_repair_equivalence() {
    let (store, run) = setup().await;
    let text = format!("{} actual\0 and literal \\u0000", "body".repeat(8192));
    let inputs = (0..12)
        .map(|index| event(run.id, format!("old-{index}"), text.clone()))
        .collect::<Vec<_>>();
    store.append_runtime_events(&inputs).await.unwrap();
    let before = tuples(&store, run.id).await;
    install_counters(&store).await;
    store
        .append_runtime_event(&event(run.id, "new".into(), text.clone()))
        .await
        .unwrap();
    let after = tuples(&store, run.id).await;
    for old in &before {
        assert!(
            after.contains(old),
            "an ordinary append must not rewrite an existing tuple"
        );
    }
    let writes = counters(&store).await;
    let projection = writes
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["relation"] == "application_run_conversation_message_items")
        .collect::<Vec<_>>();
    assert_eq!(
        projection
            .iter()
            .filter(|row| row["operation"] == "INSERT")
            .map(|row| row["tuples"].as_i64().unwrap())
            .sum::<i64>(),
        1
    );
    assert_eq!(
        projection
            .iter()
            .filter(|row| row["operation"] != "INSERT")
            .map(|row| row["tuples"].as_i64().unwrap())
            .sum::<i64>(),
        0
    );
    assert_published_revision(&store, &run).await;
    let incremental = without_revision(messages(&store, &run).await);
    assert_eq!(
        incremental.last().unwrap()["native_message"]["_source_item"]["content"],
        text
    );
    let summary = task_and_summary(&store, run.id).await;
    sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    refresh_retained_projection(&store, &run).await;
    assert_eq!(
        without_revision(messages(&store, &run).await),
        incremental,
        "the retained production full fold is the oracle"
    );
    assert_eq!(task_and_summary(&store, run.id).await, summary);
    assert_published_revision(&store, &run).await;
}

#[tokio::test]
async fn equal_retransmission_and_metadata_only_refresh_do_not_rewrite_messages() {
    let (store, run) = setup().await;
    let input = event(run.id, "same".into(), "exact original\0\\u0000".into());
    store.append_runtime_event(&input).await.unwrap();
    let before = tuples(&store, run.id).await;
    store.append_runtime_event(&input).await.unwrap();
    assert_eq!(
        tuples(&store, run.id).await,
        before,
        "equal retransmission only advances progress"
    );
    refresh_retained_projection(&store, &run).await;
    assert_eq!(
        tuples(&store, run.id).await,
        before,
        "updated_at is not another native body"
    );
    assert_published_revision(&store, &run).await;
    let finish = UpdateFlowRunInput {
        flow_run_id: run.id,
        status: FlowRunStatus::Succeeded,
        output_payload: json!({}),
        error_payload: None,
        finished_at: Some(datetime!(2026-10-26 00:00:00 UTC)),
    };
    store.update_flow_run(&finish).await.unwrap();
    let before = tuples(&store, run.id).await;
    let progress: String = sqlx::query_scalar(
        "select ctid::text from application_run_native_projection_progress where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    store.update_flow_run(&finish).await.unwrap();
    assert_eq!(tuples(&store, run.id).await, before);
    let after: String = sqlx::query_scalar(
        "select ctid::text from application_run_native_projection_progress where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(after, progress);
}

#[tokio::test]
async fn append_slices_tool_results_and_never_restores_unrelated_context_or_bodies() {
    let (store, run) = setup().await;
    let results=(0..64).map(|i|json!({"type":"function_call_output","call_id":format!("result-{i}"),"output":"r".repeat(4096)})).collect::<Vec<_>>();
    correct_context_field(&store, run.id, "tool_results", json!(results)).await;
    refresh_retained_projection(&store, &run).await;
    let before = tuples(&store, run.id).await;
    sqlx::raw_sql(r#"create or replace function runtime_original_json(projection jsonb, originals jsonb, field_name text)
        returns json language plpgsql immutable parallel safe as $$ begin
            if field_name='log_context' or (field_name='native_message' and projection#>>'{_source_item,call_id}' like 'result-%')
                then raise exception 'fixture unrelated original restored'; end if;
            return case when originals ? field_name then (originals->>field_name)::json else projection::json end;
        end $$"#).execute(store.pool()).await.unwrap();
    let mut appended = results;
    appended.push(json!({"type":"function_call_output","call_id":"new-result","output":"new"}));
    correct_context_field(&store, run.id, "tool_results", json!(appended)).await;
    refresh_retained_projection(&store, &run).await;
    let after = tuples(&store, run.id).await;
    assert!(before.iter().all(|tuple| after.contains(tuple)));
    assert_eq!(after.len(), before.len() + 1);
    store
        .append_runtime_event(&event(run.id, "new-output".into(), "new output".into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn replayed_same_rank_different_body_preserves_delta_before_old_left_bias() {
    let (store, run) = setup().await;
    store
        .append_runtime_event(&event(run.id, "tie".into(), "retained source".into()))
        .await
        .unwrap();
    sqlx::query("update application_run_conversation_message_items set native_message=jsonb_set(native_message,'{_source_item,content}',to_jsonb('older materialized body'::text)) where flow_run_id=$1 and source_item_key='output:message:tie'")
        .bind(run.id).execute(store.pool()).await.unwrap();
    sqlx::query("update application_run_native_projection_progress set output_sequence='-9223372036854775808'::bigint,source_revision=-1 where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    refresh_retained_projection(&store, &run).await;
    let body = canonical(&store, run.id, "output:message:tie")
        .await
        .unwrap();
    assert_eq!(body["_source_item"]["content"], "retained source");
    assert_eq!(body["_log_conflicting"], true);
}

#[tokio::test]
async fn canceled_progress_publication_rolls_back_sources_messages_and_progress() {
    let (store, run) = setup().await;
    store
        .append_runtime_event(&event(run.id, "before".into(), "before".into()))
        .await
        .unwrap();
    let before = messages(&store, &run).await;
    let progress: Value = sqlx::query_scalar(
        "select to_jsonb(p) from application_run_native_projection_progress p where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    sqlx::raw_sql("create function delay_native_progress() returns trigger language plpgsql as $$ begin perform pg_sleep(10); return new; end $$; create trigger delay_native_progress before update on application_run_native_projection_progress for each row execute function delay_native_progress()")
        .execute(store.pool()).await.unwrap();
    let task_store = store.clone();
    let operation = tokio::spawn(async move {
        task_store
            .append_runtime_event(&event(run.id, "canceled".into(), "must roll back".into()))
            .await
    });
    let pid=tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let pid: Option<i32>=sqlx::query_scalar("select pid from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event='PgSleep' and query like 'insert into application_run_native_projection_progress%' limit 1")
                .fetch_optional(store.pool()).await.unwrap();
            if let Some(pid)=pid {break pid;}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("own test transaction must reach delayed progress publication");
    let canceled: bool = sqlx::query_scalar("select pg_cancel_backend($1)")
        .bind(pid)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert!(canceled);
    assert!(operation.await.unwrap().is_err());
    assert_eq!(messages(&store, &run).await, before);
    let after: Value = sqlx::query_scalar(
        "select to_jsonb(p) from application_run_native_projection_progress p where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(after, progress);
    let count: i64=sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done'")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 1);
}

async fn observable_snapshot(store: &PgControlPlaneStore, run: &domain::FlowRunRecord) -> Value {
    let ranks: Value = sqlx::query_scalar("select jsonb_agg(jsonb_build_array(flow_run_id,source_item_key,source_occurrence_sequence,display_sequence) order by id) from application_run_conversation_message_items where flow_run_id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    let revision: i64 = sqlx::query_scalar("select source_revision from application_run_native_projection_progress where flow_run_id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    json!({"messages":messages(store,run).await,"ranks":ranks,"summary_task":task_and_summary(store,run.id).await,"published_revision":revision})
}

// This identical test is run against both production binaries. Its fixed
// counterexamples are projection faults that the pre-change contract detects;
// it does not count passing tests as defects or infer a retention percentage.
#[tokio::test]
async fn native_projection_detection_oracle_rejects_fixed_controlled_defects() {
    let (store, run) = setup().await;
    for (key, text) in [
        ("first", "source"),
        ("first", "conflicting observation"),
        ("second", "second source"),
    ] {
        store
            .append_runtime_event(&event(run.id, key.into(), text.into()))
            .await
            .unwrap();
    }
    sqlx::raw_sql(r#"
        create table cost_defect_messages as select id,native_message,raw_json_payloads,source_occurrence_sequence,display_sequence from application_run_conversation_message_items;
        create table cost_defect_summary as select flow_run_id,status from application_run_log_summaries;
        create table cost_defect_task as select id,user_input from application_run_log_tasks;
        create table cost_defect_progress as select flow_run_id,source_revision from application_run_native_projection_progress;
    "#).execute(store.pool()).await.unwrap();
    let expected = observable_snapshot(&store, &run).await;
    let trials=[
        ("body_corruption","update application_run_conversation_message_items set native_message=jsonb_set(native_message,'{_source_item,content}',to_jsonb('corrupt'::text)) where flow_run_id=$1 and source_item_key='output:message:first'"),
        ("sticky_conflict_erased","update application_run_conversation_message_items set native_message=jsonb_set(native_message,'{_log_conflicting}','false'::jsonb) where flow_run_id=$1 and source_item_key='output:message:first'"),
        ("canonical_rank_corruption","update application_run_conversation_message_items set source_occurrence_sequence=source_occurrence_sequence+1000 where flow_run_id=$1 and source_item_key='output:message:first'"),
        ("display_order_corruption","update application_run_conversation_message_items set display_sequence=10000 where flow_run_id=$1 and source_item_key='output:message:first'"),
        ("summary_status_corruption","update application_run_log_summaries set status='failed' where flow_run_id=$1"),
        ("task_input_corruption","update application_run_log_tasks set user_input='corrupt' where id=$1"),
        ("progress_revision_corruption","update application_run_native_projection_progress set source_revision=source_revision+1 where flow_run_id=$1"),
    ];
    let mut detected = Vec::new();
    for (name, statement) in trials {
        // SQL safety: every statement is a fixed literal above; the run is bound.
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .bind(run.id)
            .execute(store.pool())
            .await
            .unwrap();
        assert_ne!(
            observable_snapshot(&store, &run).await,
            expected,
            "oracle must reject {name}"
        );
        detected.push(name);
        sqlx::raw_sql(r#"
            update application_run_conversation_message_items m set native_message=b.native_message,raw_json_payloads=b.raw_json_payloads,
                source_occurrence_sequence=b.source_occurrence_sequence,display_sequence=b.display_sequence from cost_defect_messages b where m.id=b.id;
            update application_run_log_summaries s set status=b.status from cost_defect_summary b where s.flow_run_id=b.flow_run_id;
            update application_run_log_tasks t set user_input=b.user_input from cost_defect_task b where t.id=b.id;
            update application_run_native_projection_progress p set source_revision=b.source_revision from cost_defect_progress b where p.flow_run_id=b.flow_run_id;
        "#).execute(store.pool()).await.unwrap();
        assert_eq!(
            observable_snapshot(&store, &run).await,
            expected,
            "restore must isolate the next counterexample"
        );
    }
    if let Ok(output) = std::env::var("NATIVE_PROJECTION_DEFECT_OUT") {
        std::fs::write(output,serde_json::to_vec_pretty(&json!({"fixed_defect_count":trials.len(),"detected":detected,"scope":"controlled faults of actual retained projection, summary, task and progress; not exhaustive mutation testing"})).unwrap()).unwrap();
    }
}
