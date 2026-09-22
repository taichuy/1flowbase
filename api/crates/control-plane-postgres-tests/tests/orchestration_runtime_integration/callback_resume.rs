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
    persist_callback_wait_with_tool_count(
        store,
        seeded,
        run,
        node_run,
        wait_index,
        parent_context_version_id,
        resume_claim,
        1,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn persist_callback_wait_with_tool_count(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run: &domain::FlowRunRecord,
    node_run: &domain::NodeRunRecord,
    wait_index: usize,
    parent_context_version_id: Option<Uuid>,
    resume_claim: Option<(Uuid, Uuid)>,
    tool_count: usize,
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
            tool_delivery_events: vec![AppendRuntimeEventInput {
                flow_run_id: run.id,
                node_run_id: Some(node_run.id),
                span_id: None,
                parent_span_id: None,
                event_type: "provider_output_item_done".into(),
                layer: domain::RuntimeEventLayer::ProviderRaw,
                source: domain::RuntimeEventSource::ProviderPlugin,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "output_index": wait_index - 1,
                    "item": { "type": "function_call", "call_id": format!("call-{wait_index}") }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            }],
            kind: PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
                id: Uuid::now_v7(),
                callback_kind: "llm_tool_calls".into(),
                request_payload: json!({
                    "tool_calls": (0..tool_count).map(|index| json!({
                        "id": format!("call-{wait_index}-{index}"),
                        "name": "Bash",
                        "arguments": { "command": format!("step-{wait_index}-{index}") }
                    })).collect::<Vec<_>>()
                }),
                external_ref_payload: None,
            }),
        })
        .await
        .unwrap()
        .expect("the callback wait should win its expected status transition")
}

#[tokio::test]
async fn committed_tool_delivery_is_durably_claimed_released_and_acked() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-09-17 10:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;

    let waiting = persist_callback_wait(&store, &seeded, &run, &node_run, 1, None, None).await;
    assert_eq!(waiting.tool_delivery_events.len(), 1);
    assert!(waiting.tool_delivery_events[0].sequence < waiting.waiting_event.sequence);

    let first = store
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            limit: 10,
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].event.id, waiting.tool_delivery_events[0].id);
    assert!(store
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            limit: 10,
            lease_seconds: 30,
        })
        .await
        .unwrap()
        .is_empty());

    store
        .release_runtime_event_delivery(&ReleaseRuntimeEventDeliveryInput {
            event_id: first[0].event.id,
            claim_token: first[0].claim_token,
            expected_generation: first[0].generation,
        })
        .await
        .unwrap();
    let second = store
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            limit: 10,
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(second[0].generation, first[0].generation + 1);
    store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: second[0].event.id,
            claim_token: second[0].claim_token,
            expected_generation: second[0].generation,
            acknowledged_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert!(store
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            limit: 10,
            lease_seconds: 30,
        })
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn uncertain_tool_delivery_leaves_the_replay_set_and_is_fenced_by_its_claim() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-09-17 10:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node_run, 1, None, None).await;

    let claim_input = ClaimRuntimeEventDeliveriesInput {
        flow_run_id: run.id,
        limit: 10,
        lease_seconds: 30,
    };
    let claimed = store
        .claim_runtime_event_deliveries(&claim_input)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    let claim = &claimed[0];

    // A stale claim token cannot settle the row.
    let stale = store
        .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
            event_id: claim.event.id,
            claim_token: Uuid::now_v7(),
            expected_generation: claim.generation,
            marked_at: OffsetDateTime::now_utc(),
        })
        .await;
    assert!(stale.is_err());

    store
        .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
            event_id: claim.event.id,
            claim_token: claim.claim_token,
            expected_generation: claim.generation,
            marked_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let status =
        sqlx::query_scalar::<_, String>("select delivery_status from runtime_events where id = $1")
            .bind(waiting.tool_delivery_events[0].id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(status, "uncertain");
    assert!(
        store
            .claim_runtime_event_deliveries(&claim_input)
            .await
            .unwrap()
            .is_empty(),
        "an uncertain delivery never replays automatically, even after the lease"
    );
    // Neither ACK nor release may move an uncertain row: it is settled.
    assert!(store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: claim.event.id,
            claim_token: claim.claim_token,
            expected_generation: claim.generation,
            acknowledged_at: OffsetDateTime::now_utc(),
        })
        .await
        .is_err());
    assert!(store
        .release_runtime_event_delivery(&ReleaseRuntimeEventDeliveryInput {
            event_id: claim.event.id,
            claim_token: claim.claim_token,
            expected_generation: claim.generation,
        })
        .await
        .is_err());
}

