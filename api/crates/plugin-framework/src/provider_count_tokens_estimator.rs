use serde_json::Value;

use crate::provider_contract::{
    ProviderCountTokensCoverage, ProviderCountTokensMethod, ProviderCountTokensResult,
    ProviderInvocationInput, ProviderWireOperation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderCountTokensEstimatorError {
    #[error("canonical prompt estimate overflowed")]
    Overflow,
}

/// Provider-neutral, deterministic coverage for the canonical prompt envelope.
/// It intentionally does not contain model, tokenizer, or supplier tables.
pub fn estimate_provider_count_tokens(
    input: &ProviderInvocationInput,
) -> Result<ProviderCountTokensResult, ProviderCountTokensEstimatorError> {
    let mut estimate = CanonicalPromptEstimate::default();

    for block in &input.system {
        estimate.add_text(block.text_content())?;
        estimate.add_tokens(2)?;
    }
    for message in &input.messages {
        estimate.add_tokens(4)?;
        estimate.add_text(&message.content)?;
        if let Some(name) = &message.name {
            estimate.add_text(name)?;
        }
        if let Some(tool_call_id) = &message.tool_call_id {
            estimate.add_text(tool_call_id)?;
        }
        if let Some(tool_calls) = &message.tool_calls {
            estimate.add_json(tool_calls)?;
        }
        if let Some(blocks) = &message.content_blocks {
            estimate.add_content_blocks(blocks)?;
        }
    }
    for tool in &input.tools {
        estimate.add_json(tool)?;
    }
    for binding in &input.mcp_bindings {
        estimate.add_json(binding)?;
    }
    if let Some(response_format) = &input.response_format {
        estimate.add_json(response_format)?;
    }

    Ok(ProviderCountTokensResult {
        operation: ProviderWireOperation::CountTokens,
        input_tokens: estimate.tokens,
        method: ProviderCountTokensMethod::GenericEstimate,
        coverage: if estimate.unknown_block_count == 0 {
            ProviderCountTokensCoverage::Complete
        } else {
            ProviderCountTokensCoverage::Partial
        },
        unknown_block_count: estimate.unknown_block_count,
        fallback_reason: None,
    })
}

#[derive(Default)]
struct CanonicalPromptEstimate {
    tokens: u64,
    unknown_block_count: u64,
}

impl CanonicalPromptEstimate {
    fn add_tokens(&mut self, tokens: u64) -> Result<(), ProviderCountTokensEstimatorError> {
        self.tokens = self
            .tokens
            .checked_add(tokens)
            .ok_or(ProviderCountTokensEstimatorError::Overflow)?;
        Ok(())
    }

    fn add_text(&mut self, text: &str) -> Result<(), ProviderCountTokensEstimatorError> {
        if text.is_empty() {
            return Ok(());
        }
        let characters = u64::try_from(text.chars().count())
            .map_err(|_| ProviderCountTokensEstimatorError::Overflow)?;
        self.add_tokens(characters.div_ceil(4).max(1))
    }

    fn add_json(&mut self, value: &Value) -> Result<(), ProviderCountTokensEstimatorError> {
        match value {
            Value::Null => self.add_tokens(1),
            Value::Bool(_) | Value::Number(_) => self.add_tokens(1),
            Value::String(text) => self.add_text(text),
            Value::Array(values) => {
                self.add_tokens(1)?;
                for value in values {
                    self.add_json(value)?;
                }
                Ok(())
            }
            Value::Object(entries) => {
                self.add_tokens(1)?;
                for (key, value) in entries {
                    self.add_text(key)?;
                    self.add_json(value)?;
                }
                Ok(())
            }
        }
    }

    fn add_content_blocks(
        &mut self,
        blocks: &Value,
    ) -> Result<(), ProviderCountTokensEstimatorError> {
        match blocks {
            Value::Array(blocks) => {
                for block in blocks {
                    self.add_content_block(block)?;
                }
                Ok(())
            }
            block => self.add_content_block(block),
        }
    }

    fn add_content_block(
        &mut self,
        block: &Value,
    ) -> Result<(), ProviderCountTokensEstimatorError> {
        let block_type = block.get("type").and_then(Value::as_str);
        match block_type {
            Some("text" | "input_text" | "output_text") => {
                self.add_text(
                    block
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )?;
                self.add_tokens(2)
            }
            Some("json" | "input_json" | "output_json") => {
                self.add_json(block.get("json").unwrap_or(block))
            }
            Some("image" | "input_image" | "audio" | "input_audio" | "document") => {
                // Inline media cost is represented by a bounded semantic placeholder; its
                // encoded byte length is not a model-token count and must not dominate totals.
                self.add_tokens(256)?;
                if let Some(media_type) =
                    block.pointer("/source/media_type").and_then(Value::as_str)
                {
                    self.add_text(media_type)?;
                }
                Ok(())
            }
            Some("url" | "input_url") => {
                self.add_tokens(16)?;
                self.add_text(block.get("url").and_then(Value::as_str).unwrap_or_default())
            }
            _ => {
                self.unknown_block_count = self
                    .unknown_block_count
                    .checked_add(1)
                    .ok_or(ProviderCountTokensEstimatorError::Overflow)?;
                self.add_json(block)
            }
        }
    }
}
