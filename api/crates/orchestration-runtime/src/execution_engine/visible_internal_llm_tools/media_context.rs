use super::*;

pub(in crate::execution_engine) fn project_visible_internal_llm_tool_shared_messages(
    messages: &mut Vec<ProviderMessage>,
    variable_pool: &Map<String, Value>,
) {
    if !variable_pool.contains_key(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE) {
        return;
    }

    for message in messages.iter_mut() {
        let Some(blocks) = message
            .content_blocks
            .as_mut()
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        blocks.retain(|block| {
            !matches!(
                block.get("type").and_then(Value::as_str),
                Some("reasoning" | "reasoning_redacted")
            )
        });
    }
    messages.retain(visible_internal_llm_tool_message_has_shared_content);
}

fn visible_internal_llm_tool_message_has_shared_content(message: &ProviderMessage) -> bool {
    !message.content.trim().is_empty()
        || message
            .content_blocks
            .as_ref()
            .and_then(Value::as_array)
            .is_some_and(|blocks| !blocks.is_empty())
        || message
            .tool_calls
            .as_ref()
            .and_then(Value::as_array)
            .is_some_and(|tool_calls| !tool_calls.is_empty())
        || message.tool_call_id.is_some()
        || message.is_error.is_some()
}

pub(super) fn visible_internal_llm_tool_llm_resolved_inputs(
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> Map<String, Value> {
    let mut inputs = resolved_inputs.clone();
    let Some(context) = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("context"))
        .and_then(Value::as_object)
    else {
        return inputs;
    };

    if !inputs.contains_key("history") {
        if let Some(history) = context
            .get("history")
            .and_then(Value::as_array)
            .filter(|history| !history.is_empty())
        {
            inputs.insert("history".to_string(), Value::Array(history.clone()));
        }
    }
    if !inputs.contains_key("files") {
        if let Some(files) = context
            .get("files")
            .and_then(Value::as_array)
            .filter(|files| !files.is_empty())
        {
            inputs.insert("files".to_string(), Value::Array(files.clone()));
        }
    }
    let inherited_tools = (!inputs.contains_key("tools")
        && visible_internal_llm_tool_external_tool_policy(variable_pool)
            == VisibleInternalLlmToolExternalToolPolicy::Inherited)
        .then(|| {
            context
                .get("tools")
                .and_then(Value::as_array)
                .filter(|tools| !tools.is_empty())
        })
        .flatten();
    if let Some(tools) = inherited_tools {
        inputs.insert("tools".to_string(), Value::Array(tools.clone()));
    }

    inputs
}

