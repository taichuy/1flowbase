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
    ProviderConfigField,
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    application::{
        ensure_existing_application_non_crud_console_operation, ApplicationNonCrudConsoleOperation,
    },
    audit::audit_log,
    capability_plugin_runtime::{CapabilityPluginRuntimePort, ExecuteCapabilityNodeInput},
    errors::ControlPlaneError,
    model_provider::failover_queue::{freeze_queue_items, FailoverQueueSnapshotItem},
    plugin_lifecycle::reconcile_installation_snapshot,
    plugin_management::ready_current_node_plugin_installation,
    ports::{
        AppendRunEventInput, ApplicationJsDependencySelectionRepository, ApplicationRepository,
        CacheStore, CallbackResumeWaitingNode, CommitFlowRunTerminalInput,
        CommitFlowRunTerminalResult, CompleteCallbackTaskInput, FlowRepository,
        ModelDefinitionRepository, ModelProviderRepository, NodeContributionRepository,
        OrchestrationRuntimeRepository, PluginRepository, ProviderRuntimePort,
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventStream, TaskQueue,
        UpdateFlowRunInput, UpdateNodeRunInput,
    },
    state_transition::{ensure_flow_run_transition, ensure_node_run_transition},
};

mod answer_presentation;
pub(crate) use answer_presentation::is_canonical_answer_presentation_output;
pub mod canonical_stream;
pub(crate) mod compile_context;
mod data_model_runtime;
pub mod debug_artifacts;
pub mod debug_stream_events;
mod debug_variable_cache;
mod http_response_files;
pub(crate) mod inputs;
mod json_payload;
mod live_debug_run;
mod llm_observability_refs;
mod payloads;
mod persistence;
mod provider_invoker;
mod provider_transport;
mod runtime_event_persister;
pub mod scheduler_admission;
mod stream_terminal_recovery;
pub mod trace_projection;

pub use stream_terminal_recovery::FinalizePublishedRunMissingStreamTerminalCommand;

#[cfg(test)]
pub(crate) use provider_invoker::test_support;

