use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor,
    callback_resume::{PublishedCallbackResumeSource, PublishedCallbackResumeTarget},
    native_tool_resume::correlate_native_responses_callback,
};
use control_plane::ports::{
    AppendRuntimeEventInput, PersistWaitingCallbackTaskInput, PersistWaitingKind,
    PersistWaitingStateInput, ProviderTransportPayload, RuntimeEventSubscription,
    RuntimeEventTrimPolicy,
};
use orchestration_runtime::{
    compiled_plan::CompiledNode,
    execution_engine::{
        RuntimeInternalToolInvoker, RuntimeInternalToolOutput, RuntimeInternalToolRegistration,
    },
};

struct NoTools;
#[async_trait::async_trait]
impl RuntimeInternalToolInvoker for NoTools {
    fn registrations_for_node(&self, _: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        Vec::new()
    }
    async fn invoke_runtime_internal_tool(
        &self,
        _: &CompiledNode,
        _: &RuntimeInternalToolRegistration,
        _: serde_json::Value,
    ) -> anyhow::Result<RuntimeInternalToolOutput> {
        anyhow::bail!("reservation fixture cannot invoke a tool")
    }
}

struct NoToolsFactory;
impl crate::routes::application_public_api::native_read_interface::NativeRuntimeInvokerFactory
    for NoToolsFactory
{
    fn without_mcp(&self) -> Result<Arc<dyn RuntimeInternalToolInvoker>, NativeApiError> {
        Ok(Arc::new(NoTools) as Arc<dyn RuntimeInternalToolInvoker>)
    }

    fn for_actor<'a>(
        &'a self,
        _: &'a ApplicationApiKeyActor,
    ) -> crate::routes::application_public_api::native_read_interface::RuntimeInvokerFuture<'a>
    {
        Box::pin(async { Ok(Arc::new(NoTools) as Arc<dyn RuntimeInternalToolInvoker>) })
    }
}

fn native_receipt_consumer() -> CompatibleResumeConsumer {
    CompatibleResumeConsumer::for_binding(crate::routes::application_public_api::compatibility_interface::NATIVE_WEBSOCKET_STREAM_BINDING_ID)
}
fn partial_native_command(
    mut command: ResumePublishedCallbackCommand,
) -> ResumePublishedCallbackCommand {
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.native_transport = None;
    command.responses_continuation = None;
    command.response_payload["tool_results"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    command
}
async fn delivery_rows(state: &ApiState, run: Uuid) -> Value {
    sqlx::query_scalar("select coalesce(jsonb_agg(jsonb_build_object('id',id,'status',delivery_status,'token',delivery_claim_token,'generation',delivery_generation,'lease',delivery_lease_expires_at,'acked',delivery_acked_at,'uncertain',delivery_uncertain_at) order by sequence,id),'[]') from runtime_events where flow_run_id=$1 and delivery_status is not null")
        .bind(run).fetch_one(state.store.pool()).await.unwrap()
}

async fn open_partial(
    dependencies: CompatibilityExecutionDependencies,
    run: NativeRunResult,
    actor: ApplicationApiKeyActor,
    mut command: ResumePublishedCallbackCommand,
    service: Arc<callback_reservation::CallbackService>,
    stream: Arc<PreparationStream>,
) -> OpenedCompatibleTurn {
    let acquired = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    command.reserved_attempt_id = Some(acquired.attempt_id());
    let mut reservation = CompatibleCallbackReservation::new(acquired, service);
    reservation
        .capture_previous_generation(stream.as_ref(), run.id)
        .await;
    open_compatible_turn_with_invoker(
        dependencies,
        run,
        CompatibleTurnAction::ResumeForActor {
            command,
            actor,
            reservation: Some(reservation),
        },
        None,
        Arc::new(NoTools),
    )
    .await
    .unwrap()
}

async fn collect_native_waiting(
    mut typed: CompatibleTypedTurnStream,
    callback: Uuid,
) -> Vec<RuntimeEventEnvelope> {
    use crate::routes::application_public_api::{
        native_websocket::NativeWebSocketProjector, sse::IncludeWorkflowEvents,
    };
    let mut projector =
        NativeWebSocketProjector::new("pending-receipt".into(), IncludeWorkflowEvents::Public);
    let expected_action =
        serde_json::to_value(typed.initial_run.required_action.as_ref().unwrap()).unwrap();
    let mut events = Vec::new();
    let mut frames = Vec::new();
    while let Some(input) = typed.events.recv().await {
        let (run, event, receipt) = input.into_parts();
        assert!(
            receipt.is_none(),
            "pending waiting observation never grants settlement"
        );
        assert_ne!(event.event_type, "provider_output_item_done");
        assert_eq!(run.metadata["response_round_id"], json!(callback));
        if let Some(frame) = projector.project(&run, event.clone()).unwrap() {
            frames.push(serde_json::from_str::<Value>(&frame).unwrap());
        }
        events.push(event);
    }
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["type"], "required_action");
    assert_eq!(frames[0]["data"]["required_action"], expected_action);
    assert_eq!(
        frames[0]["data"]["required_action"]["action_type"],
        "submit_tool_outputs"
    );
    assert_eq!(
        frames[0]["data"]["required_action"]["payload"]["callback_task_id"],
        json!(callback)
    );
    assert!(projector.has_terminal());
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "waiting_callback")
            .count(),
        1
    );
    events
}

