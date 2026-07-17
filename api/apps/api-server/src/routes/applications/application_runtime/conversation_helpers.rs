#[cfg(test)]
async fn to_application_conversation_message_response<F, Fut>(
    run: domain::FlowRunRecord,
    current_run_id: Option<Uuid>,
    load_debug_artifact: &F,
) -> ApplicationConversationMessageResponse
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let run_id = run.id.to_string();
    let mut answer =
        application_conversation_answer_text(&run.output_payload, &load_debug_artifact).await;
    if answer.is_none() {
        if let Some(error_payload) = run.error_payload.as_ref() {
            answer =
                application_conversation_answer_text(error_payload, &load_debug_artifact).await;
        }
    }

    ApplicationConversationMessageResponse {
        run_id: run_id.clone(),
        detail_run_id: Some(run_id),
        can_open_detail: true,
        role: None,
        content: None,
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: run.status.as_str().to_string(),
        query: application_run_query(&run.input_payload),
        model: application_run_model(&run.input_payload),
        answer,
        is_current: current_run_id == Some(run.id),
    }
}

fn to_application_conversation_message_summary_response(
    run: domain::ApplicationConversationRunSummary,
    current_run_id: Option<Uuid>,
) -> ApplicationConversationMessageResponse {
    let run_id = run.id.to_string();

    ApplicationConversationMessageResponse {
        run_id: run_id.clone(),
        detail_run_id: Some(run_id),
        can_open_detail: true,
        role: None,
        content: None,
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: run.status.as_str().to_string(),
        query: run.query,
        model: run.model,
        answer: run.answer,
        is_current: current_run_id == Some(run.id),
    }
}

#[cfg(test)]
async fn application_conversation_answer_text<F, Fut>(
    payload: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<String>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    if let Some(full_payload) =
        load_referenced_runtime_debug_artifact(payload, load_debug_artifact).await
    {
        if let Some(text) = inline_conversation_value_text(&full_payload)
            .or_else(|| inline_application_conversation_answer_text(&full_payload))
        {
            return Some(text);
        }
    }

    if let Some(text) = inline_application_conversation_answer_text(payload) {
        return Some(text);
    }

    for key in ["answer", "text", "content", "message"] {
        let Some(value) = payload.get(key) else {
            continue;
        };
        let Some(full_value) =
            load_referenced_runtime_debug_artifact(value, load_debug_artifact).await
        else {
            continue;
        };
        if let Some(text) = inline_conversation_value_text(&full_value)
            .or_else(|| inline_application_conversation_answer_text(&full_value))
        {
            return Some(text);
        }
    }

    inline_application_conversation_error_text(payload)
}

#[cfg(test)]
async fn load_referenced_runtime_debug_artifact<F, Fut>(
    value: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<serde_json::Value>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let artifact_id = runtime_debug_artifact_id(value)?;
    load_debug_artifact(artifact_id).await
}

#[cfg(test)]
fn inline_application_conversation_answer_text(payload: &serde_json::Value) -> Option<String> {
    ["answer", "text", "content", "message"]
        .into_iter()
        .filter_map(|key| payload.get(key))
        .find_map(inline_conversation_value_text)
}

#[cfg(test)]
fn inline_application_conversation_error_text(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("error")
        .and_then(|value| value.get("message"))
        .and_then(inline_conversation_value_text)
}

#[cfg(test)]
fn inline_conversation_value_text(value: &serde_json::Value) -> Option<String> {
    if runtime_debug_artifact_id(value).is_some() {
        return None;
    }

    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    if let Some(parts) = value.as_array() {
        let text = parts
            .iter()
            .filter_map(inline_conversation_value_text)
            .collect::<Vec<_>>()
            .join("");
        return (!text.is_empty()).then_some(text);
    }

    let object = value.as_object()?;
    ["text", "content", "message"]
        .into_iter()
        .filter_map(|key| object.get(key))
        .find_map(inline_conversation_value_text)
}

fn parse_optional_uuid_cursor(value: Option<&str>) -> Option<Uuid> {
    value.and_then(|value| Uuid::parse_str(value).ok())
}

fn parse_run_conversation_message_sequence_cursor(
    run_id: Uuid,
    value: Option<&str>,
) -> Option<i64> {
    value
        .and_then(|cursor| parse_imported_context_cursor(run_id, cursor))
        .and_then(|sequence| i64::try_from(sequence).ok())
}

