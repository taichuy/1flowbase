use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::provider_contract::{
    NativePromptBlock, NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY,
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::ApplicationApiKeyService,
    run_service::{
        ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
    },
};
use crate::{
    errors::ControlPlaneError,
    orchestration_runtime::{CompleteCallbackTaskCommand, OrchestrationRuntimeService},
    ports::{
        ApiKeyRepository, ApplicationRepository, AuthRepository, CacheStore,
        FinishFlowRunCallbackResumeAttemptInput, OrchestrationRuntimeRepository,
        ProviderRuntimePort, RecordFlowRunCallbackResumeAttemptInput,
        RecordFlowRunCallbackResumeAttemptOutput,
    },
};

const ANTHROPIC_MESSAGES_COMPATIBILITY_MODE: &str = "anthropic-messages-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishedCallbackResumeSource {
    NativeAgent,
    OpenAiChat,
    OpenAiResponses,
    AnthropicMessages,
}

impl PublishedCallbackResumeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeAgent => "native_agent",
            Self::OpenAiChat => "openai_chat",
            Self::OpenAiResponses => "openai_responses",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishedCallbackResumeTarget {
    FlowRun {
        flow_run_id: Uuid,
        callback_task_id: Uuid,
    },
    CallbackTask {
        callback_task_id: Uuid,
    },
}

