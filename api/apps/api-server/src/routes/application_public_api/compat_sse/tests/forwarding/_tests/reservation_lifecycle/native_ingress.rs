use super::*;
use crate::routes::application_public_api::{
    compatibility_interface as binding, native_websocket::NativeWebSocketProjector,
    sse::IncludeWorkflowEvents,
};
use control_plane::application_public_api::run_service::ApplicationPublishedRunControlRepository;
use interface_runtime::{
    ApplicationPrincipal, BindingId, CompiledInterfaceRegistry, InterfaceInvocationFailure,
    InterfaceInvocationKernel, InterfaceProtocol, InterfaceStreamInvocation,
    InterfaceStreamTerminal, InvocationEnvelope, InvocationId, InvocationLineage,
};
use std::sync::atomic::Ordering;

type Invocation = InterfaceStreamInvocation<
    binding::CompatibilityStreamEvent,
    binding::CompatibilityBlockingOutput,
    binding::CompatibilityBlockingTargetError,
>;

fn native_command(
    mut command: ResumePublishedCallbackCommand,
    run: Uuid,
    tool_index: usize,
) -> ResumePublishedCallbackCommand {
    let callback_task_id = callback_task_id_from_resume_command(&command);
    let result = command.response_payload["tool_results"][tool_index].clone();
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.target = PublishedCallbackResumeTarget::FlowRun {
        flow_run_id: run,
        callback_task_id,
    };
    command.native_transport = None;
    command.responses_continuation = None;
    command.response_payload = json!({"tool_results":[result]});
    command
}

// Public ingress prepares the actual None/None Native command. No manual reserve.
async fn public_plan(
    state: Arc<ApiState>,
    actor: ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
) -> Box<CompatibleResumePlan> {
    assert!(command.native_transport.is_none());
    assert!(command.responses_continuation.is_none());
    let CompatibleResumeAdmission::Resume(plan) =
        prepare_compatible_resume_for_actor(state, actor, command)
            .await
            .unwrap()
    else {
        panic!("Native tool delivery must reach its existing reservation owner")
    };
    assert!(plan.reservation.as_ref().unwrap().is_acquired());
    plan
}

// Use the real compiler, Native binding, authorization and execution adapter.
// The principal is the same established application identity used by the WS bridge.
async fn invoke_plan(
    state: Arc<ApiState>,
    registry: Arc<CompiledInterfaceRegistry>,
    actor: ApplicationApiKeyActor,
    plan: Box<CompatibleResumePlan>,
) -> Result<Invocation, InterfaceInvocationFailure> {
    let principal = ApplicationPrincipal::new(
        actor.application_id,
        actor.api_key_id,
        actor.workspace_id,
        actor.actor,
    )
    .unwrap();
    let binding_id = BindingId::new(binding::NATIVE_WEBSOCKET_STREAM_BINDING_ID).unwrap();
    let activation = registry.authentication(&binding_id).unwrap();
    let target = native::application_runtime_target(
        &state,
        &registry,
        &binding_id,
        principal.application_id(),
    );
    let envelope = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        binding_id.clone(),
        InterfaceProtocol::Http,
        activation.adapter().clone(),
        activation.activation().clone(),
        principal,
        None,
        binding::CompatibilityBlockingInput {
            command: binding::CompatibilityInvocationCommand::Resume {
                initial_run: plan.initial_run,
                command: plan.command,
                reservation: plan.reservation,
            },
        },
    );
    InterfaceInvocationKernel::new(Arc::new(binding::CompatibilityBlockingAuthorization))
        .invoke_server_stream_with_dispatch_target::<binding::CompatibilityBlockingInput,
            binding::CompatibilityStreamEvent, binding::CompatibilityBlockingOutput,
            binding::CompatibilityBlockingTargetError>(registry, envelope, target).await
}

