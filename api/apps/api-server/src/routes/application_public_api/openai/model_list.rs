use super::*;

pub(super) fn to_openai_model_list_response(
    models: Vec<OpenAiCompatibleModel>,
    created: i64,
) -> OpenAiModelListResponse {
    OpenAiModelListResponse {
        object: "list",
        data: models
            .into_iter()
            .map(|model| OpenAiModelObject {
                id: model.id.clone(),
                object: "model",
                created,
                owned_by: "1flowbase",
                name: model.name.clone(),
                context_window: model.context_window,
                max_context_window: model.max_context_window.or(model.context_window),
                max_output_tokens: model.max_output_tokens,
                auto_compact_token_limit: model.auto_compact_token_limit,
                capabilities: (!model.capabilities.is_empty())
                    .then(|| serde_json::to_value(&model.capabilities).unwrap_or(Value::Null)),
                reasoning: model
                    .reasoning
                    .as_ref()
                    .map(|reasoning| serde_json::to_value(reasoning).unwrap_or(Value::Null)),
                limit: opencode_limit_projection(&model),
            })
            .collect(),
    }
}

pub(super) fn is_codex_model_list_request(query: &OpenAiModelListQuery) -> bool {
    query
        .client_version
        .as_deref()
        .is_some_and(|client_version| !client_version.trim().is_empty())
}

pub(super) fn to_codex_model_list_response(models: Vec<OpenAiCompatibleModel>) -> Value {
    let models = models
        .into_iter()
        .map(codex_model_metadata)
        .collect::<Vec<_>>();
    json!({ "models": models })
}

fn codex_model_metadata(model: OpenAiCompatibleModel) -> Value {
    let display_name = model.name.clone().unwrap_or_else(|| model.id.clone());
    let max_context_window = model.max_context_window.or(model.context_window);
    let default_reasoning_level = model
        .reasoning
        .as_ref()
        .and_then(|reasoning| reasoning.default_effort.as_deref());
    let supported_reasoning_levels = model
        .reasoning
        .as_ref()
        .map(|reasoning| {
            reasoning
                .supported_efforts
                .iter()
                .map(|effort| json!({"effort": effort, "description": effort}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "slug": model.id,
        "display_name": display_name,
        "description": null,
        "default_reasoning_level": default_reasoning_level,
        "supported_reasoning_levels": supported_reasoning_levels,
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 0,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "You are a helpful coding assistant.",
        "supports_reasoning_summaries": false,
        "default_reasoning_summary": "auto",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "web_search_tool_type": "text",
        "truncation_policy": { "mode": "bytes", "limit": 10000 },
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": model.context_window,
        "max_context_window": max_context_window,
        "max_output_tokens": model.max_output_tokens,
        "auto_compact_token_limit": model.auto_compact_token_limit,
        "effective_context_window_percent": 95,
        "limit": opencode_limit_projection(&model),
        "experimental_supported_tools": [],
        "input_modalities": ["text"]
    })
}

fn opencode_limit_projection(model: &OpenAiCompatibleModel) -> Option<Value> {
    let context = model.max_context_window.or(model.context_window);
    let output = model.max_output_tokens;
    if context.is_none() && output.is_none() {
        return None;
    }

    Some(json!({
        "context": context,
        "input": context,
        "output": output,
    }))
}
