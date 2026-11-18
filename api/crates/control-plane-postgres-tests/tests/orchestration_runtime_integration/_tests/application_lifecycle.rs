use super::*;
use control_plane_contracts::ports::{
    ApplicationLifecycleFact, ApplicationLifecycleObserver, ApplicationLifecyclePhase,
};
use std::sync::Mutex;

#[derive(Default)]
struct Facts(Mutex<Vec<ApplicationLifecycleFact>>);
impl ApplicationLifecycleObserver for Facts {
    fn observe(&self, fact: ApplicationLifecycleFact) {
        self.0.lock().unwrap().push(fact);
    }
}
impl Facts {
    fn for_run(&self, id: Uuid) -> Vec<ApplicationLifecycleFact> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|fact| fact.flow_run_id == id)
            .cloned()
            .collect()
    }
}
async fn fixture() -> (
    PgControlPlaneStore,
    RuntimeSeedState,
    domain::CompiledPlanRecord,
    Arc<Facts>,
    Vec<Uuid>,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let facts = Arc::new(Facts::default());
    let ancestors = vec![Uuid::now_v7()];
    let store = PgControlPlaneStore::new(pool)
        .with_application_lifecycle_observer(facts.clone())
        .with_workflow_ancestor_ids(ancestors.clone());
    let seed = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seed).await;
    (store, seed, compiled, facts, ancestors)
}
fn terminal(run_id: Uuid, result: CommitFlowRunTerminalResult) -> CommitFlowRunTerminalInput {
    CommitFlowRunTerminalInput {
        flow_run_id: run_id,
        expected_status: FlowRunStatus::Running,
        require_initial_execution: false,
        tool_delivery_events: vec![],
        result,
        flow_run_event_payload: json!({"fixture":"application lifecycle"}),
        terminal_event_payload: json!({"fixture":"application lifecycle"}),
        finished_at: OffsetDateTime::now_utc(),
    }
}
fn create_input(
    seed: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
) -> CreateFlowRunInput {
    CreateFlowRunInput {
        application_run_log_context: None,
        actor_user_id: seed.actor_user_id,
        application_id: seed.application_id,
        flow_id: seed.flow_id,
        flow_draft_id: seed.draft_id,
        compiled_plan_id: compiled.id,
        debug_session_id: "lifecycle-fixture".into(),
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: "test-document-hash".into(),
        run_mode: FlowRunMode::WorkflowEventRun,
        target_node_id: None,
        title: "lifecycle fixture".into(),
        status: FlowRunStatus::Running,
        input_payload: json!({"node-start":{"query":"hello"}}),
        started_at: OffsetDateTime::now_utc(),
        api_key_id: None,
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: Some("lifecycle-idempotency".into()),
    }
}

#[tokio::test]
async fn application_lifecycle_initial_running_and_queued_attachment_start_once() {
    let (store, seed, compiled, facts, ancestors) = fixture().await;
    let run = seed_flow_run_with_mode(
        &store,
        &seed,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let observed = facts.for_run(run.id);
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].phase, ApplicationLifecyclePhase::Started);
    assert_eq!(observed[0].workspace_id, seed.workspace_id);
    assert_eq!(observed[0].application_id, seed.application_id);
    assert_eq!(observed[0].occurred_at, run.started_at);
    assert_eq!(observed[0].workflow_ancestor_ids, ancestors);

    let shell = store
        .create_flow_run_shell(&CreateFlowRunShellInput {
            actor_user_id: seed.actor_user_id,
            application_id: seed.application_id,
            flow_id: seed.flow_id,
            flow_draft_id: seed.draft_id,
            debug_session_id: "lifecycle-shell".into(),
            flow_schema_version: compiled.schema_version.clone(),
            document_hash: "test-document-hash".into(),
            run_mode: FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "queued".into(),
            status: FlowRunStatus::Queued,
            input_payload: json!({}),
            started_at: OffsetDateTime::now_utc(),
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        })
        .await
        .unwrap();
    assert!(facts.for_run(shell.id).is_empty());
    let attachment = AttachCompiledPlanToFlowRunInput {
        flow_run_id: shell.id,
        compiled_plan_id: compiled.id,
        flow_schema_version: compiled.schema_version.clone(),
        document_hash: "test-document-hash".into(),
        status: FlowRunStatus::Running,
    };
    store
        .attach_compiled_plan_to_flow_run(&attachment)
        .await
        .unwrap();
    assert!(store
        .attach_compiled_plan_to_flow_run(&attachment)
        .await
        .is_err());
    assert_eq!(facts.for_run(shell.id).len(), 1);

    let input = create_input(&seed, &compiled);
    let first = control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input).await.unwrap();
    let replay = control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input).await.unwrap();
    assert!(first.created && !replay.created);
    assert_eq!(first.flow_run.id, replay.flow_run.id);
    assert_eq!(facts.for_run(first.flow_run.id).len(), 1);

    let mut queued = input;
    queued.status = FlowRunStatus::Queued;
    queued.idempotency_key = None;
    let queued = store.create_flow_run(&queued).await.unwrap();
    assert!(facts.for_run(queued.id).is_empty());
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: queued.id,
            status: FlowRunStatus::Running,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    assert_eq!(facts.for_run(queued.id).len(), 1);
}