async fn native_waiting(invocation: Invocation, callback: Uuid, expected: &Value) {
    let (mut events, completion) = invocation.into_parts();
    let mut projector =
        NativeWebSocketProjector::new("real-native-ingress".into(), IncludeWorkflowEvents::Public);
    let mut frames = Vec::new();
    while let Some(event) = events.recv().await {
        let (run, envelope, receipt) = event.into_parts();
        assert!(receipt.is_none());
        assert_eq!(envelope.event_type, "waiting_callback");
        assert_eq!(run.metadata["response_round_id"], json!(callback));
        assert_eq!(
            selected_delivery_scope(&run),
            RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback
            }
        );
        frames.push(
            serde_json::from_str::<Value>(&projector.project(&run, envelope).unwrap().unwrap())
                .unwrap(),
        );
    }
    assert!(projector.has_terminal());
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["type"], "required_action");
    assert_eq!(&frames[0]["data"]["required_action"], expected);
    let outcome = completion.complete().await.unwrap();
    let InterfaceStreamTerminal::Completed(output) = outcome.terminal() else {
        panic!("Native waiting observation must complete the compiled invocation")
    };
    assert_eq!(output.0.status, NativeRunStatus::Waiting);
    assert_eq!(
        output.0.required_action.as_ref().unwrap().payload["callback_task_id"],
        json!(callback)
    );
}

async fn inbox(state: &ApiState, callback: Uuid) -> Value {
    sqlx::query_scalar("select jsonb_agg(to_jsonb(i) order by tool_ordinal) from flow_run_tool_callback_inbox i where callback_task_id=$1")
        .bind(callback).fetch_one(state.store.pool()).await.unwrap()
}
async fn attempt(state: &ApiState, callback: Uuid) -> (Uuid, String, time::OffsetDateTime, Value) {
    sqlx::query_as("select id,status,updated_at,response_payload from flow_run_callback_resume_attempts where callback_task_id=$1")
        .bind(callback).fetch_one(state.store.pool()).await.unwrap()
}
async fn claims(state: &ApiState, run: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from flow_run_resume_claims where flow_run_id=$1")
        .bind(run)
        .fetch_one(state.store.pool())
        .await
        .unwrap()
}
async fn producer_facts(state: &ApiState, run: Uuid) -> i64 {
    sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1 and event_type in ('flow_started','runtime_stream_opened','model_call')")
        .bind(run).fetch_one(state.store.pool()).await.unwrap()
}