#[tokio::test]
async fn parked_tool_round_attempt_is_reacquired_by_exactly_one_delivery() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-09-17 10:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node_run, 1, None, None).await;
    let callback_task_id = waiting.callback_task.as_ref().unwrap().id;

    let recorded = store
        .record_flow_run_callback_resume_attempt(&RecordFlowRunCallbackResumeAttemptInput {
            flow_run_id: run.id,
            callback_task_id,
            source: "openai_responses".to_string(),
            response_payload: json!({"tool_results":[{"tool_call_id":"call-1","content":"a"}]}),
            idempotency_key: format!("callback_task:{callback_task_id}"),
        })
        .await
        .unwrap();
    assert!(recorded.inserted);
    let attempt_id = recorded.attempt.id;

    // Processing cannot be claimed again.
    assert!(store
        .claim_flow_run_callback_resume_attempt(attempt_id, &json!({"x":1}))
        .await
        .unwrap()
        .is_none());

    let parked = store
        .park_flow_run_callback_resume_attempt(attempt_id)
        .await
        .unwrap()
        .expect("processing attempt parks");
    assert_eq!(
        parked.status,
        domain::FlowRunCallbackResumeAttemptStatus::Received
    );

    let second_payload = json!({"tool_results":[{"tool_call_id":"call-2","content":"b"}]});
    let store = std::sync::Arc::new(store);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(4));
    let mut claims = Vec::new();
    for _ in 0..4 {
        let store = store.clone();
        let barrier = barrier.clone();
        let payload = second_payload.clone();
        claims.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .claim_flow_run_callback_resume_attempt(attempt_id, &payload)
                .await
                .unwrap()
        }));
    }
    let mut winners = 0;
    for claim in claims {
        if let Some(attempt) = claim.await.unwrap() {
            winners += 1;
            assert_eq!(
                attempt.status,
                domain::FlowRunCallbackResumeAttemptStatus::Processing
            );
            assert_eq!(attempt.response_payload, second_payload);
        }
    }
    assert_eq!(winners, 1);
}

fn tool_result(tool_call_id: &str, content: Value) -> ToolCallbackResultInput {
    ToolCallbackResultInput::from_payload(json!({
        "tool_call_id": tool_call_id,
        "content": content,
    }))
    .unwrap()
}

#[tokio::test]
async fn tool_callback_inbox_replays_same_result_conflicts_on_difference_and_waits_for_round() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-09-17 11:00:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let waiting =
        persist_callback_wait_with_tool_count(&store, &seeded, &run, &node_run, 1, None, None, 2)
            .await;
    let callback = waiting.callback_task.unwrap();
    let input = |results| CommitToolCallbackResultsInput {
        responses_continuation: None,
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: waiting.checkpoint.id,
        callback_task_id: callback.id,
        results,
    };

    let first = store
        .commit_tool_callback_results(&input(vec![tool_result("call-1-0", json!("A"))]))
        .await
        .unwrap();
    assert_eq!(
        first.disposition,
        ToolCallbackRoundDisposition::WaitingForResults
    );
    assert_eq!(first.callback_task.status, CallbackTaskStatus::Pending);

    let replay = store
        .commit_tool_callback_results(&input(vec![tool_result("call-1-0", json!("A"))]))
        .await
        .unwrap();
    assert_eq!(
        replay.disposition,
        ToolCallbackRoundDisposition::WaitingForResults
    );
    let conflict = store
        .commit_tool_callback_results(&input(vec![tool_result("call-1-0", json!("different"))]))
        .await
        .unwrap_err();
    assert!(matches!(
        conflict.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("tool_callback_result_conflict"))
    ));

    let completed = store
        .commit_tool_callback_results(&input(vec![tool_result("call-1-1", json!("B"))]))
        .await
        .unwrap();
    assert_eq!(
        completed.disposition,
        ToolCallbackRoundDisposition::Acquired
    );
    assert_eq!(
        completed.callback_task.status,
        CallbackTaskStatus::Completed
    );
    let claim = completed.claim.unwrap();
    let stale = store
        .finish_resume_claim(&FinishResumeClaimInput {
            claim_id: claim.id,
            claim_token: claim.claim_token,
            expected_generation: claim.generation + 1,
            status: ResumeClaimStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        stale.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("resume_claim_not_owned"))
    ));
}

