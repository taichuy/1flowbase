use plugin_framework::provider_contract::{
    NativePromptBlock, NativePromptCacheControl, NativePromptCacheControlType, NativePromptCacheTtl,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use uuid::Uuid;

pub use control_plane_contracts::application_public_runtime::claude_code_control::claude_code_control_kind;
use control_plane_contracts::application_public_runtime::claude_code_control::{
    CLAUDE_CODE_SESSION_TITLE_JSON_MARKER, CLAUDE_CODE_SESSION_TITLE_SYSTEM_MARKER,
};

use crate::application_public_api::callback_tool_ids::decode_anthropic_callback_tool_use_id;
use crate::application_public_api::client_protocol_envelope::{
    anthropic_context_1m_requested, protocol_context_field_is_safe,
};
use crate::application_public_api::native::{NativeObject, NativeRunRequest};
use crate::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationReport, TranslationSafeRepresentation,
};

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
#[cfg(test)]
mod tests;
