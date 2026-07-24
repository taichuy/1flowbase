use super::*;

const CALLBACK_COUNT: usize = 200;
const SNAPSHOT_BYTES: i32 = 3 * 1024 * 1024;
const MAX_RSS_GROWTH_BYTES: u64 = 192 * 1024 * 1024;

#[cfg(target_os = "linux")]
fn current_rss_bytes() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let rss_kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("VmRSS should be available on Linux");
    rss_kib * 1024
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "explicit 200 callback / 3 MiB RSS regression gate"]
async fn callback_resume_context_keeps_200_large_snapshots_out_of_process_history() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-07-17 11:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let mut callback_task_ids = Vec::with_capacity(CALLBACK_COUNT);

    for index in 0..CALLBACK_COUNT {
        let node_id = format!("node-llm-{index}");
        let node_run = seed_node_run_for(
            &store,
            &run,
            &node_id,
            "llm",
            "LLM",
            json!({ "index": index }),
            started_at + Duration::seconds(index as i64 + 1),
        )
        .await;
        sqlx::query(
            r#"
            insert into flow_run_checkpoints (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                status,
                reason,
                locator_payload,
                variable_snapshot,
                external_ref_payload
            ) values (
                $1,
                $2,
                $3,
                $4,
                'waiting_callback',
                'rss regression fixture',
                jsonb_build_object('node_id', $5::text),
                jsonb_build_object('blob', repeat('x', $6)),
                null
            )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(seeded.workspace_id)
        .bind(run.id)
        .bind(node_run.id)
        .bind(&node_id)
        .bind(SNAPSHOT_BYTES)
        .execute(store.pool())
        .await
        .unwrap();
        let callback_task =
            <PgControlPlaneStore as OrchestrationRuntimeRepository>::create_callback_task(
                &store,
                &CreateCallbackTaskInput {
                    flow_run_id: run.id,
                    node_run_id: node_run.id,
                    callback_kind: "llm_tool_calls".to_string(),
                    request_payload: json!({
                        "tool_calls": [{
                            "id": format!("call-{index}"),
                            "name": "Read",
                            "arguments": { "path": format!("file-{index}") }
                        }]
                    }),
                    external_ref_payload: None,
                },
            )
            .await
            .unwrap();
        callback_task_ids.push(callback_task.id);
    }

    let baseline_rss = current_rss_bytes();
    let mut peak_rss = baseline_rss;
    for callback_task_id in callback_task_ids {
        let context =
            <PgControlPlaneStore as OrchestrationRuntimeRepository>::get_callback_resume_context(
                &store,
                run.application_id,
                callback_task_id,
            )
            .await
            .unwrap()
            .expect("callback resume context should exist");
        assert_eq!(
            context.checkpoint.variable_snapshot["blob"]
                .as_str()
                .map(str::len),
            Some(SNAPSHOT_BYTES as usize)
        );
        assert_eq!(
            context.callback_task.request_payload["tool_calls"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        drop(context);
        peak_rss = peak_rss.max(current_rss_bytes());
    }

    let rss_growth = peak_rss.saturating_sub(baseline_rss);
    eprintln!(
        "callback_resume_rss baseline_bytes={baseline_rss} peak_bytes={peak_rss} growth_bytes={rss_growth} limit_bytes={MAX_RSS_GROWTH_BYTES}"
    );
    assert!(
        rss_growth <= MAX_RSS_GROWTH_BYTES,
        "callback resume RSS grew by {rss_growth} bytes; expected at most {MAX_RSS_GROWTH_BYTES}"
    );
}
