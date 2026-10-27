use super::projection_cost::{
    counters, event, install_counters, install_task_counters, setup, task_writes,
};
use super::*;
use control_plane_contracts::application_public_runtime::ApplicationPublishedRunControlRepository;

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

async fn repair_observables(store: &PgControlPlaneStore, runs: &[domain::FlowRunRecord]) -> Value {
    let application_id = runs[0].application_id;
    let scope: Uuid = sqlx::query_scalar("select workspace_id from applications where id=$1")
        .bind(application_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let page = store
        .list_application_run_logs_page(
            application_id,
            ListApplicationRunsPageInput {
                page: 1,
                page_size: 100,
                created_after: None,
                sort_by: None,
                sort_order: None,
            },
        )
        .await
        .unwrap();
    let mut list = serde_json::to_value(page.items).unwrap();
    for item in list.as_array_mut().unwrap() {
        // A no-op source writer advances updated_at; compare every other list field.
        item["run"].as_object_mut().unwrap().remove("updated_at");
    }
    let mut members = Vec::new();
    for run in runs {
        let task_id: Uuid = sqlx::query_scalar("select coalesce(log_task_run_id,flow_run_id) from application_run_log_summaries where flow_run_id=$1")
            .bind(run.id).fetch_one(store.pool()).await.unwrap();
        let retained = store
            .get_flow_run(application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        let task = store
            .get_application_run_log_task(application_id, task_id)
            .await
            .unwrap()
            .unwrap();
        let detail = store
            .application_log_record(application_id, task_id)
            .await
            .unwrap()
            .unwrap();
        let contexts = store
            .find_published_responses_contexts(
                scope,
                application_id,
                run.api_key_id.unwrap(),
                run.created_by,
                &format!("resp_{}", run.id),
            )
            .await
            .unwrap();
        assert_eq!(contexts.len(), 1);
        let response = &contexts[0];
        members.push(json!({
            "run_id":run.id, "messages":without_revision(messages(store,run).await),
            "summary_task":task_and_summary(store,run.id).await, "task":task, "detail":detail,
            "raw_output":retained.output_payload, "raw_input":retained.input_payload,
            "raw_error":retained.error_payload, "status":retained.status,
            "responses_context":{"flow_run_id":response.flow_run_id,"status":response.status,
                "answer":response.answer,"history":response.history,
                "external_user":response.external_user,"external_conversation_id":response.external_conversation_id}
        }));
    }
    json!({"list":list,"total":page.total,"members":members})
}

#[tokio::test]
async fn full_native_repair_preserves_readers_and_every_owner_across_task_sharing() {
    let (store, _) = setup().await;
    install_counters(&store).await;
    install_task_counters(&store).await;
    // Include partial sharing on either side of the current UUID. Each run is
    // current once, exercising the changed-owner loop's first/middle/last order.
    for (topology, anchors) in [[0, 1, 2], [0, 0, 0], [0, 0, 2], [0, 1, 1]]
        .into_iter()
        .enumerate()
    {
        let seeded =
            seed_runtime_base_with_workspace_name(&store, &format!("Full repair {topology}")).await;
        let compiled = seed_compiled_plan(&store, &seeded).await;
        let key = seed_application_api_key(&store, &seeded).await;
        let mut runs = Vec::new();
        for (index, anchor) in anchors.iter().enumerate() {
            // Reusing the turn index lets the real published writer bind shared
            // tasks at creation, without manually reparenting derived rows.
            runs.push(native_member(&store, &seeded, &compiled, key, *anchor,
                &format!("full-repair-{topology}"),
                vec![json!({"type":"function_call_output","call_id":format!("result-{index}"),"output":format!("result-{index}")})],
            ).await);
        }
        assert!(runs.windows(2).all(|pair| pair[0].id < pair[1].id));
        for (index, run) in runs.iter().enumerate() {
            let answer = format!("final-{topology}-{index}");
            let item = json!({"type":"message","id":format!("owner-{index}"),"role":"assistant",
                "phase":"final_answer","content":answer,"original":"actual\u{0} and literal \\u0000"});
            append_provider_output_item(&store, run.id, item.clone()).await;
            store.update_flow_run(&UpdateFlowRunInput {
                flow_run_id:run.id, status:FlowRunStatus::Succeeded,
                output_payload:json!({"answer":answer,"responses_round":{"response_id":format!("resp_{}",run.id),"output":[item]}}),
                error_payload:None, finished_at:Some(run.started_at+Duration::seconds(5)),
            }).await.unwrap();
        }
        for anchor in anchors
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            let task = store
                .get_application_run_log_task(seeded.application_id, runs[anchor].id)
                .await
                .unwrap()
                .unwrap();
            let expected = runs
                .iter()
                .enumerate()
                .filter(|(index, _)| anchors[*index] == anchor)
                .map(|(_, run)| run.id)
                .collect::<Vec<_>>();
            assert_eq!(task.member_run_ids, expected);
            assert_eq!(task.status, FlowRunStatus::Succeeded);
            assert_eq!(task.outcome, "final_answer_observed");
        }
        let expected = repair_observables(&store, &runs).await;
        for current in &runs {
            let mut old_tuples = Vec::new();
            for run in &runs {
                old_tuples.push(tuples(&store, run.id).await);
            }
            sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=$1")
                .bind(current.id).execute(store.pool()).await.unwrap();
            sqlx::raw_sql("truncate projection_cost_counter,projection_task_cost_counter")
                .execute(store.pool())
                .await
                .unwrap();
            refresh_retained_projection(&store, current).await;
            for (index, run) in runs.iter().enumerate() {
                assert_ne!(
                    tuples(&store, run.id).await,
                    old_tuples[index],
                    "full repair must rebuild historical owners too"
                );
                assert_published_revision(&store, run).await;
            }
            for anchor in anchors
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
            {
                assert_eq!(task_writes(&store,runs[anchor].id).await,
                    anchors.iter().filter(|group| **group==anchor).count() as i64,
                    "one refresh per historical owner plus the final current-owner refresh; topology {topology}, current {}",current.id);
            }
            let writes = counters(&store).await;
            assert_eq!(repair_observables(&store,&runs).await,expected,
                "list/detail/raw Responses output/messages/tasks must agree; topology {topology}, current {}",current.id);
            assert_eq!(
                counters(&store).await,
                writes,
                "readers must not repair projections"
            );
        }
    }
}

// Central QA removes the OUTER update_flow_run task refresh in its controlled
// negative. This single-owner case must then fail: no historical owner can
// refresh the task accidentally, and full repair has skipped its inner refresh.
#[tokio::test]
async fn full_native_repair_requires_final_current_task_refresh_and_keeps_trace_dirty() {
    let (store, run) = setup().await;
    let before = store
        .get_application_run_log_task(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.status, FlowRunStatus::Running);
    assert!(before.final_output.is_none());
    let old_job = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_job.flow_run_id, run.id);
    sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=$1")
        .bind(run.id).execute(store.pool()).await.unwrap();
    install_task_counters(&store).await;
    let output = json!({"answer":"new final answer","responses_round":{"response_id":format!("resp_{}",run.id),
        "output":[{"type":"message","id":"raw-final","content":"actual\u{0} and literal \\u0000"}]}});
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Succeeded,
            output_payload: output.clone(),
            error_payload: None,
            finished_at: Some(run.started_at + Duration::seconds(5)),
        })
        .await
        .unwrap();
    let task = store
        .get_application_run_log_task(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.status, FlowRunStatus::Succeeded);
    assert_eq!(task.outcome, "final_answer_observed");
    assert_eq!(task.final_output.as_deref(), Some("new final answer"));
    assert_eq!(task.final_output_run_id, Some(run.id));
    assert_eq!(task_writes(&store, run.id).await, 1);
    assert_eq!(
        store
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .output_payload,
        output
    );
    let has_original: bool = sqlx::query_scalar(
        "select raw_json_payloads ? 'output_payload' from flow_runs where id=$1",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        has_original,
        "exercise the original Responses output sidecar"
    );
    let visible = repair_observables(&store, std::slice::from_ref(&run)).await;
    assert_eq!(
        visible["members"][0]["responses_context"]["answer"],
        "new final answer"
    );
    assert_eq!(visible["members"][0]["raw_output"], output);
    assert_published_revision(&store, &run).await;

    store
        .finish_application_run_trace_refresh(&old_job, true)
        .await
        .unwrap();
    let (revision, released): (i64,bool) = sqlx::query_as("select revision,lease_until is null from application_run_trace_refresh_queue where flow_run_id=$1")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert!(
        revision > old_job.revision,
        "a stale finish must preserve newer dirtiness"
    );
    assert!(released);
    let new_job = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(job) = store.claim_application_run_trace_refresh().await.unwrap() {
                break job;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(new_job.flow_run_id, run.id);
    assert_eq!(new_job.revision, revision);
    store
        .finish_application_run_trace_refresh(&new_job, true)
        .await
        .unwrap();
    let pending: bool = sqlx::query_scalar(
        "select exists(select 1 from application_run_trace_refresh_queue where flow_run_id=$1)",
    )
    .bind(run.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        !pending,
        "only a finish of the current revision clears the queue"
    );
}