#[tokio::test]
async fn public_native_partial_duplicate_and_c2_reach_compiled_binding_and_single_execution_owner()
{
    let (state, mut dependencies, run, actor, full, service, stream, _app) =
        fixture_with_tool_count("cancel", 2).await;
    dependencies.native.runtime_invoker_factory = Arc::new(NoToolsFactory);
    let registry = binding::compile_registry(
        binding::compatibility_execution_port(dependencies),
        binding::compatibility_models_port(state.store.clone()),
    )
    .unwrap();
    let callback = callback_task_id_from_resume_command(&full);
    let c1 = native_command(full.clone(), run.id, 0);
    let c2 = native_command(full.clone(), run.id, 1);
    let expected = json!({"action_type":"submit_tool_outputs","payload":{
        "callback_task_id":callback,"callback_kind":"llm_tool_calls","flow_run_id":run.id,
        "node_run_id":state.store.get_published_callback_task(callback).await.unwrap().unwrap().node_run_id,
        "tool_calls":[{"id":"call_fixture","name":"fixture","arguments":{}},{"id":"call_pending","name":"other","arguments":{}}]
    }});
    // A real existing generation is captured by PUBLIC prepare. Partial receipts
    // must neither retire it nor open another one through the compiled adapter.
    state
        .runtime_event_stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let prior = state
        .runtime_event_stream
        .terminal_writer(run.id)
        .await
        .unwrap();
    let custom = stream.local.terminal_writer(run.id).await.unwrap();
    let before = delivery_rows(&state, run.id).await;
    let markers = producer_facts(&state, run.id).await;
    let mut first_inbox: Option<Value> = None;
    let mut first_receipt: Option<
        control_plane::application_public_api::callback_resume::PublishedCallbackReservationReceipt,
    > = None;
    let mut prior_version: Option<time::OffsetDateTime> = None;
    for _ in 0..2 {
        let plan = public_plan(state.clone(), actor.clone(), c1.clone()).await;
        let acquired = attempt(&state, callback).await;
        assert_eq!(acquired.1, "processing");
        assert_eq!(acquired.3, c1.response_payload);
        let receipt = plan.reservation.as_ref().unwrap().receipt().unwrap();
        if let Some(original) = &first_receipt {
            assert_ne!(&receipt, original);
        }
        if let Some(previous) = prior_version {
            assert!(acquired.2 > previous);
        }
        let invocation = invoke_plan(state.clone(), registry.clone(), actor.clone(), plan)
            .await
            .unwrap();
        native_waiting(invocation, callback, &expected).await;
        let parked = attempt(&state, callback).await;
        assert_eq!(parked.0, acquired.0);
        assert_eq!(parked.1, "received");
        assert!(parked.2 > acquired.2);
        if let Some(original) = &first_receipt {
            assert_eq!(receipt.attempt_id(), original.attempt_id());
            assert!(
                !service
                    .release_unstarted_native_callback(original)
                    .await
                    .unwrap()
            );
        }
        first_receipt = Some(receipt);
        prior_version = Some(parked.2);
        let rows = inbox(&state, callback).await;
        assert_eq!(rows[0]["status"], "received");
        assert_eq!(rows[1]["status"], "pending");
        if let Some(original) = &first_inbox {
            assert_eq!(
                &rows, original,
                "same c1 leaves every inbox field and timestamp unchanged"
            );
        }
        first_inbox = Some(rows);
        assert_eq!(
            state
                .store
                .get_published_callback_task(callback)
                .await
                .unwrap()
                .unwrap()
                .status,
            domain::CallbackTaskStatus::Pending
        );
        assert_eq!(claims(&state, run.id).await, 0);
        assert_eq!(delivery_rows(&state, run.id).await, before);
        assert_eq!(producer_facts(&state, run.id).await, markers);
        assert_eq!(stream.open_attempts.load(Ordering::SeqCst), 0);
        assert_eq!(stream.subscriptions.load(Ordering::SeqCst), 0);
        assert!(prior.closure().is_none());
        assert!(custom.closure().is_none());
        assert_eq!(
            state
                .runtime_event_stream
                .terminal_writer(run.id)
                .await
                .unwrap()
                .generation_id(),
            prior.generation_id()
        );
        assert_eq!(
            stream
                .local
                .terminal_writer(run.id)
                .await
                .unwrap()
                .generation_id(),
            custom.generation_id()
        );
    }
    // Content validation still belongs to the same fenced inbox transaction.
    let unchanged = first_inbox.unwrap();
    for kind in ["unknown", "changed", "duplicate"] {
        let mut bad = c1.clone();
        match kind {
            "unknown" => bad.response_payload["tool_results"][0]["tool_call_id"] = json!("unknown"),
            "changed" => bad.response_payload["tool_results"][0]["content"] = json!("different"),
            "duplicate" => {
                let entry = bad.response_payload["tool_results"][0].clone();
                bad.response_payload["tool_results"]
                    .as_array_mut()
                    .unwrap()
                    .push(entry);
            }
            _ => unreachable!(),
        }
        let plan = public_plan(state.clone(), actor.clone(), bad).await;
        assert!(
            invoke_plan(state.clone(), registry.clone(), actor.clone(), plan)
                .await
                .is_err(),
            "{kind}"
        );
        assert_eq!(
            inbox(&state, callback).await,
            unchanged,
            "{kind} must roll back"
        );
        assert_eq!(attempt(&state, callback).await.1, "received");
        assert_eq!(claims(&state, run.id).await, 0);
        assert_eq!(delivery_rows(&state, run.id).await, before);
        assert_eq!(stream.open_attempts.load(Ordering::SeqCst), 0);
        assert!(prior.closure().is_none());
    }
    let plan = public_plan(state.clone(), actor.clone(), c2.clone()).await;
    let final_receipt = plan.reservation.as_ref().unwrap().receipt().unwrap();
    assert_eq!(
        final_receipt.attempt_id(),
        first_receipt.as_ref().unwrap().attempt_id()
    );
    assert_eq!(attempt(&state, callback).await.3, c2.response_payload);
    // A read-plan cannot confer a second admission while this CAS owner lives.
    assert!(
        prepare_compatible_resume_for_actor(state.clone(), actor.clone(), c2)
            .await
            .is_err()
    );
    let completing = tokio::spawn(invoke_plan(state.clone(), registry, actor, plan));
    tokio::time::timeout(Duration::from_secs(3), stream.entered.notified())
        .await
        .expect("full c2 reaches normal generation setup with an actual execution owner");
    let completed = state
        .store
        .get_published_callback_task(callback)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, domain::CallbackTaskStatus::Completed);
    assert_eq!(
        completed.response_payload.as_ref().unwrap()["tool_results"],
        full.response_payload["tool_results"]
    );
    assert_eq!(claims(&state, run.id).await, 1);
    let owner: (String, Value) = sqlx::query_as(
        "select status,request_payload from flow_run_resume_claims where callback_task_id=$1",
    )
    .bind(callback)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(owner.0, "processing");
    assert_eq!(
        owner.1["tool_results"],
        full.response_payload["tool_results"]
    );
    let running: i64 = sqlx::query_scalar("select count(*) from flow_run_recovery_history where flow_run_id=$1 and state_code='running'")
        .bind(run.id).fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(running, 1);
    assert_eq!(
        inbox(&state, callback).await[0],
        unchanged[0],
        "c2 cannot rewrite c1"
    );
    assert_eq!(inbox(&state, callback).await[1]["status"], "received");
    assert!(
        !service
            .release_unstarted_native_callback(&final_receipt)
            .await
            .unwrap(),
        "active execution owns admission"
    );
    assert!(
        !service
            .release_unstarted_native_callback(first_receipt.as_ref().unwrap())
            .await
            .unwrap()
    );
    assert_eq!(stream.open_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        producer_facts(&state, run.id).await,
        markers,
        "controlled open blocks before provider execution"
    );
    assert_eq!(delivery_rows(&state, run.id).await, before);
    let attempt_count: i64 = sqlx::query_scalar(
        "select count(*) from flow_run_callback_resume_attempts where callback_task_id=$1",
    )
    .bind(callback)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(attempt_count, 1);
    completing.abort();
    assert!(matches!(completing.await, Err(error) if error.is_cancelled()));
    await_received(&state, final_receipt.attempt_id()).await;
    assert_single_cleaned_claim(&state, callback).await;
}