use self::{
    compile_context::{ensure_compiled_plan_runnable, ensure_compiled_plan_runnable_for_node},
    debug_variable_cache::{persist_debug_variable_cache_entries, public_node_variable_cache},
    inputs::{
        build_compiled_plan_input, build_complete_flow_run_input, build_complete_node_run_input,
        build_flow_run_input, build_node_run_input,
    },
    json_payload::escape_json_nul_characters,
    payloads::persisted_node_output_payload,
    persistence::{
        checkpoint_node_id, checkpoint_snapshot_from_record, next_node_started_at,
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

pub struct CompleteCallbackTaskCommand {
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

pub(crate) fn ensure_llm_tool_callback_results_complete(
    request_payload: &Value,
    response_payload: &Value,
) -> Result<()> {
    let tool_calls = request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("llm tool callback request is missing tool_calls"))?;
    let tool_results = response_payload
        .get("tool_results")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("llm tool callback response requires tool_results"))?;
    let mut expected_ids = std::collections::BTreeSet::new();
    let mut received_ids = std::collections::BTreeSet::new();

    for tool_call in tool_calls {
        let id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("llm tool callback request has tool call without id"))?;
        expected_ids.insert(id.to_string());
    }
    for tool_result in tool_results {
        let id = tool_result
            .get("tool_call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("llm tool callback result is missing tool_call_id"))?;
        if !expected_ids.contains(id) {
            return Err(anyhow!("unexpected tool result for {id}"));
        }
        if !received_ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate tool result for {id}"));
        }
    }
    for expected_id in expected_ids {
        if !received_ids.contains(&expected_id) {
            return Err(anyhow!("missing tool result for {expected_id}"));
        }
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

pub use runtime_event_persister::{
    project_runtime_event_stream_terminal, spawn_runtime_debug_event_persister,
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
    active_node_id: Option<String>,
    active_node_run_id: Option<Uuid>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
    flow_execution_context: Option<Arc<RuntimeFlowExecutionContext>>,
    answer_presentation:
        Option<Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>>,
    provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    provider_transport_store: Option<Arc<dyn crate::ports::ProviderTransportStore>>,
    provider_continuation: Option<crate::ports::ProviderContinuation>,
}

struct RuntimeFlowExecutionContext {
    active_node: Mutex<Option<RuntimeActiveNode>>,
    data_model: RuntimeDataModelExecutionContext,
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
    outcome: orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    prepared_node_runs: PreparedNodeRuns,
    answer_presentation:
        Option<Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>>,
}

pub struct OrchestrationRuntimeService<R, H> {
    repository: R,
    runtime: H,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    file_storage_registry: Option<Arc<storage_object::FileStorageDriverRegistry>>,
    llm_routing_counter_store:
        Option<Arc<dyn orchestration_runtime::execution_engine::LlmRoutingCounterStore>>,
    model_routing_cache_store: Option<Arc<dyn CacheStore>>,
    provider_secret_master_key: String,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    pub(super) provider_request_log_queue: Option<Arc<dyn TaskQueue>>,
    provider_transport_store: Option<Arc<dyn crate::ports::ProviderTransportStore>>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
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
    ) -> Self {
        Self {
            repository,
            runtime,
            runtime_engine,
            file_storage_registry: None,
            llm_routing_counter_store: None,
            model_routing_cache_store: None,
            provider_secret_master_key: provider_secret_master_key.into(),
            runtime_event_stream: None,
            provider_request_log_queue: None,
            provider_transport_store: None,
            api_node_id: None,
            provider_install_root: None,
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

    pub fn with_file_storage_registry(
        mut self,
        registry: Arc<storage_object::FileStorageDriverRegistry>,
    ) -> Self {
        self.file_storage_registry = Some(registry);
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

    pub fn with_provider_transport_store(
        mut self,
        store: Arc<dyn crate::ports::ProviderTransportStore>,
    ) -> Self {
        self.provider_transport_store = Some(store);
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
        Ok(match &self.llm_routing_counter_store {
            Some(store) => context.with_llm_routing_counter_store(store.clone()),
            None => context,
        })
    }

    fn runtime_flow_execution_context(
        &self,
        actor: domain::ActorContext,
        application_id: Uuid,
        draft_id: Uuid,
        flow_run_id: Uuid,
        active_node: Option<RuntimeActiveNode>,
    ) -> Arc<RuntimeFlowExecutionContext> {
        Arc::new(RuntimeFlowExecutionContext {
            active_node: Mutex::new(active_node),
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
            flow_execution_context: None,
            answer_presentation: None,
            provider_transport_payload: None,
            provider_transport_store: self.provider_transport_store.clone(),
            provider_continuation: None,
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
            flow_execution_context: None,
            answer_presentation: None,
            provider_transport_payload: None,
            provider_transport_store: self.provider_transport_store.clone(),
            provider_continuation: None,
        }
    }

    async fn resume_execution_segment(
        &self,
        input: ResumeExecutionSegmentInput<'_>,
    ) -> Result<ResumeExecutionSegmentOutput>
    where
        R: crate::ports::FileManagementRepository,
    {
        let flow_execution_context = self.runtime_flow_execution_context(
            input.actor.clone(),
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
        );
        let invoker = self
            .runtime_invoker(input.application.workspace_id)
            .for_flow_run(input.flow_run.id)
            .with_flow_execution_context(flow_execution_context);
        let provider_continuation = if orchestration_runtime::execution_engine::pending_llm_tool_callback_requires_ephemeral_provider_continuation(
            &input.snapshot.variable_pool,
            input.waiting_node_id,
        ) {
            let store = self
                .provider_transport_store
                .as_ref()
                .ok_or_else(|| anyhow!("ephemeral_continuation_missing"))?;
            Some(
                store
                    .get_continuation(crate::ports::ProviderContinuationSlotId::for_flow_run(
                        input.flow_run.id,
                    ))
                    .await?
                    .ok_or_else(|| anyhow!("ephemeral_continuation_missing"))?,
            )
        } else {
            None
        };
        let invoker = invoker.with_provider_continuation(provider_continuation);
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
        if let Some(http_file_persister) = self.http_response_file_persister(input.actor.clone()) {
            runtime_context =
                runtime_context.with_http_response_file_persister(Arc::new(http_file_persister));
        }

        let outcome = orchestration_runtime::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            input.compiled_plan,
            input.snapshot,
            input.waiting_node_id,
            input.resume_payload,
            runtime_context,
            &invoker,
            &lifecycle,
        )
        .await?;

        Ok(ResumeExecutionSegmentOutput {
            outcome,
            prepared_node_runs: lifecycle.prepared_node_runs()?,
            answer_presentation,
        })
    }

    async fn build_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<orchestration_runtime::compiler::FlowCompileContext>
    where
        R: ApplicationJsDependencySelectionRepository,
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
                .load_role_console_policies_for_user(actor_user_id, actor.current_workspace_id)
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

    pub async fn start_node_debug_preview(
        &self,
        command: StartNodeDebugPreviewCommand,
    ) -> Result<domain::NodeDebugPreviewResult>
    where
        R: ApplicationJsDependencySelectionRepository + crate::ports::FileManagementRepository,
    {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let editor_state = self
            .repository
            .get_or_create_editor_state(
                context.actor.current_workspace_id,
                context.application.id,
                context.actor.user_id,
            )
            .await?;
        let ApplicationRunContext { actor, application } = context;
        let compile_context = self
            .build_compile_context(application.workspace_id, application.id)
            .await?;

        let preview_document = command
            .document_snapshot
            .as_ref()
            .unwrap_or(&editor_state.draft.document);

        let mut compiled_plan = orchestration_runtime::compiler::FlowCompiler::compile(
            editor_state.flow.id,
            &editor_state.draft.id.to_string(),
            preview_document,
            &compile_context,
        )?;
        freeze_failover_queue_routes(
            &self.repository,
            application.workspace_id,
            &mut compiled_plan,
        )
        .await?;
        ensure_compiled_plan_runnable_for_node(&compiled_plan, &command.node_id)?;
        let started_at = OffsetDateTime::now_utc();
        let compiled_record = self
            .repository
            .upsert_compiled_plan(&build_compiled_plan_input(
                command.actor_user_id,
                &editor_state,
                &compiled_plan,
                preview_document,
            )?)
            .await?;
        let flow_run = self
            .repository
            .create_flow_run(&build_flow_run_input(
                command.actor_user_id,
                command.application_id,
                &editor_state,
                &compiled_record,
                &command,
                preview_document,
                started_at,
            ))
            .await?;
        let flow_execution_context = self.runtime_flow_execution_context(
            actor.clone(),
            application.id,
            editor_state.draft.id,
            flow_run.id,
            None,
        );
        let invoker = self
            .runtime_invoker(application.workspace_id)
            .for_flow_run(flow_run.id)
            .with_flow_execution_context(flow_execution_context);
        let http_file_persister = self.http_response_file_persister(actor);
        let preview_result = orchestration_runtime::preview_executor::run_node_preview_with_http_file_persister_and_counter_store(
            &compiled_plan,
            &command.node_id,
            &command.input_payload,
            &invoker,
            http_file_persister.as_ref().map(|persister| {
                persister as &dyn orchestration_runtime::execution_engine::HttpResponseFilePersister
            }),
            self.llm_routing_counter_store.clone(),
        )
        .await;
        let preview = match preview_result {
            Ok(preview) => preview,
            Err(error) => {
                live_debug_run::fail_flow_run(self, command.application_id, flow_run.id, &error)
                    .await?;
                return Err(error);
            }
        };
        let node_run = self
            .repository
            .create_node_run(&build_node_run_input(
                flow_run.id,
                &compiled_plan,
                &command.node_id,
                &preview,
                started_at,
            )?)
            .await?;
        let events =
            persist_preview_events(&self.repository, &flow_run, &node_run, &preview).await?;
        let finished_at = OffsetDateTime::now_utc();
        ensure_node_run_transition(
            node_run.status,
            if preview.is_failed() {
                domain::NodeRunStatus::Failed
            } else {
                domain::NodeRunStatus::Succeeded
            },
            "complete_node_debug_preview",
        )?;
        let node_run = self
            .repository
            .complete_node_run(&build_complete_node_run_input(
                &node_run,
                &preview,
                finished_at,
            ))
            .await?;
        ensure_flow_run_transition(
            flow_run.status,
            if preview.is_failed() {
                domain::FlowRunStatus::Failed
            } else {
                domain::FlowRunStatus::Succeeded
            },
            "complete_flow_debug_preview",
        )?;
        let completion = build_complete_flow_run_input(&flow_run, &preview, finished_at);
        let terminal_event = if completion.status == domain::FlowRunStatus::Failed {
            debug_stream_events::flow_failed(
                flow_run.id,
                completion
                    .error_payload
                    .clone()
                    .unwrap_or_else(|| json!({ "message": "node preview failed" })),
            )
        } else {
            debug_stream_events::flow_finished(flow_run.id, completion.output_payload.clone())
        };
        let result = if completion.status == domain::FlowRunStatus::Failed {
            CommitFlowRunTerminalResult::Failed {
                output_payload: completion.output_payload,
                error_payload: completion
                    .error_payload
                    .unwrap_or_else(|| json!({ "message": "node preview failed" })),
            }
        } else {
            CommitFlowRunTerminalResult::Succeeded {
                output_payload: completion.output_payload,
            }
        };
        let flow_run_event_payload = result
            .error_payload()
            .cloned()
            .unwrap_or_else(|| result.output_payload().clone());
        let receipt = self
            .repository
            .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                flow_run_id: flow_run.id,
                expected_status: flow_run.status,
                result,
                flow_run_event_payload,
                terminal_event_payload: terminal_event.payload,
                finished_at,
            })
            .await?;
        let flow_run = match stream_terminal_recovery::resolve_terminal_commit(
            &self.repository,
            command.application_id,
            flow_run.id,
            receipt,
        )
        .await?
        {
            stream_terminal_recovery::TerminalCommitResolution::Winner(flow_run)
            | stream_terminal_recovery::TerminalCommitResolution::Loser(flow_run) => flow_run,
        };
        let mut variable_cache = command
            .input_payload
            .as_object()
            .cloned()
            .unwrap_or_default();
        variable_cache.insert(node_run.node_id.clone(), node_run.output_payload.clone());
        let variable_cache = public_node_variable_cache(&compiled_plan, &variable_cache);
        persist_debug_variable_cache_entries(
            &self.repository,
            application.workspace_id,
            &flow_run,
            &variable_cache,
        )
        .await?;

        Ok(domain::NodeDebugPreviewResult {
            flow_run,
            node_run,
            events,
            preview_payload: preview.as_payload(),
        })
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
        R: crate::ports::FileManagementRepository,
    {
        live_debug_run::continue_flow_debug_run(self, command).await
    }

    pub async fn start_published_flow_run(
        &self,
        command: StartPublishedFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        self.start_published_flow_run_inner(
            command.application_id,
            command.flow_run_id,
            command.provider_transport_slot,
        )
        .await
    }

    async fn start_published_flow_run_inner(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        provider_transport_slot: Option<crate::ports::ProviderTransportSlotId>,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        let flow_run = self
            .repository
            .get_flow_run(application_id, flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        if !matches!(
            flow_run.run_mode,
            domain::FlowRunMode::PublishedApiRun
                | domain::FlowRunMode::WorkflowHttpRun
                | domain::FlowRunMode::WorkflowScheduleRun
        ) {
            return Err(ControlPlaneError::InvalidInput("run_mode").into());
        }
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            flow_run.created_by,
        )
        .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;

        let running = match flow_run.status {
            domain::FlowRunStatus::Queued => {
                ensure_flow_run_transition(
                    domain::FlowRunStatus::Queued,
                    domain::FlowRunStatus::Running,
                    "start_published_flow_run",
                )?;
                self.repository
                    .update_flow_run_if_status(
                        &UpdateFlowRunInput {
                            flow_run_id: flow_run.id,
                            status: domain::FlowRunStatus::Running,
                            output_payload: flow_run.output_payload.clone(),
                            error_payload: flow_run.error_payload.clone(),
                            finished_at: None,
                        },
                        domain::FlowRunStatus::Queued,
                    )
                    .await?
            }
            domain::FlowRunStatus::Running => Some(flow_run.clone()),
            _ => None,
        };
        let Some(running) = running else {
            return self
                .repository
                .get_application_run_detail(application_id, flow_run.id)
                .await?
                .ok_or_else(|| anyhow!("flow run detail not found"));
        };

        self.repository
            .append_run_event(&AppendRunEventInput {
                flow_run_id: running.id,
                node_run_id: None,
                event_type: "public_run_execution_started".to_string(),
                payload: json!({
                    "api_key_id": running.api_key_id,
                    "application_id": running.application_id,
                    "publication_version_id": running.publication_version_id,
                    "creator_user_id": running.created_by,
                    "external_user": running.external_user,
                    "external_conversation_id": running.external_conversation_id,
                    "external_trace_id": running.external_trace_id,
                    "compatibility_mode": running.compatibility_mode,
                }),
            })
            .await?;
        let flow_started_event = debug_stream_events::flow_started(running.id);
        if let Err(error) = runtime_event_persister::persist_runtime_event_payload(
            &self.repository,
            running.id,
            &flow_started_event,
        )
        .await
        {
            tracing::warn!(
                flow_run_id = %running.id,
                event_type = %flow_started_event.event_type,
                error = %error,
                "failed to persist published flow runtime start event"
            );
        }
        if let Some(stream) = &self.runtime_event_stream {
            let _ = stream.append(running.id, flow_started_event).await;
        }

        let continuation = ContinueFlowDebugRunCommand {
            application_id,
            flow_run_id: running.id,
            workspace_id: application.workspace_id,
        };
        // Keep the slot available until the complete execution segment has exhausted Provider
        // retries. A disconnected SSE receiver does not own this background execution lifetime.
        let provider_transport_payload = self
            .resolve_provider_transport_payload(&running, provider_transport_slot)
            .await?;
        let result = match provider_transport_payload {
            Some(payload) => {
                live_debug_run::continue_flow_debug_run_with_provider_transport(
                    self,
                    continuation,
                    payload,
                )
                .await
            }
            None => self.continue_flow_debug_run(continuation).await,
        };
        if let Some(slot_id) = provider_transport_slot {
            self.delete_provider_transport_slot(slot_id).await;
        }
        let detail = match result {
            Ok(detail) => detail,
            Err(error) => {
                if let Ok(Some(failed)) = self
                    .repository
                    .get_flow_run(application_id, running.id)
                    .await
                {
                    self.append_published_terminal_audit(&application, &failed)
                        .await;
                }
                return Err(error);
            }
        };
        self.append_published_terminal_audit(&application, &detail.flow_run)
            .await;
        Ok(detail)
    }

    pub async fn continue_flow_debug_run_with_live_provider_events(
        &self,
        command: ContinueFlowDebugRunCommand,
        live_provider_events: LiveProviderStreamEventSender,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        live_debug_run::continue_flow_debug_run_with_live_provider_events(
            self,
            command,
            live_provider_events,
        )
        .await
    }

    pub async fn cancel_flow_run(
        &self,
        command: CancelFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail> {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        live_debug_run::cancel_flow_run(self, command, &context).await
    }

    pub async fn resume_flow_run(
        &self,
        command: ResumeFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let flow_run = self
            .repository
            .get_flow_run(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        let checkpoint = self
            .repository
            .get_checkpoint(command.flow_run_id, command.checkpoint_id)
            .await?
            .ok_or_else(|| anyhow!("checkpoint not found"))?;
        let current_detail = self
            .repository
            .get_application_run_detail(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run detail not found"))?;
        let ApplicationRunContext { actor, application } = context;
        let compiled_plan_id = flow_run
            .compiled_plan_id
            .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
        let compiled_record = self
            .repository
            .get_compiled_plan(compiled_plan_id)
            .await?
            .ok_or_else(|| anyhow!("compiled plan not found"))?;
        let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
            serde_json::from_value(compiled_record.plan.clone())?;
        ensure_compiled_plan_runnable(&compiled_plan)?;
        let snapshot = checkpoint_snapshot_from_record(&checkpoint)?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let resume_patch = command
            .input_payload
            .as_object()
            .and_then(|payload| payload.get(&waiting_node_id))
            .cloned()
            .ok_or_else(|| anyhow!("resume payload is missing node input for {waiting_node_id}"))?;
        let waiting_node_resume = if let Some(node_run_id) = checkpoint.node_run_id {
            let waiting_node = current_detail
                .node_runs
                .iter()
                .find(|record| record.id == node_run_id)
                .ok_or_else(|| anyhow!("waiting node run not found for checkpoint"))?;
            Some(WaitingNodeResumeUpdate {
                node_run_id,
                from_status: waiting_node.status,
                output_payload: resume_patch.clone(),
                metrics_payload: json!({ "resumed": true }),
                debug_payload: json!({}),
            })
        } else {
            None
        };
        let execution = self
            .resume_execution_segment(ResumeExecutionSegmentInput {
                actor: &actor,
                application: &application,
                flow_run: &flow_run,
                compiled_plan: &compiled_plan,
                snapshot: &snapshot,
                waiting_node_id: &waiting_node_id,
                waiting_node_run_id: checkpoint.node_run_id,
                resume_payload: &resume_patch,
            })
            .await?;

        self.persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &execution.outcome,
            prepared_node_runs: Some(&execution.prepared_node_runs),
            answer_presentation: execution.answer_presentation.as_ref(),
            trigger_event_type: "flow_run_resumed",
            trigger_event_payload: json!({
                "checkpoint_id": checkpoint.id,
                "input_payload": command.input_payload,
            }),
            base_started_at: next_node_started_at(&current_detail),
            waiting_node_resume,
        })
        .await
    }

    pub async fn complete_callback_task(
        &self,
        command: CompleteCallbackTaskCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: crate::ports::FileManagementRepository,
    {
        let application_id = command.application_id;
        let flow_run = self.complete_callback_task_run(command).await?;
        self.repository
            .get_application_run_detail(application_id, flow_run.id)
            .await?
            .ok_or_else(|| anyhow!("flow run detail not found"))
    }

    pub(crate) async fn complete_callback_task_run(
        &self,
        mut command: CompleteCallbackTaskCommand,
    ) -> Result<domain::FlowRunRecord>
    where
        R: crate::ports::FileManagementRepository,
    {
        command.response_payload = escape_json_nul_characters(command.response_payload);
        let context = self
            .load_application_run_context(command.actor_user_id, command.application_id)
            .await?;
        let ApplicationRunContext { actor, application } = context;
        let resume_context = self
            .repository
            .get_callback_resume_context(command.application_id, command.callback_task_id)
            .await?;
        let resume_context = match resume_context {
            Some(context) => context,
            None => {
                if self
                    .repository
                    .get_callback_task(command.callback_task_id)
                    .await?
                    .is_some()
                {
                    return Err(anyhow!("flow run not found for callback task"));
                }
                return Err(ControlPlaneError::NotFound("callback_task").into());
            }
        };
        let pending_callback_task = &resume_context.callback_task;
        if pending_callback_task.callback_kind == "data_model_side_effect_confirmation" {
            let confirmation_payload = pending_callback_task
                .external_ref_payload
                .as_ref()
                .unwrap_or(&pending_callback_task.request_payload);
            ensure_data_model_side_effect_confirmation_approved(&command.response_payload)?;
            ensure_data_model_side_effect_confirmation_metadata(&actor, confirmation_payload)?;
        }
        if pending_callback_task.callback_kind == "llm_tool_calls" {
            ensure_llm_tool_callback_results_complete(
                &pending_callback_task.request_payload,
                &command.response_payload,
            )?;
        }
        let checkpoint = resume_context.checkpoint;
        let flow_run = resume_context.flow_run;
        let waiting_node = resume_context.waiting_node;
        let base_started_at = resume_context.next_node_started_at;
        let compiled_plan_id = flow_run
            .compiled_plan_id
            .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
        let compiled_record = self
            .repository
            .get_compiled_plan(compiled_plan_id)
            .await?
            .ok_or_else(|| anyhow!("compiled plan not found"))?;
        let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
            serde_json::from_value(compiled_record.plan.clone())?;
        ensure_compiled_plan_runnable(&compiled_plan)?;
        let callback_task = self
            .repository
            .complete_callback_task(&CompleteCallbackTaskInput {
                callback_task_id: command.callback_task_id,
                response_payload: command.response_payload.clone(),
                completed_at: OffsetDateTime::now_utc(),
            })
            .await?;
        if callback_task.callback_kind == "data_model_side_effect_confirmation" {
            return self
                .complete_data_model_side_effect_callback(
                    command,
                    &actor,
                    &callback_task,
                    &waiting_node,
                    base_started_at,
                    &application,
                    &checkpoint,
                    &flow_run,
                    &compiled_plan,
                )
                .await;
        }
        let snapshot = checkpoint_snapshot_from_record(&checkpoint)?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let execution = self
            .resume_execution_segment(ResumeExecutionSegmentInput {
                actor: &actor,
                application: &application,
                flow_run: &flow_run,
                compiled_plan: &compiled_plan,
                snapshot: &snapshot,
                waiting_node_id: &waiting_node_id,
                waiting_node_run_id: Some(callback_task.node_run_id),
                resume_payload: &command.response_payload,
            })
            .await?;
        let waiting_node_output_payload = if callback_task.callback_kind == "llm_tool_calls" {
            waiting_node.output_payload.clone()
        } else {
            callback_task
                .response_payload
                .clone()
                .ok_or_else(|| anyhow!("completed callback task is missing response payload"))?
        };

        self.persist_flow_debug_outcome_record(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &execution.outcome,
            prepared_node_runs: Some(&execution.prepared_node_runs),
            answer_presentation: execution.answer_presentation.as_ref(),
            trigger_event_type: "flow_run_resumed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
            }),
            base_started_at,
            waiting_node_resume: Some(WaitingNodeResumeUpdate {
                node_run_id: callback_task.node_run_id,
                from_status: waiting_node.status,
                output_payload: waiting_node_output_payload,
                metrics_payload: json!({
                    "resumed": true,
                    "callback_kind": callback_task.callback_kind,
                }),
                debug_payload: json!({
                    "callback_task_id": callback_task.id,
                    "callback_kind": callback_task.callback_kind,
                }),
            }),
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn complete_data_model_side_effect_callback(
        &self,
        command: CompleteCallbackTaskCommand,
        actor: &domain::ActorContext,
        callback_task: &domain::CallbackTaskRecord,
        waiting_node: &CallbackResumeWaitingNode,
        base_started_at: OffsetDateTime,
        application: &domain::ApplicationRecord,
        checkpoint: &domain::CheckpointRecord,
        flow_run: &domain::FlowRunRecord,
        compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    ) -> Result<domain::FlowRunRecord>
    where
        R: crate::ports::FileManagementRepository,
    {
        let waiting_node_id = checkpoint_node_id(checkpoint)?;
        let node = compiled_plan
            .nodes
            .get(&waiting_node_id)
            .ok_or_else(|| anyhow!("waiting data_model node not found in compiled plan"))?;
        let confirmation_payload = callback_task
            .external_ref_payload
            .as_ref()
            .unwrap_or(&callback_task.request_payload);
        let execution = data_model_runtime::execute_confirmed_data_model_side_effect(
            self.repository.clone(),
            self.runtime_engine.clone(),
            actor,
            node,
            &data_model_runtime::DataModelRunContext {
                workspace_id: application.workspace_id,
                application_id: command.application_id,
                draft_id: flow_run.draft_id,
                flow_run_id: flow_run.id,
                node_run_id: callback_task.node_run_id,
            },
            confirmation_payload,
        )
        .await;

        if let Some(error_payload) = execution.error_payload.clone() {
            ensure_node_run_transition(
                waiting_node.status,
                domain::NodeRunStatus::Failed,
                "complete_data_model_side_effect_callback",
            )?;
            self.repository
                .update_node_run(&UpdateNodeRunInput {
                    node_run_id: callback_task.node_run_id,
                    status: domain::NodeRunStatus::Failed,
                    output_payload: json!({}),
                    error_payload: Some(error_payload.clone()),
                    metrics_payload: execution.metrics_payload,
                    debug_payload: json!({
                        "callback_task_id": callback_task.id,
                        "callback_kind": callback_task.callback_kind,
                    }),
                    finished_at: Some(OffsetDateTime::now_utc()),
                })
                .await?;
            ensure_flow_run_transition(
                flow_run.status,
                domain::FlowRunStatus::Failed,
                "complete_data_model_side_effect_callback",
            )?;
            let terminal_event =
                debug_stream_events::flow_failed(flow_run.id, error_payload.clone());
            let receipt = self
                .repository
                .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
                    flow_run_id: flow_run.id,
                    expected_status: flow_run.status,
                    result: CommitFlowRunTerminalResult::Failed {
                        output_payload: flow_run.output_payload.clone(),
                        error_payload: error_payload.clone(),
                    },
                    flow_run_event_payload: error_payload,
                    terminal_event_payload: terminal_event.payload,
                    finished_at: OffsetDateTime::now_utc(),
                })
                .await?;
            let failed_flow_run = match stream_terminal_recovery::resolve_terminal_commit(
                &self.repository,
                command.application_id,
                flow_run.id,
                receipt,
            )
            .await?
            {
                stream_terminal_recovery::TerminalCommitResolution::Winner(flow_run)
                | stream_terminal_recovery::TerminalCommitResolution::Loser(flow_run) => flow_run,
            };
            live_debug_run::project_committed_terminal(self, &failed_flow_run).await;
            return Ok(failed_flow_run);
        }

        let snapshot = checkpoint_snapshot_from_record(checkpoint)?;
        let resumed_execution = self
            .resume_execution_segment(ResumeExecutionSegmentInput {
                actor,
                application,
                flow_run,
                compiled_plan,
                snapshot: &snapshot,
                waiting_node_id: &waiting_node_id,
                waiting_node_run_id: Some(callback_task.node_run_id),
                resume_payload: &execution.output_payload,
            })
            .await?;
        let side_effect_receipt = execution
            .metrics_payload
            .get("side_effect_receipt")
            .cloned()
            .unwrap_or(Value::Null);

        self.persist_flow_debug_outcome_record(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run,
            compiled_plan: Some(compiled_plan),
            outcome: &resumed_execution.outcome,
            prepared_node_runs: Some(&resumed_execution.prepared_node_runs),
            answer_presentation: resumed_execution.answer_presentation.as_ref(),
            trigger_event_type: "data_model_side_effect_confirmed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
                "side_effect_receipt": side_effect_receipt,
            }),
            base_started_at,
            waiting_node_resume: Some(WaitingNodeResumeUpdate {
                node_run_id: callback_task.node_run_id,
                from_status: waiting_node.status,
                output_payload: persisted_node_output_payload(
                    &execution.output_payload,
                    &execution.metrics_payload,
                    None,
                    &json!({
                        "callback_task_id": callback_task.id,
                        "callback_kind": callback_task.callback_kind,
                        "confirmed": true,
                    }),
                ),
                metrics_payload: execution.metrics_payload,
                debug_payload: json!({
                    "callback_task_id": callback_task.id,
                    "callback_kind": callback_task.callback_kind,
                    "confirmed": true,
                }),
            }),
        })
        .await
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
                | domain::FlowRunMode::WorkflowHttpRun
                | domain::FlowRunMode::WorkflowScheduleRun
        ) {
            return;
        }
        let (event_type, audit_action) = match flow_run.status {
            domain::FlowRunStatus::Succeeded => (
                "public_run_succeeded",
                "application_public_api.run_succeeded",
            ),
            domain::FlowRunStatus::Incomplete => (
                "public_run_incomplete",
                "application_public_api.run_incomplete",
            ),
            domain::FlowRunStatus::Failed => {
                ("public_run_failed", "application_public_api.run_failed")
            }
            domain::FlowRunStatus::Cancelled => (
                "public_run_cancelled",
                "application_public_api.run_cancelled",
            ),
            _ => return,
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
                "application_public_api_run",
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
