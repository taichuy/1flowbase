use super::*;

pub(super) fn anthropic_usage(
    usage: Option<control_plane::application_public_api::native::NativeUsage>,
) -> AnthropicUsage {
    let Some(usage) = usage else {
        return AnthropicUsage::default();
    };
    AnthropicUsage {
        input_tokens: usage.prompt_tokens.unwrap_or_default(),
        cache_creation_input_tokens: usage.cache_write_tokens.unwrap_or_default(),
        cache_read_input_tokens: usage
            .cache_read_tokens
            .or(usage.input_cache_hit_tokens)
            .unwrap_or_default(),
        output_tokens: usage.completion_tokens.unwrap_or_default(),
    }
}

pub(super) fn to_anthropic_count_tokens_response(
    input_tokens: u64,
) -> AnthropicCountTokensResponse {
    AnthropicCountTokensResponse { input_tokens }
}
