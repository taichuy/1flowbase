use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::{ApplicationApiKeyActor, ApplicationApiKeyService},
    conversations::{
        ApplicationPublicConversationRepository, BindApplicationPublicConversationInput,
    },
    native::{
        CreateNativeRunCommand, NativeInputMapper, NativeRunRequest, NativeRunResult,
        NativeRunStatus, NativeRunValidationError,
    },
    publications::ApplicationPublicationVersionRecord,
};
use crate::{
    audit::audit_log,
    flow_run_title::build_flow_run_title,
    ports::{
        ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
        ApplicationRepository, AuthRepository, CacheStore, CreateFlowRunInput,
    },
    state_transition::ensure_flow_run_transition,
};

pub struct ApplicationPublishedRunService<R> {
    repository: R,
    last_used_cache: Option<Arc<dyn CacheStore>>,
}

impl<R> ApplicationPublishedRunService<R>
where
    R: ApplicationRepository
        + ApiKeyRepository
        + AuthRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationPublishedFlowRunRepository
        + ApplicationPublishedRunControlRepository
        + ApplicationPublicConversationRepository
        + Clone,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            last_used_cache: None,
        }
    }

    pub fn with_last_used_cache(mut self, cache: Arc<dyn CacheStore>) -> Self {
        self.last_used_cache = Some(cache);
        self
    }

    pub async fn start_native_run(
        &self,
        command: CreateNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let mut api_key_service = ApplicationApiKeyService::new(self.repository.clone());
        if let Some(cache) = &self.last_used_cache {
            api_key_service = api_key_service.with_last_used_cache(cache.clone());
        }
        let actor = api_key_service
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        self.ensure_application_exists(&actor).await?;

        let publication = self.load_enabled_publication(&actor).await?;
        let request = self
            .bind_conversation(actor.application_id, actor.api_key_id, command.request)
            .await?;

        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await
            .map_err(|_| NativeRunValidationError::ApplicationNotPublished)?
            .ok_or(NativeRunValidationError::ApplicationNotPublished)?;

        let mapped = NativeInputMapper::map(&request, &publication.mapping_snapshot)
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;
        let metadata = mapped.metadata;
        let idempotency_key = metadata
            .get("idempotency_key")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        if let Some(idempotency_key) = idempotency_key.as_deref() {
            if let Some(flow_run) = self
                .repository
                .find_published_flow_run_by_idempotency_key(
                    actor.application_id,
                    actor.api_key_id,
                    idempotency_key,
                )
                .await
                .map_err(|_| NativeRunValidationError::InvalidMapping)?
            {
                return Ok(native_result_from_flow_run(&flow_run, metadata));
            }
        }

        let started_at = OffsetDateTime::now_utc();
        let flow_run = self
            .repository
            .create_published_flow_run(&CreateFlowRunInput {
                actor_user_id: actor.creator_user_id,
                application_id: actor.application_id,
                flow_id: publication.flow_id,
                flow_draft_id: compiled_plan.draft_id,
                compiled_plan_id: publication.compiled_plan_id,
                debug_session_id: String::new(),
                flow_schema_version: publication.flow_schema_version.clone(),
                document_hash: publication.document_hash.clone(),
                run_mode: domain::FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: build_flow_run_title(request.title.as_deref(), &request.query),
                status: domain::FlowRunStatus::Queued,
                input_payload: mapped.node_input_payload,
                started_at,
                api_key_id: Some(actor.api_key_id),
                publication_version_id: Some(publication.id),
                external_user: metadata
                    .get("external_user")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                external_conversation_id: metadata
                    .get("external_conversation_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                external_trace_id: metadata
                    .get("external_trace_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                compatibility_mode: metadata
                    .get("compatibility_mode")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                idempotency_key,
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;

        self.append_started_audit(&actor, &publication, &flow_run, &metadata)
            .await;

        Ok(native_result_from_flow_run(&flow_run, metadata))
    }

    pub async fn cancel_published_run(
        &self,
        actor: &ApplicationApiKeyActor,
        flow_run: &domain::FlowRunRecord,
    ) -> std::result::Result<domain::FlowRunRecord, NativeRunValidationError> {
        ensure_flow_run_transition(
            flow_run.status,
            domain::FlowRunStatus::Cancelled,
            "cancel_published_api_run",
        )
        .map_err(|_| NativeRunValidationError::InvalidState)?;
        let cancelled = self
            .repository
            .cancel_published_flow_run(&CancelPublishedFlowRunInput {
                flow_run_id: flow_run.id,
                from_status: flow_run.status,
                output_payload: flow_run.output_payload.clone(),
                error_payload: flow_run.error_payload.clone(),
                finished_at: OffsetDateTime::now_utc(),
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidState)?
            .unwrap_or_else(|| flow_run.clone());

        self.append_cancelled_audit(actor, &cancelled).await;

        Ok(cancelled)
    }

    async fn ensure_application_exists(
        &self,
        actor: &ApplicationApiKeyActor,
    ) -> std::result::Result<(), NativeRunValidationError> {
        self.repository
            .get_application(actor.workspace_id, actor.application_id)
            .await
            .map_err(|_| NativeRunValidationError::ApplicationNotPublished)?
            .ok_or(NativeRunValidationError::ApplicationNotPublished)?;
        Ok(())
    }

    async fn load_enabled_publication(
        &self,
        actor: &ApplicationApiKeyActor,
    ) -> std::result::Result<ApplicationPublicationVersionRecord, NativeRunValidationError> {
        let publication = self
            .repository
            .load_active_application_publication(actor.application_id)
            .await
            .map_err(|_| NativeRunValidationError::ApplicationNotPublished)?;
        let Some(publication) = publication.filter(|publication| publication.api_enabled) else {
            self.append_denied_audit(actor, "application_not_published")
                .await;
            return Err(NativeRunValidationError::ApplicationNotPublished);
        };
        Ok(publication)
    }

    async fn append_denied_audit(&self, actor: &ApplicationApiKeyActor, reason: &str) {
        let _ = ApplicationRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(actor.workspace_id),
                Some(actor.creator_user_id),
                "application_public_api_run",
                None,
                "application_public_api.run_denied",
                json!({
                    "api_key_id": actor.api_key_id,
                    "application_id": actor.application_id,
                    "creator_user_id": actor.creator_user_id,
                    "reason": reason,
                }),
            ),
        )
        .await;
    }

    async fn append_started_audit(
        &self,
        actor: &ApplicationApiKeyActor,
        publication: &ApplicationPublicationVersionRecord,
        flow_run: &domain::FlowRunRecord,
        metadata: &Value,
    ) {
        let payload = json!({
            "api_key_id": actor.api_key_id,
            "application_id": actor.application_id,
            "publication_version_id": publication.id,
            "creator_user_id": actor.creator_user_id,
            "external_user": flow_run.external_user,
            "external_conversation_id": flow_run.external_conversation_id,
            "external_trace_id": flow_run.external_trace_id,
            "response_mode": metadata
                .get("request")
                .and_then(|request| request.get("response_mode"))
                .cloned()
                .unwrap_or(Value::Null),
            "compatibility_mode": flow_run.compatibility_mode,
        });

        let _ = self
            .repository
            .append_published_run_event(&crate::ports::AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: "public_run_started".to_string(),
                payload: payload.clone(),
            })
            .await;
        let _ = ApplicationRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(actor.workspace_id),
                Some(actor.creator_user_id),
                "application_public_api_run",
                Some(flow_run.id),
                "application_public_api.run_started",
                payload,
            ),
        )
        .await;
    }

    async fn append_cancelled_audit(
        &self,
        actor: &ApplicationApiKeyActor,
        flow_run: &domain::FlowRunRecord,
    ) {
        let payload = json!({
            "api_key_id": actor.api_key_id,
            "application_id": actor.application_id,
            "publication_version_id": flow_run.publication_version_id,
            "creator_user_id": actor.creator_user_id,
            "external_user": flow_run.external_user,
            "external_conversation_id": flow_run.external_conversation_id,
            "external_trace_id": flow_run.external_trace_id,
            "compatibility_mode": flow_run.compatibility_mode,
            "reason": "manual_stop",
        });
        let _ = self
            .repository
            .append_published_run_event(&crate::ports::AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: "public_run_cancelled".to_string(),
                payload: payload.clone(),
            })
            .await;
        let _ = ApplicationRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(actor.workspace_id),
                Some(actor.creator_user_id),
                "application_public_api_run",
                Some(flow_run.id),
                "application_public_api.run_cancelled",
                payload,
            ),
        )
        .await;
    }

    async fn bind_conversation(
        &self,
        application_id: Uuid,
        api_key_id: Uuid,
        mut request: NativeRunRequest,
    ) -> std::result::Result<NativeRunRequest, NativeRunValidationError> {
        let Some(external_user) = request
            .expand_id
            .clone()
            .or_else(|| request.conversation.string("user"))
        else {
            return Ok(request);
        };
        request
            .conversation
            .insert_string("user", external_user.clone());
        let external_conversation_id = request
            .conversation
            .string("id")
            .unwrap_or_else(generate_external_conversation_id);
        request
            .conversation
            .insert_string("id", external_conversation_id.clone());
        self.repository
            .bind_application_public_conversation(&BindApplicationPublicConversationInput {
                application_id,
                api_key_id,
                external_user,
                external_conversation_id,
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;
        Ok(request)
    }
}

#[async_trait]
pub trait ApplicationPublishedFlowRunRepository: Send + Sync {
    async fn create_published_flow_run(
        &self,
        input: &CreateFlowRunInput,
    ) -> Result<domain::FlowRunRecord>;

    async fn find_published_flow_run_by_idempotency_key(
        &self,
        application_id: Uuid,
        api_key_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn append_published_run_event(
        &self,
        input: &crate::ports::AppendRunEventInput,
    ) -> Result<domain::RunEventRecord>;
}

#[derive(Debug, Clone)]
pub struct CancelPublishedFlowRunInput {
    pub flow_run_id: Uuid,
    pub from_status: domain::FlowRunStatus,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub finished_at: OffsetDateTime,
}

#[async_trait]
pub trait ApplicationPublishedRunControlRepository: Send + Sync {
    async fn get_published_flow_run(
        &self,
        flow_run_id: Uuid,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn cancel_published_flow_run(
        &self,
        input: &CancelPublishedFlowRunInput,
    ) -> Result<Option<domain::FlowRunRecord>>;

    async fn get_published_callback_task(
        &self,
        callback_task_id: Uuid,
    ) -> Result<Option<domain::CallbackTaskRecord>>;
}

pub fn native_result_from_flow_run(
    flow_run: &domain::FlowRunRecord,
    metadata: Value,
) -> NativeRunResult {
    let error = flow_run.error_payload.as_ref().map(|payload| {
        let message = payload
            .get("message")
            .or_else(|| payload.get("error"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| payload.to_string());
        super::native::NativeError {
            code: payload
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("runtime_error")
                .to_string(),
            message,
            details: payload.clone(),
        }
    });
    NativeRunResult {
        id: flow_run.id,
        application_id: flow_run.application_id,
        api_key_id: flow_run.api_key_id.unwrap_or_default(),
        publication_version_id: flow_run.publication_version_id.unwrap_or_default(),
        status: native_status(flow_run.status),
        node_input_payload: flow_run.input_payload.clone(),
        metadata,
        answer: extract_answer(&flow_run.output_payload),
        reasoning: None,
        required_action: None,
        tool_calls: extract_tool_calls(&flow_run.output_payload),
        usage: extract_usage(&flow_run.output_payload),
        error,
        created_at: flow_run.created_at,
    }
}

pub fn native_result_from_application_run_detail(
    detail: &domain::ApplicationRunDetail,
    metadata: Value,
) -> NativeRunResult {
    let mut result = native_result_from_flow_run(&detail.flow_run, metadata);
    result.reasoning = extract_reasoning(&detail.events);
    result
}

fn extract_answer(output_payload: &Value) -> Option<String> {
    output_payload
        .get("answer")
        .or_else(|| output_payload.get("text"))
        .or_else(|| output_payload.get("output"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_reasoning(events: &[domain::RunEventRecord]) -> Option<String> {
    let reasoning = events
        .iter()
        .filter(|event| event.event_type == "reasoning_delta")
        .filter_map(|event| {
            event
                .payload
                .get("text")
                .or_else(|| event.payload.get("delta"))
                .and_then(Value::as_str)
        })
        .collect::<String>();

    (!reasoning.is_empty()).then_some(reasoning)
}

fn extract_tool_calls(output_payload: &Value) -> Option<Value> {
    output_payload
        .get("tool_calls")
        .filter(|value| value.is_array())
        .cloned()
}

fn extract_usage(output_payload: &Value) -> Option<super::native::NativeUsage> {
    let usage = output_payload.get("usage")?;
    Some(super::native::NativeUsage {
        prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
        completion_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
    })
}

fn generate_external_conversation_id() -> String {
    format!("conv_{}", Uuid::now_v7().simple())
}

fn native_status(status: domain::FlowRunStatus) -> NativeRunStatus {
    match status {
        domain::FlowRunStatus::Queued => NativeRunStatus::Queued,
        domain::FlowRunStatus::Running => NativeRunStatus::Running,
        domain::FlowRunStatus::WaitingCallback | domain::FlowRunStatus::WaitingHuman => {
            NativeRunStatus::Waiting
        }
        domain::FlowRunStatus::Paused => NativeRunStatus::Running,
        domain::FlowRunStatus::Succeeded => NativeRunStatus::Succeeded,
        domain::FlowRunStatus::Failed => NativeRunStatus::Failed,
        domain::FlowRunStatus::Cancelled => NativeRunStatus::Cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn flow_run(output_payload: Value) -> domain::FlowRunRecord {
        domain::FlowRunRecord {
            id: Uuid::nil(),
            application_id: Uuid::nil(),
            flow_id: Uuid::nil(),
            draft_id: Uuid::nil(),
            compiled_plan_id: None,
            debug_session_id: String::new(),
            flow_schema_version: "1flowbase.flow/v2".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "test".to_string(),
            status: domain::FlowRunStatus::Succeeded,
            input_payload: json!({}),
            output_payload,
            error_payload: None,
            created_by: Uuid::nil(),
            authorized_account: None,
            api_key_id: Some(Uuid::nil()),
            publication_version_id: Some(Uuid::nil()),
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn run_event(sequence: i64, event_type: &str, text: &str) -> domain::RunEventRecord {
        domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::nil(),
            node_run_id: Some(Uuid::nil()),
            sequence,
            event_type: event_type.to_string(),
            payload: json!({ "text": text }),
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn native_result_from_application_run_detail_restores_reasoning_deltas() {
        let detail = domain::ApplicationRunDetail {
            flow_run: flow_run(json!({ "answer": "正式回答" })),
            node_runs: Vec::new(),
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: vec![
                run_event(1, "reasoning_delta", "先分析"),
                run_event(2, "text_delta", "正式回答"),
                run_event(3, "reasoning_delta", "用户意图"),
            ],
        };

        let result = native_result_from_application_run_detail(&detail, json!({}));

        assert_eq!(result.answer.as_deref(), Some("正式回答"));
        assert_eq!(result.reasoning.as_deref(), Some("先分析用户意图"));
    }
}
