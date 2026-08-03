use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenAiToolMapping {
    ChatCompletions,
    ResponsesSemantic,
    ResponsesNative,
}

pub(super) fn openai_inputs(
    object: &Map<String, Value>,
    tool_mapping: OpenAiToolMapping,
    report: &mut TranslationReport,
) -> Result<crate::application_public_api::native::NativeObject, OpenAiCompatError> {
    let mut inputs = Map::new();
    if let Some(value) = object.get("tools") {
        let tools = value.as_array().ok_or_else(|| {
            OpenAiCompatError::invalid("tools", "tools must be an array")
                .with_report(report.clone())
        })?;
        let mut normalized = Vec::with_capacity(tools.len());
        for (index, tool) in tools.iter().enumerate() {
            let tool = tool.as_object().ok_or_else(|| {
                OpenAiCompatError::invalid("tools", "tool definitions must be objects")
                    .with_report(report.clone())
            })?;
            if tool_mapping == OpenAiToolMapping::ResponsesNative {
                report.record(
                    &format!("$.tools[{index}]"),
                    None,
                    TranslationDecisionKind::Exact,
                    Some("preserved only in native Responses provider transport"),
                    TranslationSafeRepresentation::Redacted,
                );
                continue;
            }
            if tool_mapping == OpenAiToolMapping::ResponsesSemantic
                && tool.get("type").and_then(Value::as_str) != Some("function")
            {
                report.record(
                    &format!("$.tools[{index}]"),
                    Some("$.client_protocol_envelope.body.responses_optional_tools[]"),
                    TranslationDecisionKind::Exact,
                    Some("unsupported optional Responses tool retained in protocol context"),
                    TranslationSafeRepresentation::Redacted,
                );
                continue;
            }
            let function = if tool_mapping == OpenAiToolMapping::ChatCompletions {
                if tool.get("type").and_then(Value::as_str) != Some("function") {
                    return Err(OpenAiCompatError::invalid(
                        "tools",
                        "only function tools are supported",
                    )
                    .with_report(report.clone()));
                }
                tool.get("function")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        OpenAiCompatError::invalid("tools", "function tool payload is required")
                            .with_report(report.clone())
                    })?
            } else {
                tool
            };
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    OpenAiCompatError::invalid("tools", "tool name is required")
                        .with_report(report.clone())
                })?;
            let input_schema = function
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            let mut native_tool = json!({
                "name": name,
                "input_schema": input_schema,
                "source": "client"
            });
            if let Some(description) = function.get("description").and_then(Value::as_str) {
                native_tool["description"] = Value::String(description.to_string());
            }
            normalized.push(native_tool);
            report.record(
                &format!("$.tools[{index}]"),
                Some("$.inputs.tools[]"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
        }
        if tool_mapping == OpenAiToolMapping::ResponsesNative {
            report.record(
                "$.tools",
                None,
                TranslationDecisionKind::Exact,
                Some("preserved only in native Responses provider transport"),
                TranslationSafeRepresentation::Redacted,
            );
        } else {
            report.record(
                "$.tools",
                Some("$.inputs.tools"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Redacted,
            );
            inputs.insert("tools".to_string(), Value::Array(normalized));
        }
    }
    if let Some(choice) = object.get("tool_choice") {
        if tool_mapping == OpenAiToolMapping::ResponsesNative {
            report.record(
                "$.tool_choice",
                None,
                TranslationDecisionKind::Exact,
                Some("preserved only in native Responses provider transport"),
                TranslationSafeRepresentation::Redacted,
            );
            return Ok(crate::application_public_api::native::NativeObject::from_map(inputs));
        }
        let normalized = match choice {
            Value::String(choice) if matches!(choice.as_str(), "auto" | "none" | "required") => {
                json!({ "type": choice })
            }
            Value::Object(choice) if tool_mapping == OpenAiToolMapping::ChatCompletions => {
                let name = choice
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        OpenAiCompatError::invalid("tool_choice", "tool_choice name is required")
                            .with_report(report.clone())
                    })?;
                json!({ "type": "tool", "name": name })
            }
            Value::Object(choice) => {
                let name = choice.get("name").and_then(Value::as_str).ok_or_else(|| {
                    OpenAiCompatError::invalid("tool_choice", "tool_choice name is required")
                        .with_report(report.clone())
                })?;
                if tool_mapping == OpenAiToolMapping::ResponsesSemantic
                    && !inputs
                        .get("tools")
                        .and_then(Value::as_array)
                        .is_some_and(|tools| {
                            tools
                                .iter()
                                .any(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
                        })
                {
                    return Err(OpenAiCompatError::invalid(
                        "tool_choice",
                        "tool_choice must select a declared function tool",
                    )
                    .with_report(report.clone()));
                }
                json!({ "type": "tool", "name": name })
            }
            _ => {
                return Err(
                    OpenAiCompatError::invalid("tool_choice", "unsupported tool_choice")
                        .with_report(report.clone()),
                );
            }
        };
        report.record(
            "$.tool_choice",
            Some("$.inputs.tool_choice"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        inputs.insert("tool_choice".to_string(), normalized);
    }
    Ok(crate::application_public_api::native::NativeObject::from_map(inputs))
}

pub(super) fn openai_reasoning(
    object: &Map<String, Value>,
    chat_completions: bool,
    report: &mut TranslationReport,
) -> Result<
    Option<crate::application_public_api::native::NativeReasoningParameters>,
    OpenAiCompatError,
> {
    let (path, effort) = if chat_completions {
        ("$.reasoning_effort", object.get("reasoning_effort"))
    } else {
        let reasoning = match object.get("reasoning") {
            Some(Value::Object(reasoning)) => Some(reasoning),
            Some(Value::Null) => {
                report.record(
                    "$.reasoning",
                    None,
                    TranslationDecisionKind::Dropped,
                    Some("null reasoning is equivalent to an absent optional parameter"),
                    TranslationSafeRepresentation::Absent,
                );
                None
            }
            Some(_) => {
                return Err(
                    OpenAiCompatError::invalid("reasoning", "reasoning must be an object")
                        .with_report(report.clone()),
                );
            }
            None => None,
        };
        (
            "$.reasoning.effort",
            reasoning.and_then(|value| value.get("effort")),
        )
    };
    let Some(effort) = effort else {
        return Ok(None);
    };
    let effort = effort
        .as_str()
        .filter(|value| matches!(*value, "minimal" | "low" | "medium" | "high" | "xhigh"))
        .ok_or_else(|| {
            OpenAiCompatError::invalid("reasoning", "unsupported reasoning effort")
                .with_report(report.clone())
        })?;
    report.record(
        path,
        Some("$.execution.model_parameters.reasoning.effort"),
        TranslationDecisionKind::Exact,
        None,
        TranslationSafeRepresentation::Present,
    );
    Ok(Some(
        crate::application_public_api::native::NativeReasoningParameters::with_mode_budget_and_effort(
            crate::application_public_api::native::NativeReasoningMode::Enabled,
            None,
            Some(effort),
        ),
    ))
}