#[tokio::test]
async fn concurrent_identical_tool_results_have_one_round_advancement_owner() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = datetime!(2026-09-17 11:30:00 UTC);
    let run = seed_flow_run(&store, &seeded, &compiled, started_at).await;
    let node_run = seed_node_run(&store, &run, started_at).await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node_run, 1, None, None).await;
    let callback = waiting.callback_task.unwrap();
    let input = CommitToolCallbackResultsInput {
        responses_continuation: None,
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: waiting.checkpoint.id,
        callback_task_id: callback.id,
        results: vec![tool_result("call-1-0", json!("same"))],
    };
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let store = store.clone();
        let input = input.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            store.commit_tool_callback_results(&input).await.unwrap()
        }));
    }
    let mut dispositions = Vec::new();
    for handle in handles {
        dispositions.push(handle.await.unwrap().disposition);
    }
    dispositions.sort_by_key(|disposition| match disposition {
        ToolCallbackRoundDisposition::Acquired => 0,
        ToolCallbackRoundDisposition::Completed => 1,
        ToolCallbackRoundDisposition::InProgress => 2,
        ToolCallbackRoundDisposition::WaitingForResults => 3,
    });
    assert_eq!(
        dispositions,
        vec![
            ToolCallbackRoundDisposition::Acquired,
            ToolCallbackRoundDisposition::Completed
        ]
    );
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

#[path = "callback_resume/semantic_round_lookup.rs"]
mod semantic_round_lookup;

#[path = "callback_resume/semantic_runtime.rs"]
mod semantic_runtime;

