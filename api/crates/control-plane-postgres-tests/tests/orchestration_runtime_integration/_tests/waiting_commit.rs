use super::*;
use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Semaphore;

async fn fixture() -> (
    PgControlPlaneStore,
    RuntimeSeedState,
    domain::FlowRunRecord,
    domain::NodeRunRecord,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let node = seed_node_run(&store, &run, OffsetDateTime::now_utc()).await;
    (store, seeded, run, node)
}

fn stale_failure(run: &domain::FlowRunRecord) -> CommitFlowRunTerminalInput {
    CommitFlowRunTerminalInput {
        flow_run_id: run.id,
        expected_status: run.status,
        require_initial_execution: true,
        result: CommitFlowRunTerminalResult::Failed {
            output_payload: run.output_payload.clone(),
            error_payload: json!({"message":"old execution read failed"}),
        },
        flow_run_event_payload: json!({"message":"old execution read failed"}),
        terminal_event_payload: json!({"message":"old execution read failed"}),
        finished_at: OffsetDateTime::now_utc(),
    }
}

#[tokio::test]
async fn precommit_projection_error_rolls_back_all_waiting_obligations() {
    let (store, seeded, run, node) = fixture().await;
    // Fail the real projection writer late in the transaction, after callback,
    // checkpoint, inbox, outbox and recovery rows have all been assembled.
    sqlx::raw_sql("create function reject_wait_projection() returns trigger language plpgsql as $$ begin raise exception 'injected waiting projection failure'; end $$; create trigger reject_wait_projection before insert or update on application_run_log_summaries for each row when (new.status = 'waiting_callback') execute function reject_wait_projection()")
        .execute(store.pool()).await.unwrap();
    let error = store
        .persist_waiting_state(&callback_wait_input(&seeded, &run, &node, 1, None, None, 1))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("injected waiting projection failure"),
        "{error}"
    );
    let persisted = store
        .get_flow_run(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.status, FlowRunStatus::Running);
    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as("select (select count(*) from flow_run_callback_tasks where flow_run_id=$1), (select count(*) from flow_run_checkpoints where flow_run_id=$1), (select count(*) from flow_run_tool_callback_inbox where flow_run_id=$1), (select count(*) from runtime_events where flow_run_id=$1), (select count(*) from flow_run_recovery_history where flow_run_id=$1)")
        .bind(run.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (0, 0, 0, 0, 0));
    assert!(matches!(
        store
            .commit_flow_run_terminal(&stale_failure(&run))
            .await
            .unwrap(),
        CommitFlowRunTerminalReceipt::Winner(_)
    ));
    assert_eq!(
        store
            .get_flow_run(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Failed
    );
}