pub(super) fn visible_internal_llm_tool_external_tool_policy(
    variable_pool: &Map<String, Value>,
) -> VisibleInternalLlmToolExternalToolPolicy {
    let inherited = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("external_tool_policy"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(EXTERNAL_TOOL_POLICY_INHERITED);
    if inherited {
        VisibleInternalLlmToolExternalToolPolicy::Inherited
    } else {
        VisibleInternalLlmToolExternalToolPolicy::Forbidden
    }
}

pub(super) fn visible_internal_llm_tool_mode(
    variable_pool: &Map<String, Value>,
) -> VisibleInternalLlmToolMode {
    let fusion = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("tool_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(TOOL_MODE_FUSION);
    if fusion {
        VisibleInternalLlmToolMode::Fusion
    } else {
        VisibleInternalLlmToolMode::Agent
    }
}

pub(super) fn visible_internal_llm_tool_external_callback_policy(
    variable_pool: &Map<String, Value>,
) -> VisibleInternalLlmToolExternalCallbackPolicy {
    let inherited = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("external_callback_policy"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(EXTERNAL_CALLBACK_POLICY_INHERITED);
    if inherited {
        VisibleInternalLlmToolExternalCallbackPolicy::Inherited
    } else {
        VisibleInternalLlmToolExternalCallbackPolicy::Forbidden
    }
}

pub(super) fn visible_internal_llm_tool_execution_mode(
    variable_pool: &Map<String, Value>,
) -> VisibleInternalLlmToolExecutionMode {
    let bounded_parallel = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("execution_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(EXECUTION_MODE_BOUNDED_PARALLEL_PANEL);
    if bounded_parallel {
        VisibleInternalLlmToolExecutionMode::BoundedParallelPanel
    } else {
        VisibleInternalLlmToolExecutionMode::SequentialResume
    }
}

pub(in crate::execution_engine) fn visible_internal_llm_tool_blocks_external_tools(
    variable_pool: &Map<String, Value>,
) -> bool {
    variable_pool.contains_key(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        && visible_internal_llm_tool_external_tool_policy(variable_pool)
            == VisibleInternalLlmToolExternalToolPolicy::Forbidden
}

pub(super) fn visible_internal_llm_tool_blocks_external_callback(
    variable_pool: &Map<String, Value>,
) -> bool {
    variable_pool.contains_key(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        && visible_internal_llm_tool_external_callback_policy(variable_pool)
            == VisibleInternalLlmToolExternalCallbackPolicy::Forbidden
}

pub(in crate::execution_engine) async fn inject_visible_internal_llm_tool_media_content_blocks(
    input: &mut ProviderInvocationInput,
    variable_pool: &Map<String, Value>,
) {
    let media_items = visible_internal_llm_tool_media_arguments(variable_pool);
    if media_items.is_empty() {
        return;
    }

    let mut injected_blocks = Vec::new();
    for media in media_items {
        if let Some(block) = image_content_block_from_workspace_media(&media).await {
            injected_blocks.push(block);
        }
    }
    if injected_blocks.is_empty() && !provider_input_has_image_content_blocks(input) {
        injected_blocks.extend(inherited_image_content_blocks(variable_pool));
    }
    if injected_blocks.is_empty() {
        return;
    }

    let Some(message) = input
        .messages
        .iter_mut()
        .rev()
        .find(|message| message.role == ProviderMessageRole::User)
    else {
        return;
    };

    let mut content_blocks = message
        .content_blocks
        .take()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_else(|| {
            let content = message.content.trim();
            if content.is_empty() {
                Vec::new()
            } else {
                vec![json!({ "type": "text", "text": content })]
            }
        });
    content_blocks.extend(injected_blocks);
    message.content_blocks = Some(Value::Array(content_blocks));
}

pub(super) async fn visible_internal_llm_tool_precondition_error(
    variable_pool: &Map<String, Value>,
) -> Option<Value> {
    for precondition in visible_internal_llm_tool_preconditions(variable_pool) {
        match &precondition {
            VisibleInternalLlmToolPrecondition::MediaContentAvailable(media_precondition) => {
                if let Some(error_payload) = visible_internal_llm_tool_media_unavailable_error(
                    media_precondition,
                    &precondition,
                    variable_pool,
                )
                .await
                {
                    return Some(error_payload);
                }
            }
        }
    }

    None
}

async fn visible_internal_llm_tool_media_unavailable_error(
    precondition: &VisibleInternalLlmToolMediaContentPrecondition,
    precondition_value: &VisibleInternalLlmToolPrecondition,
    variable_pool: &Map<String, Value>,
) -> Option<Value> {
    let media_items =
        visible_internal_llm_tool_media_arguments_for_precondition(precondition, variable_pool);
    let workspace_media = media_items
        .iter()
        .filter(|media| workspace_image_media_path(media).is_some())
        .collect::<Vec<_>>();
    if workspace_media.is_empty() {
        return None;
    }

    let mut unavailable = Vec::new();
    for media in workspace_media {
        if workspace_image_media_is_available(media).await {
            continue;
        } else {
            unavailable.push(media.clone());
        }
    }
    if unavailable.is_empty() || !inherited_image_content_blocks(variable_pool).is_empty() {
        return None;
    }

    Some(json!({
        "error_code": "visible_internal_llm_tool_media_unavailable",
        "message": "visible internal LLM tool media was not available to the server",
        "recoverable": true,
        "precondition": visible_internal_llm_tool_precondition_value(precondition_value),
        "media": unavailable,
        "hint": "If this path is local to an external client, read the file with a client file tool first and call the routed LLM tool again after the image content block is present in history."
    }))
}

fn visible_internal_llm_tool_preconditions(
    variable_pool: &Map<String, Value>,
) -> Vec<VisibleInternalLlmToolPrecondition> {
    variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("preconditions"))
        .map(|preconditions| {
            visible_internal_llm_tool_preconditions_from_value(Some(preconditions))
        })
        .unwrap_or_default()
}

fn visible_internal_llm_tool_media_arguments(variable_pool: &Map<String, Value>) -> Vec<Value> {
    let preconditions = visible_internal_llm_tool_preconditions(variable_pool);
    if preconditions.is_empty() {
        return visible_internal_llm_tool_media_arguments_at_path(
            &["media".to_string()],
            None,
            variable_pool,
        );
    }

    preconditions
        .iter()
        .flat_map(|precondition| match precondition {
            VisibleInternalLlmToolPrecondition::MediaContentAvailable(media_precondition) => {
                visible_internal_llm_tool_media_arguments_for_precondition(
                    media_precondition,
                    variable_pool,
                )
            }
        })
        .collect()
}

fn visible_internal_llm_tool_media_arguments_for_precondition(
    precondition: &VisibleInternalLlmToolMediaContentPrecondition,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    visible_internal_llm_tool_media_arguments_at_path(
        &precondition.argument_path,
        precondition.media_kind.as_deref(),
        variable_pool,
    )
}

fn visible_internal_llm_tool_media_arguments_at_path(
    argument_path: &[String],
    media_kind: Option<&str>,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    let Some(mut value) = variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("arguments"))
    else {
        return Vec::new();
    };

    for segment in argument_path {
        let Some(next) = value.get(segment) else {
            return Vec::new();
        };
        value = next;
    }

    let media_items = if let Some(array) = value.as_array() {
        array.clone()
    } else {
        vec![value.clone()]
    };

    media_items
        .into_iter()
        .filter(|media| {
            media_kind.is_none_or(|media_kind| {
                media.get("kind").and_then(Value::as_str) == Some(media_kind)
            })
        })
        .collect()
}

fn inherited_image_content_blocks(variable_pool: &Map<String, Value>) -> Vec<Value> {
    variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|value| value.get("context"))
        .and_then(|context| context.get("history"))
        .and_then(Value::as_array)
        .map(|history| {
            history
                .iter()
                .flat_map(image_content_blocks_from_message)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn image_content_blocks_from_message(message: &Value) -> Vec<Value> {
    [message.get("content_blocks"), message.get("content")]
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flat_map(|blocks| blocks.iter())
        .filter(|block| image_content_block_is_supported(block))
        .cloned()
        .collect()
}

fn image_content_block_is_supported(block: &Value) -> bool {
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("image" | "image_url" | "input_image")
    )
}

fn provider_input_has_image_content_blocks(input: &ProviderInvocationInput) -> bool {
    input.messages.iter().any(|message| {
        message
            .content_blocks
            .as_ref()
            .and_then(Value::as_array)
            .is_some_and(|blocks| blocks.iter().any(image_content_block_is_supported))
    })
}

async fn image_content_block_from_workspace_media(media: &Value) -> Option<Value> {
    if media.get("kind").and_then(Value::as_str) != Some("image")
        || media.get("source").and_then(Value::as_str) != Some("workspace_path")
    {
        return None;
    }
    let raw_path = workspace_image_media_path(media)?;
    if raw_path.is_empty() || !looks_like_image_path(raw_path) {
        return None;
    }
    let resolved_path = resolve_workspace_media_path(raw_path).await?;
    let bytes = tokio::fs::read(&resolved_path).await.ok()?;
    let media_type = image_media_type_from_path(&resolved_path)?;
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(json!({
        "type": "image_url",
        "image_url": {
            "url": format!("data:{media_type};base64,{encoded}")
        }
    }))
}

fn workspace_image_media_path(media: &Value) -> Option<&str> {
    if media.get("kind").and_then(Value::as_str) != Some("image")
        || media.get("source").and_then(Value::as_str) != Some("workspace_path")
    {
        return None;
    }
    media
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty() && looks_like_image_path(path))
}

async fn workspace_image_media_is_available(media: &Value) -> bool {
    let Some(raw_path) = workspace_image_media_path(media) else {
        return false;
    };
    let Some(resolved_path) = resolve_workspace_media_path(raw_path).await else {
        return false;
    };
    image_media_type_from_path(&resolved_path).is_some()
        && tokio::fs::metadata(&resolved_path)
            .await
            .is_ok_and(|metadata| metadata.is_file())
}

async fn resolve_workspace_media_path(raw_path: &str) -> Option<std::path::PathBuf> {
    let normalized = raw_path.replace('\\', "/");
    let requested_path = std::path::Path::new(&normalized);
    if !workspace_media_path_shape_allowed(requested_path) {
        return None;
    }

    let current_dir = std::env::current_dir().ok()?;
    let roots = workspace_media_roots_from(&current_dir);
    resolve_workspace_media_path_from_roots(requested_path, roots).await
}

fn workspace_media_roots_from(current_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    push_workspace_media_root(&mut roots, current_dir.to_path_buf());
    if let Some(parent) = current_dir.parent() {
        push_workspace_media_root(&mut roots, parent.to_path_buf());
    }
    for ancestor in current_dir.ancestors() {
        if workspace_media_root_marker(ancestor) {
            push_workspace_media_root(&mut roots, ancestor.to_path_buf());
        }
    }
    for ancestor in std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        if workspace_media_root_marker(ancestor) {
            push_workspace_media_root(&mut roots, ancestor.to_path_buf());
        }
    }
    roots
}

fn push_workspace_media_root(roots: &mut Vec<std::path::PathBuf>, root: std::path::PathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
    }
}

fn workspace_media_root_marker(path: &std::path::Path) -> bool {
    path.join("uploads").is_dir()
        || (path.join("AGENTS.md").is_file() && path.join("api").is_dir())
        || path.join("api/AGENTS.md").is_file()
}

async fn resolve_workspace_media_path_from_roots(
    requested_path: &std::path::Path,
    roots: Vec<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    let canonical_roots = roots
        .into_iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .collect::<Vec<_>>();

    if requested_path.is_absolute() {
        let canonical = tokio::fs::canonicalize(requested_path).await.ok()?;
        return canonical_roots
            .iter()
            .any(|root| canonical.starts_with(root))
            .then_some(canonical);
    }

    for root in canonical_roots {
        let candidate = root.join(requested_path);
        let Ok(canonical) = tokio::fs::canonicalize(candidate).await else {
            continue;
        };
        if canonical.starts_with(&root) {
            return Some(canonical);
        }
    }
    None
}

fn workspace_media_path_shape_allowed(path: &std::path::Path) -> bool {
    path.components().all(|component| {
        matches!(
            component,
            std::path::Component::Normal(_)
                | std::path::Component::CurDir
                | std::path::Component::RootDir
        )
    })
}

fn looks_like_image_path(path: &str) -> bool {
    let path = path.trim().to_ascii_lowercase();
    matches!(
        std::path::Path::new(&path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
    )
}

fn image_media_type_from_path(path: &std::path::Path) -> Option<String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("png") => Some("image/png".to_string()),
        Some("jpg" | "jpeg") => Some("image/jpeg".to_string()),
        Some("gif") => Some("image/gif".to_string()),
        Some("webp") => Some("image/webp".to_string()),
        Some("bmp") => Some("image/bmp".to_string()),
        _ => None,
    }
}

pub(super) fn visible_internal_llm_tool_inherited_context(
    variable_pool: &Map<String, Value>,
    main_node_id: &str,
) -> Value {
    let history = inherited_main_llm_history(variable_pool, main_node_id)
        .or_else(|| synthesized_run_context_history(variable_pool))
        .unwrap_or_default();
    json!({
        "history": history,
        "query": find_run_context_value(variable_pool, "query").unwrap_or(Value::Null),
        "files": find_run_context_array(variable_pool, "files"),
        "tools": find_run_context_array(variable_pool, "tools"),
    })
}

fn inherited_main_llm_history(
    variable_pool: &Map<String, Value>,
    main_node_id: &str,
) -> Option<Vec<Value>> {
    let mut history = variable_pool
        .get(main_node_id)?
        .get(LLM_TOOL_CALLBACK_STATE_KEY)?
        .get("history")?
        .as_array()?
        .clone();
    if history
        .last()
        .and_then(Value::as_object)
        .is_some_and(|message| {
            message.get("role").and_then(Value::as_str) == Some("assistant")
                && message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|tool_calls| !tool_calls.is_empty())
        })
    {
        history.pop();
    }
    (!history.is_empty()).then_some(history)
}

