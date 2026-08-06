use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
pub use orchestration_runtime::answer_projection::{
    answer_segments_from_text, answer_segments_from_value, answer_segments_value,
    AnswerProjectionSegment, AnswerProjectionSegmentKind, ANSWER_SEGMENTS_KEY,
};
use plugin_framework::provider_contract::{
    NativeModelPromptContext, NativeModelRequestContext, NativePromptBlock,
    ProtocolContextEnvelope, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
    NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY, NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    api_keys::ApplicationApiKeyService,
    callback_resume::ApplicationPublishedCallbackAttemptRepository,
    conversations::ApplicationPublicConversationRepository,
    mapping::ApplicationApiMappingConfig,
    protocol_translation::{
        TranslationDecisionKind, TranslationProtocol, TranslationReport,
        TranslationSafeRepresentation,
    },
    run_service::{
        ApplicationPublishedFlowRunRepository, ApplicationPublishedRunControlRepository,
        ApplicationPublishedRunService,
    },
};
use crate::flow_run_title::build_flow_run_title;
use crate::ports::{
    ApiKeyRepository, ApplicationCompiledPlanRepository, ApplicationPublicationRepository,
    ApplicationRepository, AuthRepository, CacheStore, ProviderProtocolContextValue,
    RuntimeEventDurability, RuntimeEventStream,
};

mod compaction;
mod metadata;
mod model_parameters;

pub use compaction::{
    compaction_intent, operation_result_requirement, CompactionIntent, CompactionProfile,
    CompactionResultRequirement,
};
pub use metadata::NativeRequestMetadata;
pub(crate) use metadata::ResponsesTransportRequirement;
pub use model_parameters::{
    NativeExecution, NativeExecutionModelParameters, NativeReasoningMode, NativeReasoningParameters,
};

mod request_translation;

use request_translation::parse_native_prompt_blocks;
pub use request_translation::{translate_native_run_request, NativeRequestTranslationError};
mod input_mapping;
mod run_lifecycle;
mod serialization;
mod wire_types;

use serialization::*;
use wire_types::{native_attachments, native_history};

pub use input_mapping::{NativeInputMapper, NativeInputMappingError};
pub use run_lifecycle::{
    ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
    GetNativeRunByProviderResponseIdCommand, GetNativeRunCommand, NativeRunRepository,
    NativeRunValidationError,
};
pub use wire_types::{
    NativeAttachment, NativeAttachmentSource, NativeError, NativeMappedInput, NativeObject,
    NativeRequiredAction, NativeRunRequest, NativeRunResult, NativeRunStatus, NativeStreamOptions,
    NativeUsage, NativeWorkflowEventVisibility,
};

pub(super) use serialization::durable_metadata_from_flow_run;
pub(crate) use serialization::write_selector;

#[cfg(test)]
mod tests;