#[tokio::test]
async fn application_lifecycle_wait_resume_and_terminal_cas_have_one_start_and_one_end() {
    let (store, seed, compiled, facts, _) = fixture().await;
    let run = seed_flow_run_with_mode(
        &store,
        &seed,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    let mut previous = FlowRunStatus::Running;
    for status in [
        FlowRunStatus::WaitingCallback,
        FlowRunStatus::Running,
        FlowRunStatus::WaitingHuman,
        FlowRunStatus::Running,
        FlowRunStatus::Paused,
        FlowRunStatus::Running,
    ] {
        let updated = store
            .update_flow_run_if_status(
                &UpdateFlowRunInput {
                    flow_run_id: run.id,
                    status,
                    output_payload: json!({}),
                    error_payload: None,
                    finished_at: None,
                },
                previous,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, status);
        previous = status;
    }
    assert_eq!(facts.for_run(run.id).len(), 1);
    let success = terminal(
        run.id,
        CommitFlowRunTerminalResult::Succeeded {
            output_payload: json!({"answer":"done"}),
        },
    );
    let failure = terminal(
        run.id,
        CommitFlowRunTerminalResult::Failed {
            output_payload: json!({}),
            error_payload: json!({"code":"failed"}),
        },
    );
    let (one, two) = tokio::join!(
        store.commit_flow_run_terminal(&success),
        store.commit_flow_run_terminal(&failure)
    );
    let receipts = [one.unwrap(), two.unwrap()];
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| matches!(receipt, CommitFlowRunTerminalReceipt::Loser))
            .count(),
        1
    );
    let observed = facts.for_run(run.id);
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].phase, ApplicationLifecyclePhase::Started);
    assert_eq!(observed[1].phase, ApplicationLifecyclePhase::Ended);
    let stored = store
        .get_flow_run(seed.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed[1].status, stored.status);
    assert_eq!(observed[1].occurred_at, stored.finished_at.unwrap());
    assert!(matches!(
        store.commit_flow_run_terminal(&success).await.unwrap(),
        CommitFlowRunTerminalReceipt::Loser
    ));
    assert_eq!(facts.for_run(run.id).len(), 2);
    // Legacy completion rewrites cannot announce the same terminal a second time.
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: stored.id,
            status: stored.status,
            output_payload: stored.output_payload,
            error_payload: stored.error_payload,
            finished_at: stored.finished_at,
        })
        .await
        .unwrap();
    assert_eq!(facts.for_run(run.id).len(), 2);
}

#[tokio::test]
async fn application_lifecycle_terminal_fact_survives_postcommit_warning_for_all_results() {
    let (store, seed, compiled, facts, _) = fixture().await;
    // This table belongs to the post-commit conversation writer, not the atomic terminal projections.
    sqlx::raw_sql("create function reject_lifecycle_conversation() returns trigger language plpgsql as $$ begin raise exception 'lifecycle postcommit fixture'; end $$; create trigger reject_lifecycle_conversation before insert or update on application_conversation_messages for each row execute function reject_lifecycle_conversation()")
        .execute(store.pool()).await.unwrap();
    for result in [
        CommitFlowRunTerminalResult::Succeeded {
            output_payload: json!({"answer":"done"}),
        },
        CommitFlowRunTerminalResult::Incomplete {
            output_payload: json!({"answer":"partial"}),
        },
        CommitFlowRunTerminalResult::Failed {
            output_payload: json!({"answer":"partial"}),
            error_payload: json!({"code":"fixture"}),
        },
        CommitFlowRunTerminalResult::Cancelled {
            output_payload: json!({"answer":"partial"}),
            error_payload: None,
        },
    ] {
        let status = result.status();
        let run = seed_flow_run_with_mode(
            &store,
            &seed,
            &compiled,
            OffsetDateTime::now_utc(),
            FlowRunMode::DebugFlowRun,
            None,
        )
        .await;
        let receipt = store
            .commit_flow_run_terminal(&terminal(run.id, result))
            .await
            .unwrap();
        assert!(matches!(
            receipt,
            CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(_)
        ));
        let observed = facts.for_run(run.id);
        assert_eq!(observed.len(), 2);
        assert_eq!(observed[1].phase, ApplicationLifecyclePhase::Ended);
        assert_eq!(observed[1].status, status);
        let stored = store
            .get_flow_run(seed.application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, status);
        assert_eq!(observed[1].application_id, seed.application_id);
    }
}