#[tokio::test]
async fn receipt_survives_postcommit_pool_starvation_and_valid_output_advances_once() {
    let database = isolated_database().await;
    let url = database.database_url().to_owned();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let observer = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&observer).await;
    let compiled = seed_compiled_plan(&observer, &seeded).await;
    let run = seed_flow_run(&observer, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let node = seed_node_run(&observer, &run, OffsetDateTime::now_utc()).await;
    let armed = Arc::new(AtomicBool::new(false));
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let fault_pool = PgPoolOptions::new().max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(200))
        .after_release({
            let armed = armed.clone(); let entered = entered.clone(); let release = release.clone();
            move |connection, _| {
                let armed = armed.clone(); let entered = entered.clone(); let release = release.clone();
                Box::pin(async move {
                    if armed.load(Ordering::SeqCst) {
                        let committed: bool = sqlx::query_scalar("select exists(select 1 from flow_runs where status='waiting_callback')")
                            .fetch_one(&mut *connection).await?;
                        if committed && armed.swap(false, Ordering::SeqCst) {
                            entered.add_permits(1);
                            release.acquire().await.unwrap().forget();
                        }
                    }
                    Ok(true)
                })
            }
        }).connect(&url).await.unwrap();
    let writer = PgControlPlaneStore::new(fault_pool.clone());
    armed.store(true, Ordering::SeqCst);
    let result = writer
        .persist_waiting_state(&callback_wait_input(&seeded, &run, &node, 1, None, None, 1))
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    // Prove the fault is live, with a real pool timeout rather than a mocked call count.
    let blocked_read = writer.get_flow_run(seeded.application_id, run.id).await;
    release.add_permits(1);
    assert!(matches!(
        blocked_read.unwrap_err().downcast_ref::<sqlx::Error>(),
        Some(sqlx::Error::PoolTimedOut)
    ));
    let waiting = result
        .expect("a committed handoff does not reacquire a connection")
        .unwrap();
    let persisted = observer
        .get_flow_run(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        waiting.flow_run, persisted,
        "receipt retains every run field including account and timestamps"
    );
    assert_eq!(
        waiting.flow_run.authorized_account.as_deref(),
        Some(
            sqlx::query_scalar::<_, String>("select account from users where id=$1")
                .bind(seeded.actor_user_id)
                .fetch_one(observer.pool())
                .await
                .unwrap()
                .as_str()
        )
    );
    assert!(matches!(
        observer
            .commit_flow_run_terminal(&stale_failure(&run))
            .await
            .unwrap(),
        CommitFlowRunTerminalReceipt::Loser
    ));
    let claims = observer
        .claim_runtime_event_deliveries(&ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            limit: 1,
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(claims.len(), 1);
    let ack_error = observer
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: claims[0].event.id,
            claim_token: claims[0].claim_token,
            expected_generation: claims[0].generation + 1,
            acknowledged_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap_err();
    assert!(ack_error.downcast_ref::<ControlPlaneError>().is_some());
    let callback = waiting.callback_task.unwrap();
    let input = CommitToolCallbackResultsInput {
        responses_continuation: None,
        scope_id: seeded.workspace_id,
        application_id: seeded.application_id,
        flow_run_id: run.id,
        checkpoint_id: waiting.checkpoint.id,
        callback_task_id: callback.id,
        results: vec![tool_result("call-1-0", json!("recovered"))],
    };
    let mut invalid = input.clone();
    invalid.results = vec![tool_result("wrong-call", json!("recovered"))];
    assert!(observer
        .commit_tool_callback_results(&invalid)
        .await
        .is_err());
    invalid = input.clone();
    invalid.scope_id = Uuid::now_v7();
    assert!(observer
        .commit_tool_callback_results(&invalid)
        .await
        .is_err());
    let completed = observer.commit_tool_callback_results(&input).await.unwrap();
    assert_eq!(
        completed.disposition,
        ToolCallbackRoundDisposition::Acquired
    );
    assert_eq!(
        completed.callback_task.status,
        CallbackTaskStatus::Completed
    );
    assert_eq!(
        observer
            .commit_tool_callback_results(&input)
            .await
            .unwrap()
            .disposition,
        ToolCallbackRoundDisposition::Completed
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where callback_task_id=$1")
            .bind(callback.id)
            .fetch_one(observer.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    invalid = input;
    invalid.results = vec![tool_result("call-1-0", json!("conflict"))];
    assert!(observer
        .commit_tool_callback_results(&invalid)
        .await
        .is_err());
    fault_pool.close().await;
}

#[tokio::test]
async fn initial_execution_cannot_fail_resumed_owner_with_same_status() {
    let (store, seeded, run, node) = fixture().await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node, 1, None, None).await;
    let callback = waiting.callback_task.as_ref().unwrap();
    let acquired = store
        .commit_tool_callback_results(&CommitToolCallbackResultsInput {
            responses_continuation: None,
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: waiting.checkpoint.id,
            callback_task_id: callback.id,
            results: vec![tool_result("call-1-0", json!("resume e2"))],
        })
        .await
        .unwrap();
    assert_eq!(acquired.disposition, ToolCallbackRoundDisposition::Acquired);
    assert!(acquired.claim.is_some());
    // The real result ingress has transferred ownership to a resume claim.
    // Reactivate through the lifecycle port to exercise the status ABA explicitly.
    let mut reactivated = store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: run.id,
            status: FlowRunStatus::Running,
            output_payload: waiting.flow_run.output_payload,
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    reactivated.authorized_account =
        Some(format!("runtime-owner-{}", seeded.actor_user_id.simple()));
    assert_ne!(run.updated_at, reactivated.updated_at);
    assert!(matches!(
        store
            .commit_flow_run_terminal(&stale_failure(&run))
            .await
            .unwrap(),
        CommitFlowRunTerminalReceipt::Loser
    ));
    assert_eq!(
        store
            .get_flow_run(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap(),
        reactivated
    );
    let mut business_failure = stale_failure(&reactivated);
    business_failure.require_initial_execution = false;
    assert!(matches!(
        store
            .commit_flow_run_terminal(&business_failure)
            .await
            .unwrap(),
        CommitFlowRunTerminalReceipt::Winner(_)
    ));
}

#[tokio::test]
async fn cancellation_before_and_after_wait_commit_never_reopens_pending_callback() {
    for cancel_first in [false, true] {
        let (store, seeded, run, node) = fixture().await;
        let waiting_input = callback_wait_input(&seeded, &run, &node, 1, None, None, 1);
        let first_wait = if cancel_first {
            None
        } else {
            store.persist_waiting_state(&waiting_input).await.unwrap()
        };
        let current = store
            .get_flow_run(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        let mut cancel = stale_failure(&current);
        cancel.require_initial_execution = false;
        cancel.result = CommitFlowRunTerminalResult::Cancelled {
            output_payload: current.output_payload,
            error_payload: None,
        };
        assert!(matches!(
            store.commit_flow_run_terminal(&cancel).await.unwrap(),
            CommitFlowRunTerminalReceipt::Winner(_)
        ));
        if cancel_first {
            assert!(store
                .persist_waiting_state(&waiting_input)
                .await
                .unwrap()
                .is_none());
        } else {
            let waiting = first_wait.unwrap();
            let callback = waiting.callback_task.unwrap();
            let output = store
                .commit_tool_callback_results(&CommitToolCallbackResultsInput {
                    responses_continuation: None,
                    scope_id: seeded.workspace_id,
                    application_id: seeded.application_id,
                    flow_run_id: run.id,
                    checkpoint_id: waiting.checkpoint.id,
                    callback_task_id: callback.id,
                    results: vec![tool_result("call-1-0", json!("too late"))],
                })
                .await;
            assert!(output.is_err());
        }
        assert!(matches!(
            store
                .commit_flow_run_terminal(&stale_failure(&run))
                .await
                .unwrap(),
            CommitFlowRunTerminalReceipt::Loser
        ));
        assert_eq!(
            store
                .get_flow_run(seeded.application_id, run.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            FlowRunStatus::Cancelled
        );
    }
}

#[tokio::test]
async fn concurrent_cancel_and_wait_have_one_linearized_owner() {
    let (store, seeded, run, node) = fixture().await;
    let waiting_input = callback_wait_input(&seeded, &run, &node, 1, None, None, 1);
    let mut cancel = stale_failure(&run);
    cancel.require_initial_execution = false;
    cancel.result = CommitFlowRunTerminalResult::Cancelled {
        output_payload: json!({}),
        error_payload: None,
    };
    let barrier = Arc::new(Barrier::new(2));
    let (waiting, cancelled) = tokio::join!(
        async {
            barrier.wait().await;
            store.persist_waiting_state(&waiting_input).await.unwrap()
        },
        async {
            barrier.wait().await;
            store.commit_flow_run_terminal(&cancel).await.unwrap()
        },
    );
    let current = store
        .get_flow_run(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    match (waiting, cancelled) {
        (Some(receipt), CommitFlowRunTerminalReceipt::Loser) => {
            assert_eq!(current, receipt.flow_run);
            assert_eq!(current.status, FlowRunStatus::WaitingCallback);
        }
        (None, CommitFlowRunTerminalReceipt::Winner(mut winner)) => {
            winner.authorized_account =
                Some(format!("runtime-owner-{}", seeded.actor_user_id.simple()));
            assert_eq!(current, winner);
            assert_eq!(current.status, FlowRunStatus::Cancelled);
            let count: i64 = sqlx::query_scalar(
                "select count(*) from flow_run_callback_tasks where flow_run_id=$1",
            )
            .bind(run.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
            assert_eq!(count, 0);
        }
        other => panic!("illegal cancel/wait linearization: {other:?}"),
    }
}

#[tokio::test]
async fn independent_business_failure_does_not_reopen_a_pending_callback() {
    let (store, seeded, run, node) = fixture().await;
    let waiting = persist_callback_wait(&store, &seeded, &run, &node, 1, None, None).await;
    let mut business_failure = stale_failure(&waiting.flow_run);
    business_failure.require_initial_execution = false;
    assert!(matches!(
        store
            .commit_flow_run_terminal(&business_failure)
            .await
            .unwrap(),
        CommitFlowRunTerminalReceipt::Winner(_)
    ));
    let callback = waiting.callback_task.unwrap();
    let result = store
        .commit_tool_callback_results(&CommitToolCallbackResultsInput {
            responses_continuation: None,
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: waiting.checkpoint.id,
            callback_task_id: callback.id,
            results: vec![tool_result("call-1-0", json!("cannot revive"))],
        })
        .await
        .unwrap_err();
    assert!(matches!(
        result.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("flow_run_not_waiting_callback"))
    ));
    assert_eq!(
        store
            .get_flow_run(seeded.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Failed
    );
}

async fn consumer_fixture(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run: &domain::FlowRunRecord,
) -> (
    control_plane::orchestration_runtime::OrchestrationRuntimeService<
        PgControlPlaneStore,
        semantic_runtime::PausedProvider,
    >,
    semantic_runtime::PausedProvider,
    semantic_runtime::ProviderPackage,
) {
    let compiled = store
        .get_compiled_plan(run.compiled_plan_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    let (service, provider, package) =
        semantic_runtime::seed_runtime_consumer(store, seeded, &compiled, None).await;
    let mut plan = store
        .get_compiled_plan(compiled.id)
        .await
        .unwrap()
        .unwrap()
        .plan;
    plan["topological_order"] = json!(["node-start", "node-llm"]);
    plan["nodes"]["node-start"] = json!({
        "node_id":"node-start","node_type":"start","alias":"Start",
        "dependency_node_ids":[],"downstream_node_ids":["node-llm"],"bindings":{},
        "outputs":[{"key":"query","title":"Query","value_type":"string","selector":[]}],"config":{}
    });
    plan["nodes"]["node-llm"]["dependency_node_ids"] = json!(["node-start"]);
    plan["nodes"]["node-llm"]["bindings"]["prompt_messages"] = json!({
        "kind":"prompt_messages", "selector_paths":[],
        "raw_value":[{"id":"waiting-fixture-user", "role":"user", "content":{
            "kind":"templated_text", "value":"waiting commit fixture"
        }}]
    });
    sqlx::query("update flow_compiled_plans set plan=$2 where id=$1")
        .bind(compiled.id)
        .bind(plan)
        .execute(store.pool())
        .await
        .unwrap();
    provider.results.lock().unwrap().push_back(
        plugin_framework::provider_contract::ProviderInvocationResult {
            tool_calls: vec![plugin_framework::provider_contract::ProviderToolCall {
                id: "call-commit-recovery".into(),
                name: "Bash".into(),
                arguments: json!({"command":"fixture"}),
                provider_metadata: json!({}),
            }],
            finish_reason: Some(
                plugin_framework::provider_contract::ProviderFinishReason::ToolCall,
            ),
            ..Default::default()
        },
    );
    (service, provider, package)
}

#[tokio::test]
async fn service_precommit_error_after_successful_node_updates_parent_still_fails_running_owner() {
    let store = PgControlPlaneStore::new(isolated_database().await.connect().await.unwrap());
    run_migrations(store.pool()).await.unwrap();
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let (service, provider, _package) = consumer_fixture(&store, &seeded, &run).await;
    sqlx::raw_sql("create function reject_service_wait() returns trigger language plpgsql as $$ begin raise exception 'service waiting precommit fault'; end $$; create trigger reject_service_wait before insert or update on application_run_log_summaries for each row when (new.status='waiting_callback') execute function reject_service_wait()")
        .execute(store.pool()).await.unwrap();
    let mut execution = tokio::spawn({
        let service = service;
        let application_id = seeded.application_id;
        let workspace_id = seeded.workspace_id;
        let flow_run_id = run.id;
        async move {
            service
                .continue_flow_debug_run(
                    control_plane::orchestration_runtime::ContinueFlowDebugRunCommand {
                        application_id,
                        workspace_id,
                        flow_run_id,
                    },
                )
                .await
        }
    });
    tokio::select! {
        _ = provider.wait_until_entered() => {},
        result = &mut execution => panic!("execution ended before provider: {result:?}"),
    }
    let active = store
        .get_application_run_detail(seeded.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert!(active
        .node_runs
        .iter()
        .any(|node| node.node_id == "node-start" && node.status == NodeRunStatus::Succeeded));
    assert_ne!(
        active.flow_run.updated_at, run.updated_at,
        "a successful node really advanced the parent's timestamp"
    );
    provider.release.add_permits(1);
    let failed = execution.await.unwrap().unwrap();
    assert_eq!(failed.flow_run.status, FlowRunStatus::Failed);
    assert!(failed.flow_run.error_payload.unwrap()["message"]
        .as_str()
        .unwrap()
        .contains("service waiting precommit fault"));
    assert!(failed.callback_tasks.is_empty());
    assert!(failed.checkpoints.is_empty());
    assert_eq!(provider.executions.load(Ordering::SeqCst), 1);
}

#[path = "waiting_commit/commit_ack_proxy.rs"]
mod commit_ack_proxy;

#[tokio::test]
async fn service_lost_commit_response_preserves_wait_and_result_resumes_exactly_once() {
    use control_plane::orchestration_runtime::{
        CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand,
    };
    let database = isolated_database().await;
    let url = database.database_url().to_owned();
    let observer = PgControlPlaneStore::new(database.connect().await.unwrap());
    run_migrations(observer.pool()).await.unwrap();
    let seeded = seed_runtime_base(&observer).await;
    let compiled = seed_compiled_plan(&observer, &seeded).await;
    let run = seed_flow_run(&observer, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let (proxy, options) = commit_ack_proxy::CommitAckProxy::start(&url).await;
    let writer = PgControlPlaneStore::new(
        PgPoolOptions::new()
            .max_connections(3)
            .connect_with(options)
            .await
            .unwrap(),
    );
    let (service, provider, _package) = consumer_fixture(&writer, &seeded, &run).await;
    provider.release.add_permits(2);
    let waiting = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            workspace_id: seeded.workspace_id,
            flow_run_id: run.id,
        })
        .await
        .unwrap();
    assert_eq!(
        waiting.flow_run.status,
        FlowRunStatus::WaitingCallback,
        "unexpected execution error: {:?}",
        waiting.flow_run.error_payload
    );
    assert_eq!(proxy.dropped_commits(), 1, "server acknowledged COMMIT but its response was dropped before the real SQLx client received it");
    assert_eq!(waiting.flow_run.status, FlowRunStatus::WaitingCallback);
    assert_eq!(waiting.callback_tasks.len(), 1);
    assert_eq!(
        waiting.callback_tasks[0].status,
        CallbackTaskStatus::Pending
    );
    assert_eq!(provider.executions.load(Ordering::SeqCst), 1);
    let command = CompleteCallbackTaskCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        native_transport: None,
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        callback_task_id: waiting.callback_tasks[0].id,
        response_payload: json!({"tool_results":[{"tool_call_id":"call-commit-recovery","content":"ok"}]}),
    };
    let completed = service.complete_callback_task(command).await.unwrap();
    assert_eq!(completed.flow_run.status, FlowRunStatus::Succeeded);
    assert_eq!(
        completed.callback_tasks[0].status,
        CallbackTaskStatus::Completed
    );
    assert_eq!(provider.executions.load(Ordering::SeqCst), 2);
    let count: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where callback_task_id=$1")
            .bind(waiting.callback_tasks[0].id)
            .fetch_one(observer.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    writer.pool().close().await;
}
