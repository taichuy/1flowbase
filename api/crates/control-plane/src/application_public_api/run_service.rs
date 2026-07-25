use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::{ApplicationApiKeyActor, ApplicationApiKeyService},
    callback_resume::ApplicationPublishedCallbackAttemptRepository,
    conversations::{
        ApplicationPublicConversationRepository, BindApplicationPublicConversationInput,
        ListApplicationPublicConversationMessagesInput,
    },
    native::{
        CreateNativeRunCommand, NativeInputMapper, NativeRunRequest, NativeRunResult,
        NativeRunValidationError,
    },
    publications::ApplicationPublicationVersionRecord,
};
mod conversation_history;
mod native_results;
mod repository_contracts;
mod run_input;

use crate::{
    audit::audit_log,
    flow_run_title::build_flow_run_title,
    ports::{
        ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
        ApplicationRepository, AuthRepository, CacheStore, CreateFlowRunInput,
    },
    state_transition::ensure_flow_run_transition,
};
use conversation_history::application_public_conversation_messages_to_native_history;
pub use native_results::{
    native_result_from_flow_run, native_result_from_run_detail, native_result_from_run_stream_state,
};
pub use repository_contracts::{
    ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
    CancelPublishedFlowRunInput, CreatePublishedFlowRunResult,
    ListWaitingCallbackPublishedRunsInput, PublishedRunNodeUsage, PublishedRunPendingCallback,
    PublishedRunStreamState,
};
use run_input::{
    compiled_plan_start_node_id, enrich_anthropic_context_beta_from_start_model,
    freeze_run_input_environment, generate_external_conversation_id,
};
pub(crate) use run_input::{
    compiled_plan_start_node_id as public_compiled_plan_start_node_id,
    freeze_workflow_run_input_environment as public_freeze_workflow_run_input_environment,
    WorkflowRunTriggerContext,
};

