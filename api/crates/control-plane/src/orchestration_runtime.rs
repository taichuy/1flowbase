use std::{
    path::PathBuf,
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
    audit::audit_log,
    capability_plugin_runtime::{CapabilityPluginRuntimePort, ExecuteCapabilityNodeInput},
    errors::ControlPlaneError,
    flow::FlowService,
    model_provider::failover_queue::{freeze_queue_items, FailoverQueueSnapshotItem},
    plugin_lifecycle::reconcile_installation_snapshot,
    plugin_management::ready_current_node_plugin_installation,
    ports::{
        AppendRunEventInput, ApplicationJsDependencySelectionRepository, ApplicationRepository,
        CacheStore, CompleteCallbackTaskInput, FlowRepository, ModelDefinitionRepository,
        ModelProviderRepository, NodeContributionRepository, OrchestrationRuntimeRepository,
        PluginRepository, ProviderRuntimePort, RuntimeEventEnvelope, RuntimeEventStream, TaskQueue,
        UpdateFlowRunInput, UpdateNodeRunInput,
    },
    state_transition::{ensure_flow_run_transition, ensure_node_run_transition},
};

mod answer_presentation;
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
mod runtime_event_persister;
pub mod scheduler_admission;
pub mod trace_projection;

#[cfg(test)]
pub(crate) use provider_invoker::test_support;

use self::{
    compile_context::ensure_compiled_plan_runnable_for_node,
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
        WaitingNodeResumeUpdate,
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
}

#[derive(Debug, Clone)]
pub struct LiveProviderStreamEvent {
    pub node_id: String,
    pub node_run_id: Uuid,
    pub event: ProviderStreamEvent,
}

pub type LiveProviderStreamEventSender = mpsc::UnboundedSender<LiveProviderStreamEvent>;

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
    fail_runtime_event_stream_if_missing_terminal, spawn_runtime_debug_event_persister,
    wait_for_runtime_debug_event_persister,
};

