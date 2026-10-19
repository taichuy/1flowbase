use std::fmt;

use super::*;

const PROVIDER_PROJECTION_LOCATOR_LIMIT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderGenerateTranslationDecision {
    OmittedSystemPromptCacheControl,
    OmittedEndUserReference,
    OmittedProtocolContextProfileMismatch,
    DelegatedReasoningHistoryToProvider,
    DelegatedRedactedReasoningHistoryToProvider,
    OmittedReasoningHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProjectionFidelity {
    Exact,
    Lossy,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProjectionLossCode {
    ReasoningHistoryOmitted,
    SignedReasoningOmitted,
    RedactedReasoningOmitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProjectionErrorCode {
    ReasoningOnlyMessageUnsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCanonicalBlockKind {
    Text,
    Image,
    ImageUrl,
    Document,
    ToolUse,
    ToolResult,
    Reasoning,
    RedactedReasoning,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCanonicalBlockLocator {
    pub message_index: u32,
    pub block_index: u32,
    pub block_kind: ProviderCanonicalBlockKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProjectionSource {
    CanonicalInvocation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderProjectionProvenance {
    pub source: ProviderProjectionSource,
    pub preserved_block_count: u32,
    pub omitted_block_count: u32,
    pub capped: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preserved_blocks: Vec<ProviderCanonicalBlockLocator>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_blocks: Vec<ProviderCanonicalBlockLocator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderGenerateTranslationReceipt {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub decisions: BTreeSet<ProviderGenerateTranslationDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<ProviderProjectionFidelity>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub loss_codes: BTreeSet<ProviderProjectionLossCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ProviderProjectionErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProviderProjectionProvenance>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderGenerateProjection {
    pub provider_bound_input: ProviderInvocationInput,
    pub receipt: ProviderGenerateTranslationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderGenerateProjectionError {
    InvalidContract {
        message: String,
    },
    Unsupported {
        code: ProviderProjectionErrorCode,
        block: ProviderCanonicalBlockLocator,
        receipt: ProviderGenerateTranslationReceipt,
    },
}

impl fmt::Display for ProviderGenerateProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract { message } => formatter.write_str(message),
            Self::Unsupported { code, block, .. } => write!(
                formatter,
                "Generate projection failed with {code:?} at messages[{}].content_blocks[{}]",
                block.message_index, block.block_index
            ),
        }
    }
}

impl std::error::Error for ProviderGenerateProjectionError {}

impl ProviderGenerateTranslationReceipt {
    pub fn attach_to_provider_metadata(
        &self,
        provider_metadata: &mut Value,
    ) -> Result<(), ExtensionContractError> {
        if self.decisions.is_empty()
            && self.fidelity.is_none()
            && self.loss_codes.is_empty()
            && self.error_code.is_none()
            && self.provenance.is_none()
        {
            return Ok(());
        }
        if provider_metadata.is_null() {
            *provider_metadata = empty_provider_metadata();
        }
        let metadata = provider_metadata.as_object_mut().ok_or_else(|| {
            ExtensionContractError::invalid_provider_contract(
                "provider metadata must be an object when Generate translation decisions exist",
            )
        })?;
        if metadata.contains_key(PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY) {
            return Err(ExtensionContractError::invalid_provider_contract(format!(
                "provider metadata must not define reserved key {PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY}"
            )));
        }
        metadata.insert(
            PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY.to_string(),
            serde_json::to_value(self).map_err(|error| {
                ExtensionContractError::invalid_provider_contract(error.to_string())
            })?,
        );
        Ok(())
    }
}

// Runtime errors intentionally keep the complete upstream diagnostics payload.
#[allow(clippy::result_large_err)]
pub(super) fn project_generate_reasoning_history(
    invocation: &mut ProviderInvocationInput,
    declared_capabilities: &BTreeSet<&str>,
    receipt: &mut ProviderGenerateTranslationReceipt,
) -> Result<(), ProviderGenerateProjectionError> {
    let generic_history_input =
        declared_capabilities.contains(PROVIDER_REASONING_HISTORY_INPUT_SUPPORTED_CAPABILITY);
    let mut preserved_blocks = Vec::new();
    let mut omitted_blocks = Vec::new();
    let mut preserved_block_count = 0usize;
    let mut omitted_block_count = 0usize;
    let mut has_reasoning = false;
    let mut projected_blocks = Vec::new();

    for (message_index, message) in invocation.messages.iter().enumerate() {
        let Some(blocks) = message.content_blocks.as_ref().and_then(Value::as_array) else {
            continue;
        };
        let mut retained = Vec::with_capacity(blocks.len());
        let mut first_omitted = None;
        for (block_index, block) in blocks.iter().enumerate() {
            let block_type = block
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| ProviderGenerateProjectionError::InvalidContract {
                    message: format!(
                        "messages[{message_index}].content_blocks[{block_index}].type must be a string"
                    ),
                })?;
            let block_kind = canonical_block_kind(block_type).ok_or_else(|| {
                ProviderGenerateProjectionError::InvalidContract {
                    message: format!(
                        "messages[{message_index}].content_blocks[{block_index}] has unsupported canonical block type: {block_type}"
                    ),
                }
            })?;
            let locator = ProviderCanonicalBlockLocator {
                message_index: u32::try_from(message_index).map_err(|_| {
                    ProviderGenerateProjectionError::InvalidContract {
                        message: "canonical message index exceeds the projection locator range"
                            .to_string(),
                    }
                })?,
                block_index: u32::try_from(block_index).map_err(|_| {
                    ProviderGenerateProjectionError::InvalidContract {
                        message: "canonical block index exceeds the projection locator range"
                            .to_string(),
                    }
                })?,
                block_kind,
            };
            let required_legacy_capability = match block_kind {
                ProviderCanonicalBlockKind::Reasoning => {
                    Some(PROVIDER_MESSAGE_BLOCKS_REASONING_HISTORY_V1_CAPABILITY)
                }
                ProviderCanonicalBlockKind::RedactedReasoning => {
                    Some(PROVIDER_MESSAGE_BLOCKS_REDACTED_REASONING_HISTORY_V1_CAPABILITY)
                }
                _ => None,
            };
            has_reasoning |= required_legacy_capability.is_some();
            let omit = message.role == ProviderMessageRole::Assistant
                && required_legacy_capability.is_some_and(|capability| {
                    let generic_supports_block = generic_history_input
                        && block_kind == ProviderCanonicalBlockKind::Reasoning
                        && !block.get("signature").is_some_and(|value| !value.is_null());
                    !generic_supports_block && !declared_capabilities.contains(capability)
                });
            if omit {
                first_omitted.get_or_insert_with(|| locator.clone());
                omitted_block_count = omitted_block_count.saturating_add(1);
                if omitted_blocks.len() < PROVIDER_PROJECTION_LOCATOR_LIMIT {
                    omitted_blocks.push(locator);
                }
                match block_kind {
                    ProviderCanonicalBlockKind::Reasoning
                        if block.get("signature").is_some_and(|value| !value.is_null()) =>
                    {
                        receipt
                            .loss_codes
                            .insert(ProviderProjectionLossCode::SignedReasoningOmitted);
                    }
                    ProviderCanonicalBlockKind::Reasoning => {
                        receipt
                            .loss_codes
                            .insert(ProviderProjectionLossCode::ReasoningHistoryOmitted);
                    }
                    ProviderCanonicalBlockKind::RedactedReasoning => {
                        receipt
                            .loss_codes
                            .insert(ProviderProjectionLossCode::RedactedReasoningOmitted);
                    }
                    _ => {}
                }
            } else {
                preserved_block_count = preserved_block_count.saturating_add(1);
                if preserved_blocks.len() < PROVIDER_PROJECTION_LOCATOR_LIMIT {
                    preserved_blocks.push(locator);
                }
                retained.push(block.clone());
            }
        }
        projected_blocks.push((message_index, retained, first_omitted));
    }

    if !has_reasoning {
        return Ok(());
    }

    let locators_capped = preserved_block_count > preserved_blocks.len()
        || omitted_block_count > omitted_blocks.len();
    let (preserved_block_count, preserved_count_capped) =
        bounded_projection_count(preserved_block_count);
    let (omitted_block_count, omitted_count_capped) = bounded_projection_count(omitted_block_count);
    let provenance = ProviderProjectionProvenance {
        source: ProviderProjectionSource::CanonicalInvocation,
        preserved_block_count,
        omitted_block_count,
        capped: preserved_count_capped || omitted_count_capped || locators_capped,
        preserved_blocks,
        omitted_blocks,
    };
    for (message_index, retained, first_omitted) in &projected_blocks {
        let Some(block) = first_omitted else {
            continue;
        };
        let message = &invocation.messages[*message_index];
        let has_visible_or_tool_content = !message.content.trim().is_empty()
            || message
                .tool_calls
                .as_ref()
                .is_some_and(|calls| !calls.as_array().is_some_and(Vec::is_empty));
        if retained.is_empty() && !has_visible_or_tool_content {
            let code = ProviderProjectionErrorCode::ReasoningOnlyMessageUnsupported;
            let unsupported_receipt = ProviderGenerateTranslationReceipt {
                fidelity: Some(ProviderProjectionFidelity::Unsupported),
                error_code: Some(code),
                provenance: Some(provenance),
                ..receipt.clone()
            };
            return Err(ProviderGenerateProjectionError::Unsupported {
                code,
                block: block.clone(),
                receipt: unsupported_receipt,
            });
        }
    }

    if provenance.omitted_block_count == 0 {
        receipt.fidelity = Some(ProviderProjectionFidelity::Exact);
    } else {
        for (message_index, retained, first_omitted) in projected_blocks {
            if first_omitted.is_some() {
                invocation.messages[message_index].content_blocks =
                    (!retained.is_empty()).then(|| Value::Array(retained));
            }
        }
        invocation
            .required_capabilities
            .remove(&ProviderInvocationCapability::MessageBlocksReasoningHistoryV1);
        invocation
            .required_capabilities
            .remove(&ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1);
        receipt.fidelity = Some(ProviderProjectionFidelity::Lossy);
        receipt
            .decisions
            .insert(ProviderGenerateTranslationDecision::OmittedReasoningHistory);
    }
    receipt.provenance = Some(provenance);
    Ok(())
}

fn bounded_projection_count(count: usize) -> (u32, bool) {
    u32::try_from(count).map_or((u32::MAX, true), |count| (count, false))
}

fn canonical_block_kind(block_type: &str) -> Option<ProviderCanonicalBlockKind> {
    match block_type {
        "text" => Some(ProviderCanonicalBlockKind::Text),
        "image" => Some(ProviderCanonicalBlockKind::Image),
        "image_url" => Some(ProviderCanonicalBlockKind::ImageUrl),
        "document" => Some(ProviderCanonicalBlockKind::Document),
        "tool_use" => Some(ProviderCanonicalBlockKind::ToolUse),
        "tool_result" => Some(ProviderCanonicalBlockKind::ToolResult),
        "reasoning" => Some(ProviderCanonicalBlockKind::Reasoning),
        "reasoning_redacted" => Some(ProviderCanonicalBlockKind::RedactedReasoning),
        _ => None,
    }
}
