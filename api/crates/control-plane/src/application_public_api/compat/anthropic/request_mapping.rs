use super::*;

pub(super) fn anthropic_inputs(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<NativeObject, AnthropicCompatError> {
    let mut inputs = Map::new();
    if let Some(value) = object.get("tools") {
        let tools = value.as_array().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.tools",
                "tools must be an array",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let mut normalized = Vec::with_capacity(tools.len());
        for (index, tool) in tools.iter().enumerate() {
            normalized.push(normalize_anthropic_tool(tool, index, report)?);
        }
        report.record(
            "$.tools",
            Some("$.inputs.tools"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Redacted,
        );
        inputs.insert("tools".to_string(), Value::Array(normalized));
    }
    if let Some(value) = object.get("tool_choice") {
        let normalized = normalize_anthropic_tool_choice(value, report)?;
        report.record(
            "$.tool_choice",
            Some("$.inputs.tool_choice"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        inputs.insert("tool_choice".to_string(), normalized);
    }
    Ok(NativeObject::from_map(inputs))
}

pub(super) fn anthropic_reasoning(
    object: &Map<String, Value>,
    report: &mut TranslationReport,
) -> Result<
    Option<crate::application_public_api::native::NativeReasoningParameters>,
    AnthropicCompatError,
> {
    let mut mode = crate::application_public_api::native::NativeReasoningMode::Enabled;
    let mut budget_tokens = None;
    let mut has_reasoning = false;
    if let Some(value) = object.get("thinking") {
        let thinking = value.as_object().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.thinking",
                "thinking must be an object",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let unknown_fields = thinking
            .keys()
            .filter(|field| !matches!(field.as_str(), "type" | "budget_tokens" | "display"))
            .collect::<Vec<_>>();
        let unknown_field_names = unknown_fields
            .iter()
            .map(|field| field.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if report.record_anonymous_unknown_fields(
            "$.thinking",
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Anthropic thinking field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(AnthropicCompatError::invalid(format!(
                "unknown Anthropic thinking field: {unknown_field_names}"
            ))
            .with_report(report.clone()));
        }
        let thinking_type = thinking
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.thinking.type",
                    "thinking type must be text",
                    if thinking.contains_key("type") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                )
            })?;
        mode = match thinking_type {
            "adaptive" => crate::application_public_api::native::NativeReasoningMode::Adaptive,
            "enabled" => crate::application_public_api::native::NativeReasoningMode::Enabled,
            "disabled" => crate::application_public_api::native::NativeReasoningMode::Disabled,
            _ => {
                return Err(reject_anthropic_nested_field(
                    report,
                    "$.thinking.type",
                    "unknown Anthropic thinking type",
                    TranslationSafeRepresentation::Present,
                ));
            }
        };
        budget_tokens = thinking
            .get("budget_tokens")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(std::num::NonZeroU64::new)
                    .ok_or_else(|| {
                        reject_anthropic_nested_field(
                            report,
                            "$.thinking.budget_tokens",
                            "thinking budget_tokens must be a positive integer",
                            TranslationSafeRepresentation::Present,
                        )
                    })
            })
            .transpose()?;
        if let Some(display) = thinking.get("display") {
            if !display.is_string() {
                return Err(reject_anthropic_nested_field(
                    report,
                    "$.thinking.display",
                    "thinking display must be text",
                    TranslationSafeRepresentation::Present,
                ));
            }
            report.record(
                "$.thinking.display",
                None,
                TranslationDecisionKind::Dropped,
                Some("Native reasoning visibility follows runtime event semantics"),
                TranslationSafeRepresentation::Present,
            );
        }
        report.record(
            "$.thinking",
            Some("$.execution.model_parameters.reasoning"),
            TranslationDecisionKind::Normalized,
            None,
            TranslationSafeRepresentation::Present,
        );
        has_reasoning = true;
    }

    let mut effort = None;
    if let Some(value) = object.get("output_config") {
        let output_config = value.as_object().ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                "$.output_config",
                "output_config must be an object",
                TranslationSafeRepresentation::Present,
            )
        })?;
        let unknown_fields = output_config
            .keys()
            .filter(|field| !matches!(field.as_str(), "effort" | "format"))
            .collect::<Vec<_>>();
        if report.record_anonymous_unknown_fields(
            "$.output_config",
            unknown_fields,
            TranslationDecisionKind::Rejected,
            "unknown Anthropic output_config field",
            TranslationSafeRepresentation::Present,
        ) > 0
        {
            return Err(
                AnthropicCompatError::invalid("unknown Anthropic output_config field")
                    .with_report(report.clone()),
            );
        }
        if output_config.contains_key("format") {
            report.record(
                "$.output_config.format",
                None,
                TranslationDecisionKind::Unsupported,
                Some("structured output format has no current Native owner"),
                TranslationSafeRepresentation::Present,
            );
            return Err(
                AnthropicCompatError::unsupported("output_config").with_report(report.clone())
            );
        }
        if let Some(value) = output_config.get("effort") {
            let value = value.as_str().ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.output_config.effort",
                    "output_config effort must be text",
                    TranslationSafeRepresentation::Present,
                )
            })?;
            effort = Some(match value {
                "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => value.to_string(),
                _ => {
                    return Err(reject_anthropic_nested_field(
                        report,
                        "$.output_config.effort",
                        "unknown Anthropic output effort",
                        TranslationSafeRepresentation::Present,
                    ));
                }
            });
            report.record(
                "$.output_config.effort",
                Some("$.execution.model_parameters.reasoning.effort"),
                TranslationDecisionKind::Normalized,
                None,
                TranslationSafeRepresentation::Present,
            );
            has_reasoning = true;
        }
        report.record(
            "$.output_config",
            effort
                .as_ref()
                .map(|_| "$.execution.model_parameters.reasoning"),
            if effort.is_some() {
                TranslationDecisionKind::Normalized
            } else {
                TranslationDecisionKind::Dropped
            },
            effort
                .is_none()
                .then_some("empty output_config has no Native effect"),
            TranslationSafeRepresentation::Present,
        );
    }

    if !has_reasoning {
        return Ok(None);
    }
    Ok(Some(
        crate::application_public_api::native::NativeReasoningParameters::with_mode_budget_and_effort(
            mode,
            budget_tokens,
            effort.as_deref(),
        ),
    ))
}