const APPLICATION_PUBLIC_CONVERSATION_HISTORY_LIMIT: i64 = 50;
const PUBLIC_RUN_IDEMPOTENCY_FINGERPRINT: &str = "public_run_idempotency_fingerprint";
const PUBLIC_PROVIDER_TRANSPORT_SUMMARY: &str = "public_provider_transport";
const PUBLISHED_RUN_CANCELLED_ERROR_CODE: &str = "cancelled";
const PUBLISHED_RUN_CANCELLED_ERROR_MESSAGE: &str = "published run cancelled";

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
        + ApplicationPublishedCallbackAttemptRepository
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
        self.start_agentflow_run(command).await
    }

    async fn start_agentflow_run(
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
        let mut client_request = command.request;
        enrich_anthropic_context_beta_from_start_model(
            &mut client_request,
            &publication.document_snapshot,
        );
        // The request model is workflow input. Only the LLM node selected by the
        // graph owns provider/model capability validation.
        let external_model_parameters = client_request.execution.model_parameters().cloned();
        let idempotency_key = client_request
            .execution
            .idempotency_key()
            .map(ToOwned::to_owned);
        let idempotency_fingerprint = idempotency_key
            .as_ref()
            .map(|_| public_run_idempotency_fingerprint(&client_request))
            .transpose()?;
        let provider_transport_summary = client_request.metadata.provider_transport_summary_value();
        let request = self
            .bind_conversation(actor.application_id, actor.api_key_id, client_request)
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

        if let Some(idempotency_key) = idempotency_key.as_deref() {
            if let Some(flow_run) = self
                .repository
                .find_published_flow_run_by_idempotency_key(
                    actor.application_id,
                    Some(actor.api_key_id),
                    idempotency_key,
                )
                .await
                .map_err(|_| NativeRunValidationError::InvalidMapping)?
            {
                ensure_idempotency_fingerprint_matches(
                    &flow_run,
                    idempotency_fingerprint.as_deref(),
                )?;
                return Ok(native_result_from_flow_run(&flow_run, metadata));
            }
        }

        let environment_variables = self
            .repository
            .list_application_environment_variables(actor.workspace_id, actor.application_id)
            .await
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;
        let started_at = OffsetDateTime::now_utc();
        let input_payload = freeze_run_input_environment(
            mapped.node_input_payload,
            &environment_variables,
            external_model_parameters.as_ref(),
            compiled_plan_start_node_id(&compiled_plan.plan).as_deref(),
        );
        let input_payload = with_public_run_idempotency_fingerprint(
            input_payload,
            idempotency_fingerprint.as_deref(),
        );
        let input_payload =
            with_public_provider_transport_summary(input_payload, provider_transport_summary);
        let created = self
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
                input_payload,
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
                compatibility_mode: None,
                idempotency_key,
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;
        let flow_run = created.flow_run;
        if !created.created {
            ensure_idempotency_fingerprint_matches(&flow_run, idempotency_fingerprint.as_deref())?;
            return Ok(native_result_from_flow_run(&flow_run, metadata));
        }

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
        let terminal_event =
            crate::orchestration_runtime::debug_stream_events::flow_cancelled(flow_run.id);
        let cancelled = self
            .repository
            .cancel_published_flow_run(&CancelPublishedFlowRunInput {
                flow_run_id: flow_run.id,
                from_status: flow_run.status,
                // Cancellation owns a failed terminal result. Do not retain a
                // stale answer/error payload that could make it look successful.
                output_payload: json!({}),
                error_payload: Some(json!({
                    "code": PUBLISHED_RUN_CANCELLED_ERROR_CODE,
                    "message": PUBLISHED_RUN_CANCELLED_ERROR_MESSAGE,
                })),
                flow_run_event_payload: json!({
                    "code": PUBLISHED_RUN_CANCELLED_ERROR_CODE,
                    "message": PUBLISHED_RUN_CANCELLED_ERROR_MESSAGE,
                }),
                terminal_event_payload: terminal_event.payload,
                finished_at: OffsetDateTime::now_utc(),
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidState)?;
        let (cancelled, cancellation_won) = match cancelled {
            crate::ports::CommitFlowRunTerminalReceipt::Winner(cancelled) => (cancelled, true),
            crate::ports::CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(
                cancelled,
            ) => {
                tracing::warn!(
                    flow_run_id = %cancelled.id,
                    application_id = %cancelled.application_id,
                    "published cancellation committed with a post-commit projection warning"
                );
                (cancelled, true)
            }
            crate::ports::CommitFlowRunTerminalReceipt::Loser => (
                self.repository
                    .get_published_flow_run(flow_run.id)
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidState)?
                    .ok_or(NativeRunValidationError::InvalidState)?,
                false,
            ),
        };

        if cancellation_won {
            self.append_cancelled_audit(actor, &cancelled).await;
        }

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
        let admission =
            crate::orchestration_runtime::scheduler_admission::derive_scheduler_admission_metadata(
                flow_run,
            );
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
            "admission": admission,
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
                external_user: external_user.clone(),
                external_conversation_id: external_conversation_id.clone(),
            })
            .await
            .map_err(|_| NativeRunValidationError::InvalidMapping)?;
        if request.history.is_empty() {
            let messages = self
                .repository
                .list_application_public_conversation_messages(
                    &ListApplicationPublicConversationMessagesInput {
                        application_id,
                        api_key_id,
                        external_user,
                        external_conversation_id,
                        limit: APPLICATION_PUBLIC_CONVERSATION_HISTORY_LIMIT,
                    },
                )
                .await
                .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            request.history = application_public_conversation_messages_to_native_history(messages);
        }
        Ok(request)
    }
}

fn public_run_idempotency_fingerprint(
    request: &NativeRunRequest,
) -> std::result::Result<String, NativeRunValidationError> {
    let mut value =
        serde_json::to_value(request).map_err(|_| NativeRunValidationError::InvalidMapping)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "execution".to_string(),
            request.execution.fingerprint_value(),
        );
        if let Some(summary) = request.metadata.provider_transport_summary_value() {
            object.insert("provider_transport".to_string(), summary);
        }
    }
    let mut canonical = Vec::new();
    write_canonical_json(&value, &mut canonical)
        .map_err(|_| NativeRunValidationError::InvalidMapping)?;
    let hash = Sha256::digest(canonical);
    Ok(format!("sha256:{}", hex_lower(&hash)))
}

fn with_public_provider_transport_summary(
    mut input_payload: Value,
    summary: Option<Value>,
) -> Value {
    let Some(summary) = summary else {
        return input_payload;
    };
    let payload = input_payload
        .as_object_mut()
        .expect("frozen run input payload");
    let sys = payload.entry("sys").or_insert_with(|| json!({}));
    if !sys.is_object() {
        *sys = json!({});
    }
    sys.as_object_mut()
        .expect("sys payload")
        .insert(PUBLIC_PROVIDER_TRANSPORT_SUMMARY.to_string(), summary);
    input_payload
}

