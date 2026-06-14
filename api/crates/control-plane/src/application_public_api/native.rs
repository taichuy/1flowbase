use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::ApplicationApiKeyService,
    callback_resume::ApplicationPublishedCallbackAttemptRepository,
    conversations::ApplicationPublicConversationRepository,
    mapping::ApplicationApiMappingConfig,
    run_service::{
        ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
        ApplicationPublishedRunService,
    },
};
use crate::flow_run_title::build_flow_run_title;
use crate::ports::{
    ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
    ApplicationRepository, AuthRepository, CacheStore,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRunRequest {
    pub query: String,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub system: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub model: Option<String>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub inputs: NativeObject,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub attachments: Vec<Value>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub conversation: NativeObject,
    #[serde(
        rename = "expand_id",
        default,
        deserialize_with = "deserialize_optional_string_reject_null"
    )]
    pub expand_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub response_mode: Option<String>,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub stream_options: NativeObject,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub execution: NativeObject,
    #[serde(default, deserialize_with = "deserialize_native_object")]
    pub metadata: NativeObject,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_reject_null")]
    pub compatibility_mode: Option<String>,
    // Protocol mappers set this after deserialization; public Native JSON cannot own wire policy.
    #[serde(skip)]
    pub protocol_compatibility_mode: Option<String>,
    // Protocol mappers set this after deserialization; public Native JSON cannot own run-control policy.
    #[serde(skip)]
    pub protocol_request_kind: Option<NativeProtocolRequestKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProtocolRequestKind {
    AnthropicToolResultContinuation,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NativeObject(Map<String, Value>);

impl NativeObject {
    pub fn into_value(self) -> Value {
        Value::Object(self.0)
    }

    pub fn as_value(&self) -> Value {
        Value::Object(self.0.clone())
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        string_field(self, key)
    }

    pub fn insert_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.0.insert(key.into(), Value::String(value.into()));
    }
}

impl std::ops::Index<&str> for NativeObject {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        self.0.index(index)
    }
}