fn synthesized_run_context_history(variable_pool: &Map<String, Value>) -> Option<Vec<Value>> {
    let mut content_parts = Vec::new();
    if let Some(query) =
        find_run_context_value(variable_pool, "query").and_then(|value| value_to_text(&value))
    {
        if !query.trim().is_empty() {
            content_parts.push(query);
        }
    }
    let files = find_run_context_array(variable_pool, "files");
    if !files.is_empty() {
        content_parts.push(format!("Files: {}", Value::Array(files)));
    }

    let content = content_parts.join("\n\n");
    (!content.trim().is_empty()).then(|| {
        vec![json!({
            "role": "user",
            "content": content,
        })]
    })
}

fn find_run_context_value(variable_pool: &Map<String, Value>, key: &str) -> Option<Value> {
    variable_pool
        .get("node-start")
        .and_then(|payload| payload.get(key))
        .cloned()
        .or_else(|| {
            variable_pool
                .values()
                .find_map(|payload| payload.as_object()?.get(key).cloned())
        })
}

fn find_run_context_array(variable_pool: &Map<String, Value>, key: &str) -> Vec<Value> {
    find_run_context_value(variable_pool, key)
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolves_workspace_media_from_repo_root_when_started_in_api_server_dir() {
        let test_root =
            std::env::temp_dir().join(format!("1flowbase-visible-media-{}", uuid::Uuid::now_v7()));
        let repo_root = test_root.join("repo");
        let api_server_dir = repo_root.join("api/apps/api-server");
        let uploads_dir = repo_root.join("uploads");
        tokio::fs::create_dir_all(&api_server_dir)
            .await
            .expect("api-server test directory should be created");
        tokio::fs::create_dir_all(&uploads_dir)
            .await
            .expect("uploads test directory should be created");
        tokio::fs::write(repo_root.join("AGENTS.md"), b"workspace")
            .await
            .expect("workspace marker should be written");
        tokio::fs::write(uploads_dir.join("image-1.png"), b"image")
            .await
            .expect("test image should be written");

        let roots = workspace_media_roots_from(&api_server_dir);
        let resolved = resolve_workspace_media_path_from_roots(
            std::path::Path::new("uploads/image-1.png"),
            roots,
        )
        .await
        .expect("repo-root uploads image should resolve from api-server cwd");

        assert_eq!(
            resolved,
            std::fs::canonicalize(uploads_dir.join("image-1.png"))
                .expect("test image should canonicalize")
        );

        let _ = tokio::fs::remove_dir_all(test_root).await;
    }
}