#[tokio::test]
async fn public_native_non_tool_retains_ordinary_plan_and_tool_owner_rejects_foreign_context() {
    let (state, _, run, actor, mut ordinary, service, stream, _app) =
        fixture_with_callback_kind("open", 0, "external_callback").await;
    ordinary.source = PublishedCallbackResumeSource::NativeAgent;
    ordinary.native_transport = None;
    ordinary.responses_continuation = None;
    ordinary.target = PublishedCallbackResumeTarget::FlowRun {
        flow_run_id: run.id,
        callback_task_id: callback_task_id_from_resume_command(&ordinary),
    };
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &ordinary)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::Resume { .. }
    ));
    let CompatibleResumeAdmission::Resume(plan) =
        prepare_compatible_resume_for_actor(state.clone(), actor, ordinary)
            .await
            .unwrap()
    else {
        panic!("ordinary Native callback must retain Resume")
    };
    assert!(
        plan.reservation.is_none(),
        "non-LLM Native must never acquire prepared admission"
    );
    assert_eq!(stream.open_attempts.load(Ordering::SeqCst), 0);
    let attempts: i64 = sqlx::query_scalar(
        "select count(*) from flow_run_callback_resume_attempts where flow_run_id=$1",
    )
    .bind(run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(attempts, 0);

    let (state, _, run, actor, full, _, _, _app) = fixture_with_tool_count("open", 2).await;
    let c1 = native_command(full, run.id, 0);
    let before = delivery_rows(&state, run.id).await;
    for field in ["run", "app", "key", "creator"] {
        let mut bad_actor = actor.clone();
        let mut bad_command = c1.clone();
        match field {
            "run" => {
                bad_command.target = PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id: Uuid::now_v7(),
                    callback_task_id: callback_task_id_from_resume_command(&c1),
                }
            }
            "app" => bad_actor.application_id = Uuid::now_v7(),
            "key" => bad_actor.api_key_id = Uuid::now_v7(),
            "creator" => bad_actor.creator_user_id = Uuid::now_v7(),
            _ => unreachable!(),
        }
        let error = match prepare_compatible_resume_for_actor(state.clone(), bad_actor, bad_command)
            .await
        {
            Ok(_) => panic!("{field} must be rejected before Native classification"),
            Err(error) => error,
        };
        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN, "{field}");
    }
    let callback = callback_task_id_from_resume_command(&c1);
    let plan = public_plan(state.clone(), actor.clone(), c1.clone()).await;
    let mut foreign = c1;
    foreign.source = PublishedCallbackResumeSource::OpenAiChat;
    assert!(
        prepare_compatible_resume_for_actor(state.clone(), actor, foreign)
            .await
            .is_err()
    );
    assert_eq!(
        attempt(&state, callback).await.1,
        "processing",
        "source rejection cannot park another owner"
    );
    assert_eq!(delivery_rows(&state, run.id).await, before);
    drop(plan);
    await_received(&state, attempt(&state, callback).await.0).await;
}