impl<'de> Deserialize<'de> for NativeObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Object(object) => Ok(Self(object)),
            Value::Null => Err(de::Error::custom("expected object, found null")),
            _ => Err(de::Error::custom("expected object")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeAttachmentSource {
    UploadFileId,
    Url,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeAttachment {
    pub source: NativeAttachmentSource,
    pub value: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRunResult {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub publication_version_id: Uuid,
    pub status: NativeRunStatus,
    pub node_input_payload: Value,
    pub metadata: Value,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub required_action: Option<NativeRequiredAction>,
    #[serde(default)]
    pub tool_calls: Option<Value>,
    #[serde(default)]
    pub usage: Option<NativeUsage>,
    #[serde(default)]
    pub error: Option<NativeError>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRunStatus {
    Created,
    Queued,
    Running,
    Waiting,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeRequiredAction {
    pub action_type: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeUsage {
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    #[serde(default)]
    pub completion_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_hit_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_miss_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeMappedInput {
    pub node_input_payload: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeInputMappingError {
    SelectorCollision { selector: String },
    InvalidSelector { selector: String },
}

pub struct NativeInputMapper;

impl NativeInputMapper {
    pub fn map(
        request: &NativeRunRequest,
        mapping: &ApplicationApiMappingConfig,
    ) -> std::result::Result<NativeMappedInput, NativeInputMappingError> {
        let mut node_input_payload = Value::Object(Map::new());
        let input = &mapping.input;

        write_selector(
            &mut node_input_payload,
            &input.query_target,
            Value::String(request.query.clone()),
        )?;
        if let (Some(model), Some(model_target)) = (&request.model, &input.model_target) {
            write_selector(
                &mut node_input_payload,
                model_target,
                Value::String(model.clone()),
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.inputs_target.as_deref(),
            request.inputs.as_value(),
        )?;
        let (system, history) = split_system_context_from_history(request);
        write_optional_selector(
            &mut node_input_payload,
            input.history_target.as_deref(),
            Value::Array(history),
        )?;
        if let Some(system) = system {
            write_optional_selector(
                &mut node_input_payload,
                system_target(input).as_deref(),
                Value::String(system),
            )?;
        }
        write_optional_selector(
            &mut node_input_payload,
            input.attachments_target.as_deref(),
            Value::Array(request.attachments.clone()),
        )?;

        Ok(NativeMappedInput {
            node_input_payload,
            metadata: build_run_metadata(request),
        })
    }
}

fn split_system_context_from_history(request: &NativeRunRequest) -> (Option<String>, Vec<Value>) {
    let mut system_parts = request
        .system
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let mut history = Vec::new();

    for message in &request.history {
        if message.get("role").and_then(Value::as_str) == Some("system") {
            if let Some(content) = message
                .get("content")
                .and_then(native_message_content_text)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                system_parts.push(content.to_string());
            }
            continue;
        }
        history.push(message.clone());
    }

    (
        (!system_parts.is_empty()).then(|| system_parts.join("\n\n")),
        history,
    )
}

fn native_message_content_text(value: &Value) -> Option<&str> {
    value.as_str()
}

fn system_target(input: &super::mapping::ApplicationApiMappingInput) -> Option<String> {
    if let Some(history_target) = input.history_target.as_deref() {
        if let Some(prefix) = history_target.strip_suffix(".history") {
            return Some(format!("{prefix}.system"));
        }
    }

    input
        .inputs_target
        .as_deref()
        .map(|target| format!("{target}.system"))
}

#[derive(Debug, Clone)]
pub struct CreateNativeRunCommand {
    pub bearer_token: String,
    pub request: NativeRunRequest,
}

#[derive(Debug, Clone)]
pub struct GetNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CancelNativeRunCommand {
    pub bearer_token: String,
    pub run_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRunValidationError {
    NotAuthenticated,
    ApplicationNotPublished,
    Forbidden,
    NotFound,
    InvalidMapping,
    InvalidModelParameters(&'static str),
    InvalidToolResults(String),
    InvalidState,
    IdempotencyConflict,
}

pub struct ApplicationNativeRunService<R> {
    repository: R,
    last_used_cache: Option<Arc<dyn CacheStore>>,
}

impl<R> ApplicationNativeRunService<R>
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

    pub async fn create_native_run(
        &self,
        command: CreateNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let run = self
            .published_run_service()
            .start_native_run(command)
            .await?;

        Ok(run)
    }

    pub async fn get_native_run(
        &self,
        command: GetNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;
        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;

        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        let metadata = durable_metadata_from_flow_run(&flow_run);
        if let Some(detail) = self
            .repository
            .get_published_run_detail(actor.application_id, flow_run.id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
        {
            return Ok(super::run_service::native_result_from_run_detail(
                &detail, metadata,
            ));
        }

        Ok(super::run_service::native_result_from_flow_run(
            &flow_run, metadata,
        ))
    }

    pub async fn cancel_native_run(
        &self,
        command: CancelNativeRunCommand,
    ) -> std::result::Result<NativeRunResult, NativeRunValidationError> {
        let actor = self
            .api_key_service()
            .authenticate_bearer_token(&command.bearer_token)
            .await
            .map_err(|_| NativeRunValidationError::NotAuthenticated)?;

        let flow_run = self
            .repository
            .get_published_flow_run(command.run_id)
            .await
            .map_err(|_| NativeRunValidationError::NotFound)?
            .ok_or(NativeRunValidationError::NotFound)?;
        if !published_run_belongs_to_actor(&flow_run, actor.application_id, actor.api_key_id) {
            return Err(NativeRunValidationError::Forbidden);
        }

        let cancelled = self
            .published_run_service()
            .cancel_published_run(&actor, &flow_run)
            .await?;
        if cancelled.status == domain::FlowRunStatus::Cancelled {
            let completed_at = cancelled
                .finished_at
                .unwrap_or_else(OffsetDateTime::now_utc);
            let cancelled_callback_tasks = self
                .repository
                .cancel_published_pending_callback_tasks_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for callback_task in cancelled_callback_tasks {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: Some(callback_task.node_run_id),
                        event_type: "public_run_callback_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": callback_task.id,
                            "callback_kind": callback_task.callback_kind,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
            let cancelled_attempts = self
                .repository
                .cancel_published_callback_resume_attempts_for_run(cancelled.id, completed_at)
                .await
                .map_err(|_| NativeRunValidationError::InvalidState)?;
            for attempt in cancelled_attempts {
                self.repository
                    .append_published_run_event(&crate::ports::AppendRunEventInput {
                        flow_run_id: cancelled.id,
                        node_run_id: None,
                        event_type: "public_run_resume_cancelled".to_string(),
                        payload: json!({
                            "callback_task_id": attempt.callback_task_id,
                            "resume_attempt_id": attempt.id,
                        }),
                    })
                    .await
                    .map_err(|_| NativeRunValidationError::InvalidMapping)?;
            }
        }

        Ok(super::run_service::native_result_from_flow_run(
            &cancelled,
            durable_metadata_from_flow_run(&cancelled),
        ))
    }

    fn api_key_service(&self) -> ApplicationApiKeyService<R> {
        let service = ApplicationApiKeyService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }

    fn published_run_service(&self) -> ApplicationPublishedRunService<R> {
        let service = ApplicationPublishedRunService::new(self.repository.clone());
        match &self.last_used_cache {
            Some(cache) => service.with_last_used_cache(cache.clone()),
            None => service,
        }
    }
}

#[async_trait]
pub trait NativeRunRepository: Send + Sync {
    async fn create_native_run_result(&self, run: &NativeRunResult) -> Result<NativeRunResult>;
    async fn get_native_run_result(&self, run_id: Uuid) -> Result<Option<NativeRunResult>>;
}

fn deserialize_native_object<'de, D>(deserializer: D) -> std::result::Result<NativeObject, D::Error>
where
    D: Deserializer<'de>,
{
    NativeObject::deserialize(deserializer)
}

fn deserialize_optional_string_reject_null<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(value) => Ok(Some(value)),
        Value::Null => Err(de::Error::custom("expected string, found null")),
        _ => Err(de::Error::custom("expected string")),
    }
}

fn build_run_metadata(request: &NativeRunRequest) -> Value {
    let compatibility_mode = request.protocol_compatibility_mode.clone();
    let idempotency_key = string_field(&request.execution, "idempotency_key");
    let external_user = request
        .expand_id
        .clone()
        .or_else(|| string_field(&request.conversation, "user"));
    let external_conversation_id = string_field(&request.conversation, "id");
    let external_trace_id = string_field(&request.metadata, "trace_id");
    let title = build_flow_run_title(request.title.as_deref(), &request.query);

    json!({
        "model": request.model,
        "execution": request.execution.as_value(),
        "metadata": request.metadata.as_value(),
        "title": title,
        "expand_id": external_user,
        "compatibility_mode": compatibility_mode,
        "idempotency_key": idempotency_key,
        "external_user": external_user,
        "external_conversation_id": external_conversation_id,
        "external_trace_id": external_trace_id,
        "request": {
            "conversation": request.conversation.as_value(),
            "response_mode": request.response_mode,
            "stream_options": request.stream_options.as_value()
        }
    })
}

fn durable_metadata_from_flow_run(flow_run: &domain::FlowRunRecord) -> Value {
    json!({
        "title": flow_run.title,
        "expand_id": flow_run.external_user,
        "external_user": flow_run.external_user,
        "external_conversation_id": flow_run.external_conversation_id,
        "external_trace_id": flow_run.external_trace_id,
        "compatibility_mode": flow_run.compatibility_mode,
        "idempotency_key": flow_run.idempotency_key,
        "request": {
            "conversation": {
                "id": flow_run.external_conversation_id,
                "user": flow_run.external_user,
            }
        }
    })
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

fn string_field(object: &NativeObject, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn write_optional_selector(
    root: &mut Value,
    selector: Option<&str>,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let Some(selector) = selector else {
        return Ok(());
    };
    write_selector(root, selector, value)
}

fn write_selector(
    root: &mut Value,
    selector: &str,
    value: Value,
) -> std::result::Result<(), NativeInputMappingError> {
    let parts = selector.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(NativeInputMappingError::InvalidSelector {
            selector: selector.to_string(),
        });
    }

    let mut cursor = root;
    for part in parts.iter().take(parts.len() - 1) {
        let object =
            cursor
                .as_object_mut()
                .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                    selector: selector.to_string(),
                })?;
        cursor = object
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }

    let leaf = parts[parts.len() - 1];
    let object =
        cursor
            .as_object_mut()
            .ok_or_else(|| NativeInputMappingError::SelectorCollision {
                selector: selector.to_string(),
            })?;
    if let Some(existing) = object.get_mut(leaf) {
        if let (Some(existing), Value::Object(next)) = (existing.as_object_mut(), value) {
            for (key, value) in next {
                if existing.contains_key(&key) {
                    return Err(NativeInputMappingError::SelectorCollision {
                        selector: format!("{selector}.{key}"),
                    });
                }
                existing.insert(key, value);
            }
            return Ok(());
        }

        return Err(NativeInputMappingError::SelectorCollision {
            selector: selector.to_string(),
        });
    }
    object.insert(leaf.to_string(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::application_public_api::mapping::{
        ApplicationApiMappingInput, ApplicationApiMappingOutput,
    };

    fn request_with_model(model: &str) -> NativeRunRequest {
        serde_json::from_value(json!({
            "query": "hello",
            "model": model,
            "execution": {
                "compatibility_mode": "native-v1",
                "idempotency_key": "idem-1"
            },
            "metadata": {
                "trace_id": "trace-1"
            }
        }))
        .unwrap()
    }

    #[test]
    fn mapper_rejects_selector_collisions() {
        let mapping = ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "start.query".into(),
                model_target: Some("start.query".into()),
                inputs_target: None,
                history_target: None,
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
        };

        let error =
            NativeInputMapper::map(&request_with_model("any/provider"), &mapping).unwrap_err();

        assert_eq!(
            error,
            NativeInputMappingError::SelectorCollision {
                selector: "start.query".into()
            }
        );
    }

    #[test]
    fn mapper_preserves_model_metadata_when_model_target_is_null() {
        let mapping = ApplicationApiMappingConfig {
            input: ApplicationApiMappingInput {
                query_target: "start.query".into(),
                model_target: None,
                inputs_target: None,
                history_target: None,
                attachments_target: None,
            },
            output: ApplicationApiMappingOutput::default(),
        };

        let mapped =
            NativeInputMapper::map(&request_with_model("unlisted-model"), &mapping).unwrap();

        assert!(mapped.node_input_payload["start"].get("model").is_none());
        assert_eq!(mapped.metadata["model"], json!("unlisted-model"));
        assert_eq!(mapped.metadata["compatibility_mode"], json!(null));
        assert_eq!(mapped.metadata["idempotency_key"], json!("idem-1"));
        assert_eq!(mapped.metadata["external_trace_id"], json!("trace-1"));
    }

    #[test]
    fn mapper_places_tool_registry_under_default_start_input() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello",
            "inputs": {
                "tools": [
                    {
                        "name": "read_file",
                        "source": "openai_compatible",
                        "input_schema": {
                            "type": "object"
                        }
                    }
                ],
                "tool_choice": "auto"
            }
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["tools"][0]["name"],
            json!("read_file")
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["tool_choice"],
            json!("auto")
        );
        assert!(mapped.node_input_payload["node-start"]
            .get("compatibility")
            .is_none());
    }

    #[test]
    fn mapper_promotes_system_context_out_of_native_history() {
        let request: NativeRunRequest = serde_json::from_value(json!({
            "query": "hello",
            "system": "Use the request system.",
            "history": [
                { "role": "system", "content": "Use the legacy history system." },
                { "role": "user", "content": "Earlier question" }
            ]
        }))
        .unwrap();

        let mapped =
            NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
                .unwrap();

        assert_eq!(
            mapped.node_input_payload["node-start"]["system"],
            json!("Use the request system.\n\nUse the legacy history system.")
        );
        assert_eq!(
            mapped.node_input_payload["node-start"]["history"],
            json!([{ "role": "user", "content": "Earlier question" }])
        );
    }
}