fn conversation_messages_from_projection_page(
    run_id: Uuid,
    page: control_plane::ports::ApplicationRunConversationMessageItemsPage,
) -> ApplicationConversationMessagesPageResponse {
    ApplicationConversationMessagesPageResponse {
        items: page
            .items
            .into_iter()
            .map(|item| application_run_conversation_message_item_response(run_id, item))
            .collect(),
        page: ApplicationConversationMessagesPageInfoResponse {
            has_before: page.has_before,
            has_after: page.has_after,
            before_cursor: page
                .before_cursor
                .and_then(|cursor| usize::try_from(cursor).ok())
                .map(|cursor| imported_context_cursor(run_id, cursor)),
            after_cursor: page
                .after_cursor
                .and_then(|cursor| usize::try_from(cursor).ok())
                .map(|cursor| imported_context_cursor(run_id, cursor)),
        },
    }
}

fn conversation_messages_from_current_item(
    run_id: Uuid,
    item: domain::ApplicationRunConversationMessageItem,
) -> ApplicationConversationMessagesPageResponse {
    ApplicationConversationMessagesPageResponse {
        items: vec![application_run_conversation_message_item_response(run_id, item)],
        page: ApplicationConversationMessagesPageInfoResponse {
            has_before: false,
            has_after: false,
            before_cursor: None,
            after_cursor: None,
        },
    }
}

fn application_run_conversation_message_item_response(
    run_id: Uuid,
    item: domain::ApplicationRunConversationMessageItem,
) -> ApplicationConversationMessageResponse {
    let item_run_id = if item.source_kind == "current_run" {
        item.flow_run_id.to_string()
    } else {
        usize::try_from(item.display_sequence)
            .ok()
            .map(|sequence| imported_context_cursor(run_id, sequence))
            .unwrap_or_else(|| item.flow_run_id.to_string())
    };

    ApplicationConversationMessageResponse {
        run_id: item_run_id,
        detail_run_id: item.detail_run_id.map(|value| value.to_string()),
        can_open_detail: item.can_open_detail,
        role: item.role,
        content: item.content,
        started_at: format_time(item.started_at),
        finished_at: format_optional_time(item.finished_at),
        status: item.status,
        query: item.query,
        model: item.model,
        answer: item.answer,
        is_current: item.is_current,
    }
}

#[cfg(test)]
async fn conversation_messages_from_single_run<F, Fut>(
    run: &domain::FlowRunRecord,
    query: &ApplicationConversationMessagesQuery,
    load_debug_artifact: &F,
) -> ApplicationConversationMessagesPageResponse
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let items = imported_context_messages_from_run(run, load_debug_artifact).await;
    conversation_messages_from_context_items(run, items, query, load_debug_artifact).await
}

#[cfg(test)]
async fn conversation_messages_from_run_detail<F, Fut>(
    detail: &domain::ApplicationRunDetail,
    query: &ApplicationConversationMessagesQuery,
    load_debug_artifact: &F,
) -> ApplicationConversationMessagesPageResponse
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let mut items = imported_context_messages_from_run(&detail.flow_run, load_debug_artifact).await;
    if !items
        .iter()
        .any(|item| item.role.as_deref() == Some("system"))
    {
        if let Some(system) = llm_system_content_from_node_runs(detail, load_debug_artifact).await {
            items.insert(
                0,
                imported_context_item(&detail.flow_run, 0, "system", system),
            );
        }
    }

    conversation_messages_from_context_items(&detail.flow_run, items, query, load_debug_artifact)
        .await
}

#[cfg(test)]
async fn conversation_messages_from_context_items<F, Fut>(
    run: &domain::FlowRunRecord,
    mut items: Vec<ApplicationConversationMessageResponse>,
    query: &ApplicationConversationMessagesQuery,
    load_debug_artifact: &F,
) -> ApplicationConversationMessagesPageResponse
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let limit = query.limit.unwrap_or(5).clamp(1, 50) as usize;
    renumber_imported_context_items(run.id, &mut items);
    items.push(
        to_application_conversation_message_response(
            run.clone(),
            Some(run.id),
            load_debug_artifact,
        )
        .await,
    );

    let total = items.len();
    let (start, end) = imported_context_window(run.id, total, limit, query);

    ApplicationConversationMessagesPageResponse {
        items: items
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect(),
        page: ApplicationConversationMessagesPageInfoResponse {
            has_before: start > 0,
            has_after: end < total,
            before_cursor: (start > 0).then(|| imported_context_cursor(run.id, start)),
            after_cursor: (end < total).then(|| imported_context_cursor(run.id, end - 1)),
        },
    }
}

