use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor,
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PreparedPublishedCallback,
        PublishedCallbackReservation, PublishedCallbackResumeSource, PublishedCallbackResumeTarget,
        ResumePublishedCallbackCommand,
    },
    compat::openai::OpenAiResponsesEnvelope,
    native_tool_resume::correlate_semantic_responses_callback,
};
use control_plane_contracts::{
    application_public_runtime::ApplicationPublishedCallbackAttemptRepository,
    ports::{CallbackResumeAdmissionFence, ProviderTransportPayload},
};

type Service = ApplicationPublishedCallbackResumeService<
    PgControlPlaneStore,
    control_plane::orchestration_runtime::OrchestrationRuntimeService<
        PgControlPlaneStore,
        semantic_runtime::PausedProvider,
    >,
>;

struct Fixture {
    store: PgControlPlaneStore,
    service: Arc<Service>,
    actor: ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
    provider: semantic_runtime::PausedProvider,
    _package: semantic_runtime::ProviderPackage,
}

async fn fixture(tool_count: usize) -> Fixture {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let started = OffsetDateTime::now_utc();
    let run = seed_flow_run_with_mode(
        &store,
        &seeded,
        &compiled,
        started,
        FlowRunMode::PublishedApiRun,
        None,
    )
    .await;
    let api_key_id = seed_application_api_key(&store, &seeded).await;
    let node = seed_node_run(&store, &run, started).await;
    let waiting = persist_callback_wait_with_tool_count(
        &store, &seeded, &run, &node, 1, None, None, tool_count,
    )
    .await;
    let callback = waiting.callback_task.unwrap();
    let request = json!({"model":"fixture","previous_response_id":format!("resp_{}",run.id),"input":[
        {"type":"function_call_output","call_id":"call-1-0","output":"ok"}]});
    let digest = ProviderTransportPayload::openai_responses(request.clone())
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
    let mut request_payload = callback.request_payload;
    request_payload["responses_round"] = json!({"response_id":format!("resp_{}",run.id),
        "history":current_callback_history_fixture(),"output":[]});
    sqlx::query("update flow_run_callback_tasks set request_payload=$2 where id=$1")
        .bind(callback.id)
        .bind(request_payload)
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
    let envelope = OpenAiResponsesEnvelope::capture(request).unwrap();
    let grant = if tool_count == 1 {
        Some(
            correlate_semantic_responses_callback(&store, &actor, &envelope)
                .await
                .unwrap()
                .unwrap(),
        )
    } else {
        None
    };
    let command = ResumePublishedCallbackCommand {
        responses_continuation: grant.clone(),
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
        native_transport: None,
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload: grant.map(|grant| grant.payload().clone()).unwrap_or_else(
            || json!({"tool_results":[{"tool_call_id":"call-1-0","content":"ok"}]}),
        ),
        response_mode: Some("streaming".into()),
    };
    let (runtime, provider, package) = semantic_runtime::seed_runtime_consumer(
        &store,
        &seeded,
        &compiled,
        Some(&waiting.checkpoint),
    )
    .await;
    let service = Arc::new(ApplicationPublishedCallbackResumeService::new(
        store.clone(),
        runtime,
    ));
    Fixture {
        store,
        service,
        actor,
        command,
        provider,
        _package: package,
    }
}

#[tokio::test]
async fn prepared_callback_execution_keeps_inline_state_within_worker_stack_budget() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("must prepare execution");
    };
    let callback_id = owner.callback_task_id();
    let execution = fixture.service.execute_prepared_callback(*owner);
    let inline_bytes = std::mem::size_of_val(&execution);
    // This is a regression budget for one future, not a business input limit or
    // a proof of the entire poll stack. Production stack measurements also cover
    // the graph, provider and host frames below these ownership wrappers.
    const INLINE_STATE_BUDGET: usize = 64 * 1024;
    println!("prepared_callback_execution_inline_bytes={inline_bytes}");
    drop(execution);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let attempt = fixture
                .store
                .get_published_callback_resume_attempt(callback_id)
                .await
                .unwrap()
                .unwrap();
            if attempt.status == domain::FlowRunCallbackResumeAttemptStatus::Received {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropping the unpolled execution must finish fenced cleanup");
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
    assert!(
        inline_bytes <= INLINE_STATE_BUDGET,
        "prepared callback execution retains {inline_bytes} inline bytes; budget={INLINE_STATE_BUDGET}"
    );
}