pub(super) fn normalize_anthropic_tool(
    tool: &Value,
    index: usize,
    report: &mut TranslationReport,
) -> Result<Value, AnthropicCompatError> {
    let path = format!("$.tools[{index}]");
    let object = tool.as_object().ok_or_else(|| {
        reject_anthropic_nested_field(
            report,
            &path,
            "tool definitions must be objects",
            TranslationSafeRepresentation::Present,
        )
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "name" | "description" | "input_schema" | "cache_control"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        &path,
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic tool field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic tool field")
                .with_report(report.clone()),
        );
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                &format!("{path}.name"),
                "tool name must be non-empty text",
                if object.contains_key("name") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            )
        })?;
    let input_schema = object
        .get("input_schema")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| {
            reject_anthropic_nested_field(
                report,
                &format!("{path}.input_schema"),
                "tool input_schema must be an object",
                if object.contains_key("input_schema") {
                    TranslationSafeRepresentation::Present
                } else {
                    TranslationSafeRepresentation::Absent
                },
            )
        })?;
    let mut normalized = Map::new();
    normalized.insert("name".to_string(), Value::String(name.to_string()));
    normalized.insert(
        "source".to_string(),
        Value::String("anthropic_compatible".to_string()),
    );
    if let Some(description) = object.get("description").and_then(Value::as_str) {
        normalized.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    normalized.insert("input_schema".to_string(), input_schema);
    if object.contains_key("cache_control") {
        report.record(
            &format!("{path}.cache_control"),
            None,
            TranslationDecisionKind::Dropped,
            Some("tool cache hints do not affect Native tool semantics"),
            TranslationSafeRepresentation::Present,
        );
    }
    Ok(Value::Object(normalized))
}

pub(super) fn normalize_anthropic_tool_choice(
    value: &Value,
    report: &mut TranslationReport,
) -> Result<Value, AnthropicCompatError> {
    let object = value.as_object().ok_or_else(|| {
        reject_anthropic_nested_field(
            report,
            "$.tool_choice",
            "tool_choice must be an object",
            TranslationSafeRepresentation::Present,
        )
    })?;
    let unknown_fields = object
        .keys()
        .filter(|field| {
            !matches!(
                field.as_str(),
                "type" | "name" | "disable_parallel_tool_use"
            )
        })
        .collect::<Vec<_>>();
    if report.record_anonymous_unknown_fields(
        "$.tool_choice",
        unknown_fields,
        TranslationDecisionKind::Rejected,
        "unknown Anthropic tool_choice field",
        TranslationSafeRepresentation::Present,
    ) > 0
    {
        return Err(
            AnthropicCompatError::invalid("unknown Anthropic tool_choice field")
                .with_report(report.clone()),
        );
    }
    if object
        .get("disable_parallel_tool_use")
        .and_then(Value::as_bool)
        == Some(true)
    {
        report.record(
            "$.tool_choice.disable_parallel_tool_use",
            None,
            TranslationDecisionKind::Unsupported,
            Some("Native tool choice does not yet constrain parallel calls"),
            TranslationSafeRepresentation::Present,
        );
        return Err(AnthropicCompatError::unsupported("tool_choice").with_report(report.clone()));
    }
    if object.contains_key("disable_parallel_tool_use") {
        report.record(
            "$.tool_choice.disable_parallel_tool_use",
            None,
            TranslationDecisionKind::Dropped,
            Some("false preserves Native parallel tool defaults"),
            TranslationSafeRepresentation::Present,
        );
    }
    match object.get("type").and_then(Value::as_str) {
        Some("auto") => Ok(json!("auto")),
        Some("any") => Ok(json!("required")),
        Some("none") => Ok(json!("none")),
        Some("tool") => object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|name| json!({ "name": name }))
            .ok_or_else(|| {
                reject_anthropic_nested_field(
                    report,
                    "$.tool_choice.name",
                    "tool_choice name must be non-empty text",
                    if object.contains_key("name") {
                        TranslationSafeRepresentation::Present
                    } else {
                        TranslationSafeRepresentation::Absent
                    },
                )
            }),
        _ => Err(reject_anthropic_nested_field(
            report,
            "$.tool_choice.type",
            "unknown Anthropic tool_choice type",
            if object.contains_key("type") {
                TranslationSafeRepresentation::Present
            } else {
                TranslationSafeRepresentation::Absent
            },
        )),
    }
}
