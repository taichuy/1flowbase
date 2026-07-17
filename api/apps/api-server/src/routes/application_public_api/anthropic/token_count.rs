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

pub(super) fn to_anthropic_count_tokens_response(request: &Value) -> AnthropicCountTokensResponse {
    AnthropicCountTokensResponse {
        input_tokens: anthropic_count_input_tokens(request),
    }
}

pub(super) fn anthropic_count_input_tokens(request: &Value) -> u64 {
    let mut tokens = 0_u64;
    for key in ["system", "messages"] {
        tokens = tokens.saturating_add(anthropic_value_token_estimate(request.get(key)));
    }
    tokens.max(1)
}

fn anthropic_value_token_estimate(value: Option<&Value>) -> u64 {
    let Some(value) = value else {
        return 0;
    };
    let chars = anthropic_value_char_count(value) as u64;
    ((chars.saturating_add(3)) / 4).max(1)
}

fn anthropic_value_char_count(value: &Value) -> usize {
    match value {
        Value::Null => 0,
        Value::Bool(value) => value.to_string().chars().count(),
        Value::Number(value) => value.to_string().chars().count(),
        Value::String(value) => value.chars().count(),
        Value::Array(values) => values.iter().map(anthropic_value_char_count).sum(),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| key.chars().count() + anthropic_value_char_count(value))
            .sum(),
    }
}
