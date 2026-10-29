use super::*;
use control_plane::application_public_api::{
    api_keys::ApplicationApiKeyActor,
    callback_resume::{PublishedCallbackResumeSource, PublishedCallbackResumeTarget},
    native_tool_resume::correlate_native_responses_callback,
};
use control_plane::ports::{
    CreateCallbackTaskInput, CreateCheckpointInput, ProviderTransportPayload,
    RuntimeEventSubscription, RuntimeEventTrimPolicy,
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

struct PreparationStream {
    local: LocalRuntimeEventStream,
    failure: &'static str,
    entered: tokio::sync::Notify,
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
) {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
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
    let response_id = format!("resp_reservation_fixture_{}", run.id);
    let request = json!({"model":"fixture","stream":true,"store":false,
        "previous_response_id":response_id,
        "input":[{"type":"function_call_output","call_id":"call_fixture","output":"fixture-result"}]});
    let native_transport = ProviderTransportPayload::openai_responses(request.clone()).unwrap();
    let callback = state
        .store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id: run.id,
            node_run_id: node.id,
            callback_kind: "llm_tool_calls".into(),
            request_payload: json!({"tool_calls": [{"id":"call_fixture", "name":"fixture", "arguments":{}}],
                "provider_metadata":{"native_response":{
                    "response_id":response_id,
                    "configuration_digest":native_transport.configuration_digest().unwrap(),
                    "user_messages_digest":native_transport.user_messages_digest().unwrap()
                }}}),
            external_ref_payload: None,
        })
        .await
        .unwrap();
    // Use the real CP prepare path and PostgreSQL claim transaction. Setup
    // failures stop before execution, so no provider process is installed.
    sqlx::query("update flow_compiled_plans p set plan=jsonb_build_object('flow_id',p.flow_id,'source_draft_id',p.flow_draft_id::text,'schema_version','1flowbase.flow/v2','topological_order',jsonb_build_array('fixture'),'nodes',$2::jsonb) from flow_runs r where r.id=$1 and p.id=r.compiled_plan_id")
        .bind(run.id)
        .bind(json!({"fixture":{"node_id":"fixture","node_type":"llm","alias":"Fixture","container_id":null,"dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],"config":{},"llm_runtime":{"provider_instance_id":Uuid::now_v7(),"provider_code":"fixture","protocol":"openai_compatible","model":"fixture"}}}))
        .execute(state.store.pool()).await.unwrap();
    state.store.create_checkpoint(&CreateCheckpointInput {
        flow_run_id:run.id, node_run_id:Some(node.id), status:"waiting_callback".into(), reason:"llm_tool_calls".into(),
        locator_payload:json!({"node_id":"fixture","next_node_index":0,"active_node_ids":["fixture"]}),
        variable_snapshot:json!({"fixture":{"__llm_tool_callback":{"callback_kind":"llm_tool_calls","pending_tool_calls":[{"id":"call_fixture","name":"fixture","arguments":{}}],"system":[],"history":[{"role":"assistant","content":"","tool_calls":[{"id":"call_fixture","name":"fixture","arguments":{}}]}]}}}),
        external_ref_payload:None,
    }).await.unwrap();
    sqlx::query("insert into flow_run_tool_callback_inbox (id,scope_id,application_id,flow_run_id,callback_task_id,tool_call_id,tool_ordinal) select $1,a.workspace_id,a.id,$2,$3,'call_fixture',0 from applications a where a.id=$4")
        .bind(Uuid::now_v7()).bind(run.id).bind(callback.id).bind(run.application_id)
        .execute(state.store.pool()).await.unwrap();
    run.metadata["response_round_id"] = json!(callback.id);
    let (correlated_callback, response_payload) =
        correlate_native_responses_callback(&state.store, &actor, &request)
            .await
            .unwrap()
            .expect("the real native Responses ingress must recognize this owned callback");
    assert_eq!(correlated_callback.id, callback.id);
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
    (state, dependencies, run, actor, command, service, stream)
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
    assert_eq!(counts, (1, 1, 1), "pre-execution cleanup retains one admission and one failed execution claim, without duplicate ownership");
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
    let (state, mut dependencies, run, actor, mut command, service, _) = fixture("open").await;
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
        let (state, dependencies, run, actor, mut command, service, stream) =
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
    let (state, dependencies, run, actor, mut command, service, stream) = fixture("cancel").await;
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
    let (state, dependencies, run, actor, mut command, service, stream) = fixture("open").await;
    // Partial native-agent deliveries retain their existing API semantics;
    // native Responses correlation above requires the complete owned call set.
    command.source = PublishedCallbackResumeSource::NativeAgent;
    command.native_transport = None;
    let callback_id = callback_task_id_from_resume_command(&command);
    sqlx::query("insert into flow_run_tool_callback_inbox (id,scope_id,application_id,flow_run_id,callback_task_id,tool_call_id,tool_ordinal) select $1,a.workspace_id,a.id,$2,$3,'call_pending',1 from applications a where a.id=$4")
        .bind(Uuid::now_v7()).bind(run.id).bind(callback_id).bind(run.application_id)
        .execute(state.store.pool()).await.unwrap();
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
            command,
            actor,
            reservation: Some(reservation),
        },
        None,
        Arc::new(NoTools),
    )
    .await
    .expect("partial tool results must return their waiting receipt despite a rejecting open port");
    assert_eq!(opened.initial_run.status, NativeRunStatus::Waiting);
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
    let mut projected = opened_compatible_typed_stream(dependencies, opened).unwrap();
    let events = tokio::time::timeout(Duration::from_secs(3), async {
        let mut events = Vec::new();
        while let Some(event) = projected.events.recv().await {
            let (_, envelope, _) = event.into_parts();
            events.push(envelope);
        }
        events
    })
    .await
    .expect("read-only partial waiting projection must finish");
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