fn write_canonical_json(value: &Value, out: &mut Vec<u8>) -> serde_json::Result<()> {
    match value {
        Value::Object(object) => {
            out.push(b'{');
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key)?;
                out.push(b':');
                write_canonical_json(&object[key], out)?;
            }
            out.push(b'}');
            Ok(())
        }
        Value::Array(values) => {
            out.push(b'[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_canonical_json(item, out)?;
            }
            out.push(b']');
            Ok(())
        }
        _ => serde_json::to_writer(out, value),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn with_public_run_idempotency_fingerprint(
    mut input_payload: Value,
    fingerprint: Option<&str>,
) -> Value {
    let Some(fingerprint) = fingerprint else {
        return input_payload;
    };
    let payload = input_payload
        .as_object_mut()
        .expect("frozen run input payload");
    let sys = payload.entry("sys").or_insert_with(|| json!({}));
    if !sys.is_object() {
        *sys = json!({});
    }
    sys.as_object_mut().expect("sys payload").insert(
        PUBLIC_RUN_IDEMPOTENCY_FINGERPRINT.to_string(),
        Value::String(fingerprint.to_string()),
    );
    input_payload
}

fn ensure_idempotency_fingerprint_matches(
    flow_run: &domain::FlowRunRecord,
    expected: Option<&str>,
) -> std::result::Result<(), NativeRunValidationError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let stored = flow_run
        .input_payload
        .get("sys")
        .and_then(Value::as_object)
        .and_then(|sys| sys.get(PUBLIC_RUN_IDEMPOTENCY_FINGERPRINT))
        .and_then(Value::as_str);
    if stored == Some(expected) {
        return Ok(());
    }
    Err(NativeRunValidationError::IdempotencyConflict)
}

#[cfg(test)]
mod tests {
    use super::super::conversations::ApplicationPublicConversationMessageRecord;
    use super::*;
    use serde_json::json;
    use time::OffsetDateTime;
    use uuid::Uuid;

    #[test]
    fn native_result_from_run_detail_aggregates_node_usage_when_flow_output_has_none() {
        let detail = domain::ApplicationRunDetail {
            flow_run: test_flow_run(json!({ "answer": "ok" })),
            node_runs: vec![
                test_node_run(
                    "node-llm",
                    json!({
                        "input_tokens": 47,
                        "output_tokens": 59,
                        "total_tokens": 106,
                        "input_cache_hit_tokens": 4,
                        "cache_write_tokens": 2
                    }),
                ),
                test_node_run(
                    "node-llm-1",
                    json!({
                        "input_tokens": 196,
                        "output_tokens": 1120,
                        "total_tokens": 1316,
                        "reasoning_tokens": 1063,
                        "cache_read_tokens": 8
                    }),
                ),
            ],
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: Vec::new(),
            stitched_trace: Vec::new(),
            subagent_traces: Vec::new(),
        };

        let run = native_result_from_run_detail(&detail, json!({}));
        let usage = run.usage.expect("node usage should be projected");

        assert_eq!(usage.prompt_tokens, Some(243));
        assert_eq!(usage.completion_tokens, Some(1179));
        assert_eq!(usage.total_tokens, Some(1422));
        assert_eq!(usage.reasoning_tokens, Some(1063));
        assert_eq!(usage.input_cache_hit_tokens, Some(4));
        assert_eq!(usage.cache_read_tokens, Some(8));
        assert_eq!(usage.cache_write_tokens, Some(2));
    }

    #[test]
    fn native_result_from_run_detail_prefers_flow_output_usage_selector() {
        let detail = domain::ApplicationRunDetail {
            flow_run: test_flow_run(json!({
                "answer": "ok",
                "usage": {
                    "prompt_tokens": 11,
                    "completion_tokens": 7,
                    "total_tokens": 18
                }
            })),
            node_runs: vec![test_node_run(
                "node-llm",
                json!({
                    "input_tokens": 100,
                    "output_tokens": 200,
                    "total_tokens": 300
                }),
            )],
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: Vec::new(),
            stitched_trace: Vec::new(),
            subagent_traces: Vec::new(),
        };

        let run = native_result_from_run_detail(&detail, json!({}));
        let usage = run.usage.expect("flow usage should be projected");

        assert_eq!(usage.prompt_tokens, Some(11));
        assert_eq!(usage.completion_tokens, Some(7));
        assert_eq!(usage.total_tokens, Some(18));
    }