#[cfg(test)]
fn renumber_imported_context_items(
    run_id: Uuid,
    items: &mut [ApplicationConversationMessageResponse],
) {
    for (index, item) in items.iter_mut().enumerate() {
        item.run_id = imported_context_cursor(run_id, index);
    }
}

#[cfg(test)]
async fn imported_context_messages_from_run<F, Fut>(
    run: &domain::FlowRunRecord,
    load_debug_artifact: &F,
) -> Vec<ApplicationConversationMessageResponse>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let source = resolve_runtime_debug_artifact_value(&run.input_payload, &load_debug_artifact)
        .await
        .unwrap_or_else(|| run.input_payload.clone());
    let start_payload = start_input_payload(&source);
    let mut items = Vec::new();

    if let Some(system) = run_level_system_content(&source, &load_debug_artifact).await {
        items.push(imported_context_item(run, items.len(), "system", system));
    }

    let Some(history_value) = start_payload
        .get("history")
        .or_else(|| start_payload.get("messages"))
    else {
        return items;
    };
    let history_source =
        match resolve_runtime_debug_artifact_value(history_value, &load_debug_artifact).await {
            Some(value) => value,
            None => history_value.clone(),
    };
    let Some(history) = history_source.as_array() else {
        return items;
    };

    for message in history {
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(content) = conversation_message_content(message) else {
            continue;
        };
        if is_hidden_conversation_history_message(message) {
            continue;
        }

        match role {
            "system"
                if !items
                    .iter()
                    .any(|item| item.role.as_deref() == Some("system")) =>
            {
                items.push(imported_context_item(run, items.len(), role, content))
            }
            "user" | "assistant" => {
                items.push(imported_context_item(run, items.len(), role, content))
            }
            _ => {}
        }
    }

    items
}

