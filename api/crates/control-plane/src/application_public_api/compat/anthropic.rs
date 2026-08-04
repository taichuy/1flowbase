use plugin_framework::provider_contract::{
    NativePromptBlock, NativePromptCacheControl, NativePromptCacheControlType, NativePromptCacheTtl,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::application_public_api::callback_tool_ids::decode_anthropic_callback_tool_use_id;
use crate::application_public_api::client_protocol_envelope::{
    anthropic_context_1m_requested, protocol_context_field_is_safe,
};
use crate::application_public_api::native::{NativeObject, NativeRunRequest};
use crate::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

const CLAUDE_CODE_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the conversation so far";
const CLAUDE_CODE_PARTIAL_COMPACT_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of the RECENT portion of the conversation";
const CLAUDE_CODE_CONTEXT_CONTINUATION_SUMMARY_PROMPT_PREFIX: &str =
    "Your task is to create a detailed summary of this conversation. This summary will be placed at the start of a continuing session";
const CLAUDE_CODE_AWAY_SUMMARY_PROMPT_PREFIX: &str =
    "The user stepped away and is coming back. Write exactly 1-3 short sentences.";
const CLAUDE_CODE_AWAY_SUMMARY_NEXT_STEP_MARKER: &str = "Next: the concrete next step.";
const CLAUDE_CODE_COMPACT_RESUME_MARKER: &str =
    "This session is being continued from a previous conversation that ran out of context.";
const CLAUDE_CODE_COMPACT_RESUME_SUMMARY_MARKER: &str =
    "The summary below covers the earlier portion of the conversation.";
const CLAUDE_CODE_COMPACT_TRANSCRIPT_MARKER: &str =
    "If you need specific details from before compaction";
const CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER: &str = "Generate a concise, sentence-case title";
const CLAUDE_CODE_SESSION_TITLE_JSON_MARKER: &str = "Return JSON with a single \"title\" field";
const ANTHROPIC_CONTEXT_1M_MODEL_SUFFIX: &str = "[1m]";
const ANTHROPIC_CONTEXT_1M_TOKENS: u64 = 1_000_000;
const ANTHROPIC_TYPED_ROOT_FIELDS: &[&str] = &[
    "model",
    "messages",
    "max_tokens",
    "system",
    "stream",
    "metadata",
    "tools",
    "tool_choice",
    "container",
    "mcp_servers",
    "thinking",
    "output_config",
    "service_tier",
    "temperature",
    "top_k",
    "top_p",
    "stop_sequences",
    "stream_options",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicCompatError {
    pub message: String,
    pub error_type: String,
    pub report: TranslationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnthropicContextWindowRequest {
    OneMillion,
}

impl AnthropicContextWindowRequest {
    pub fn from_beta_values<'a>(values: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        anthropic_context_1m_requested(values).then_some(Self::OneMillion)
    }

    fn tokens(self) -> u64 {
        match self {
            Self::OneMillion => ANTHROPIC_CONTEXT_1M_TOKENS,
        }
    }
}

impl AnthropicCompatError {
    fn translation_invariant(report: TranslationReport) -> Self {
        Self {
            message: "translation receipt invariant violated".to_string(),
            error_type: "api_error".to_string(),
            report,
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_request".to_string(),
            report: TranslationReport::new(TranslationProtocol::AnthropicMessages),
        }
    }

    fn unsupported(param: impl AsRef<str>) -> Self {
        let param = param.as_ref();
        Self {
            message: format!("{param} is not supported by this endpoint"),
            error_type: "unsupported_feature".to_string(),
            report: TranslationReport::new(TranslationProtocol::AnthropicMessages),
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

pub fn map_messages_request(request: Value) -> Result<NativeRunRequest, AnthropicCompatError> {
    translate_messages_request(request).map(|translated| translated.request)
}

mod request_translation;

pub use request_translation::{
    translate_messages_request, translate_messages_request_with_context_window,
};

mod content_mapping;
mod request_mapping;
mod request_metadata;
mod validation;

use content_mapping::*;
use request_mapping::*;
use request_metadata::*;
use validation::*;

pub use content_mapping::anthropic_content_is_tool_result_only;
pub use request_metadata::claude_code_control_kind;

#[cfg(test)]
mod tests;