async fn atomic_repair_snapshot(store: &PgControlPlaneStore) -> Value {
    sqlx::query_scalar(r#"select jsonb_build_object(
        'runs',(select jsonb_agg(to_jsonb(r) order by id) from flow_runs r),
        'events',(select jsonb_agg(to_jsonb(e) order by id) from runtime_events e),
        'messages',(select jsonb_agg(to_jsonb(m) order by id) from application_run_conversation_message_items m),
        'summaries',(select jsonb_agg(to_jsonb(s) order by flow_run_id) from application_run_log_summaries s),
        'tasks',(select jsonb_agg(to_jsonb(t) order by id) from application_run_log_tasks t),
        'progress',(select jsonb_agg(to_jsonb(p) order by flow_run_id) from application_run_native_projection_progress p),
        'queue',(select jsonb_agg(to_jsonb(q) order by flow_run_id) from application_run_trace_refresh_queue q))"#)
        .fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn full_native_repair_failure_rolls_back_current_and_shared_historical_owners() {
    let (store, _) = setup().await;
    let seeded = seed_runtime_base_with_workspace_name(&store, "Full repair rollback").await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let key = seed_application_api_key(&store, &seeded).await;
    let mut runs = Vec::new();
    for turn in [0, 0, 2] {
        runs.push(
            native_member(
                &store,
                &seeded,
                &compiled,
                key,
                turn,
                "full-repair-rollback",
                vec![],
            )
            .await,
        );
    }
    for (index, run) in runs.iter().enumerate() {
        store
            .append_runtime_event(&event(
                run.id,
                format!("retained-{index}"),
                format!("body-{index}"),
            ))
            .await
            .unwrap();
    }
    let current = &runs[2];
    assert!(runs[0].id < runs[1].id && runs[1].id < current.id);
    sqlx::query("update application_run_native_projection_progress set invalidated=true where flow_run_id=$1")
        .bind(current.id).execute(store.pool()).await.unwrap();
    sqlx::query("create table full_repair_failure_owner(id uuid primary key)")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("insert into full_repair_failure_owner values($1)")
        .bind(current.id)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql(r#"
        create function reject_full_repair_progress() returns trigger language plpgsql as $$
        begin
            if exists(select 1 from full_repair_failure_owner where id=new.flow_run_id) then
                raise exception 'full repair failure after historical owners';
            end if;
            return new;
        end $$;
        create trigger reject_full_repair_progress before insert or update on application_run_native_projection_progress
            for each row execute function reject_full_repair_progress();
    "#).execute(store.pool()).await.unwrap();
    install_counters(&store).await;
    install_task_counters(&store).await;
    let before = atomic_repair_snapshot(&store).await;
    let input = event(
        current.id,
        "retry-full-repair".into(),
        "committed retry".into(),
    );
    let error = store.append_runtime_event(&input).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("full repair failure after historical owners"));
    assert_eq!(
        atomic_repair_snapshot(&store).await,
        before,
        "source/message/summary/task/progress/queue writes must roll back together"
    );
    assert_eq!(
        counters(&store).await,
        json!([]),
        "instrumented writes must roll back too"
    );
    sqlx::query(
        "drop trigger reject_full_repair_progress on application_run_native_projection_progress",
    )
    .execute(store.pool())
    .await
    .unwrap();
    store.append_runtime_event(&input).await.unwrap();
    assert_eq!(
        task_writes(&store, runs[0].id).await,
        2,
        "both historical owners retain their shared task refresh"
    );
    assert_eq!(task_writes(&store, current.id).await, 1);
    assert_eq!(
        canonical(&store, current.id, "output:message:retry-full-repair")
            .await
            .unwrap()["_source_item"]["content"],
        "committed retry"
    );
    for run in &runs {
        assert_published_revision(&store, run).await;
    }
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
