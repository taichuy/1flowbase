use super::responses_index::ResponsesInputIndex;
use super::*;

pub(super) fn validate_responses_input(
    input: &Value,
    is_v2_compaction: bool,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    validate_responses_input_items(input, is_v2_compaction, report).map_err(|error| {
        if !report.has_decision("$.input", TranslationDecisionKind::Rejected) {
            report.record(
                "$.input",
                None,
                TranslationDecisionKind::Rejected,
                Some("Responses input contains invalid items"),
                TranslationSafeRepresentation::Present,
            );
        }
        error.with_report(report.clone())
    })
}

pub(super) fn validate_native_responses_input(
    input: &Value,
    index: &ResponsesInputIndex,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    if input.is_string() {
        report.record(
            "$.input",
            Some("$.query"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        return Ok(());
    }
    let items = input
        .as_array()
        .expect("indexed non-text Responses input must be an array");
    debug_assert_eq!(index.item_count(), items.len());
    debug_assert_eq!(index.item_types().len(), items.len());
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("$.input[{index}]");
        let _object = item
            .as_object()
            .expect("ResponsesInputIndex validated every item as an object");
        report.record(
            &item_path,
            None,
            TranslationDecisionKind::Exact,
            Some("preserved only in native Responses provider transport"),
            TranslationSafeRepresentation::Redacted,
        );
    }
    report.record(
        "$.input",
        None,
        TranslationDecisionKind::Exact,
        Some("preserved only in native Responses provider transport"),
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn validate_responses_compaction_v2_input(
    input: &Value,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    let items = input.as_array().ok_or_else(|| {
        OpenAiCompatError::invalid("input", "V2 compaction input must be an array")
            .with_report(report.clone())
    })?;
    let mut trigger_index = None;
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("$.input[{index}]");
        let object = item.as_object().ok_or_else(|| {
            OpenAiCompatError::invalid("input", "V2 compaction input items must be objects")
                .with_report(report.clone())
        })?;
        let item_type = object
            .get("type")
            .and_then(Value::as_str)
            .filter(|item_type| !item_type.trim().is_empty())
            .ok_or_else(|| {
                OpenAiCompatError::invalid(
                    "input",
                    "V2 compaction input item type must be non-empty text",
                )
                .with_report(report.clone())
            })?;
        if item_type == "compaction_trigger" {
            if trigger_index.replace(index).is_some() {
                return Err(OpenAiCompatError::invalid(
                    "input",
                    "V2 compaction input must contain exactly one compaction_trigger",
                )
                .with_report(report.clone()));
            }
            if object.len() != 1 {
                return Err(OpenAiCompatError::invalid(
                    "input",
                    "unknown V2 compaction trigger field",
                )
                .with_report(report.clone()));
            }
        }
        report.record(
            &item_path,
            None,
            TranslationDecisionKind::Exact,
            Some("preserved only in V2 compaction provider transport"),
            TranslationSafeRepresentation::Redacted,
        );
    }
    match trigger_index {
        Some(index) if index + 1 == items.len() => {}
        Some(_) => {
            return Err(OpenAiCompatError::invalid(
                "input",
                "V2 compaction compaction_trigger must be the final input item",
            )
            .with_report(report.clone()));
        }
        None => {
            return Err(OpenAiCompatError::invalid(
                "input",
                "V2 compaction input must contain exactly one compaction_trigger",
            )
            .with_report(report.clone()));
        }
    }
    report.record(
        "$.input",
        None,
        TranslationDecisionKind::Exact,
        Some("preserved only in V2 compaction provider transport"),
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn validate_native_mcp_approval_continuation(
    input: &Value,
    previous_response: Option<&OpenAiPreviousResponseContext>,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    let Some(items) = input.as_array() else {
        return Ok(());
    };
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("mcp_approval_response") {
            continue;
        }
        let path = format!("$.input[{index}]");
        if previous_response.is_none() {
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("MCP approval response requires provider continuation"),
                TranslationSafeRepresentation::Present,
            );
            return Err(OpenAiCompatError::invalid(
                "previous_response_id",
                "mcp_approval_response requires previous_response_id",
            )
            .with_report(report.clone()));
        }
        if !object
            .get("approval_request_id")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(OpenAiCompatError::invalid(
                "input",
                "mcp_approval_response approval_request_id is required",
            )
            .with_report(report.clone()));
        }
        if !object.get("approve").is_some_and(Value::is_boolean) {
            return Err(OpenAiCompatError::invalid(
                "input",
                "mcp_approval_response approve must be a boolean",
            )
            .with_report(report.clone()));
        }
    }
    Ok(())
}

pub(super) fn validate_responses_input_items(
    input: &Value,
    is_v2_compaction: bool,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    validate_responses_items(input, is_v2_compaction, report, false)
}

/// Shared semantic item validation. Callback ownership/pairing was already proven
/// by the scoped callback owner; all message, role and content field rules remain identical.
fn validate_responses_items(
    input: &Value,
    is_v2_compaction: bool,
    report: &mut TranslationReportBuilder,
    verified_callback: bool,
) -> Result<(), OpenAiCompatError> {
    if input.is_string() {
        report.record(
            "$.input",
            Some("$.query"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        return Ok(());
    }
    let Some(items) = input.as_array() else {
        report.record(
            "$.input",
            None,
            TranslationDecisionKind::Rejected,
            Some("Responses input must be text or messages"),
            TranslationSafeRepresentation::Present,
        );
        return Err(
            OpenAiCompatError::invalid("input", "input must be text or messages")
                .with_report(report.clone()),
        );
    };
    let reconstructable_tool_continuation =
        verified_callback || responses_end_with_reconstructable_tool_output(items)?;
    let mut has_user_message = false;
    let mut has_compaction_trigger = false;
    for (index, item) in items.iter().enumerate() {
        let item_path = format!("$.input[{index}]");
        let Some(object) = item.as_object() else {
            report.record(
                &item_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("Responses input items must be objects"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("input", "input message must be an object")
                    .with_report(report.clone()),
            );
        };
        let type_path = format!("$.input[{index}].type");
        let item_type = match object.get("type") {
            Some(Value::String(item_type)) => Some(item_type.as_str()),
            Some(_) => {
                report.record(
                    &type_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Responses input item type must be text"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    OpenAiCompatError::invalid("input", "input item type must be text")
                        .with_report(report.clone()),
                );
            }
            None => {
                report.record(
                    &type_path,
                    Some("$.input[].type"),
                    TranslationDecisionKind::Defaulted,
                    Some("message is the default Responses input item type"),
                    TranslationSafeRepresentation::Defaulted,
                );
                None
            }
        };
        if item_type == Some("compaction_trigger") && is_v2_compaction {
            let unknown_fields = object
                .keys()
                .filter(|field| field.as_str() != "type")
                .collect::<Vec<_>>();
            if report.record_anonymous_unknown_fields(
                &item_path,
                unknown_fields,
                TranslationDecisionKind::Rejected,
                "unknown V2 compaction trigger field",
                TranslationSafeRepresentation::Present,
            ) > 0
            {
                return Err(OpenAiCompatError::invalid(
                    "input",
                    "unknown V2 compaction trigger field",
                )
                .with_report(report.clone()));
            }
            report.record(
                &item_path,
                Some("$.execution.operation"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
            report.record(
                &type_path,
                Some("$.execution.operation"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
            has_compaction_trigger = true;
            continue;
        }
        if super::is_standalone_function_output(item) {
            external_context_message(item)?;
            report.record(
                &item_path,
                Some("$.history"),
                TranslationDecisionKind::Normalized,
                Some("standalone output is attributed external context"),
                TranslationSafeRepresentation::Redacted,
            );
            continue;
        }
        if matches!(
            item_type,
            Some("function_call") | Some("function_call_output") | Some("reasoning")
        ) {
            let required_fields: &[&str] = match item_type {
                Some("function_call") => &["call_id", "name", "arguments"],
                Some("function_call_output") => &["call_id", "output"],
                Some("reasoning") => &[],
                _ => unreachable!(),
            };
            for field in required_fields {
                if !object.contains_key(*field) {
                    report.record(
                        &format!("{item_path}.{field}"),
                        None,
                        TranslationDecisionKind::Rejected,
                        Some("required Responses continuation field is missing"),
                        TranslationSafeRepresentation::Absent,
                    );
                    return Err(OpenAiCompatError::invalid(
                        "input",
                        format!("{field} is required"),
                    )
                    .with_report(report.clone()));
                }
            }
            report.record(
                &item_path,
                Some("$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            report.record(
                &type_path,
                Some("$.history"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            continue;
        }
        report.record(
            &item_path,
            Some("$.query,$.history"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        if !matches!(item_type, None | Some("message")) {
            let kind = if matches!(item_type, Some("item_reference")) {
                TranslationDecisionKind::Unsupported
            } else {
                TranslationDecisionKind::Rejected
            };
            report.record(
                &type_path,
                None,
                kind,
                Some("Responses continuation and tool items have no current canonical owner"),
                TranslationSafeRepresentation::Present,
            );
            let error = if kind == TranslationDecisionKind::Unsupported {
                OpenAiCompatError::unsupported("input")
            } else {
                OpenAiCompatError::invalid("input", "unknown Responses input item type")
            };
            return Err(error.with_report(report.clone()));
        }
        if item_type.is_some() {
            report.record(
                &type_path,
                Some("$.input[].type"),
                TranslationDecisionKind::Exact,
                None,
                TranslationSafeRepresentation::Present,
            );
        }
        let unknown_fields = object
            .keys()
            .filter(|field| !matches!(field.as_str(), "type" | "role" | "content"))
            .collect::<Vec<_>>();
        if report.record_anonymous_unknown_fields(
            &item_path,
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Responses input field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(
                OpenAiCompatError::invalid("input", "unknown Responses input field")
                    .with_report(report.clone()),
            );
        }
        let role_path = format!("$.input[{index}].role");
        let role_was_explicit = object.contains_key("role");
        let role = match object.get("role") {
            Some(Value::String(role)) => role.as_str(),
            Some(_) => {
                report.record(
                    &role_path,
                    None,
                    TranslationDecisionKind::Rejected,
                    Some("Responses message role must be text"),
                    TranslationSafeRepresentation::Present,
                );
                return Err(
                    OpenAiCompatError::invalid("input", "message role must be text")
                        .with_report(report.clone()),
                );
            }
            None => {
                report.record(
                    &role_path,
                    Some("$.query,$.history"),
                    TranslationDecisionKind::Defaulted,
                    Some("user is the default Responses message role"),
                    TranslationSafeRepresentation::Defaulted,
                );
                "user"
            }
        };
        if !matches!(role, "system" | "developer" | "user" | "assistant") {
            report.record(
                &role_path,
                None,
                TranslationDecisionKind::Rejected,
                Some("unsupported Responses message role"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                OpenAiCompatError::invalid("input", "unsupported message role")
                    .with_report(report.clone()),
            );
        }
        if role_was_explicit {
            report.record(
                &role_path,
                Some(if matches!(role, "system" | "developer") {
                    "$.system"
                } else {
                    "$.query,$.history"
                }),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
        }
        has_user_message |= role == "user";
        if !object.contains_key("content") {
            let path = format!("$.input[{index}].content");
            report.record(
                &path,
                None,
                TranslationDecisionKind::Rejected,
                Some("input content is required"),
                TranslationSafeRepresentation::Absent,
            );
            return Err(
                OpenAiCompatError::invalid("input", "input content is required")
                    .with_report(report.clone()),
            );
        }
        let content_path = format!("$.input[{index}].content");
        let content = object.get("content").expect("content exists");
        if matches!(content, Value::String(_) | Value::Array(_)) {
            report.record(
                &content_path,
                Some(if matches!(role, "system" | "developer") {
                    "$.system"
                } else {
                    "$.query,$.history"
                }),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        if matches!(role, "system" | "developer") {
            reject_responses_system_media(content, index, report)?;
        }
        validate_responses_content_parts(content, index, report)?;
    }
    if !has_user_message
        && !items.iter().any(super::is_standalone_function_output)
        && !reconstructable_tool_continuation
        && !(is_v2_compaction && has_compaction_trigger)
    {
        report.record(
            "$.input",
            None,
            TranslationDecisionKind::Rejected,
            Some("Responses input requires a user message"),
            TranslationSafeRepresentation::Redacted,
        );
        return Err(
            OpenAiCompatError::invalid("input", "user input is required")
                .with_report(report.clone()),
        );
    }
    report.record(
        "$.input",
        Some("$.query,$.history"),
        TranslationDecisionKind::Normalized,
        None,
        TranslationSafeRepresentation::Redacted,
    );
    Ok(())
}

pub(super) fn reject_responses_system_media(
    content: &Value,
    message_index: usize,
    report: &mut TranslationReportBuilder,
) -> Result<(), OpenAiCompatError> {
    let Some(parts) = content.as_array() else {
        return Ok(());
    };
    for (part_index, part) in parts.iter().enumerate() {
        let Some(part) = part.as_object() else {
            continue;
        };
        if !matches!(
            part.get("type").and_then(Value::as_str),
            Some("image_url" | "input_image")
        ) {
            continue;
        }
        let type_path = format!("$.input[{message_index}].content[{part_index}].type");
        report.record(
            &type_path,
            None,
            TranslationDecisionKind::Unsupported,
            Some("Responses system and developer media has no current canonical owner"),
            TranslationSafeRepresentation::Present,
        );
        return Err(OpenAiCompatError::unsupported("input").with_report(report.clone()));
    }
    Ok(())
}

pub(super) fn responses_input_to_native_run_input(
    input: &Value,
    is_v2_compaction: bool,
) -> Result<ResponsesInputMapping, OpenAiCompatError> {
    if let Some(text) = input.as_str() {
        return Ok(ResponsesInputMapping {
            query: text.to_string(),
            history: Vec::new(),
            system_parts: Vec::new(),
        });
    }

    let items = input
        .as_array()
        .ok_or_else(|| OpenAiCompatError::invalid("input", "input must be text or messages"))?;
    let reconstructable_tool_continuation = responses_end_with_reconstructable_tool_output(items)?;
    let last_user_index = if reconstructable_tool_continuation {
        None
    } else {
        items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (!is_v2_compaction || !compaction::is_compaction_trigger_item(item))
                    .then_some((index, item))
            })
            .filter(|(_, item)| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    None | Some("message")
                ) && item.get("role").and_then(Value::as_str).unwrap_or("user") == "user"
            })
            .map(|(index, _)| index)
            .next_back()
    };
    if last_user_index.is_none()
        && !reconstructable_tool_continuation
        && !items.iter().any(super::is_standalone_function_output)
    {
        if is_v2_compaction {
            return Ok(ResponsesInputMapping {
                query: String::new(),
                history: Vec::new(),
                system_parts: Vec::new(),
            });
        }
        return Err(OpenAiCompatError::invalid(
            "input",
            "user input is required",
        ));
    }

    let mut query = String::new();
    let mut history: Vec<Value> = Vec::new();
    let mut system_parts = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if is_v2_compaction && compaction::is_compaction_trigger_item(item) {
            continue;
        }
        match item.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                let tool_call = json!({
                    "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                    "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                    "arguments": item.get("arguments").map(parse_openai_tool_arguments).unwrap_or_else(|| json!({})),
                });
                if let Some(existing) = history.last_mut().filter(|entry| {
                    entry.get("role").and_then(Value::as_str) == Some("assistant")
                        && entry.get("tool_calls").is_some_and(Value::is_array)
                }) {
                    existing["tool_calls"]
                        .as_array_mut()
                        .expect("validated tool_calls array")
                        .push(tool_call);
                } else {
                    history.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [tool_call]
                    }));
                }
                continue;
            }
            Some("function_call_output") if super::is_standalone_function_output(item) => {
                history.push(external_context_message(item)?);
                continue;
            }
            Some("function_call_output") => {
                let content = match item.get("output") {
                    Some(Value::String(output)) => output.clone(),
                    Some(output) => output.to_string(),
                    None => String::new(),
                };
                history.push(json!({
                    "role": "tool",
                    "content": content,
                    "tool_call_id": item.get("call_id").and_then(Value::as_str).unwrap_or_default()
                }));
                continue;
            }
            Some("reasoning") => {
                history.push(json!({
                    "role": "assistant",
                    "content": "",
                    "reasoning": item.clone()
                }));
                continue;
            }
            _ => {}
        }
        let message = responses_input_message(item)?;
        if Some(index) == last_user_index && index + 1 == items.len() {
            query = message.content.clone();
            if message.content_blocks.is_none() {
                continue;
            }
        }

        if matches!(message.role.as_str(), "system" | "developer") {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content);
            }
            continue;
        }

        let mut history_entry = serde_json::json!({
            "role": message.role,
            "content": message.content,
        });
        if let Some(content_blocks) = message.content_blocks {
            history_entry["content_blocks"] = content_blocks;
        }
        history.push(history_entry);
    }

    Ok(ResponsesInputMapping {
        query,
        history,
        system_parts,
    })
}

pub(super) fn responses_native_input_to_run_input(input: &Value) -> ResponsesInputMapping {
    if let Some(text) = input.as_str() {
        return ResponsesInputMapping {
            query: text.to_string(),
            history: Vec::new(),
            system_parts: Vec::new(),
        };
    }
    let mut messages = input
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            matches!(
                item.get("type").and_then(Value::as_str),
                None | Some("message")
            )
        })
        .filter_map(|item| responses_input_message(item).ok())
        .collect::<Vec<_>>();
    let last_user_index = messages.iter().rposition(|message| message.role == "user");
    let query = last_user_index
        .map(|index| messages.remove(index).content)
        .unwrap_or_default();
    let mut history = Vec::new();
    let mut system_parts = Vec::new();
    for message in messages {
        if matches!(message.role.as_str(), "system" | "developer") {
            if !message.content.trim().is_empty() {
                system_parts.push(message.content);
            }
            continue;
        }
        let mut entry = json!({
            "role": message.role,
            "content": message.content,
        });
        if let Some(content_blocks) = message.content_blocks {
            entry["content_blocks"] = content_blocks;
        }
        history.push(entry);
    }
    ResponsesInputMapping {
        query,
        history,
        system_parts,
    }
}

pub(super) fn responses_opaque_input_to_run_input() -> ResponsesInputMapping {
    ResponsesInputMapping {
        query: String::new(),
        history: Vec::new(),
        system_parts: Vec::new(),
    }
}

pub(super) fn responses_compaction_v2_input_to_run_input() -> ResponsesInputMapping {
    responses_opaque_input_to_run_input()
}

pub(super) fn responses_end_with_reconstructable_tool_output(
    items: &[Value],
) -> Result<bool, OpenAiCompatError> {
    if items
        .iter()
        .rev()
        .find(|item| !super::is_standalone_function_output(item))
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        != Some("function_call_output")
    {
        return Ok(false);
    }
    let call_ids = items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("function_call"))
        .filter_map(|item| item.get("call_id").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let outputs_are_paired = items
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && !super::is_standalone_function_output(item)
        })
        .all(|item| {
            item.get("call_id")
                .and_then(Value::as_str)
                .is_some_and(|call_id| call_ids.contains(call_id))
        });
    if call_ids.is_empty() || !outputs_are_paired {
        return Err(OpenAiCompatError::invalid(
            "input",
            "function_call_output requires a matching function_call",
        ));
    }
    Ok(true)
}

pub(super) fn openai_chat_history_tool_calls(tool_calls: &Value) -> Value {
    Value::Array(
        tool_calls
            .as_array()
            .into_iter()
            .flatten()
            .map(|tool_call| {
                let id = tool_call
                    .get("id")
                    .and_then(Value::as_str)
                    .map(openai_chat_history_tool_call_id)
                    .unwrap_or_default();
                let function = tool_call.get("function").and_then(Value::as_object);
                let name = function
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = function
                    .and_then(|value| value.get("arguments"))
                    .map(parse_openai_tool_arguments)
                    .unwrap_or_else(|| json!({}));
                json!({ "id": id, "name": name, "arguments": arguments })
            })
            .collect(),
    )
}

pub(super) fn openai_chat_history_tool_call_id(tool_call_id: &str) -> String {
    decode_openai_callback_tool_call_id(tool_call_id)
        .map(|(_, original_tool_call_id)| original_tool_call_id)
        .unwrap_or_else(|| tool_call_id.to_string())
}

#[path = "response_input/content_mapping.rs"]
mod content_mapping;

#[path = "response_input/media_validation.rs"]
mod media_validation;

pub(crate) use content_mapping::continuation_messages;
#[cfg(test)]
pub(super) use content_mapping::openai_content;
pub(super) use content_mapping::openai_message_content;
pub(super) use content_mapping::parse_openai_tool_arguments;
pub(super) use content_mapping::responses_input_message;
pub(super) use content_mapping::responses_previous_history;
pub(super) use content_mapping::ResponsesInputMapping;
use content_mapping::*;

pub(super) use media_validation::validate_openai_supported_content_part;
pub(super) use media_validation::validate_responses_content_parts;