#[tokio::test]
async fn application_lifecycle_rolled_back_terminal_has_no_ended_fact() {
    let (store, seed, compiled, facts, _) = fixture().await;
    let run = seed_flow_run_with_mode(
        &store,
        &seed,
        &compiled,
        OffsetDateTime::now_utc(),
        FlowRunMode::DebugFlowRun,
        None,
    )
    .await;
    sqlx::raw_sql("create function reject_lifecycle_terminal() returns trigger language plpgsql as $$ begin raise exception 'lifecycle rollback fixture'; end $$; create trigger reject_lifecycle_terminal before insert on runtime_events for each row execute function reject_lifecycle_terminal()")
        .execute(store.pool()).await.unwrap();
    assert!(store
        .commit_flow_run_terminal(&terminal(
            run.id,
            CommitFlowRunTerminalResult::Succeeded {
                output_payload: json!({})
            }
        ))
        .await
        .is_err());
    assert_eq!(facts.for_run(run.id).len(), 1);
    assert_eq!(
        store
            .get_flow_run(seed.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Running
    );
}

#[tokio::test]
async fn application_lifecycle_supersession_observes_only_this_transactions_cancelled_predecessor()
{
    use control_plane_contracts::{
        application_public_runtime::ApplicationPublishedFlowRunRepository,
        ports::ApplicationRunLogContext,
    };
    let (store, seed, compiled, facts, _) = fixture().await;
    let api_key = seed_application_api_key(&store, &seed).await;
    let mut input = create_input(&seed, &compiled);
    input.run_mode = FlowRunMode::PublishedApiRun;
    input.api_key_id = Some(api_key);
    input.external_user = Some("lifecycle-source-user".into());
    input.compatibility_mode = Some("openai-responses-v1".into());
    input.application_run_log_context = Some(ApplicationRunLogContext {
        identity_status: "identified".into(),
        protocol: Some("openai_responses".into()),
        call_kind: Some("generate".into()),
        request_kind: Some("turn".into()),
        thread_id: Some("lifecycle-thread".into()),
        turn_id: Some("lifecycle-turn".into()),
        ..Default::default()
    });
    let predecessor =
        ApplicationPublishedFlowRunRepository::create_published_flow_run(&store, &input)
            .await
            .unwrap()
            .flow_run;
    store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: predecessor.id,
            status: FlowRunStatus::WaitingCallback,
            output_payload: json!({}),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    let mut successor_input = input;
    successor_input.idempotency_key = Some("lifecycle-successor".into());
    successor_input.started_at += Duration::seconds(1);
    successor_input
        .application_run_log_context
        .as_mut()
        .unwrap()
        .call_kind = Some("compact".into());
    successor_input
        .application_run_log_context
        .as_mut()
        .unwrap()
        .request_kind = Some("compaction".into());
    let successor = ApplicationPublishedFlowRunRepository::create_published_flow_run_superseding_callback_predecessors(&store, &successor_input).await.unwrap();
    assert!(successor.created);
    let observed = facts.for_run(predecessor.id);
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[1].phase, ApplicationLifecyclePhase::Ended);
    assert_eq!(observed[1].status, FlowRunStatus::Cancelled);
    assert_eq!(observed[1].workspace_id, seed.workspace_id);
    let replay = ApplicationPublishedFlowRunRepository::create_published_flow_run_superseding_callback_predecessors(&store, &successor_input).await.unwrap();
    assert!(!replay.created);
    assert_eq!(facts.for_run(predecessor.id).len(), 2);
    assert_eq!(facts.for_run(successor.flow_run.id).len(), 1);
}
