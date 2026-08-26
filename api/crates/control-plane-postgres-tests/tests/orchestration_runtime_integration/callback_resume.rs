use super::*;

const CALLBACK_COUNT: usize = 200;
const SNAPSHOT_BYTES: i32 = 3 * 1024 * 1024;
const MAX_RSS_GROWTH_BYTES: u64 = 192 * 1024 * 1024;

async fn persist_callback_wait(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run: &domain::FlowRunRecord,
    node_run: &domain::NodeRunRecord,
    wait_index: usize,
    parent_context_version_id: Option<Uuid>,
    resume_claim: Option<(Uuid, Uuid)>,
) -> control_plane_contracts::ports::PersistedWaitingState {
    let (resume_claim_id, resume_claim_token) = resume_claim.unzip();
    store
        .persist_waiting_state(&PersistWaitingStateInput {
            checkpoint_id: Uuid::now_v7(),
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            node_run_id: node_run.id,
            expected_status: if wait_index == 1 {
                FlowRunStatus::Running
            } else {
                FlowRunStatus::WaitingCallback
            },
            output_payload: json!({ "wait_index": wait_index }),
            checkpoint_status: "waiting_callback".into(),
            checkpoint_reason: "issue_1736_consecutive_callback".into(),
            locator_payload: json!({
                "node_id": "node-llm",
                "next_node_index": 1,
                "active_node_ids": ["node-llm"]
            }),
            variable_snapshot: json!({ "wait_index": wait_index }),
            checkpoint_external_ref_payload: None,
            context_content: json!({
                "format": "runtime_snapshot_v1",
                "variable_pool": { "wait_index": wait_index }
            }),
            parent_context_version_id,
            context_transition_kind: if wait_index == 1 {
                ContextTransitionKind::Append
            } else {
                ContextTransitionKind::Callback
            },
            recovery_idempotency_key: format!("issue-1736-wait-{wait_index}"),
            resume_claim_id,
            resume_claim_token,
            waiting_event: AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: Some(node_run.id),
                span_id: None,
                parent_span_id: None,
                event_type: "flow.waiting_callback".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "wait_index": wait_index }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            kind: PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
                id: Uuid::now_v7(),
                callback_kind: "llm_tool_calls".into(),
                request_payload: json!({
                    "tool_calls": [{
                        "id": format!("call-{wait_index}"),
                        "name": "Bash",
                        "arguments": { "command": format!("step-{wait_index}") }
                    }]
                }),
                external_ref_payload: None,
            }),
        })
        .await
        .unwrap()
        .expect("the callback wait should win its expected status transition")
}

#[tokio::test]
async fn issue_1736_ac_004_resume_claim_records_running_between_consecutive_callback_waits() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-08-16 08:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;

    let first_wait = persist_callback_wait(&store, &seeded, &run, &node_run, 1, None, None).await;
    let first_callback = first_wait.callback_task.expect("first callback task");
    let first_claim_input = AcquireResumeClaimInput {
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: first_wait.checkpoint.id,
        callback_task_id: Some(first_callback.id),
        kind: ResumeClaimKind::Callback,
        request_payload: json!({ "tool_results": [{ "id": "call-1", "output": "ok" }] }),
    };
    let first_claim = store
        .acquire_resume_claim(&first_claim_input)
        .await
        .unwrap();
    assert_eq!(first_claim.disposition, ResumeClaimDisposition::Acquired);
    sqlx::query(
        "update flow_run_resume_claims set lease_expires_at = now() - interval '1 second' where id = $1",
    )
    .bind(first_claim.claim.id)
    .execute(store.pool())
    .await
    .unwrap();
    let reacquired_claim = store
        .acquire_resume_claim(&first_claim_input)
        .await
        .expect("an expired claim already in running recovery remains recoverable");
    assert_eq!(
        reacquired_claim.disposition,
        ResumeClaimDisposition::Acquired
    );
    assert_eq!(
        reacquired_claim.claim.generation,
        first_claim.claim.generation + 1
    );

    let second_wait = persist_callback_wait(
        &store,
        &seeded,
        &run,
        &node_run,
        2,
        Some(first_wait.recovery_history.context_version_id),
        Some((
            reacquired_claim.claim.id,
            reacquired_claim.claim.claim_token,
        )),
    )
    .await;
    let second_callback = second_wait.callback_task.expect("second callback task");
    let second_claim = store
        .acquire_resume_claim(&AcquireResumeClaimInput {
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: second_wait.checkpoint.id,
            callback_task_id: Some(second_callback.id),
            kind: ResumeClaimKind::Callback,
            request_payload: json!({ "tool_results": [{ "id": "call-2", "output": "ok" }] }),
        })
        .await
        .unwrap();
    assert_eq!(second_claim.disposition, ResumeClaimDisposition::Acquired);

    let recorded_states = sqlx::query_scalar::<_, String>(
        "select state_code from flow_run_recovery_history where flow_run_id = $1 order by sequence",
    )
    .bind(run.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        recorded_states,
        vec!["waiting_callback", "running", "waiting_callback", "running"]
    );
}

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
