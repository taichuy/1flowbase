use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use plugin_framework::{
    provider_contract::{ProviderInvocationInput, ProviderModelDescriptor, ProviderStreamEvent},
    provider_package::ProviderPackage,
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

pub use extension_contracts::semantic_terminal::count_tokens_input_tokens_from_output_payload;

use crate::{
    application::{
        ensure_existing_application_non_crud_console_operation, ApplicationNonCrudConsoleOperation,
    },
    audit::audit_log,
    capability_plugin_runtime::{CapabilityPluginRuntimePort, ExecuteCapabilityNodeInput},
    errors::ControlPlaneError,
    model_provider::failover_queue::{freeze_queue_items, FailoverQueueSnapshotItem},
    plugin_management::ready_current_node_plugin_installation,
    ports::{
        AcquireResumeClaimInput, AppendRunEventInput, ApplicationJsDependencySelectionRepository,
        ApplicationRepository, BillingRepository, CacheStore, CallbackResumeWaitingNode,
        CommitFlowRunTerminalInput, CommitFlowRunTerminalResult, CommitToolCallbackResultsInput,
        CommitToolCallbackResultsOutput, CompleteCallbackTaskInput, FinishResumeClaimInput,
        FlowRepository, ModelDefinitionRepository, ModelProviderRepository,
        NodeContributionRepository, OrchestrationRuntimeRepository, PluginRepository,
        ProviderRuntimePort, RenewResumeClaimInput, ResumeClaimDisposition, ResumeClaimKind,
        ResumeClaimRecord, ResumeClaimStatus, RuntimeEventDurability, RuntimeEventEnvelope,
        RuntimeEventPayload, RuntimeEventStream, TaskQueue, ToolCallbackResultInput,
        ToolCallbackRoundDisposition, UpdateFlowRunInput, UpdateNodeRunInput,
    },
    state_transition::{ensure_flow_run_transition, ensure_node_run_transition},
};

mod answer_presentation;
pub(crate) use answer_presentation::is_canonical_answer_presentation_output;
pub mod callback_completion;
mod canonical_stream;
mod communication_dispatch;
mod communication_execution;
pub use communication_execution::{
    CommunicationExecutionBinding, CommunicationExecutionResolver, CommunicationInputPreparer,
    CommunicationReservedReply, CommunicationTurnExecutionBridge,
};
pub(crate) mod compile_context;
mod data_model_runtime;
#[path = "orchestration_runtime/debug/debug_artifacts.rs"]
pub mod debug_artifacts;
pub mod debug_stream_events;
#[path = "orchestration_runtime/debug/debug_variable_cache.rs"]
mod debug_variable_cache;
mod frozen_plan;
mod http_response_files;
pub(crate) mod inputs;
mod live_debug_run;
#[path = "orchestration_runtime/debug/llm_observability_refs.rs"]
mod llm_observability_refs;
mod payloads;
mod persistence;
mod provider_invoker;
mod provider_transport;
mod runtime_event_persister;
pub(crate) use runtime_event_persister::RUNTIME_EVENT_BATCH_MAX_DELAY;
pub mod scheduler_admission;
mod stream_terminal_recovery;
pub mod trace_projection;

pub use stream_terminal_recovery::FinalizePublishedRunMissingStreamTerminalCommand;

#[cfg(test)]
pub(crate) use provider_invoker::test_support;
#[cfg(test)]
pub(crate) use provider_transport::tests::TestProviderTransportStore;

use self::{
    compile_context::{ensure_compiled_plan_runnable, ensure_compiled_plan_runnable_for_node},
    debug_variable_cache::{persist_debug_variable_cache_entries, public_node_variable_cache},
    inputs::{
        build_compiled_plan_input, build_complete_flow_run_input, build_complete_node_run_input,
        build_flow_run_input, build_node_run_input,
    },
    payloads::persisted_node_output_payload,
    persistence::{
        checkpoint_node_id, checkpoint_snapshot_from_record_with_context, next_node_started_at,
        persist_flow_debug_outcome, persist_preview_events, PersistFlowDebugOutcomeInput,
        PreparedNodeRuns, WaitingNodeResumeUpdate,
    },
    provider_invoker::{
        freeze_failover_queue_routes, is_expected_runtime_event_stream_closed_error,
        DebugDeltaKind, ThinkTagStreamSplitter,
    },
};
pub struct StartNodeDebugPreviewCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub node_id: String,
    pub input_payload: serde_json::Value,
    pub document_snapshot: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

pub struct StartFlowDebugRunCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub input_payload: serde_json::Value,
    pub document_snapshot: Option<serde_json::Value>,
    pub debug_session_id: Option<String>,
}

pub struct PrepareFlowDebugRunCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub input_payload: serde_json::Value,
    pub document_snapshot: Option<serde_json::Value>,
    pub debug_session_id: String,
}

pub struct ContinueFlowDebugRunCommand {
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub workspace_id: Uuid,
}

pub struct StartPublishedFlowRunCommand {
    /// Private host execution state; never stored in the flow input or protocol envelope.
    pub transport_connection_scope: Option<String>,
    pub observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub provider_transport_slot: Option<crate::ports::ProviderTransportSlotId>,
}

#[derive(Debug, Clone)]
pub struct LiveProviderStreamEvent {
    pub node_id: String,
    pub node_run_id: Uuid,
    pub event: ProviderStreamEvent,
}

pub type LiveProviderStreamEventSender = mpsc::Sender<LiveProviderStreamEvent>;

#[derive(Debug, Clone, Copy)]
struct FirstTokenTiming {
    first_token_at: OffsetDateTime,
    time_to_first_token_ms: u64,
}

pub struct CancelFlowRunCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
}

pub struct ResumeFlowRunCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub checkpoint_id: Uuid,
    pub input_payload: serde_json::Value,
}

/// Host-private connection context, removed before RuntimeExtension dispatch.
pub const HOST_TRANSPORT_CONNECTION_SCOPE_HEADER: &str = "x-1flowbase-transport-connection-scope";

pub struct CompleteCallbackTaskCommand {
    pub responses_continuation:
        Option<crate::application_public_api::native_tool_resume::VerifiedResponsesContinuation>,
    pub transport_connection_scope: Option<String>,
    pub observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    pub native_transport: Option<crate::ports::ProviderTransportPayload>,
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub callback_task_id: Uuid,
    pub response_payload: serde_json::Value,
}