#[derive(Debug, Clone)]
pub struct ResumePublishedCallbackCommand {
    pub bearer_token: String,
    pub target: PublishedCallbackResumeTarget,
    pub source: PublishedCallbackResumeSource,
    pub response_payload: Value,
    pub response_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletePublishedCallbackInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub callback_task_id: Uuid,
    pub response_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResumePublishedCallbackResult {
    pub detail: domain::ApplicationRunDetail,
    pub attempt: domain::FlowRunCallbackResumeAttemptRecord,
}

pub struct ApplicationPublishedCallbackResumeService<R, C> {
    repository: R,
    consumer: C,
    last_used_cache: Option<Arc<dyn CacheStore>>,
}

impl<R, C> ApplicationPublishedCallbackResumeService<R, C>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublishedCallbackAttemptRepository
        + Clone,
    C: ApplicationPublishedCallbackConsumer,
{
    pub fn new(repository: R, consumer: C) -> Self {
        Self {
            repository,
            consumer,
            last_used_cache: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub async fn resume_callback(
        &self,
        mut command: ResumePublishedCallbackCommand,
    ) -> Result<ResumePublishedCallbackResult> {
        command.response_payload = escape_json_nul_characters(command.response_payload);
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| ControlPlaneError::NotAuthenticated)?;
        let callback_task_id = match command.target {
            PublishedCallbackResumeTarget::FlowRun {
                callback_task_id, ..
            }
            | PublishedCallbackResumeTarget::CallbackTask { callback_task_id } => callback_task_id,
        };
        let callback_task = self
            .repository
            .get_published_callback_task(callback_task_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("callback_task"))?;
        if let PublishedCallbackResumeTarget::FlowRun { flow_run_id, .. } = command.target {
            if callback_task.flow_run_id != flow_run_id {
                return Err(ControlPlaneError::PermissionDenied("callback_task_flow_run").into());
            }
        }
        let flow_run = self
            .repository
            .get_published_flow_run(callback_task.flow_run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(
                ControlPlaneError::PermissionDenied("application_public_callback_resume").into(),
            );
        }

        if let Some(existing) = self
            .repository
            .get_published_callback_resume_attempt(callback_task.id)
            .await?
        {
            return self
                .resume_existing_attempt(&actor, &callback_task, existing, &command)
                .await;
        }

        ensure_callback_is_consumable(&flow_run, &callback_task, &command.response_payload)?;
        let attempt_output = self
            .repository
            .record_published_callback_resume_attempt(&RecordFlowRunCallbackResumeAttemptInput {
                flow_run_id: flow_run.id,
                callback_task_id: callback_task.id,
                source: command.source.as_str().to_string(),
                response_payload: command.response_payload.clone(),
                idempotency_key: format!("callback_task:{}", callback_task.id),
            })
            .await?;
        if !attempt_output.inserted {
            return self
                .resume_existing_attempt(&actor, &callback_task, attempt_output.attempt, &command)
                .await;
        }

        self.append_resume_event(
            flow_run.id,
            Some(callback_task.node_run_id),
            "public_run_resume_requested",
            json!({
                "callback_task_id": callback_task.id,
                "resume_attempt_id": attempt_output.attempt.id,
                "source": command.source.as_str(),
                "response_mode": command.response_mode,
                "response_payload": command.response_payload,
            }),
        )
        .await?;

        let result = self
            .consumer
            .complete_published_callback(CompletePublishedCallbackInput {
                actor_user_id: actor.creator_user_id,
                application_id: actor.application_id,
                callback_task_id: callback_task.id,
                response_payload: attempt_output.attempt.response_payload.clone(),
            })
            .await;

        match result {
            Ok(detail) => {
                let finished = self
                    .repository
                    .finish_published_callback_resume_attempt(
                        &FinishFlowRunCallbackResumeAttemptInput {
                            attempt_id: attempt_output.attempt.id,
                            status: domain::FlowRunCallbackResumeAttemptStatus::Succeeded,
                            error_payload: None,
                            completed_at: OffsetDateTime::now_utc(),
                        },
                    )
                    .await?;
                self.append_resume_event(
                    flow_run.id,
                    Some(callback_task.node_run_id),
                    "public_run_resume_succeeded",
                    json!({
                        "callback_task_id": callback_task.id,
                        "resume_attempt_id": finished.id,
                        "source": command.source.as_str(),
                    }),
                )
                .await?;
                self.project_anthropic_agent_results_to_matching_subagents(
                    &actor,
                    &flow_run,
                    &callback_task,
                    &finished.response_payload,
                    finished
                        .completed_at
                        .unwrap_or_else(OffsetDateTime::now_utc),
                )
                .await?;
                Ok(ResumePublishedCallbackResult {
                    detail,
                    attempt: finished,
                })
            }
            Err(error) => {
                let error_payload = json!({ "message": error.to_string() });
                let _ = self
                    .repository
                    .fail_waiting_callback_published_run(
                        flow_run.id,
                        error_payload.clone(),
                        OffsetDateTime::now_utc(),
                    )
                    .await;
                let finished = self
                    .repository
                    .finish_published_callback_resume_attempt(
                        &FinishFlowRunCallbackResumeAttemptInput {
                            attempt_id: attempt_output.attempt.id,
                            status: domain::FlowRunCallbackResumeAttemptStatus::Failed,
                            error_payload: Some(error_payload.clone()),
                            completed_at: OffsetDateTime::now_utc(),
                        },
                    )
                    .await?;
                self.append_resume_event(
                    flow_run.id,
                    Some(callback_task.node_run_id),
                    "public_run_resume_failed",
                    json!({
                        "callback_task_id": callback_task.id,
                        "resume_attempt_id": finished.id,
                        "source": command.source.as_str(),
                        "error": error_payload,
                    }),
                )
                .await?;
                Err(error)
            }
        }
    }

    async fn resume_existing_attempt(
        &self,
        actor: &super::api_keys::ApplicationApiKeyActor,
        callback_task: &domain::CallbackTaskRecord,
        attempt: domain::FlowRunCallbackResumeAttemptRecord,
        command: &ResumePublishedCallbackCommand,
    ) -> Result<ResumePublishedCallbackResult> {
        if attempt.response_payload != command.response_payload {
            return Err(ControlPlaneError::Conflict("callback_resume_payload_conflict").into());
        }
        if attempt.status != domain::FlowRunCallbackResumeAttemptStatus::Succeeded {
            return Err(ControlPlaneError::Conflict("callback_resume_not_completed").into());
        }
        if callback_task.status != domain::CallbackTaskStatus::Completed {
            return Err(ControlPlaneError::Conflict("callback_task_not_completed").into());
        }
        let detail = self
            .repository
            .get_published_run_detail(actor.application_id, attempt.flow_run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(ResumePublishedCallbackResult { detail, attempt })
    }

    async fn append_resume_event(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        event_type: &str,
        payload: Value,
    ) -> Result<()> {
        self.repository
            .append_published_run_event(&crate::ports::AppendRunEventInput {
                flow_run_id,
                node_run_id,
                event_type: event_type.to_string(),
                payload,
            })
            .await?;
        Ok(())
    }

    async fn project_anthropic_agent_results_to_matching_subagents(
        &self,
        actor: &super::api_keys::ApplicationApiKeyActor,
        flow_run: &domain::FlowRunRecord,
        callback_task: &domain::CallbackTaskRecord,
        response_payload: &Value,
        completed_at: OffsetDateTime,
    ) -> Result<()> {
        if flow_run.compatibility_mode.as_deref() != Some(ANTHROPIC_MESSAGES_COMPATIBILITY_MODE) {
            return Ok(());
        }
        let (Some(external_user), Some(external_conversation_id)) = (
            flow_run.external_user.as_deref(),
            flow_run.external_conversation_id.as_deref(),
        ) else {
            return Ok(());
        };
        let agent_results =
            anthropic_agent_tool_results(&callback_task.request_payload, response_payload);
        if agent_results.is_empty() {
            return Ok(());
        }
        let waiting_run_ids = self
            .repository
            .list_waiting_callback_published_flow_run_ids_for_conversation(
                &super::run_service::ListWaitingCallbackPublishedRunsInput {
                    application_id: actor.application_id,
                    api_key_id: actor.api_key_id,
                    external_user: external_user.to_string(),
                    external_conversation_id: external_conversation_id.to_string(),
                    compatibility_mode: ANTHROPIC_MESSAGES_COMPATIBILITY_MODE.to_string(),
                },
            )
            .await?;
        let mut waiting_runs = Vec::with_capacity(waiting_run_ids.len());
        for waiting_run_id in waiting_run_ids {
            if let Some(waiting_run) = self
                .repository
                .get_published_flow_run(waiting_run_id)
                .await?
            {
                waiting_runs.push(waiting_run);
            }
        }

        for agent_result in agent_results {
            let matching_runs = waiting_runs
                .iter()
                .filter(|run| {
                    run.id != flow_run.id
                        && is_anthropic_claude_code_subagent_run(run)
                        && application_public_run_query(&run.input_payload)
                            .is_some_and(|query| query.trim() == agent_result.prompt)
                })
                .collect::<Vec<_>>();
            if matching_runs.len() != 1 {
                continue;
            }
            let subagent_run = matching_runs[0];
            let output_payload = json!({
                "answer": agent_result.content,
                "compatibility": {
                    "claude_code_internal_agent_result": true,
                    "parent_flow_run_id": flow_run.id,
                    "parent_callback_task_id": callback_task.id,
                    "tool_call_id": agent_result.tool_call_id,
                }
            });
            let completed = self
                .repository
                .complete_waiting_callback_published_internal_run(
                    subagent_run.id,
                    output_payload,
                    completed_at,
                )
                .await?;
            if completed.is_none() {
                continue;
            }
            self.append_resume_event(
                subagent_run.id,
                None,
                "public_run_internal_agent_result_projected",
                json!({
                    "parent_flow_run_id": flow_run.id,
                    "parent_callback_task_id": callback_task.id,
                    "tool_call_id": agent_result.tool_call_id,
                }),
            )
            .await?;
            self.cancel_callback_state_for_run(subagent_run.id, completed_at)
                .await?;
        }

        Ok(())
    }

    async fn cancel_callback_state_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<()> {
        let cancelled_callback_tasks = self
            .repository
            .cancel_published_pending_callback_tasks_for_run(flow_run_id, completed_at)
            .await?;
        for callback_task in cancelled_callback_tasks {
            self.append_resume_event(
                flow_run_id,
                Some(callback_task.node_run_id),
                "public_run_callback_cancelled",
                json!({
                    "callback_task_id": callback_task.id,
                    "callback_kind": callback_task.callback_kind,
                }),
            )
            .await?;
        }
        let cancelled_attempts = self
            .repository
            .cancel_published_callback_resume_attempts_for_run(flow_run_id, completed_at)
            .await?;
        for attempt in cancelled_attempts {
            self.append_resume_event(
                flow_run_id,
                None,
                "public_run_resume_cancelled",
                json!({
                    "callback_task_id": attempt.callback_task_id,
                    "resume_attempt_id": attempt.id,
                }),
            )
            .await?;
        }
        Ok(())
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let service = ApplicationApiKeyService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }
}

#[async_trait]
pub trait ApplicationPublishedCallbackConsumer: Send + Sync {
    async fn complete_published_callback(
        &self,
        input: CompletePublishedCallbackInput,
    ) -> Result<domain::ApplicationRunDetail>;
}

#[async_trait]
impl<R, H> ApplicationPublishedCallbackConsumer for OrchestrationRuntimeService<R, H>
where
    R: ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + crate::capability_plugin_runtime::CapabilityPluginRuntimePort + Clone,
{
    async fn complete_published_callback(
        &self,
        input: CompletePublishedCallbackInput,
    ) -> Result<domain::ApplicationRunDetail> {
        self.complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: input.actor_user_id,
            application_id: input.application_id,
            callback_task_id: input.callback_task_id,
            response_payload: input.response_payload,
        })
        .await
    }
}

#[async_trait]
pub trait ApplicationPublishedCallbackAttemptRepository: Send + Sync {
    async fn record_published_callback_resume_attempt(
        &self,
        input: &RecordFlowRunCallbackResumeAttemptInput,
    ) -> Result<RecordFlowRunCallbackResumeAttemptOutput>;