    #[test]
    fn native_result_from_run_detail_projects_structured_answer_segments() {
        let detail = domain::ApplicationRunDetail {
            flow_run: test_flow_run(json!({
                "answer": "<think>旧思考</think>旧回答",
                "answer_segments": [
                    { "kind": "reasoning", "text": "结构化思考" },
                    { "kind": "message", "text": "结构化回答" }
                ]
            })),
            node_runs: Vec::new(),
            checkpoints: Vec::new(),
            callback_tasks: Vec::new(),
            events: Vec::new(),
            stitched_trace: Vec::new(),
            subagent_traces: Vec::new(),
        };

        let run = native_result_from_run_detail(&detail, json!({}));

        assert_eq!(run.answer, Some("<think>旧思考</think>旧回答".to_string()));
        assert_eq!(
            serde_json::to_value(run.answer_segments.expect("segments should be projected"))
                .expect("segments should serialize"),
            json!([
                { "kind": "reasoning", "text": "结构化思考" },
                { "kind": "message", "text": "结构化回答" }
            ])
        );
    }

    #[test]
    fn conversation_history_rehydration_preserves_canonical_order_and_marker_like_text() {
        let messages = vec![
            conversation_message(
                "user",
                "<system-reminder>internal skills</system-reminder>\n\nhi ?",
                1,
            ),
            conversation_message(
                "assistant",
                "<think>private reasoning</think>嗨，有什么需要我帮忙的？",
                2,
            ),
            conversation_message("user", "Describe image", 3),
            conversation_message(
                "assistant",
                "<think>need file</think><tool_call>read image</tool_call>Reading image",
                4,
            ),
            conversation_message("user", "Describe image", 5),
            conversation_message(
                "assistant",
                "<think>done</think>The diagram is an Agent scheduler.",
                6,
            ),
            conversation_message("user", "Find related code", 7),
            conversation_message(
                "assistant",
                "<think>search</think>Raw preface<tool_call>grep</tool_call>\n\n---\n\n下面是美化后内容\n\nFinal visible answer",
                8,
            ),
        ];

        let history = application_public_conversation_messages_to_native_history(messages);

        assert_eq!(
            history,
            vec![
                json!({"role": "user", "content": "<system-reminder>internal skills</system-reminder>\n\nhi ?"}),
                json!({"role": "assistant", "content": "<think>private reasoning</think>嗨，有什么需要我帮忙的？"}),
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "<think>need file</think><tool_call>read image</tool_call>Reading image"}),
                json!({"role": "user", "content": "Describe image"}),
                json!({"role": "assistant", "content": "<think>done</think>The diagram is an Agent scheduler."}),
                json!({"role": "user", "content": "Find related code"}),
                json!({"role": "assistant", "content": "<think>search</think>Raw preface<tool_call>grep</tool_call>\n\n---\n\n下面是美化后内容\n\nFinal visible answer"}),
            ]
        );
    }

    fn test_flow_run(output_payload: Value) -> domain::FlowRunRecord {
        let created_at = OffsetDateTime::UNIX_EPOCH;
        domain::FlowRunRecord {
            id: Uuid::from_u128(0x11111111111111111111111111111111),
            application_id: Uuid::from_u128(0x22222222222222222222222222222222),
            flow_id: Uuid::from_u128(0x33333333333333333333333333333333),
            draft_id: Uuid::from_u128(0x44444444444444444444444444444444),
            compiled_plan_id: Some(Uuid::from_u128(0x55555555555555555555555555555555)),
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
            created_by: Uuid::from_u128(0x66666666666666666666666666666666),
            authorized_account: None,
            api_key_id: Some(Uuid::from_u128(0x77777777777777777777777777777777)),
            publication_version_id: Some(Uuid::from_u128(0x88888888888888888888888888888888)),
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: Some("anthropic-messages-v1".to_string()),
            idempotency_key: None,
            started_at: created_at,
            finished_at: Some(created_at),
            created_at,
            updated_at: created_at,
        }
    }

    fn test_node_run(node_id: &str, usage: Value) -> domain::NodeRunRecord {
        let created_at = OffsetDateTime::UNIX_EPOCH;
        domain::NodeRunRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::from_u128(0x11111111111111111111111111111111),
            node_id: node_id.to_string(),
            node_type: "llm".to_string(),
            node_alias: node_id.to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({}),
            output_payload: json!({ "usage": usage.clone() }),
            error_payload: None,
            metrics_payload: json!({ "usage": usage }),
            debug_payload: json!({}),
            started_at: created_at,
            finished_at: Some(created_at),
        }
    }

    fn conversation_message(
        role: &str,
        content: &str,
        sequence: i64,
    ) -> ApplicationPublicConversationMessageRecord {
        ApplicationPublicConversationMessageRecord {
            role: role.to_string(),
            content: content.to_string(),
            sequence,
        }
    }
}