fn ensure_data_model_side_effect_confirmation_approved(response_payload: &Value) -> Result<()> {
    let approved = response_payload
        .get("approved")
        .or_else(|| response_payload.get("confirmed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if approved {
        Ok(())
    } else {
        Err(anyhow!(
            "DATA_MODEL_SIDE_EFFECT_CONFIRMATION_REJECTED: data_model write requires approved confirmation"
        ))
    }
}

fn ensure_data_model_side_effect_confirmation_metadata(
    actor: &domain::ActorContext,
    confirmation_payload: &Value,
) -> Result<()> {
    let expected_actor_user_id = confirmation_payload
        .get("actor_user_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("data_model side-effect confirmation actor is required"))
        .and_then(|value| Uuid::parse_str(value).map_err(Into::into))?;
    if expected_actor_user_id != actor.user_id {
        return Err(ControlPlaneError::PermissionDenied(
            "data_model_side_effect_confirmation_actor",
        )
        .into());
    }

    let expires_at = confirmation_payload
        .get("expires_at")
        .cloned()
        .ok_or_else(|| anyhow!("data_model side-effect confirmation expiry is required"))
        .and_then(|value| serde_json::from_value::<OffsetDateTime>(value).map_err(Into::into))?;
    if OffsetDateTime::now_utc() > expires_at {
        return Err(anyhow!(
            "DATA_MODEL_SIDE_EFFECT_CONFIRMATION_EXPIRED: data_model write confirmation expired"
        ));
    }

    Ok(())
}

pub async fn persist_runtime_debug_stream_events<R>(
    repository: &R,
    events: Vec<RuntimeEventEnvelope>,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    runtime_event_persister::persist_runtime_debug_stream_events(repository, events).await
}

pub async fn persist_runtime_debug_stream_events_with_after_commit<R>(
    repository: &R,
    events: Vec<RuntimeEventEnvelope>,
    after_commit: &crate::ports::RuntimeEventAfterCommitLane,
) -> Result<Vec<crate::ports::RuntimeEventAfterCommitReceipt>>
where
    R: OrchestrationRuntimeRepository,
{
    runtime_event_persister::persist_runtime_debug_stream_events_with_after_commit(
        repository,
        events,
        after_commit,
    )
    .await
}

pub async fn persist_runtime_event_payload_with_after_commit<R>(
    repository: &R,
    flow_run_id: Uuid,
    event: &crate::ports::RuntimeEventPayload,
    after_commit: &crate::ports::RuntimeEventAfterCommitLane,
) -> Result<Vec<crate::ports::RuntimeEventAfterCommitReceipt>>
where
    R: OrchestrationRuntimeRepository,
{
    runtime_event_persister::persist_runtime_event_payload_with_after_commit(
        repository,
        flow_run_id,
        event,
        after_commit,
    )
    .await
}

pub use runtime_event_persister::{
    project_runtime_event_stream_terminal, spawn_runtime_debug_event_persister,
    spawn_runtime_debug_event_persister_with_after_commit, start_runtime_debug_event_persister,
    wait_for_runtime_debug_event_persister,
};

#[derive(Clone)]
struct RuntimeProviderInvoker<R, H> {
    repository: R,
    runtime: H,
    workspace_id: Uuid,
    provider_secret_master_key: String,
    live_provider_events: Option<LiveProviderStreamEventSender>,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    flow_run_id: Option<Uuid>,
    provider_session_scope: Option<String>,
    active_node_id: Option<String>,
    active_node_run_id: Option<Uuid>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
    flow_execution_context: Option<Arc<RuntimeFlowExecutionContext>>,
    answer_presentation:
        Option<Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>>,
    transport_connection_scope_override: Option<Option<String>>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    provider_transport_store: Option<Arc<dyn crate::ports::ProviderTransportStore>>,
    provider_continuation: Option<crate::ports::ProviderContinuation>,
    response_round_id: Option<Uuid>,
    native_user_messages_digest: Option<String>,
    native_history: Option<Value>,
    model_pricing_cache_store: Option<Arc<dyn CacheStore>>,
}

struct RuntimeFlowExecutionContext {
    notification_sender: Option<Arc<dyn crate::notifications::NotificationSenderPort>>,
    user_account: Option<String>,
    require_provider_usage_for_billing: bool,
    active_node: Mutex<Option<RuntimeActiveNode>>,
    tool_delivery_events: Mutex<Vec<RuntimeEventPayload>>,
    data_model: RuntimeDataModelExecutionContext,
}

impl RuntimeFlowExecutionContext {
    fn take_tool_delivery_events(&self) -> Result<Vec<RuntimeEventPayload>> {
        let mut events = self
            .tool_delivery_events
            .lock()
            .map_err(|_| anyhow!("tool delivery buffer lock is poisoned"))?;
        Ok(std::mem::take(&mut *events))
    }
}

#[derive(Clone)]
struct RuntimeActiveNode {
    node_id: String,
    node_run_id: Uuid,
}

struct RuntimeDataModelExecutionContext {
    actor: domain::ActorContext,
    application_id: Uuid,
    draft_id: Uuid,
    flow_run_id: Uuid,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
}

struct ResumeExecutionSegmentInput<'a> {
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
    resumed_node_run: Option<crate::ports::CallbackResumeWaitingNode>,
    native_transport: Option<crate::ports::ProviderTransportPayload>,
    response_round_id: Option<Uuid>,
    resume_claim_id: Uuid,
    resume_claim_owner: Option<RenewResumeClaimInput>,
    actor: &'a domain::ActorContext,
    application: &'a domain::ApplicationRecord,
    flow_run: &'a domain::FlowRunRecord,
    compiled_plan: &'a orchestration_runtime::compiled_plan::CompiledPlan,
    snapshot: &'a orchestration_runtime::execution_state::CheckpointSnapshot,
    waiting_node_id: &'a str,
    waiting_node_run_id: Option<Uuid>,
    resume_payload: &'a Value,
}

struct ResumeExecutionSegmentOutput {
    _communication_lease: Option<
        Box<
            dyn orchestration_runtime::execution_engine::communication::CommunicationExecutionLease,
        >,
    >,
    outcome: orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    prepared_node_runs: PreparedNodeRuns,
    answer_presentation:
        Option<Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>>,
    tool_delivery_events: Vec<RuntimeEventPayload>,
}

pub struct OrchestrationRuntimeService<R, H> {
    communication_execution_resolver: Option<Arc<dyn CommunicationExecutionResolver>>,
    communication_execution: Option<(
        Uuid,
        Arc<orchestration_runtime::execution_engine::communication::CommunicationExecution>,
    )>,
    notification_sender: Option<Arc<dyn crate::notifications::NotificationSenderPort>>,
    repository: R,
    runtime: H,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    file_storage_registry: Option<Arc<storage_object::FileStorageDriverRegistry>>,
    llm_routing_counter_store:
        Option<Arc<dyn orchestration_runtime::execution_engine::LlmRoutingCounterStore>>,
    model_routing_cache_store: Option<Arc<dyn CacheStore>>,
    published_plan_cache: Option<Arc<dyn crate::ports::PublishedPlanCache>>,
    provider_secret_master_key: String,
    require_provider_usage_for_billing: bool,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    pub(super) provider_request_log_queue: Option<Arc<dyn TaskQueue>>,
    provider_transport_store: Arc<dyn crate::ports::ProviderTransportStore>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
    runtime_internal_tool_invoker:
        Option<Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>>,
}

pub(super) struct ApplicationRunContext {
    pub(super) actor: domain::ActorContext,
    pub(super) application: domain::ApplicationRecord,
}

impl<R, H> OrchestrationRuntimeService<R, H>
where
    R: ApplicationRepository
        + FlowRepository
        + OrchestrationRuntimeRepository
        + ModelDefinitionRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone,
{
    pub fn new(
        repository: R,
        runtime: H,
        runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
        provider_secret_master_key: impl Into<String>,
        provider_transport_store: Arc<dyn crate::ports::ProviderTransportStore>,
        require_provider_usage_for_billing: bool,
    ) -> Self {
        Self {
            repository,
            communication_execution: None,
            communication_execution_resolver: None,
            runtime,
            runtime_engine,
            notification_sender: None,
            file_storage_registry: None,
            llm_routing_counter_store: None,
            model_routing_cache_store: None,
            published_plan_cache: None,
            provider_secret_master_key: provider_secret_master_key.into(),
            require_provider_usage_for_billing,
            runtime_event_stream: None,
            provider_request_log_queue: None,
            provider_transport_store,
            api_node_id: None,
            provider_install_root: None,
            runtime_internal_tool_invoker: None,
        }
    }

    pub fn with_node_artifact_context(
        mut self,
        node_id: impl Into<String>,
        install_root: impl Into<PathBuf>,
    ) -> Self {
        self.api_node_id = Some(node_id.into());
        self.provider_install_root = Some(install_root.into());
        self
    }

    pub fn with_notification_sender(
        mut self,
        sender: Arc<dyn crate::notifications::NotificationSenderPort>,
    ) -> Self {
        self.notification_sender = Some(sender);
        self
    }

    pub fn with_file_storage_registry(
        mut self,
        registry: Arc<storage_object::FileStorageDriverRegistry>,
    ) -> Self {
        self.file_storage_registry = Some(registry);
        self
    }

    pub fn with_runtime_internal_tool_invoker(
        mut self,
        invoker: Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
    ) -> Self {
        self.runtime_internal_tool_invoker = Some(invoker);
        self
    }

    pub fn with_llm_routing_counter_store(mut self, cache_store: Arc<dyn CacheStore>) -> Self {
        self.model_routing_cache_store = Some(cache_store.clone());
        self.llm_routing_counter_store =
            Some(Arc::new(CacheStoreLlmRoutingCounterStore { cache_store }));
        self
    }

    pub fn with_runtime_event_stream(mut self, stream: Arc<dyn RuntimeEventStream>) -> Self {
        self.runtime_event_stream = Some(stream);
        self
    }

    pub fn with_provider_request_log_queue(mut self, queue: Arc<dyn TaskQueue>) -> Self {
        self.provider_request_log_queue = Some(queue);
        self
    }

    pub(super) fn execution_runtime_context(
        &self,
        plan: &orchestration_runtime::compiled_plan::CompiledPlan,
        variable_pool: &serde_json::Map<String, Value>,
    ) -> Result<orchestration_runtime::execution_engine::ExecutionRuntimeContext> {
        let context =
            orchestration_runtime::execution_engine::ExecutionRuntimeContext::from_plan_input(
                plan,
                variable_pool,
            )?;
        let context = match &self.llm_routing_counter_store {
            Some(store) => context.with_llm_routing_counter_store(store.clone()),
            None => context,
        };
        Ok(match &self.runtime_internal_tool_invoker {
            Some(invoker) => context.with_runtime_internal_tool_invoker(invoker.clone()),
            None => context,
        })
    }

    fn runtime_flow_execution_context(
        &self,
        actor: domain::ActorContext,
        user_account: Option<String>,
        application_id: Uuid,
        draft_id: Uuid,
        flow_run_id: Uuid,
        active_node: Option<RuntimeActiveNode>,
    ) -> Arc<RuntimeFlowExecutionContext> {
        Arc::new(RuntimeFlowExecutionContext {
            notification_sender: self.notification_sender.clone(),
            user_account,
            require_provider_usage_for_billing: self.require_provider_usage_for_billing,
            active_node: Mutex::new(active_node),
            tool_delivery_events: Mutex::new(Vec::new()),
            data_model: RuntimeDataModelExecutionContext {
                actor,
                application_id,
                draft_id,
                flow_run_id,
                runtime_engine: self.runtime_engine.clone(),
            },
        })
    }

    fn runtime_invoker(&self, workspace_id: Uuid) -> RuntimeProviderInvoker<R, H> {
        RuntimeProviderInvoker {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: None,
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: None,
            active_node_id: None,
            active_node_run_id: None,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            provider_session_scope: None,
            flow_execution_context: None,
            answer_presentation: None,
            transport_connection_scope_override: None,
            observation_context: None,
            provider_transport_payload: None,
            provider_transport_store: Some(self.provider_transport_store.clone()),
            provider_continuation: None,
            response_round_id: None,
            native_user_messages_digest: None,
            native_history: None,
            model_pricing_cache_store: self.model_routing_cache_store.clone(),
        }
    }

    fn runtime_invoker_with_live_provider_events(
        &self,
        workspace_id: Uuid,
        live_provider_events: LiveProviderStreamEventSender,
    ) -> RuntimeProviderInvoker<R, H> {
        RuntimeProviderInvoker {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: Some(live_provider_events),
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: None,
            active_node_id: None,
            active_node_run_id: None,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            provider_session_scope: None,
            flow_execution_context: None,
            answer_presentation: None,
            transport_connection_scope_override: None,
            observation_context: None,
            provider_transport_payload: None,
            provider_transport_store: Some(self.provider_transport_store.clone()),
            provider_continuation: None,
            response_round_id: None,
            native_user_messages_digest: None,
            native_history: None,
            model_pricing_cache_store: self.model_routing_cache_store.clone(),
        }
    }

    async fn resume_execution_segment(
        &self,
        input: ResumeExecutionSegmentInput<'_>,
    ) -> Result<ResumeExecutionSegmentOutput>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        let flow_execution_context = self.runtime_flow_execution_context(
            input.actor.clone(),
            input.flow_run.authorized_account.clone(),
            input.application.id,
            input.flow_run.draft_id,
            input.flow_run.id,
            input
                .waiting_node_run_id
                .map(|node_run_id| RuntimeActiveNode {
                    node_id: input.waiting_node_id.to_string(),
                    node_run_id,
                }),
        );
        let lifecycle = live_debug_run::PersistedNodeLifecycle::new(
            self,
            input.flow_run.id,
            flow_execution_context.clone(),
        )
        .with_resumed_node_run(
            input
                .resumed_node_run
                .map(|record| (input.waiting_node_id.to_string(), record)),
        );
        let lifecycle = if let Some(owner) = input.resume_claim_owner.clone() {
            lifecycle.with_resume_claim_owner(owner)
        } else {
            lifecycle
        };
        let invoker = self
            .runtime_invoker(input.application.workspace_id)
            .for_flow_run(input.flow_run.id)
            .with_provider_session_scope(input.flow_run)
            .with_flow_execution_context(flow_execution_context.clone());
        let provider_continuation = if orchestration_runtime::execution_engine::pending_llm_tool_callback_requires_ephemeral_provider_continuation(
            &input.snapshot.variable_pool,
            input.waiting_node_id,
        ) {
            let store = &self.provider_transport_store;
            Some(
                store
                    .claim_continuation(input.flow_run.id, input.resume_claim_id)
                    .await?,
            )
        } else {
            None
        };
        let transport = match (input.native_transport, provider_continuation.as_ref()) {
            (Some(payload), Some(continuation))
                if payload.wire_body().get("previous_response_id").is_some() =>
            {
                Some(payload.bind_openai_continuation(continuation.clone())?)
            }
            (Some(payload), Some(continuation)) => {
                Some(payload.with_affinity(continuation.affinity().clone()))
            }
            (Some(payload), None) if payload.is_preferred() => Some(payload),
            (Some(_), None) => return Err(anyhow!("ephemeral_continuation_missing")),
            (None, _) => None,
        };
        let transport = transport.map(|payload| {
            crate::application_public_api::native::NativeExecutionModelParameters::seal_published_reasoning_default(
                &input.flow_run.input_payload, payload,
            )
        }).transpose()?;
        let requires_native_transport = transport
            .as_ref()
            .is_some_and(|payload| !payload.is_preferred());
        let mut invoker = invoker
            .with_provider_continuation(if requires_native_transport {
                None
            } else {
                provider_continuation
            })
            .with_provider_transport_payload(transport)
            .with_transport_connection_scope_override(input.transport_connection_scope)
            .with_observation_context(input.observation_context);
        invoker.response_round_id = input.response_round_id;
        invoker.native_history = input
            .snapshot
            .variable_pool
            .get(input.waiting_node_id)
            .and_then(|node| {
                node.pointer("/__llm_tool_callback/provider_metadata/native_response/history")
            })
            .filter(|value| value.is_object())
            .cloned();
        invoker.native_user_messages_digest = input
            .snapshot
            .variable_pool
            .get(input.waiting_node_id)
            .and_then(|node| {
                node.pointer(
                    "/__llm_tool_callback/provider_metadata/native_response/user_messages_digest",
                )
            })
            .and_then(Value::as_str)
            .map(str::to_owned);
        let answer_presentation = answer_presentation::AnswerPresentationCursor::from_plan(
            input.compiled_plan,
        )
        .map(|mut cursor| {
            for node_id in &input.compiled_plan.topological_order {
                if node_id == input.waiting_node_id {
                    break;
                }
                if let Some(output_payload) = input.snapshot.variable_pool.get(node_id) {
                    let _ = cursor.complete_node_with_run_id(node_id, None, output_payload);
                }
            }
            Arc::new(tokio::sync::Mutex::new(cursor))
        });
        let invoker = match &answer_presentation {
            Some(answer_presentation) => {
                invoker.with_answer_presentation(answer_presentation.clone())
            }
            None => invoker,
        };
        let mut runtime_context =
            self.execution_runtime_context(input.compiled_plan, &input.snapshot.variable_pool)?;
        runtime_context = self
            .attach_provider_protocol_context(
                input.flow_run.id,
                &input.flow_run.input_payload,
                runtime_context,
            )
            .await;
        if requires_native_transport {
            runtime_context = runtime_context.with_provider_invocation_capability(
                extension_contracts::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
            );
        }
        if let Some(http_file_persister) = self.http_response_file_persister(input.actor.clone()) {
            runtime_context =
                runtime_context.with_http_response_file_persister(Arc::new(http_file_persister));
        }

        runtime_context = self
            .attach_communication_execution(input.flow_run, runtime_context)
            .await?;
        let communication_lease = match runtime_context.communication_execution() {
            Some(execution) => Some(execution.enter_execution().await?),
            None => None,
        };
        let execution = Box::pin(async {
            lifecycle
                .restore_communication_nodes(
                    input.application.id,
                    input.waiting_node_id,
                    &runtime_context,
                )
                .await?;
            let outcome = orchestration_runtime::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            input.compiled_plan,
            input.snapshot,
            input.waiting_node_id,
            input.resume_payload,
            runtime_context.clone(),
            &invoker,
            &lifecycle,
        )
        .await?;
            communication_execution::finish_communication_execution(
                input.compiled_plan,
                outcome,
                &runtime_context,
                &invoker,
                &lifecycle,
            )
            .await
        });
        let outcome = match runtime_context.communication_execution() {
            Some(communication) => communication.run_until_cancelled(execution).await?,
            None => execution.await?,
        };
        let tool_delivery_events = flow_execution_context.take_tool_delivery_events()?;

        Ok(ResumeExecutionSegmentOutput {
            _communication_lease: communication_lease,
            outcome,
            prepared_node_runs: lifecycle.prepared_node_runs()?,
            answer_presentation,
            tool_delivery_events,
        })
    }

    async fn build_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<orchestration_runtime::compiler::FlowCompileContext>
    where
        R: ApplicationJsDependencySelectionRepository + ApplicationRepository,
    {
        compile_context::build_application_compile_context_with_cache(
            &self.repository,
            workspace_id,
            application_id,
            self.model_routing_cache_store.as_deref(),
        )
        .await
    }

    async fn load_application_run_context(
        &self,
        actor_user_id: Uuid,
        application_id: Uuid,
    ) -> Result<ApplicationRunContext> {
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(&actor)
                .await?;
            ensure_existing_application_non_crud_console_operation(
                &actor,
                &application,
                &policies,
                ApplicationNonCrudConsoleOperation::Run,
            )?;
        }

        Ok(ApplicationRunContext { actor, application })
    }

    pub async fn start_flow_debug_run(
        &self,
        command: StartFlowDebugRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: ApplicationJsDependencySelectionRepository,
    {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        live_debug_run::start_flow_debug_run(self, command, &context).await
    }

    pub async fn open_flow_debug_run_shell(
        &self,
        command: StartFlowDebugRunCommand,
    ) -> Result<domain::FlowRunRecord> {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        live_debug_run::open_flow_debug_run_shell(self, command, &context).await
    }

    pub async fn prepare_flow_debug_run_from_shell(
        &self,
        command: PrepareFlowDebugRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: ApplicationJsDependencySelectionRepository,
    {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        live_debug_run::prepare_flow_debug_run_from_shell(self, command, &context).await
    }

    pub async fn continue_flow_debug_run(
        &self,
        command: ContinueFlowDebugRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: BillingRepository + crate::ports::FileManagementRepository,
    {
        live_debug_run::continue_flow_debug_run(self, command).await
    }

    fn persist_flow_debug_outcome<'a>(
        &'a self,
        input: PersistFlowDebugOutcomeInput<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<domain::ApplicationRunDetail>> + Send + 'a>> {
        // Keep the full-detail adapter from expanding the already-deep runtime handler future.
        Box::pin(async move {
            let application_id = input.application_id;
            let flow_run_id = input.flow_run.id;
            self.persist_flow_debug_outcome_record(input).await?;
            self.repository
                .get_application_run_detail(application_id, flow_run_id)
                .await?
                .ok_or_else(|| anyhow!("persisted flow run detail not found"))
        })
    }

    async fn persist_flow_debug_outcome_record(
        &self,
        input: PersistFlowDebugOutcomeInput<'_>,
    ) -> Result<domain::FlowRunRecord> {
        let flow_run_id = input.flow_run.id;
        let workspace_id = input.scope_id;
        let compiled_plan = input.compiled_plan;
        let variable_pool = &input.outcome.variable_pool;
        let persisted = persist_flow_debug_outcome(&self.repository, input).await?;
        if persisted.flow_run.run_mode == domain::FlowRunMode::DebugFlowRun {
            if let Some(compiled_plan) = compiled_plan {
                let variable_cache = public_node_variable_cache(compiled_plan, variable_pool);
                persist_debug_variable_cache_entries(
                    &self.repository,
                    workspace_id,
                    &persisted.flow_run,
                    &variable_cache,
                )
                .await?;
            }
        }
        if let Some(stream) = &self.runtime_event_stream {
            for event in &persisted.stream_events {
                let mut stream_event = event.clone();
                stream_event.persist_required = false;
                stream_event.durability = RuntimeEventDurability::Ephemeral;
                if let Err(error) = stream.append(flow_run_id, stream_event).await {
                    if is_expected_runtime_event_stream_closed_error(&error) {
                        tracing::debug!(
                            flow_run_id = %flow_run_id,
                            error = %error,
                            "answer presentation stream append skipped because stream is closed"
                        );
                    } else {
                        tracing::warn!(
                            flow_run_id = %flow_run_id,
                            error = %error,
                            "failed to append answer presentation event to stream"
                        );
                    }
                }
            }
            if let Some(terminal_event) = &persisted.terminal_event {
                runtime_event_persister::project_runtime_event_stream_terminal_payload(
                    stream.clone(),
                    flow_run_id,
                    persisted.flow_run.status,
                    terminal_event.clone(),
                )
                .await;
            } else if let Some(reason) = persisted.close_reason {
                if let Err(error) = stream.close_run(flow_run_id, reason).await {
                    if is_expected_runtime_event_stream_closed_error(&error) {
                        tracing::debug!(
                            flow_run_id = %flow_run_id,
                            reason = ?reason,
                            error = %error,
                            "runtime event stream close skipped because stream is closed"
                        );
                    } else {
                        tracing::warn!(
                            flow_run_id = %flow_run_id,
                            reason = ?reason,
                            error = %error,
                            "failed to close runtime event stream after persisted outcome"
                        );
                    }
                }
            }
        }
        if matches!(
            persisted.flow_run.status,
            domain::FlowRunStatus::Succeeded
                | domain::FlowRunStatus::Incomplete
                | domain::FlowRunStatus::Failed
                | domain::FlowRunStatus::Cancelled
        ) {
            self.clear_provider_protocol_contexts(flow_run_id).await;
        }
        Ok(persisted.flow_run)
    }

    async fn append_published_terminal_audit(
        &self,
        application: &domain::ApplicationRecord,
        flow_run: &domain::FlowRunRecord,
    ) {
        if !matches!(
            flow_run.run_mode,
            domain::FlowRunMode::PublishedApiRun
                | domain::FlowRunMode::AssistantExecution
                | domain::FlowRunMode::WorkflowHttpRun
                | domain::FlowRunMode::WorkflowScheduleRun
        ) {
            return;
        }
        let assistant = flow_run.run_mode == domain::FlowRunMode::AssistantExecution;
        let (event_type, audit_action) = match (assistant, flow_run.status) {
            (true, domain::FlowRunStatus::Succeeded) => (
                "assistant_execution_succeeded",
                "embedded_assistant.run_succeeded",
            ),
            (true, domain::FlowRunStatus::Incomplete) => (
                "assistant_execution_incomplete",
                "embedded_assistant.run_incomplete",
            ),
            (true, domain::FlowRunStatus::Failed) => (
                "assistant_execution_failed",
                "embedded_assistant.run_failed",
            ),
            (true, domain::FlowRunStatus::Cancelled) => (
                "assistant_execution_cancelled",
                "embedded_assistant.run_cancelled",
            ),
            (false, domain::FlowRunStatus::Succeeded) => (
                "public_run_succeeded",
                "application_public_api.run_succeeded",
            ),
            (false, domain::FlowRunStatus::Incomplete) => (
                "public_run_incomplete",
                "application_public_api.run_incomplete",
            ),
            (false, domain::FlowRunStatus::Failed) => {
                ("public_run_failed", "application_public_api.run_failed")
            }
            (false, domain::FlowRunStatus::Cancelled) => (
                "public_run_cancelled",
                "application_public_api.run_cancelled",
            ),
            (_, _) => return,
        };
        let payload = json!({
            "api_key_id": flow_run.api_key_id,
            "application_id": flow_run.application_id,
            "publication_version_id": flow_run.publication_version_id,
            "creator_user_id": flow_run.created_by,
            "external_user": flow_run.external_user,
            "external_conversation_id": flow_run.external_conversation_id,
            "external_trace_id": flow_run.external_trace_id,
            "compatibility_mode": flow_run.compatibility_mode,
            "status": flow_run.status.as_str(),
        });
        let _ = self
            .repository
            .append_run_event(&AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: event_type.to_string(),
                payload: payload.clone(),
            })
            .await;
        let _ = ApplicationRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(application.workspace_id),
                Some(flow_run.created_by),
                if assistant {
                    "embedded_assistant_run"
                } else {
                    "application_public_api_run"
                },
                Some(flow_run.id),
                audit_action,
                payload,
            ),
        )
        .await;
    }
}

struct CacheStoreLlmRoutingCounterStore {
    cache_store: Arc<dyn CacheStore>,
}

#[async_trait]
impl orchestration_runtime::execution_engine::LlmRoutingCounterStore
    for CacheStoreLlmRoutingCounterStore
{
    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<time::Duration>,
    ) -> Result<i64> {
        self.cache_store.increment_counter(key, amount, ttl).await
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "responses-history-migration")]
pub(crate) fn materialize_responses_history_checkpoint(
    lineage: &[crate::ports::RuntimeContextContentVersion],
) -> Result<serde_json::Map<String, Value>> {
    persistence::materialize_responses_history_checkpoint(lineage)
}

#[path = "orchestration_runtime/debug/node_debug_preview.rs"]
mod node_debug_preview;

#[path = "orchestration_runtime/published_execution.rs"]
mod published_execution;