struct PreparationStream {
    local: LocalRuntimeEventStream,
    failure: &'static str,
    entered: tokio::sync::Notify,
    open_attempts: std::sync::atomic::AtomicUsize,
    subscriptions: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl RuntimeEventStream for PreparationStream {
    async fn terminal_writer(
        &self,
        run: Uuid,
    ) -> anyhow::Result<Arc<dyn control_plane::ports::RuntimeEventTerminalWriter>> {
        self.local.terminal_writer(run).await
    }
    async fn open_run(&self, run: Uuid, policy: RuntimeEventStreamPolicy) -> anyhow::Result<()> {
        self.open_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.entered.notify_one();
        match self.failure {
            "open" => anyhow::bail!("controlled pre-handoff open failure"),
            "cancel" => std::future::pending::<anyhow::Result<()>>().await,
            _ => self.local.open_run(run, policy).await,
        }
    }
    async fn append(
        &self,
        run: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        self.local.append(run, event).await
    }
    async fn append_terminal_if_missing_and_close(
        &self,
        run: Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<control_plane::ports::AppendTerminalIfMissingAndCloseOutcome> {
        self.local
            .append_terminal_if_missing_and_close(run, event)
            .await
    }
    async fn subscribe(
        &self,
        run: Uuid,
        cursor: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        self.subscriptions
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.failure == "subscribe" {
            anyhow::bail!("controlled pre-handoff subscribe failure");
        }
        self.local.subscribe(run, cursor).await
    }
    async fn replay(
        &self,
        run: Uuid,
        cursor: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        self.local.replay(run, cursor, limit).await
    }
    async fn close_run(&self, run: Uuid, reason: RuntimeEventCloseReason) -> anyhow::Result<()> {
        self.local.close_run(run, reason).await
    }
    async fn trim(&self, run: Uuid, policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        self.local.trim(run, policy).await
    }
}

async fn fixture(
    failure: &'static str,
) -> (
    Arc<ApiState>,
    CompatibilityExecutionDependencies,
    NativeRunResult,
    ApplicationApiKeyActor,
    ResumePublishedCallbackCommand,
    Arc<callback_reservation::CallbackService>,
    Arc<PreparationStream>,
    axum::Router,
) {
    fixture_with_tool_count(failure, 1).await
}

async fn fixture_with_tool_count(
    failure: &'static str,
    tool_count: usize,
) -> (
    Arc<ApiState>,
    CompatibilityExecutionDependencies,
    NativeRunResult,
    ApplicationApiKeyActor,
    ResumePublishedCallbackCommand,
    Arc<callback_reservation::CallbackService>,
    Arc<PreparationStream>,
    axum::Router,
) {
    fixture_with_callback_kind(failure, tool_count, "llm_tool_calls").await
}

async fn fixture_with_callback_kind(
    failure: &'static str,
    tool_count: usize,
    callback_kind: &str,
) -> (
    Arc<ApiState>,
    CompatibilityExecutionDependencies,
    NativeRunResult,
    ApplicationApiKeyActor,
    ResumePublishedCallbackCommand,
    Arc<callback_reservation::CallbackService>,
    Arc<PreparationStream>,
    axum::Router,
) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    // Real ingress upgrades the generation's Weak edge. Keep the external
    // serving root alive for the entire reservation/retry fixture lifetime.
    let app = crate::app_with_state(state.clone());
    let mut run = native_run();
    seed_flow_run_for_compat_sse_test(&state, &run).await;
    sqlx::query("update flow_runs set api_key_id=$2 where id=$1")
        .bind(run.id)
        .bind(run.api_key_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let (owner, workspace_id, tenant_id): (Uuid, Uuid, Uuid) = sqlx::query_as("select r.created_by,a.workspace_id,w.tenant_id from flow_runs r join applications a on a.id=r.application_id join workspaces w on w.id=a.workspace_id where r.id=$1")
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = ApplicationApiKeyActor {
        api_key_id: run.api_key_id,
        application_id: run.application_id,
        creator_user_id: owner,
        tenant_id,
        workspace_id,
        actor: domain::ActorContext::root_in_scope(owner, tenant_id, workspace_id, "root"),
    };
    let node = state
        .store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id: run.id,
            node_id: "fixture".into(),
            node_type: "llm".into(),
            node_alias: "Fixture".into(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    // Match the native Responses ingress in openai.rs: correlate the response
    // cursor/provider call ID and retain the same native transport in the command.
    let tool_calls = (0..tool_count)
        .map(|index| {
            if index == 0 {
                json!({"id":"call_fixture","name":"fixture","arguments":{}})
            } else {
                json!({"id":"call_pending","name":"other","arguments":{}})
            }
        })
        .collect::<Vec<_>>();
    let response_id = format!("resp_reservation_fixture_{}", run.id);
    let request = json!({"model":"fixture","stream":true,"store":false,
        "previous_response_id":response_id,
        "input":tool_calls.iter().map(|call| json!({"type":"function_call_output","call_id":call["id"],"output":"fixture-result"})).collect::<Vec<_>>()});
    let native_transport = ProviderTransportPayload::openai_responses(request.clone()).unwrap();
    // Use the real CP prepare path and PostgreSQL claim transaction. Setup
    // failures stop before execution, so no provider process is installed.
    sqlx::query("update flow_compiled_plans p set plan=jsonb_build_object('flow_id',p.flow_id,'source_draft_id',p.flow_draft_id::text,'schema_version','1flowbase.flow/v2','topological_order',jsonb_build_array('fixture'),'nodes',$2::jsonb) from flow_runs r where r.id=$1 and p.id=r.compiled_plan_id")
        .bind(run.id)
        .bind(json!({"fixture":{"node_id":"fixture","node_type":"llm","alias":"Fixture","container_id":null,"dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],"config":{},"llm_runtime":{"provider_instance_id":Uuid::now_v7(),"provider_code":"fixture","protocol":"openai_compatible","model":"fixture"}}}))
        .execute(state.store.pool()).await.unwrap();
    let callback_id = Uuid::now_v7();
    let variable_snapshot = json!({"fixture":{"__llm_tool_callback":{
        "callback_kind":callback_kind,
        "pending_tool_calls":tool_calls,
        "system":[],"history":[{"role":"assistant","content":"","tool_calls":tool_calls}]
    }}});
    let event = |event_type: &str, payload: Value| AppendRuntimeEventInput {
        flow_run_id: run.id,
        node_run_id: Some(node.id),
        span_id: None,
        parent_span_id: None,
        event_type: event_type.into(),
        layer: domain::RuntimeEventLayer::AgentTransition,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
    };
    let waiting = state.store.persist_waiting_state(&PersistWaitingStateInput {
        checkpoint_id: Uuid::now_v7(), scope_id: workspace_id, application_id:run.application_id,
        flow_run_id:run.id,node_run_id:node.id,expected_status:domain::FlowRunStatus::WaitingCallback,
        output_payload:json!({}),checkpoint_status:"waiting_callback".into(),checkpoint_reason:callback_kind.into(),
        locator_payload:json!({"node_id":"fixture","next_node_index":0,"active_node_ids":["fixture"]}),
        variable_snapshot:variable_snapshot.clone(),checkpoint_external_ref_payload:None,
        context_content:json!({"format":"runtime_snapshot_v1","variable_pool":variable_snapshot}),
        parent_context_version_id:None,context_transition_kind:domain::ContextTransitionKind::Append,
        recovery_idempotency_key:format!("waiting_callback:{callback_id}"),resume_claim_id:None,resume_claim_token:None,
        waiting_event:event("waiting_callback",json!({"callback_task_id":callback_id})),
        tool_delivery_events:tool_calls.iter().enumerate().map(|(index,call)|event("provider_output_item_done",json!({"output_index":index,"committed_delivery":true,
            "item":{"type":"function_call","call_id":call["id"],"name":call["name"],"arguments":"{}","opaque":{"original":index}}}))).collect(),
        kind:PersistWaitingKind::Callback(PersistWaitingCallbackTaskInput {
            id:callback_id,callback_kind:callback_kind.into(),external_ref_payload:None,
            request_payload:json!({"tool_calls":tool_calls,
                "provider_metadata":{"native_response":{
                    "response_id":response_id,
                    "configuration_digest":native_transport.configuration_digest().unwrap(),
                    "user_messages_digest":native_transport.user_messages_digest().unwrap()
                }}}),
        }),
    }).await.unwrap().unwrap();
    let callback = waiting.callback_task.unwrap();
    assert_eq!(
        waiting.recovery_history.idempotency_key,
        format!("waiting_callback:{}", callback.id)
    );
    assert_eq!(
        waiting.recovery_history.coordinate.event_sequence,
        waiting.waiting_event.sequence
    );
    run.metadata["response_round_id"] = json!(callback.id);
    let response_payload = if callback_kind == "llm_tool_calls" {
        let (correlated_callback, payload) =
            correlate_native_responses_callback(&state.store, &actor, &request)
                .await
                .unwrap()
                .expect("the real native Responses ingress must recognize this owned callback");
        assert_eq!(correlated_callback.id, callback.id);
        payload
    } else {
        json!({"answer":"approved"})
    };
    let command = ResumePublishedCallbackCommand {
        bearer_token: String::new(),
        target: PublishedCallbackResumeTarget::CallbackTask {
            callback_task_id: callback.id,
        },
        source: PublishedCallbackResumeSource::OpenAiResponses,
        response_payload,
        response_mode: Some("streaming".into()),
        native_transport: Some(native_transport),
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        reserved_attempt_id: None,
    };
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "waiting_callback",
        json!({"callback_task_id": callback.id}),
    )
    .await;
    let old_generation = Uuid::now_v7();
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "runtime_stream_opened",
        json!({"stream_generation_id": old_generation}),
    )
    .await;
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "flow_failed",
        json!({"stream_generation_id": old_generation, "error": "old pool timeout"}),
    )
    .await;
    let stream = Arc::new(PreparationStream {
        local: LocalRuntimeEventStream::new(),
        failure,
        entered: tokio::sync::Notify::new(),
        open_attempts: std::sync::atomic::AtomicUsize::new(0),
        subscriptions: std::sync::atomic::AtomicUsize::new(0),
    });
    stream
        .local
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let native = crate::routes::application_public_api::native::ApplicationNativeRunDependencies {
        store: state.store.clone(),
        cache_store: state.infrastructure.cache_store(),
        published_plan_cache: state.infrastructure.published_plan_cache(),
        published_publication_cache: state.infrastructure.published_publication_cache(),
        runtime_engine: state.runtime_engine.clone(),
        provider_runtime: state.provider_runtime.clone(),
        network_egress: Arc::new(state.network_egress_http_clients()),
        provider_secret_master_key: state.provider_secret_master_key.clone(),
        model_billing_require_provider_usage: state.model_billing_require_provider_usage,
        api_node_id: state.api_node_id.clone(),
        provider_install_root: state.provider_install_root.clone(),
        file_storage_registry: state.file_storage_registry.clone(),
        notification_sender: state.notification_service.clone(),
        task_queue: state.infrastructure.task_queue(),
        provider_transport_store: state.infrastructure.provider_transport_store(),
        runtime_event_stream: stream.clone(),
        runtime_activity: state.runtime_activity.clone(),
        runtime_invoker_factory: Arc::new(RejectAttachExecutor),
    };
    let service = Arc::new(ApplicationPublishedCallbackResumeService::new(
        state.store.clone(),
        native::native_runtime_service(&native, Arc::new(NoTools)),
    ));
    let dependencies = CompatibilityExecutionDependencies {
        native,
        provider_transport_store: state.infrastructure.provider_transport_store(),
    };
    assert!(
        matches!(
            durable_compatible_round_replay(&dependencies, &run, callback.id)
                .await
                .unwrap(),
            DurableCompatibleRoundReplay::PendingIdentity
        ),
        "an old unowned anchor/host failure cannot supply callback identity"
    );
    (
        state,
        dependencies,
        run,
        actor,
        command,
        service,
        stream,
        app,
    )
}

async fn await_received(state: &ApiState, id: Uuid) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let status: String = sqlx::query_scalar(
                "select status from flow_run_callback_resume_attempts where id=$1",
            )
            .bind(id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
            if status == "received" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unstarted admission must be released without its predecessor timeout");
}

async fn assert_single_cleaned_claim(state: &ApiState, callback_id: Uuid) {
    let counts: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from flow_run_callback_resume_attempts where callback_task_id=$1), count(*), count(*) filter (where status='failed') from flow_run_resume_claims where callback_task_id=$1",
    ).bind(callback_id).fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(
        counts,
        (1, 1, 1),
        "pre-execution cleanup retains one admission and one failed execution claim, without duplicate ownership"
    );
}

async fn ingress_retry_plan(
    state: &Arc<ApiState>,
    actor: ApplicationApiKeyActor,
    command: &ResumePublishedCallbackCommand,
) -> CompatibleResumePlan {
    match prepare_compatible_resume_for_actor(state.clone(), actor, command.clone())
        .await
        .expect("released callback retry must traverse real ingress prepare and reservation")
    {
        CompatibleResumeAdmission::Resume(plan) => *plan,
        CompatibleResumeAdmission::StartNewTurnFromHistory { .. } => {
            panic!("a pending/released callback must retain its original execution")
        }
    }
}

struct PreclaimInvokerFailure {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl crate::routes::application_public_api::native_read_interface::NativeRuntimeInvokerFactory
    for PreclaimInvokerFailure
{
    fn without_mcp(
        &self,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        NativeApiError,
    > {
        panic!("callback resume must use the actor factory")
    }

    fn for_actor<'a>(
        &'a self,
        _: &'a ApplicationApiKeyActor,
    ) -> crate::routes::application_public_api::native_read_interface::RuntimeInvokerFuture<'a>
    {
        Box::pin(async move {
            self.entered.notify_one();
            self.release.notified().await;
            Err(service_error(anyhow::anyhow!(
                "controlled pre-claim invoker failure"
            )))
        })
    }
}

#[tokio::test]
async fn callback_preclaim_invoker_failure_retries_through_ingress_prepare() {
    let (state, mut dependencies, run, actor, mut command, service, _, _app) =
        fixture("open").await;
    let failure = Arc::new(PreclaimInvokerFailure {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    dependencies.native.runtime_invoker_factory = failure.clone();
    let acquired = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    let id = acquired.attempt_id();
    command.reserved_attempt_id = Some(id);
    let reservation = CompatibleCallbackReservation::new(acquired, service);
    // The controlled invoker factory fails at the first awaited host setup
    // operation. Neither callback consumption nor execution claim has happened.
    let execution = tokio::spawn(execute_compatible_resume_for_actor(
        dependencies.clone(),
        actor.clone(),
        command.clone(),
        Some(reservation),
        CompatibleResumeConsumer::Scoped,
    ));
    tokio::time::timeout(Duration::from_secs(3), failure.entered.notified())
        .await
        .expect("reserved request must reach the pre-claim invoker barrier");
    let before_failure: (String, String, i64) = sqlx::query_as(
        "select c.status,a.status,(select count(*) from flow_run_resume_claims r where r.callback_task_id=c.id) from flow_run_callback_tasks c join flow_run_callback_resume_attempts a on a.callback_task_id=c.id where c.id=$1",
    ).bind(callback_task_id_from_resume_command(&command)).fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(before_failure, ("pending".into(), "processing".into(), 0));
    failure.release.notify_one();
    let error = tokio::time::timeout(Duration::from_secs(3), execution)
        .await
        .unwrap()
        .unwrap();
    assert!(error.is_err());
    await_received(&state, id).await;
    let callback_id = callback_task_id_from_resume_command(&command);
    let unstarted: (String, i64, i64) = sqlx::query_as(
        "select c.status,(select count(*) from flow_run_resume_claims r where r.callback_task_id=c.id),(select count(*) from flow_run_tool_callback_inbox i where i.callback_task_id=c.id and i.status='received') from flow_run_callback_tasks c where c.id=$1",
    ).bind(callback_id).fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(unstarted, ("pending".into(), 0, 0));
    // A fresh request has no server-owned reservation ID. On the old code this
    // real ingress call fails with callback_resume_not_completed before CAS.
    command.reserved_attempt_id = None;
    let plan = ingress_retry_plan(&state, actor.clone(), &command).await;
    assert_eq!(plan.initial_run.id, run.id);
    assert_ne!(plan.initial_run.metadata["active_callback_attach"], true);
    assert_eq!(plan.command.reserved_attempt_id, Some(id));
    assert!(plan.reservation.as_ref().unwrap().is_acquired());
    // Continue through real CP prepared execution. This test's stream rejects
    // open before provider handoff; the PG sibling tests complete the consumer.
    let retried = open_compatible_turn_with_invoker(
        dependencies,
        plan.initial_run,
        CompatibleTurnAction::ResumeForActor {
            actor,
            command: plan.command,
            reservation: plan.reservation,
        },
        None,
        Arc::new(NoTools),
    )
    .await;
    assert!(retried.is_err());
    await_received(&state, id).await;
    assert_single_cleaned_claim(&state, callback_id).await;
    let accepted: serde_json::Value = sqlx::query_scalar(
        "select runtime_original_json(result_payload,raw_json_payloads,'result_payload') from flow_run_tool_callback_inbox where callback_task_id=$1",
    ).bind(callback_id).fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(accepted, command.response_payload["tool_results"][0]);
}

#[tokio::test]
async fn callback_open_and_subscribe_failure_release_own_receipt_for_immediate_retry() {
    for failure in ["open", "subscribe"] {
        let (state, dependencies, run, actor, mut command, service, stream, _app) =
            fixture(failure).await;
        let obsolete_generation = stream
            .local
            .terminal_writer(run.id)
            .await
            .unwrap()
            .generation_id();
        let acquired = service
            .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
            .await
            .unwrap();
        let id = acquired.attempt_id();
        command.reserved_attempt_id = Some(id);
        let mut reservation = CompatibleCallbackReservation::new(acquired, Arc::clone(&service));
        reservation
            .capture_previous_generation(stream.as_ref(), run.id)
            .await;
        let result = open_compatible_turn_with_invoker(
            dependencies,
            run.clone(),
            CompatibleTurnAction::ResumeForActor {
                command: command.clone(),
                actor: actor.clone(),
                reservation: Some(reservation),
            },
            None,
            Arc::new(NoTools),
        )
        .await;
        assert!(result.is_err());
        await_received(&state, id).await;
        assert_single_cleaned_claim(&state, callback_task_id_from_resume_command(&command)).await;
        if failure == "subscribe" {
            assert_ne!(
                stream
                    .local
                    .terminal_writer(run.id)
                    .await
                    .unwrap()
                    .generation_id(),
                obsolete_generation,
                "acquired retry opens a new generation, rather than reusing the stale live lane"
            );
            let events = stream.local.replay(run.id, None, 64).await.unwrap();
            assert_eq!(
                events.len(),
                1,
                "fresh acquisition skips old unowned failure replay"
            );
            assert_eq!(events[0].event_type, "flow_started");
            assert_eq!(
                events[0].payload["response_round_id"],
                run.metadata["response_round_id"]
            );
        }
        let plan = ingress_retry_plan(&state, actor, &command).await;
        let mut retry = plan
            .reservation
            .expect("ingress retry must own reservation");
        assert!(retry.is_acquired());
        assert_eq!(retry.attempt_id(), id);
        retry.release().await;
    }
}

#[tokio::test]
async fn cancelled_callback_preparation_releases_receipt_but_handoff_disarms_drop() {
    let (state, dependencies, run, actor, mut command, service, stream, _app) =
        fixture("cancel").await;
    let acquired = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    let id = acquired.attempt_id();
    command.reserved_attempt_id = Some(id);
    let mut reservation = CompatibleCallbackReservation::new(acquired, Arc::clone(&service));
    reservation
        .capture_previous_generation(stream.as_ref(), run.id)
        .await;
    let preparation = tokio::spawn(open_compatible_turn_with_invoker(
        dependencies,
        run,
        CompatibleTurnAction::ResumeForActor {
            command: command.clone(),
            actor: actor.clone(),
            reservation: Some(reservation),
        },
        None,
        Arc::new(NoTools),
    ));
    tokio::time::timeout(Duration::from_secs(3), stream.entered.notified())
        .await
        .expect("acquired preparation must bypass the old missing identity and reach opening");
    preparation.abort();
    let Err(error) = preparation.await else {
        panic!("preparation must be cancelled before handoff");
    };
    assert!(error.is_cancelled());
    await_received(&state, id).await;
    assert_single_cleaned_claim(&state, callback_task_id_from_resume_command(&command)).await;
    let plan = ingress_retry_plan(&state, actor, &command).await;
    let mut guard = plan
        .reservation
        .expect("ingress retry must own reservation");
    assert!(guard.is_acquired());
    assert_eq!(
        guard.attempt_id(),
        id,
        "cancellation retry must reuse its existing admission"
    );
    let cleanup = guard.release_ticket().unwrap();
    guard.handoff();
    drop(guard);
    let status: String =
        sqlx::query_scalar("select status from flow_run_callback_resume_attempts where id=$1")
            .bind(id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        status, "processing",
        "client drop after handoff must not release execution admission"
    );
    cleanup.release().await;
    let status: String =
        sqlx::query_scalar("select status from flow_run_callback_resume_attempts where id=$1")
            .bind(id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        status, "processing",
        "a pre-dispatch error observer must also honor the handoff"
    );
}

#[tokio::test]
async fn partial_tool_results_keep_waiting_receipt_without_opening_or_execution() {
    let (state, dependencies, run, actor, mut command, service, stream, _app) =
        fixture_with_tool_count("open", 2).await;
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.native_transport = None;
    command.response_payload["tool_results"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let callback_id = callback_task_id_from_resume_command(&command);
    let peer = state
        .store
        .claim_runtime_event_deliveries(&control_plane::ports::ClaimRuntimeEventDeliveriesInput {
            flow_run_id: run.id,
            scope: RuntimeEventDeliveryScope::Start,
            include_membership: true,
            limit: 1,
            lease_seconds: 30,
        })
        .await
        .unwrap();
    assert_eq!(peer.claims.len(), 1);
    let before = delivery_rows(&state, run.id).await;
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut terminal_dependencies = super::delivery_proof::terminal_dependencies(&state);
    terminal_dependencies.delivery_read_attempts = Some(attempts.clone());
    let prior_generation = stream
        .local
        .terminal_writer(run.id)
        .await
        .unwrap()
        .generation_id();
    let acquired = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    command.reserved_attempt_id = Some(acquired.attempt_id());
    let mut reservation = CompatibleCallbackReservation::new(acquired, service);
    reservation
        .capture_previous_generation(stream.as_ref(), run.id)
        .await;
    let opened = open_compatible_turn_with_invoker(
        dependencies.clone(),
        run.clone(),
        CompatibleTurnAction::ResumeForActor {
            command: command.clone(),
            actor: actor.clone(),
            reservation: Some(reservation),
        },
        None,
        Arc::new(NoTools),
    )
    .await
    .expect("partial tool results must return their waiting receipt despite a rejecting open port");
    assert_eq!(opened.initial_run.status, NativeRunStatus::Waiting);
    assert_eq!(
        opened.delivery_scope,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback_id
        },
        "NoExecution must keep the command identity even when ignored waiting ID is absent"
    );
    assert!(opened.ignored_waiting_callback_task_id.is_none());
    assert_eq!(
        stream
            .local
            .terminal_writer(run.id)
            .await
            .unwrap()
            .generation_id(),
        prior_generation,
        "a typed waiting receipt cannot retire or open a generation"
    );
    let projected =
        project_opened_compatible_turn(terminal_dependencies, opened, native_receipt_consumer())
            .unwrap();
    let events = tokio::time::timeout(
        Duration::from_secs(3),
        collect_native_waiting(projected, callback_id),
    )
    .await
    .expect("read-only partial waiting projection must finish");
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(delivery_rows(&state, run.id).await, before);
    let inbox: Vec<(String,String)>=sqlx::query_as("select tool_call_id,status from flow_run_tool_callback_inbox where callback_task_id=$1 order by tool_ordinal")
        .bind(callback_id).fetch_all(state.store.pool()).await.unwrap();
    assert_eq!(
        inbox,
        vec![
            ("call_fixture".into(), "received".into()),
            ("call_pending".into(), "pending".into())
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "waiting_callback")
            .count(),
        1
    );
    let claims: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where callback_task_id=$1")
            .bind(callback_id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        claims, 0,
        "partial tool results cannot acquire an execution claim"
    );
    let markers: i64 = sqlx::query_scalar(
        "select count(*) from runtime_events where flow_run_id=$1 and event_type='flow_started'",
    )
    .bind(run.id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        markers, 0,
        "waiting receipt projection must not invent a durable producer marker"
    );
}

#[tokio::test]
async fn repeated_partial_native_receipt_keeps_inbox_and_rejects_incompatible_bindings() {
    use crate::routes::application_public_api::compatibility_interface as binding;
    let (state, dependencies, run, actor, command, service, stream, _app) =
        fixture_with_tool_count("open", 2).await;
    let command = partial_native_command(command);
    let callback = callback_task_id_from_resume_command(&command);
    let before = delivery_rows(&state, run.id).await;
    let generation = stream
        .local
        .terminal_writer(run.id)
        .await
        .unwrap()
        .generation_id();
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut first_inbox = None;
    for _ in 0..2 {
        let opened = open_partial(
            dependencies.clone(),
            run.clone(),
            actor.clone(),
            command.clone(),
            service.clone(),
            stream.clone(),
        )
        .await;
        assert!(matches!(
            &opened.projection,
            CompatibleTurnProjection::PendingNativeWaiting(_)
        ));
        let mut terminal = super::delivery_proof::terminal_dependencies(&state);
        terminal.delivery_read_attempts = Some(attempts.clone());
        let projected =
            project_opened_compatible_turn(terminal, opened, native_receipt_consumer()).unwrap();
        collect_native_waiting(projected, callback).await;
        let inbox:Value=sqlx::query_scalar("select jsonb_agg(to_jsonb(i) order by tool_ordinal) from flow_run_tool_callback_inbox i where callback_task_id=$1")
            .bind(callback).fetch_one(state.store.pool()).await.unwrap();
        if let Some(original) = &first_inbox {
            assert_eq!(&inbox, original, "duplicate c1 has zero new inbox writes");
        }
        first_inbox = Some(inbox);
        let status: String = sqlx::query_scalar(
            "select status from flow_run_callback_resume_attempts where callback_task_id=$1",
        )
        .bind(callback)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
        assert_eq!(status, "received");
    }
    // Native source/type alone grants no right to Responses/Chat/Anthropic fallback.
    for binding in [
        binding::OPENAI_RESPONSES_STREAM_BINDING_ID,
        binding::OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
        binding::OPENAI_CHAT_STREAM_BINDING_ID,
        binding::ANTHROPIC_MESSAGES_STREAM_BINDING_ID,
    ] {
        let opened = open_partial(
            dependencies.clone(),
            run.clone(),
            actor.clone(),
            command.clone(),
            service.clone(),
            stream.clone(),
        )
        .await;
        let error = match opened_compatible_typed_stream_for_consumer(
            dependencies.clone(),
            opened,
            CompatibleResumeConsumer::for_binding(binding),
        ) {
            Ok(_) => panic!("incompatible consumer must not receive old executable calls"),
            Err(error) => error,
        };
        assert_eq!(error.code, "native_waiting_receipt_consumer_required");
    }
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(delivery_rows(&state, run.id).await, before);
    assert_eq!(
        stream
            .local
            .terminal_writer(run.id)
            .await
            .unwrap()
            .generation_id(),
        generation
    );
    let claims: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where flow_run_id=$1")
            .bind(run.id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(claims, 0);
}

async fn complete_original_callback(
    state: &ApiState,
    run: &NativeRunResult,
    full_command: &ResumePublishedCallbackCommand,
) {
    let callback = callback_task_id_from_resume_command(full_command);
    let checkpoint:Uuid=sqlx::query_scalar("select c.id from flow_run_checkpoints c join flow_run_recovery_history h on h.flow_run_id=c.flow_run_id and c.locator_payload->>'context_version_id'=h.context_version_id::text where h.flow_run_id=$1 and h.idempotency_key=$2")
        .bind(run.id).bind(format!("waiting_callback:{callback}")).fetch_one(state.store.pool()).await.unwrap();
    let scope_id: Uuid = sqlx::query_scalar("select scope_id from flow_runs where id=$1")
        .bind(run.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let results = full_command.response_payload["tool_results"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .map(control_plane::ports::ToolCallbackResultInput::from_payload)
        .collect::<anyhow::Result<Vec<_>>>()
        .unwrap();
    let completed = state
        .store
        .commit_tool_callback_results(&control_plane::ports::CommitToolCallbackResultsInput {
            responses_continuation: None,
            scope_id,
            application_id: run.application_id,
            flow_run_id: run.id,
            checkpoint_id: checkpoint,
            callback_task_id: callback,
            results,
        })
        .await
        .unwrap();
    assert_eq!(
        completed.callback_task.status,
        domain::CallbackTaskStatus::Completed
    );
    assert!(completed.claim.is_some());
}

#[tokio::test]
async fn pending_native_receipt_freezes_a_after_real_successor_b_commits() {
    let (state, dependencies, run, actor, full, service, stream, _app) =
        fixture_with_tool_count("open", 2).await;
    let callback = callback_task_id_from_resume_command(&full);
    let opened = open_partial(
        dependencies.clone(),
        run.clone(),
        actor,
        partial_native_command(full.clone()),
        service,
        stream.clone(),
    )
    .await;
    let frozen = opened.initial_run.required_action.clone();
    complete_original_callback(&state, &run, &full).await;
    let next = super::delivery_proof::commit_wait(
        &state,
        &run,
        &[committed_delivery_payload("successor-b")],
    )
    .await;
    assert_ne!(next.callback_task.as_ref().unwrap().id, callback);
    let before = delivery_rows(&state, run.id).await;
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut terminal = super::delivery_proof::terminal_dependencies(&state);
    terminal.delivery_read_attempts = Some(attempts.clone());
    let projected =
        project_opened_compatible_turn(terminal, opened, native_receipt_consumer()).unwrap();
    assert_eq!(projected.initial_run.required_action, frozen);
    let events = collect_native_waiting(projected, callback).await;
    assert_eq!(events[0].payload["callback_task_id"], json!(callback));
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        delivery_rows(&state, run.id).await,
        before,
        "projection must not read/reclaim successor B"
    );
}

#[tokio::test]
async fn blocking_native_partial_receipt_and_duplicate_do_not_execute_or_settle_delivery() {
    let (state, mut dependencies, run, actor, command, service, stream, _app) =
        fixture_with_tool_count("open", 2).await;
    dependencies.native.runtime_invoker_factory = Arc::new(NoToolsFactory);
    let command = partial_native_command(command);
    let callback = callback_task_id_from_resume_command(&command);
    let before = delivery_rows(&state, run.id).await;
    let generation = stream
        .local
        .terminal_writer(run.id)
        .await
        .unwrap()
        .generation_id();
    for _ in 0..2 {
        let acquired = service
            .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
            .await
            .unwrap();
        let mut reservation = CompatibleCallbackReservation::new(acquired, service.clone());
        reservation
            .capture_previous_generation(stream.as_ref(), run.id)
            .await;
        let result = execute_compatible_resume_for_actor(
            dependencies.clone(),
            actor.clone(),
            command.clone(),
            Some(reservation),
            native_receipt_consumer(),
        )
        .await
        .unwrap();
        assert_eq!(result.status, NativeRunStatus::Waiting);
        assert_eq!(result.metadata["response_round_id"], json!(callback));
        assert_eq!(
            result.required_action.unwrap().payload["callback_task_id"],
            json!(callback)
        );
    }
    let acquired = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap();
    let error = execute_compatible_resume_for_actor(
        dependencies,
        actor,
        command,
        Some(CompatibleCallbackReservation::new(acquired, service)),
        CompatibleResumeConsumer::Scoped,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "native_waiting_receipt_consumer_required");
    assert_eq!(delivery_rows(&state, run.id).await, before);
    assert_eq!(
        stream
            .local
            .terminal_writer(run.id)
            .await
            .unwrap()
            .generation_id(),
        generation
    );
    let claims: i64 =
        sqlx::query_scalar("select count(*) from flow_run_resume_claims where flow_run_id=$1")
            .bind(run.id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(claims, 0);
}

#[tokio::test]
async fn ordinary_open_empty_waiting_still_requires_closed_executable_proof() {
    let (state, _, mut run, _, command, _, _, _app) = fixture_with_tool_count("open", 2).await;
    let callback = callback_task_id_from_resume_command(&command);
    freeze_delivery_scope(
        &mut run,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback,
        },
    );
    let (tx, live) = mpsc::unbounded_channel();
    tx.send(RuntimeEventEnvelope::new(
        run.id,
        20,
        RuntimeEventPayload {
            event_type: "waiting_callback".into(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::Ephemeral,
            persist_required: false,
            trace_visible: true,
            payload: json!({"callback_task_id":callback}),
        },
    ))
    .unwrap();
    drop(tx);
    let (sender, receiver) = mpsc::channel(8);
    let task = spawn_scoped_typed_stream(
        &state,
        &run,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback,
        },
        Vec::new(),
        Vec::new(),
        live,
        sender,
    );
    let (events, claims) = super::delivery_proof::collect_projection(receiver).await;
    task.await.unwrap();
    assert_eq!(claims, 0);
    super::response_scoped_delivery::assert_protocol_failure(
        &events,
        "runtime_event_delivery_proof_unavailable",
    );
}

#[tokio::test]
async fn native_attach_keeps_llm_required_action_and_native_resume_owner_checks() {
    use control_plane::application_public_api::run_service::ApplicationPublishedRunControlRepository;
    use control_plane::application_public_api::{
        callback_resume::{PreparedPublishedCallback, PublishedCallbackReservation},
        native::ApplicationNativeRunService,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (state, dependencies, run, actor, mut command, service, stream, _app) =
        fixture_with_tool_count("open", 2).await;
    let callback_id = callback_task_id_from_resume_command(&command);
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.native_transport = None;
    command.response_payload["tool_results"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    command.target = PublishedCallbackResumeTarget::FlowRun {
        flow_run_id: run.id,
        callback_task_id: callback_id,
    };
    let callback = state
        .store
        .get_published_callback_task(callback_id)
        .await
        .unwrap()
        .unwrap();
    let actual = ApplicationNativeRunService::new(state.store.clone())
        .get_native_run_for_actor(actor.clone(), run.id)
        .await
        .unwrap();
    assert!(
        actual.metadata["response_round_id"].is_null(),
        "real ordinary Native read has no Responses identity"
    );
    let expected = debug_stream_events::waiting_callback_with_task(
        run.id,
        callback.node_run_id,
        "fixture",
        &callback,
    );
    stream.append(run.id, expected.clone()).await.unwrap();
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut terminal_dependencies = native::native_run_terminal_dependencies(&dependencies.native);
    terminal_dependencies.delivery_read_attempts = Some(attempts.clone());
    let mut attached =
        start_native_run_attachment_stream(terminal_dependencies, stream, actual, None)
            .await
            .unwrap();
    let (snapshot, event) = attached.next_native().await.unwrap();
    assert_eq!(event.event_type, "waiting_callback");
    let (_, payload) =
        crate::routes::application_public_api::sse::native_sse_payload_for_runtime_event(
            &snapshot,
            crate::routes::application_public_api::sse::IncludeWorkflowEvents::Public,
            event,
        )
        .unwrap();
    let payload = serde_json::to_value(payload).unwrap();
    assert_eq!(
        payload["required_action"],
        expected.payload["required_action"]
    );
    assert_eq!(
        payload["required_action"]["action_type"],
        "submit_tool_outputs"
    );
    assert_eq!(
        payload["required_action"]["payload"]["callback_task_id"],
        json!(callback_id)
    );
    assert_eq!(
        payload["required_action"]["payload"]["flow_run_id"],
        json!(run.id)
    );
    assert_eq!(
        payload["required_action"]["payload"]["tool_calls"],
        callback.request_payload["tool_calls"]
    );
    assert!(attached.next_native().await.is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    // Native WS run.resume delegates these exact FlowRun/NativeAgent commands to
    // this CP owner; no Responses identity can replace its authorization checks.
    for bad_field in ["run", "app", "key", "creator"] {
        let mut bad_actor = actor.clone();
        let mut bad_command = command.clone();
        match bad_field {
            "run" => {
                bad_command.target = PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id: Uuid::now_v7(),
                    callback_task_id: callback_id,
                }
            }
            "app" => bad_actor.application_id = Uuid::now_v7(),
            "key" => bad_actor.api_key_id = Uuid::now_v7(),
            "creator" => bad_actor.creator_user_id = Uuid::now_v7(),
            _ => unreachable!(),
        }
        let error = service
            .prepare_callback_resume_for_actor(bad_actor, &bad_command)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                Some(control_plane::errors::ControlPlaneError::PermissionDenied(
                    _
                ))
            ),
            "{bad_field}: {error}"
        );
    }
    assert!(matches!(
        service
            .prepare_callback_resume_for_actor(actor.clone(), &command)
            .await
            .unwrap(),
        PreparedPublishedCallbackResume::NativeToolResume { .. }
    ));
    let PublishedCallbackReservation::Acquired(receipt) = service
        .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
        .await
        .unwrap()
    else {
        panic!("new owned Native receipt must reserve");
    };
    command.reserved_attempt_id = Some(receipt.attempt_id());
    let prepared = service
        .prepare_reserved_native_callback_for_actor(actor, command, receipt)
        .await
        .unwrap();
    let PreparedPublishedCallback::PendingNativeWaiting(receipt) = prepared else {
        panic!("one of two Native tool results cannot execute a model or tool");
    };
    let result = receipt.into_result();
    assert_eq!(result.run.id, run.id);
    assert_eq!(result.run.status, NativeRunStatus::Waiting);
    let action = result.run.required_action.unwrap();
    assert_eq!(action.action_type, "submit_tool_outputs");
    assert_eq!(action.payload["callback_task_id"], json!(callback_id));
    assert_eq!(action.payload["flow_run_id"], json!(run.id));
    let inbox:Vec<(String,String)>=sqlx::query_as("select tool_call_id,status from flow_run_tool_callback_inbox where callback_task_id=$1 order by tool_ordinal")
        .bind(callback_id).fetch_all(state.store.pool()).await.unwrap();
    assert_eq!(
        inbox,
        vec![
            ("call_fixture".into(), "received".into()),
            ("call_pending".into(), "pending".into())
        ]
    );
}

#[tokio::test]
async fn completed_a_waiting_b_no_execution_keeps_ordinary_scoped_projection() {
    use control_plane::application_public_api::{
        callback_resume::ResumePublishedCallbackResult, native::ApplicationNativeRunService,
    };
    let (state, dependencies, run, actor, full, _, _, _app) =
        fixture_with_tool_count("open", 2).await;
    let callback = callback_task_id_from_resume_command(&full);
    complete_original_callback(&state, &run, &full).await;
    let generation = Uuid::now_v7();
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "runtime_stream_opened",
        json!({"stream_generation_id":generation}),
    )
    .await;
    append_compat_sse_runtime_event(
        &state,
        run.id,
        "flow_started",
        json!({"stream_generation_id":generation,"response_round_id":callback}),
    )
    .await;
    let payload = super::delivery_proof::exact_payload(900);
    let next = super::delivery_proof::commit_wait(&state, &run, &[payload.clone()]).await;
    let actual = ApplicationNativeRunService::new(state.store.clone())
        .get_native_run_for_actor(actor, run.id)
        .await
        .unwrap();
    assert_eq!(actual.status, NativeRunStatus::Waiting);
    assert_eq!(
        actual.required_action.as_ref().unwrap().payload["callback_task_id"],
        json!(next.callback_task.as_ref().unwrap().id)
    );
    // Ordinary prepared results are public data, never the private CP receipt.
    // Feed the real Waiting-B snapshot to the SAME ordinary Host branch.
    let attempt = state
        .store
        .record_flow_run_callback_resume_attempt(
            &control_plane::ports::RecordFlowRunCallbackResumeAttemptInput {
                flow_run_id: run.id,
                callback_task_id: callback,
                source: "native_agent".into(),
                response_payload: full.response_payload,
                idempotency_key: format!("callback_task:{callback}"),
            },
        )
        .await
        .unwrap()
        .attempt;
    let result = ResumePublishedCallbackResult {
        run: actual,
        attempt,
    };
    let opened = callback_execution::open_compatible_no_execution_replay(
        &dependencies,
        &run,
        result,
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: callback,
        },
        Some(callback),
    )
    .await
    .unwrap();
    assert!(matches!(
        &opened.projection,
        CompatibleTurnProjection::Scoped { .. }
    ));
    assert_eq!(opened.ignored_waiting_callback_task_id, Some(callback));
    let typed = opened_compatible_typed_stream(dependencies, opened).unwrap();
    let (events, claims) = super::delivery_proof::collect_projection(typed.events).await;
    super::delivery_proof::assert_exact_response(&events, &[payload], callback);
    assert_eq!(
        claims, 1,
        "ordinary result must acquire B's actual scoped tool receipt"
    );
}

#[path = "reservation_lifecycle/native_ingress.rs"]
mod native_ingress;