#[tokio::test]
async fn prepared_callback_abort_retries_with_one_transferred_claim_and_one_execution() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("must prepare execution");
    };
    let claim_id = owner.claim_id();
    let generation = owner.claim_generation();
    let callback_id = owner.callback_task_id();
    let attempt_id = fixture
        .store
        .get_published_callback_resume_attempt(callback_id)
        .await
        .unwrap()
        .unwrap()
        .id;
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
    owner.abort().await.unwrap();
    assert_eq!(
        fixture
            .store
            .get_published_callback_resume_attempt(callback_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunCallbackResumeAttemptStatus::Received
    );
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must immediately retry");
    };
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("must re-prepare execution");
    };
    assert_eq!(owner.claim_id(), claim_id);
    assert_eq!(owner.claim_generation(), generation + 1);
    let service = fixture.service.clone();
    let execution = tokio::spawn(async move { service.execute_prepared_callback(*owner).await });
    fixture.provider.wait_until_entered().await;
    let stored_generation: i64 =
        sqlx::query_scalar("select generation from flow_run_resume_claims where id=$1")
            .bind(claim_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(
        stored_generation,
        generation + 1,
        "execution must not reacquire its prepared claim"
    );
    fixture.provider.release.add_permits(1);
    let resumed = execution.await.unwrap().unwrap();
    assert_eq!(resumed.attempt.id, attempt_id);
    assert_eq!(
        fixture
            .provider
            .executions
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn stale_prepared_receipt_cannot_commit_inbox_or_acquire_execution() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(stale) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let callback_id = match fixture.command.target {
        PublishedCallbackResumeTarget::CallbackTask { callback_task_id } => callback_task_id,
        _ => unreachable!(),
    };
    let attempt = fixture
        .store
        .get_published_callback_resume_attempt(callback_id)
        .await
        .unwrap()
        .unwrap();
    let stale_fence = CallbackResumeAdmissionFence {
        attempt_id: attempt.id,
        expected_updated_at: attempt.updated_at,
    };
    fixture
        .service
        .release_unstarted_native_callback(&stale)
        .await
        .unwrap();
    let PublishedCallbackReservation::Acquired(current) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire replacement");
    };
    // Persisting a callback wait pre-creates its pending inbox rows. Compare the
    // complete rows (including payload, fingerprint, timestamps and raw JSON) so
    // rejection proves no mutation rather than assuming the inbox starts empty.
    let inbox_snapshot_query = "select coalesce(jsonb_agg(to_jsonb(inbox) order by inbox.tool_ordinal, inbox.id), '[]'::jsonb) from flow_run_tool_callback_inbox inbox where callback_task_id=$1";
    let inbox_before: Value = sqlx::query_scalar(inbox_snapshot_query)
        .bind(callback_id)
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(inbox_before.as_array().unwrap().len(), 1);
    assert_eq!(inbox_before[0]["status"], "pending");
    for field in ["result_payload", "result_fingerprint", "received_at"] {
        assert!(
            inbox_before[0][field].is_null(),
            "pending fixture has no {field}"
        );
    }
    assert!(fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            stale
        )
        .await
        .is_err());
    let inbox_after: Value = sqlx::query_scalar(inbox_snapshot_query)
        .bind(callback_id)
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(
        inbox_after, inbox_before,
        "a stale receipt must not mutate any inbox row"
    );
    let claims: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where callback_task_id=$1")
            .bind(callback_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(
        claims, 0,
        "a stale receipt must not acquire an execution claim"
    );
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            current,
        )
        .await
        .unwrap()
    else {
        panic!("current owner must prepare");
    };
    let claim_id = owner.claim_id();
    let before: i64 =
        sqlx::query_scalar("select generation from flow_run_resume_claims where id=$1")
            .bind(claim_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    let claim = sqlx::query(
        "select flow_run_id,checkpoint_id,request_payload from flow_run_resume_claims where id=$1",
    )
    .bind(claim_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    use sqlx::Row;
    assert!(fixture
        .store
        .acquire_resume_claim_with_admission(
            &AcquireResumeClaimInput {
                scope_id: fixture.actor.workspace_id,
                application_id: fixture.actor.application_id,
                flow_run_id: claim.get("flow_run_id"),
                checkpoint_id: claim.get("checkpoint_id"),
                callback_task_id: Some(callback_id),
                kind: ResumeClaimKind::Callback,
                request_payload: claim.get("request_payload"),
            },
            &stale_fence
        )
        .await
        .is_err());
    let after: i64 =
        sqlx::query_scalar("select generation from flow_run_resume_claims where id=$1")
            .bind(claim_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(before, after);
    owner.abort().await.unwrap();
}

#[tokio::test]
async fn partial_prepared_callback_parks_admission_without_execution_claim() {
    let fixture = fixture(2).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let PreparedPublishedCallback::NoExecution(result) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("partial tool round must not open an execution");
    };
    assert_eq!(
        result.run.status,
        control_plane::application_public_api::native::NativeRunStatus::Waiting
    );
    assert_eq!(
        result.attempt.status,
        domain::FlowRunCallbackResumeAttemptStatus::Received
    );
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
    let claims: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where flow_run_id=$1")
            .bind(result.run.id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(claims, 0);
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("partial round must immediately re-acquire");
    };
    fixture
        .service
        .release_unstarted_native_callback(&receipt)
        .await
        .unwrap();
}

#[tokio::test]
async fn prepared_callback_cleanup_cannot_release_replacement_execution_owner() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("must prepare");
    };
    let callback_id = owner.callback_task_id();
    let replacement = Uuid::now_v7();
    // Controlled replacement models an owner that already lost its claim generation.
    sqlx::query(
        "update flow_run_resume_claims set claim_token=$2,generation=generation+1 where id=$1",
    )
    .bind(owner.claim_id())
    .bind(replacement)
    .execute(fixture.store.pool())
    .await
    .unwrap();
    assert!(owner.abort().await.is_err());
    let token: Uuid = sqlx::query_scalar(
        "select claim_token from flow_run_resume_claims where callback_task_id=$1",
    )
    .bind(callback_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(token, replacement);
    assert_eq!(
        fixture
            .store
            .get_published_callback_resume_attempt(callback_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::FlowRunCallbackResumeAttemptStatus::Processing
    );
}

#[tokio::test]
async fn settled_prepared_callback_replays_without_a_new_claim_or_execution() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    let PreparedPublishedCallback::Execution(owner) = fixture
        .service
        .prepare_reserved_native_callback_for_actor(
            fixture.actor.clone(),
            fixture.command.clone(),
            receipt,
        )
        .await
        .unwrap()
    else {
        panic!("must prepare");
    };
    let generation = owner.claim_generation();
    let claim_id = owner.claim_id();
    let token: Uuid =
        sqlx::query_scalar("select claim_token from flow_run_resume_claims where id=$1")
            .bind(claim_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    fixture
        .store
        .finish_resume_claim(&FinishResumeClaimInput {
            claim_id,
            claim_token: token,
            expected_generation: generation,
            status: ResumeClaimStatus::Succeeded,
            error_payload: None,
            completed_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    assert!(
        owner.abort().await.is_err(),
        "cleanup must preserve a settled result"
    );
    assert!(matches!(
        fixture
            .service
            .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
            .await
            .unwrap(),
        PublishedCallbackReservation::SettledReplay { .. }
    ));
    let stored_generation: i64 =
        sqlx::query_scalar("select generation from flow_run_resume_claims where id=$1")
            .bind(claim_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(stored_generation, generation);
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn aged_same_admission_version_can_atomically_acquire_actual_execution() {
    let fixture = fixture(1).await;
    let PublishedCallbackReservation::Acquired(receipt) = fixture
        .service
        .reserve_native_callback_reservation_for_actor(fixture.actor.clone(), &fixture.command)
        .await
        .unwrap()
    else {
        panic!("must acquire");
    };
    sqlx::query("update flow_run_callback_resume_attempts set updated_at=now()-interval '6 minutes' where id=$1")
        .bind(receipt.attempt_id()).execute(fixture.store.pool()).await.unwrap();
    // Model an old receipt's actual persisted revision: age alone does not prove
    // a replacement owner. The same transaction still acquires the unique claim.
    let callback_id = match fixture.command.target {
        PublishedCallbackResumeTarget::CallbackTask { callback_task_id } => callback_task_id,
        _ => unreachable!(),
    };
    let attempt = fixture
        .store
        .get_published_callback_resume_attempt(callback_id)
        .await
        .unwrap()
        .unwrap();
    let context = fixture
        .store
        .get_callback_resume_context(fixture.actor.application_id, callback_id)
        .await
        .unwrap()
        .unwrap();
    let results = fixture.command.response_payload["tool_results"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .map(ToolCallbackResultInput::from_payload)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let output = fixture
        .store
        .commit_tool_callback_results_with_admission(
            &CommitToolCallbackResultsInput {
                responses_continuation: Some(
                    serde_json::from_value(
                        fixture.command.response_payload["responses_continuation"].clone(),
                    )
                    .unwrap(),
                ),
                scope_id: fixture.actor.workspace_id,
                application_id: fixture.actor.application_id,
                flow_run_id: context.flow_run.id,
                checkpoint_id: context.checkpoint.id,
                callback_task_id: callback_id,
                results,
            },
            &CallbackResumeAdmissionFence {
                attempt_id: attempt.id,
                expected_updated_at: attempt.updated_at,
            },
        )
        .await
        .unwrap();
    assert_eq!(output.disposition, ToolCallbackRoundDisposition::Acquired);
    assert!(output.claim.is_some());
    assert!(fixture.provider.inputs.lock().unwrap().is_empty());
}