#[derive(Clone)]
struct RuntimeProviderInvoker<R, H> {
    repository: R,
    runtime: H,
    workspace_id: Uuid,
    provider_secret_master_key: String,
    live_provider_events: Option<LiveProviderStreamEventSender>,
    persist_events: Option<mpsc::UnboundedSender<ProviderStreamEvent>>,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    flow_run_id: Option<Uuid>,
    active_node_id: Option<String>,
    active_node_run_id: Option<Uuid>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
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
    provider_secret_master_key: String,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    pub(super) provider_request_log_queue: Option<Arc<dyn TaskQueue>>,
    api_node_id: Option<String>,
    provider_install_root: Option<PathBuf>,
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
            provider_secret_master_key: provider_secret_master_key.into(),
            runtime_event_stream: None,
            provider_request_log_queue: None,
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
    ) -> orchestration_runtime::execution_engine::ExecutionRuntimeContext {
        let context =
            orchestration_runtime::execution_engine::ExecutionRuntimeContext::from_plan_input(
                plan,
                variable_pool,
            );
        match &self.llm_routing_counter_store {
            Some(store) => context.with_llm_routing_counter_store(store.clone()),
            None => context,
        }
    }

    fn runtime_invoker(&self, workspace_id: Uuid) -> RuntimeProviderInvoker<R, H> {
        RuntimeProviderInvoker {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: None,
            persist_events: None,
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: None,
            active_node_id: None,
            active_node_run_id: None,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            answer_presentation: None,
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
            persist_events: None,
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: None,
            active_node_id: None,
            active_node_run_id: None,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            answer_presentation: None,
        }
    }

    async fn build_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<orchestration_runtime::compiler::FlowCompileContext>
    where
        R: ApplicationJsDependencySelectionRepository,
    {
        compile_context::build_application_compile_context(
            &self.repository,
            workspace_id,
            application_id,
        )
        .await
    }

    pub async fn start_node_debug_preview(
        &self,
        command: StartNodeDebugPreviewCommand,
    ) -> Result<domain::NodeDebugPreviewResult>
    where
        R: ApplicationJsDependencySelectionRepository + crate::ports::FileManagementRepository,
    {
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
        .await?;
        let editor_state = FlowService::new(self.repository.clone())
            .get_or_create_editor_state(command.actor_user_id, command.application_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
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
        freeze_failover_queue_routes(&self.repository, &mut compiled_plan).await?;
        ensure_compiled_plan_runnable_for_node(&compiled_plan, &command.node_id)?;
        let invoker = self.runtime_invoker(application.workspace_id);
        let started_at = OffsetDateTime::now_utc();
        let http_file_persister = self.http_response_file_persister(actor.clone());
        let preview = orchestration_runtime::preview_executor::run_node_preview_with_http_file_persister_and_counter_store(
            &compiled_plan,
            &command.node_id,
            &command.input_payload,
            &invoker,
            http_file_persister.as_ref().map(|persister| {
                persister as &dyn orchestration_runtime::execution_engine::HttpResponseFilePersister
            }),
            self.llm_routing_counter_store.clone(),
        )
        .await?;
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
        let flow_run = self
            .repository
            .complete_flow_run(&build_complete_flow_run_input(
                &flow_run,
                &preview,
                finished_at,
            ))
            .await?;
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
        live_debug_run::start_flow_debug_run(self, command).await
    }

    pub async fn open_flow_debug_run_shell(
        &self,
        command: StartFlowDebugRunCommand,
    ) -> Result<domain::FlowRunRecord> {
        live_debug_run::open_flow_debug_run_shell(self, command).await
    }

    pub async fn prepare_flow_debug_run_from_shell(
        &self,
        command: PrepareFlowDebugRunCommand,
    ) -> Result<domain::ApplicationRunDetail>
    where
        R: ApplicationJsDependencySelectionRepository,
    {
        live_debug_run::prepare_flow_debug_run_from_shell(self, command).await
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
        let flow_run = self
            .repository
            .get_flow_run(command.application_id, command.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found"))?;
        if flow_run.run_mode != domain::FlowRunMode::PublishedApiRun {
            return Err(ControlPlaneError::InvalidInput("run_mode").into());
        }
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            flow_run.created_by,
        )
        .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
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
                .get_application_run_detail(command.application_id, flow_run.id)
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

        let result = self
            .continue_flow_debug_run(ContinueFlowDebugRunCommand {
                application_id: command.application_id,
                flow_run_id: running.id,
                workspace_id: application.workspace_id,
            })
            .await;
        let detail = match result {
            Ok(detail) => detail,
            Err(error) => {
                if let Ok(Some(failed)) = self
                    .repository
                    .get_flow_run(command.application_id, running.id)
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
        live_debug_run::cancel_flow_run(self, command).await
    }

    pub async fn resume_flow_run(
        &self,
        command: ResumeFlowRunCommand,
    ) -> Result<domain::ApplicationRunDetail> {
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
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
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
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
        let snapshot = checkpoint_snapshot_from_record(&checkpoint)?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let resume_patch = command
            .input_payload
            .as_object()
            .and_then(|payload| payload.get(&waiting_node_id))
            .cloned()
            .ok_or_else(|| anyhow!("resume payload is missing node input for {waiting_node_id}"))?;
        let runtime_context =
            self.execution_runtime_context(&compiled_plan, &snapshot.variable_pool);
        let outcome =
            orchestration_runtime::execution_engine::resume_flow_debug_run_with_runtime_context(
                &compiled_plan,
                &snapshot,
                &waiting_node_id,
                &resume_patch,
                runtime_context,
                &self.runtime_invoker(application.workspace_id),
            )
            .await?;
        let waiting_node_resume = if let Some(node_run_id) = checkpoint.node_run_id {
            let waiting_node = current_detail
                .node_runs
                .iter()
                .find(|record| record.id == node_run_id)
                .ok_or_else(|| anyhow!("waiting node run not found for checkpoint"))?;
            Some(WaitingNodeResumeUpdate {
                node_run_id,
                from_status: waiting_node.status,
                output_payload: resume_patch,
                metrics_payload: json!({ "resumed": true }),
                debug_payload: json!({}),
            })
        } else {
            None
        };

        self.persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &outcome,
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
        mut command: CompleteCallbackTaskCommand,
    ) -> Result<domain::ApplicationRunDetail> {
        command.response_payload = escape_json_nul_characters(command.response_payload);
        let actor = ApplicationRepository::load_actor_context_for_user(
            &self.repository,
            command.actor_user_id,
        )
        .await?;
        let pending_callback_task = self
            .repository
            .get_callback_task(command.callback_task_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("callback_task"))?;
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
        let detail = self
            .repository
            .get_application_run_detail(command.application_id, pending_callback_task.flow_run_id)
            .await?
            .ok_or_else(|| anyhow!("flow run not found for callback task"))?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        let checkpoint = detail
            .checkpoints
            .iter()
            .rev()
            .find(|record| record.node_run_id == Some(pending_callback_task.node_run_id))
            .cloned()
            .ok_or_else(|| anyhow!("checkpoint not found for callback task"))?;
        let flow_run = detail.flow_run.clone();
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
                    &detail,
                    &application,
                    &checkpoint,
                    &flow_run,
                    &compiled_plan,
                )
                .await;
        }
        let snapshot = checkpoint_snapshot_from_record(&checkpoint)?;
        let waiting_node_id = checkpoint_node_id(&checkpoint)?;
        let runtime_context =
            self.execution_runtime_context(&compiled_plan, &snapshot.variable_pool);
        let outcome =
            orchestration_runtime::execution_engine::resume_flow_debug_run_with_runtime_context(
                &compiled_plan,
                &snapshot,
                &waiting_node_id,
                &command.response_payload,
                runtime_context,
                &self.runtime_invoker(application.workspace_id),
            )
            .await?;

        let waiting_node = detail
            .node_runs
            .iter()
            .find(|record| record.id == callback_task.node_run_id)
            .ok_or_else(|| anyhow!("waiting node run not found for callback task"))?;
        let waiting_node_output_payload = if callback_task.callback_kind == "llm_tool_calls" {
            waiting_node.output_payload.clone()
        } else {
            callback_task
                .response_payload
                .clone()
                .ok_or_else(|| anyhow!("completed callback task is missing response payload"))?
        };

        self.persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &outcome,
            trigger_event_type: "flow_run_resumed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
            }),
            base_started_at: next_node_started_at(&detail),
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
        detail: &domain::ApplicationRunDetail,
        application: &domain::ApplicationRecord,
        checkpoint: &domain::CheckpointRecord,
        flow_run: &domain::FlowRunRecord,
        compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    ) -> Result<domain::ApplicationRunDetail> {
        let waiting_node_id = checkpoint_node_id(checkpoint)?;
        let node = compiled_plan
            .nodes
            .get(&waiting_node_id)
            .ok_or_else(|| anyhow!("waiting data_model node not found in compiled plan"))?;
        let waiting_node = detail
            .node_runs
            .iter()
            .find(|record| record.id == callback_task.node_run_id)
            .ok_or_else(|| anyhow!("waiting node run not found for callback task"))?;
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
            self.repository
                .update_flow_run(&UpdateFlowRunInput {
                    flow_run_id: flow_run.id,
                    status: domain::FlowRunStatus::Failed,
                    output_payload: flow_run.output_payload.clone(),
                    error_payload: Some(error_payload.clone()),
                    finished_at: Some(OffsetDateTime::now_utc()),
                })
                .await?;
            self.repository
                .append_run_event(&AppendRunEventInput {
                    flow_run_id: flow_run.id,
                    node_run_id: Some(callback_task.node_run_id),
                    event_type: "flow_run_failed".to_string(),
                    payload: error_payload,
                })
                .await?;
            return self
                .repository
                .get_application_run_detail(command.application_id, flow_run.id)
                .await?
                .ok_or_else(|| anyhow!("flow run detail not found"));
        }

        let snapshot = checkpoint_snapshot_from_record(checkpoint)?;
        let runtime_context =
            self.execution_runtime_context(compiled_plan, &snapshot.variable_pool);
        let outcome =
            orchestration_runtime::execution_engine::resume_flow_debug_run_with_runtime_context(
                compiled_plan,
                &snapshot,
                &waiting_node_id,
                &execution.output_payload,
                runtime_context,
                &self.runtime_invoker(application.workspace_id),
            )
            .await?;
        let side_effect_receipt = execution
            .metrics_payload
            .get("side_effect_receipt")
            .cloned()
            .unwrap_or(Value::Null);

        self.persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: self.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run,
            compiled_plan: Some(compiled_plan),
            outcome: &outcome,
            trigger_event_type: "data_model_side_effect_confirmed",
            trigger_event_payload: json!({
                "callback_task_id": callback_task.id,
                "response_payload": command.response_payload,
                "side_effect_receipt": side_effect_receipt,
            }),
            base_started_at: next_node_started_at(detail),
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

    async fn persist_flow_debug_outcome(
        &self,
        input: PersistFlowDebugOutcomeInput<'_>,
    ) -> Result<domain::ApplicationRunDetail> {
        let flow_run_id = input.flow_run.id;
        let persisted = persist_flow_debug_outcome(&self.repository, input).await?;
        if let Some(stream) = &self.runtime_event_stream {
            for event in &persisted.answer_presentation_events {
                let mut stream_event = event.clone();
                stream_event.persist_required = false;
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
        }
        Ok(persisted.detail)
    }

    async fn append_published_terminal_audit(
        &self,
        application: &domain::ApplicationRecord,
        flow_run: &domain::FlowRunRecord,
    ) {
        if flow_run.run_mode != domain::FlowRunMode::PublishedApiRun {
            return;
        }
        let (event_type, audit_action) = match flow_run.status {
            domain::FlowRunStatus::Succeeded => (
                "public_run_succeeded",
                "application_public_api.run_succeeded",
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