// Interrupt the real public consumer at the provider execution port after durable
// receipt commit, then retry through that same consumer with an expired lease.
#[tokio::test]
async fn semantic_receipt_conflict_and_crash_retry_preserve_context_and_one_claim_owner() {
    use control_plane::application_public_api::{
        api_keys::ApplicationApiKeyActor,
        callback_resume::{
            ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
            PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
        },
        compat::openai::OpenAiResponsesEnvelope,
        native_tool_resume::correlate_semantic_responses_callback,
    };
    use control_plane_contracts::application_public_runtime::{
        ApplicationPublishedCallbackAttemptRepository, ApplicationPublishedRunControlRepository,
    };
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started_at = OffsetDateTime::now_utc();
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started_at,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let node = seed_node_run(&store, &run, started_at).await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node, 1, None, None).await;
    let callback = waiting.callback_task.unwrap();
    let request = json!({"model":"fixture","previous_response_id":format!("resp_{}",run.id),"input":[
        {"role":"user","content":"before\0context"},{"type":"function_call_output","call_id":"call-1-0","output":"ok"},{"role":"user","content":"after"}]});
    let digest =
        control_plane_contracts::ports::ProviderTransportPayload::openai_responses(request.clone())
            .unwrap()
            .configuration_digest()
            .unwrap();
    sqlx::query("update flow_runs set api_key_id=$2, input_payload=$3 where id=$1")
        .bind(run.id)
        .bind(api_key_id)
        .bind(json!({"sys":{"responses_configuration_digest":digest}}))
        .execute(store.pool())
        .await
        .unwrap();
    let mut callback_request = callback.request_payload.clone();
    callback_request["responses_round"] = json!({"response_id":format!("resp_{}",run.id),"history":{"version":2,"item_count":2,"digest":"0".repeat(64)},"output":[]});
    sqlx::query("update flow_run_callback_tasks set request_payload=$2 where id=$1")
        .bind(callback.id)
        .bind(callback_request)
        .execute(store.pool())
        .await
        .unwrap();
    let actor = ApplicationApiKeyActor {
        api_key_id,
        application_id: seeded.application_id,
        creator_user_id: seeded.actor_user_id,
        tenant_id: root_tenant_id(&store).await,
        workspace_id: seeded.workspace_id,
        actor: domain::ActorContext::scoped_in_scope(
            seeded.actor_user_id,
            Uuid::nil(),
            seeded.workspace_id,
            "application_api_key",
            Vec::<String>::new(),
        ),
    };
    let envelope = OpenAiResponsesEnvelope::capture(request.clone()).unwrap();
    let grant = correlate_semantic_responses_callback(&store, &actor, &envelope)
        .await
        .unwrap()
        .unwrap();
    assert!(store
        .find_native_responses_callbacks_by_call_ids(
            seeded.workspace_id,
            seeded.application_id,
            api_key_id,
            seeded.actor_user_id,
            &["call-1-0".into()]
        )
        .await
        .unwrap()
        .is_empty());
    let mut crossed = actor.clone();
    crossed.api_key_id = Uuid::now_v7();
    assert!(
        correlate_semantic_responses_callback(&store, &crossed, &envelope)
            .await
            .unwrap()
            .is_none()
    );
    let command = ResumePublishedCallbackCommand {
        responses_continuation: Some(grant.clone()),
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: None,
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: grant.payload().clone(),
        response_mode: Some("streaming".into()),
    };
    let (runtime, provider, _package) =
        semantic_runtime::seed_runtime_consumer(&store, &seeded, &compiled, &waiting.checkpoint)
            .await;
    let service = Arc::new(ApplicationPublishedCallbackResumeService::new(
        store.clone(),
        runtime,
    ));
    let mut first = {
        let service = service.clone();
        let actor = actor.clone();
        let command = command.clone();
        tokio::spawn(async move { service.resume_callback_for_actor(actor, command).await })
    };
    tokio::select! {
        _ = provider.wait_until_entered() => {},
        result = &mut first => panic!("consumer exited before execution boundary: {result:?}"),
    }
    // Task cancellation models process loss, without completing the consumer or lease.
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert_eq!(
        provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let attempt_id = store
        .get_published_callback_resume_attempt(callback.id)
        .await
        .unwrap()
        .unwrap()
        .id;
    let input = CommitToolCallbackResultsInput {
        responses_continuation: Some(
            serde_json::from_value(grant.payload()["responses_continuation"].clone()).unwrap(),
        ),
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: waiting.checkpoint.id,
        callback_task_id: callback.id,
        results: vec![tool_result("call-1-0", json!("ok"))],
    };
    let committed = store.commit_tool_callback_results(&input).await.unwrap();
    assert_eq!(
        committed.disposition,
        ToolCallbackRoundDisposition::Completed
    );
    let first_claim = committed.claim.unwrap();
    let receipt = committed.callback_task.response_payload.unwrap();
    assert_eq!(first_claim.request_payload, receipt);
    assert_eq!(
        receipt["responses_continuation"]["ordered_input"],
        request["input"]
    );
    let mut conflicting = input.clone();
    conflicting
        .responses_continuation
        .as_mut()
        .unwrap()
        .ordered_input[0]["content"] = json!("changed");
    assert!(store
        .commit_tool_callback_results(&conflicting)
        .await
        .is_err());
    let mut reordered = input.clone();
    reordered
        .responses_continuation
        .as_mut()
        .unwrap()
        .ordered_input
        .swap(0, 2);
    assert!(store
        .commit_tool_callback_results(&reordered)
        .await
        .is_err());
    assert!(service
        .reserve_native_callback_for_actor(actor.clone(), &command)
        .await
        .is_err());
    // Process disappeared here. Expire only this isolated task's two leases.
    sqlx::query(
        "update flow_run_resume_claims set lease_expires_at=now()-interval '1 second' where id=$1",
    )
    .bind(first_claim.id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query("update flow_run_callback_resume_attempts set updated_at=now()-interval '6 minutes' where id=$1").bind(attempt_id).execute(store.pool()).await.unwrap();
    // Both contenders traverse public admission and the actual runtime consumer.
    let mut left = {
        let service = service.clone();
        let actor = actor.clone();
        let command = command.clone();
        tokio::spawn(async move { service.resume_callback_for_actor(actor, command).await })
    };
    let mut right = {
        let service = service.clone();
        let actor = actor.clone();
        let command = command.clone();
        tokio::spawn(async move { service.resume_callback_for_actor(actor, command).await })
    };
    provider.wait_until_entered().await;
    // Keep the winner inside the external port so the competing admission sees an active lease.
    let left_lost = tokio::select! {
        result = &mut left => { assert!(result.unwrap().is_err()); true },
        result = &mut right => { assert!(result.unwrap().is_err()); false },
    };
    let restored = store
        .commit_tool_callback_results(&input)
        .await
        .unwrap()
        .claim
        .unwrap();
    assert_eq!(restored.id, first_claim.id);
    assert_eq!(restored.generation, first_claim.generation + 1);
    assert_eq!(restored.request_payload, receipt);
    assert_ne!(restored.claim_token, first_claim.claim_token);
    assert!(store
        .finish_resume_claim(&FinishResumeClaimInput {
            claim_id: first_claim.id,
            claim_token: first_claim.claim_token,
            expected_generation: first_claim.generation,
            status: ResumeClaimStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .is_err());
    {
        let inputs = provider.inputs.lock().unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].messages, inputs[1].messages);
        let messages = &inputs[1].messages;
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].content, "original");
        assert_eq!(messages[2].content, "before\0context");
        assert_eq!(messages[3].tool_call_id.as_deref(), Some("call-1-0"));
        assert_eq!(messages[3].content, "ok");
        assert_eq!(messages[4].content, "after");
    }
    provider.release.add_permits(1);
    let completed = if left_lost {
        right.await.unwrap()
    } else {
        left.await.unwrap()
    }
    .unwrap();
    assert_eq!(
        completed.run.status,
        control_plane::application_public_api::native::NativeRunStatus::Succeeded
    );
    assert_eq!(
        provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let final_claim = store
        .commit_tool_callback_results(&input)
        .await
        .unwrap()
        .claim
        .unwrap();
    assert_eq!(final_claim.status, ResumeClaimStatus::Succeeded);
    let mut forged = command.clone();
    forged.responses_continuation = None;
    assert!(service
        .prepare_callback_resume_for_actor(actor, &forged)
        .await
        .is_err());
    assert_eq!(
        store
            .get_published_callback_resume_attempt(callback.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        attempt_id
    );
}