#[cfg(test)]
fn is_hidden_conversation_history_message(message: &serde_json::Value) -> bool {
    message
        .get("metadata")
        .and_then(|metadata| metadata.get("hidden_from_conversation"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
async fn llm_system_content_from_node_runs<F, Fut>(
    detail: &domain::ApplicationRunDetail,
    load_debug_artifact: &F,
) -> Option<String>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    for node_run in detail
        .node_runs
        .iter()
        .filter(|node_run| node_run.node_type == "llm")
    {
        if let Some(system) =
            llm_prompt_messages_system_content(&node_run.input_payload, load_debug_artifact).await
        {
            return Some(system);
        }
        if let Some(system) =
            llm_effective_system_content(&node_run.debug_payload, load_debug_artifact).await
        {
            return Some(system);
        }
    }

    None
}

#[cfg(test)]
async fn llm_prompt_messages_system_content<F, Fut>(
    payload: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<String>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let prompt_messages_value = payload.get("prompt_messages")?;
    let prompt_messages =
        resolve_runtime_debug_artifact_value(prompt_messages_value, load_debug_artifact)
            .await
            .unwrap_or_else(|| prompt_messages_value.clone());
    let messages = prompt_messages.as_array()?;
    let system = messages
        .iter()
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
        .filter_map(conversation_message_content)
        .collect::<Vec<_>>()
        .join("\n\n");

    (!system.trim().is_empty()).then_some(system)
}

#[cfg(test)]
async fn llm_effective_system_content<F, Fut>(
    payload: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<String>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let effective_system = payload
        .get("llm_context")
        .and_then(|context| context.get("effective_system"))?;
    let resolved_system =
        resolve_runtime_debug_artifact_value(effective_system, load_debug_artifact).await;

    resolved_system
        .as_ref()
        .and_then(conversation_prompt_text)
        .or_else(|| conversation_prompt_text(effective_system))
}

#[cfg(test)]
async fn resolve_runtime_debug_artifact_value<F, Fut>(
    value: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<serde_json::Value>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let artifact_id = runtime_debug_artifact_id(value)?;

    load_debug_artifact(artifact_id)
        .await
        .or_else(|| decode_runtime_debug_artifact_preview(value))
}

#[cfg(test)]
fn runtime_debug_artifact_id(value: &serde_json::Value) -> Option<Uuid> {
    if !value
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    value
        .get("artifact_ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[cfg(test)]
fn imported_context_window(
    run_id: Uuid,
    total: usize,
    limit: usize,
    query: &ApplicationConversationMessagesQuery,
) -> (usize, usize) {
    if total == 0 {
        return (0, 0);
    }

    if let Some(before) = query
        .before
        .as_deref()
        .and_then(|cursor| parse_imported_context_cursor(run_id, cursor))
    {
        let end = before.min(total);
        return (end.saturating_sub(limit), end);
    }

    if let Some(after) = query
        .after
        .as_deref()
        .and_then(|cursor| parse_imported_context_cursor(run_id, cursor))
    {
        let start = (after + 1).min(total);
        return (start, (start + limit).min(total));
    }

    (total.saturating_sub(limit), total)
}

fn imported_context_cursor(run_id: Uuid, index: usize) -> String {
    format!("{run_id}:context:{index}")
}

fn parse_imported_context_cursor(run_id: Uuid, cursor: &str) -> Option<usize> {
    let (prefix, index) = cursor.rsplit_once(":context:")?;
    if prefix != run_id.to_string() {
        return None;
    }

    index.parse().ok()
}

#[cfg(test)]
fn imported_context_item(
    run: &domain::FlowRunRecord,
    index: usize,
    role: &str,
    content: String,
) -> ApplicationConversationMessageResponse {
    ApplicationConversationMessageResponse {
        run_id: imported_context_cursor(run.id, index),
        detail_run_id: None,
        can_open_detail: false,
        role: Some(role.to_string()),
        content: Some(content),
        started_at: format_time(run.started_at),
        finished_at: format_optional_time(run.finished_at),
        status: "succeeded".to_string(),
        query: None,
        model: application_run_model(&run.input_payload),
        answer: None,
        is_current: false,
    }
}

#[cfg(test)]
async fn run_level_system_content<F, Fut>(
    payload: &serde_json::Value,
    load_debug_artifact: &F,
) -> Option<String>
where
    F: Fn(Uuid) -> Fut,
    Fut: std::future::Future<Output = Option<serde_json::Value>>,
{
    let start_payload = start_input_payload(payload);
    let system_value = start_payload
        .get("system")
        .or_else(|| payload.get("system"))?;
    let resolved_system =
        resolve_runtime_debug_artifact_value(system_value, load_debug_artifact).await;

    resolved_system
        .as_ref()
        .and_then(conversation_prompt_text)
        .or_else(|| conversation_prompt_text(system_value))
}

#[cfg(test)]
fn conversation_prompt_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    if let Some(array) = value.as_array() {
        let text = array
            .iter()
            .filter_map(conversation_content_part_text)
            .collect::<Vec<_>>()
            .join("");
        return (!text.is_empty()).then_some(text);
    }

    conversation_content_part_text(value).or_else(|| conversation_preview_text(value))
}

#[cfg(test)]
fn conversation_preview_text(value: &serde_json::Value) -> Option<String> {
    let preview = value
        .get("preview")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    serde_json::from_str::<serde_json::Value>(preview)
        .ok()
        .as_ref()
        .and_then(conversation_prompt_text)
        .or_else(|| Some(preview.to_string()))
}

#[cfg(test)]
fn conversation_message_content(message: &serde_json::Value) -> Option<String> {
    let content = message.get("content")?;
    if let Some(text) = content
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    if let Some(array) = content.as_array() {
        let text = array
            .iter()
            .filter_map(conversation_content_part_text)
            .collect::<Vec<_>>()
            .join("");
        return (!text.is_empty()).then_some(text);
    }

    conversation_content_part_text(content)
}

#[cfg(test)]
fn conversation_content_part_text(part: &serde_json::Value) -> Option<String> {
    if let Some(text) = part
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(text.to_string());
    }

    part.get("text")
        .or_else(|| part.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
