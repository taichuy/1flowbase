use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::application_public_api::callback_tool_ids::decode_openai_callback_tool_call_id;
use crate::application_public_api::client_protocol_envelope::protocol_context_field_is_safe;

pub use crate::application_public_api::model_catalog::{
    extract_agent_model_catalog_from_start_node as extract_model_list_from_start_node,
    AgentModelDescriptor as OpenAiCompatibleModel,
};
use crate::application_public_api::native::NativeRunRequest;
use crate::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

const OPENAI_CHAT_TYPED_ROOT_FIELDS: &[&str] = &[
    "model",
    "messages",
    "stream",
    "user",
    "metadata",
    "max_completion_tokens",
    "max_tokens",
    "audio",
    "modalities",
    "tools",
    "tool_choice",
    "function_call",
    "parallel_tool_calls",
    "response_format",
    "reasoning_effort",
    "temperature",
    "top_p",
    "presence_penalty",
    "frequency_penalty",
    "seed",
    "stop",
    "stream_options",
];
const OPENAI_RESPONSES_TYPED_ROOT_FIELDS: &[&str] = &[
    "model",
    "input",
    "instructions",
    "stream",
    "user",
    "metadata",
    "max_output_tokens",
    "store",
    "previous_response_id",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "response_format",
    "text",
    "reasoning",
    "background",
    "include",
    "prompt_cache_key",
    "client_metadata",
    "max_tool_calls",
    "truncation",
];
const OPENAI_RESPONSES_OPTIONAL_TOOLS_CONTEXT_FIELD: &str = "responses_optional_tools";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiCompatError {
    pub message: String,
    pub error_type: String,
    pub param: Option<String>,
    pub code: String,
    pub report: TranslationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiPreviousResponseContext {
    pub response_id: String,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub answer: Option<String>,
}

impl OpenAiCompatError {
    fn translation_invariant(report: TranslationReport) -> Self {
        Self {
            message: "translation receipt invariant violated".to_string(),
            error_type: "server_error".to_string(),
            param: None,
            code: "translation_invariant".to_string(),
            report,
        }
    }

    fn invalid(param: &'static str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_request_error".to_string(),
            param: Some(param.to_string()),
            code: "invalid_request".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiChat),
        }
    }

    fn unsupported(param: impl AsRef<str>) -> Self {
        let param = param.as_ref();
        Self {
            message: format!("{param} is not supported by this endpoint"),
            error_type: "invalid_request_error".to_string(),
            param: Some(param.to_string()),
            code: "unsupported_feature".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiChat),
        }
    }

    fn unsupported_compaction_profile() -> Self {
        Self {
            message: "Codex compaction profile is not supported by this endpoint".to_string(),
            error_type: "invalid_request_error".to_string(),
            param: Some("x-codex-turn-metadata.compaction.implementation".to_string()),
            code: "unsupported_compaction_profile".to_string(),
            report: TranslationReport::new(TranslationProtocol::OpenAiResponses),
        }
    }

    fn with_report(mut self, report: TranslationReport) -> Self {
        if report.ensure_consistent().is_err() {
            return Self::translation_invariant(report);
        }
        self.report = report;
        self
    }
}

mod compaction;
mod request_translation;

use compaction::classify_response_operation;
pub use compaction::{OpenAiResponsesEndpoint, OpenAiResponsesRequestContext};
pub use request_translation::{
    translate_chat_completion_request, translate_response_request,
    translate_response_request_with_context, translate_response_request_with_context_and_previous,
};

pub fn map_chat_completion_request(request: Value) -> Result<NativeRunRequest, OpenAiCompatError> {
    translate_chat_completion_request(request).map(|translated| translated.request)
}
pub fn map_response_request(
    request: Value,
    _previous_response: Option<OpenAiPreviousResponseContext>,
) -> Result<NativeRunRequest, OpenAiCompatError> {
    translate_response_request(request).map(|translated| translated.request)
}

fn system_from_parts(parts: Vec<String>) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

pub fn response_id_from_run_id(run_id: Uuid) -> String {
    format!("resp_{run_id}")
}

pub fn run_id_from_response_id(response_id: &str) -> Result<Uuid, OpenAiCompatError> {
    let run_id = response_id
        .strip_prefix("resp_")
        .ok_or_else(|| OpenAiCompatError::invalid("previous_response_id", "invalid response id"))?;
    Uuid::parse_str(run_id)
        .map_err(|_| OpenAiCompatError::invalid("previous_response_id", "invalid response id"))
}

mod chat_validation;
mod request_mapping;
mod response_input;

use chat_validation::*;
use request_mapping::*;
use response_input::*;

#[cfg(test)]
mod tests;