    async fn get_published_callback_resume_attempt(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::FlowRunCallbackResumeAttemptRecord>>;

    async fn finish_published_callback_resume_attempt(
        &self,
        input: &FinishFlowRunCallbackResumeAttemptInput,
    ) -> Result<domain::FlowRunCallbackResumeAttemptRecord>;

    async fn cancel_published_callback_resume_attempts_for_run(
        &self,
        flow_run_id: Uuid,
        completed_at: OffsetDateTime,
    ) -> Result<Vec<domain::FlowRunCallbackResumeAttemptRecord>>;

    async fn fail_waiting_callback_published_run(
        &self,
        flow_run_id: Uuid,
        error_payload: Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn complete_waiting_callback_published_internal_run(
        &self,
        flow_run_id: Uuid,
        output_payload: Value,
        finished_at: OffsetDateTime,
    ) -> Result<Option<domain::FlowRunRecord>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnthropicAgentToolResult {
    tool_call_id: String,
    prompt: String,
    content: String,
}

fn anthropic_agent_tool_results(
    request_payload: &Value,
    response_payload: &Value,
) -> Vec<AnthropicAgentToolResult> {
    let agent_prompts_by_call_id = request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|tool_call| tool_call.get("name").and_then(Value::as_str) == Some("Agent"))
        .filter_map(|tool_call| {
            let id = tool_call.get("id").and_then(Value::as_str)?.trim();
            let prompt = tool_call
                .get("arguments")
                .and_then(|arguments| arguments.get("prompt"))
                .and_then(Value::as_str)?
                .trim();
            (!id.is_empty() && !prompt.is_empty()).then(|| (id.to_string(), prompt.to_string()))
        })
        .collect::<std::collections::HashMap<_, _>>();
    if agent_prompts_by_call_id.is_empty() {
        return Vec::new();
    }

    response_payload
        .get("tool_results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool_result| {
            let tool_call_id = tool_result
                .get("tool_call_id")
                .and_then(Value::as_str)?
                .trim();
            let prompt = agent_prompts_by_call_id.get(tool_call_id)?;
            let content = tool_result.get("content").and_then(Value::as_str)?.trim();
            if content.is_empty() || content.starts_with("<tool_use_error>") {
                return None;
            }
            Some(AnthropicAgentToolResult {
                tool_call_id: tool_call_id.to_string(),
                prompt: prompt.clone(),
                content: content.to_string(),
            })
        })
        .collect()
}

fn is_anthropic_claude_code_subagent_run(flow_run: &domain::FlowRunRecord) -> bool {
    flow_run.compatibility_mode.as_deref() == Some(ANTHROPIC_MESSAGES_COMPATIBILITY_MODE)
        && application_public_run_system(&flow_run.input_payload)
            .is_some_and(|system| is_claude_code_subagent_system(&system))
}

fn is_claude_code_subagent_system(system: &str) -> bool {
    system.contains("cc_is_subagent=true")
        || (system.contains("Agent threads always have their cwd reset between bash calls")
            && system.contains("the parent agent reads your text output"))
}

fn application_public_run_query(payload: &Value) -> Option<String> {
    for source in [payload, application_public_run_start_payload(payload)] {
        if let Some(query) = source
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|query| !query.is_empty())
        {
            return Some(query.to_string());
        }
    }
    None
}

fn application_public_run_system(payload: &Value) -> Option<String> {
    let start_payload = application_public_run_start_payload(payload);
    for system in [
        payload
            .get(NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY)
            .and_then(|context| context.get("system")),
        payload.get("system"),
        start_payload.get("system"),
    ]
    .into_iter()
    .flatten()
    {
        match system {
            Value::String(text) if !text.trim().is_empty() => {
                return Some(text.trim().to_string());
            }
            Value::Array(_) => {
                let blocks =
                    serde_json::from_value::<Vec<NativePromptBlock>>(system.clone()).ok()?;
                let text = blocks
                    .iter()
                    .map(NativePromptBlock::text_content)
                    .filter(|text| !text.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if !text.is_empty() {
                    return Some(text);
                }
            }
            _ => {}
        }
    }
    None
}

fn application_public_run_start_payload(payload: &Value) -> &Value {
    payload
        .get("node-start")
        .or_else(|| payload.get("start"))
        .unwrap_or(payload)
}

fn ensure_callback_is_consumable(
    flow_run: &domain::FlowRunRecord,
    callback_task: &domain::CallbackTaskRecord,
    response_payload: &Value,
) -> Result<()> {
    if flow_run.status != domain::FlowRunStatus::WaitingCallback {
        return Err(ControlPlaneError::Conflict("flow_run_not_waiting_callback").into());
    }
    if callback_task.status != domain::CallbackTaskStatus::Pending {
        return Err(ControlPlaneError::Conflict("callback_task_not_pending").into());
    }
    if callback_task.callback_kind == "llm_tool_calls" {
        crate::orchestration_runtime::ensure_llm_tool_callback_results_complete(
            &callback_task.request_payload,
            response_payload,
        )?;
    }
    Ok(())
}

fn published_run_belongs_to_actor(
    flow_run: &domain::FlowRunRecord,
    application_id: Uuid,
    api_key_id: Uuid,
) -> bool {
    flow_run.run_mode == domain::FlowRunMode::PublishedApiRun
        && flow_run.application_id == application_id
        && flow_run.api_key_id == Some(api_key_id)
}

fn escape_json_nul_characters(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace('\0', "\\u0000")),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(escape_json_nul_characters).collect())
        }
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, escape_json_nul_characters(value)))
                .collect(),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) => value,
    }
}
