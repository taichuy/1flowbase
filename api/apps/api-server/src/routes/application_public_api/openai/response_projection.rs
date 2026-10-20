use super::*;

pub(super) fn to_openai_response(
    run: NativeRunResult,
    model: String,
    completion_id: String,
) -> Result<OpenAiChatCompletionResponse, OpenAiRouteError> {
    let callback_task_id = callback_task_id_from_required_action(&run);
    let tool_calls = openai_tool_calls(run.tool_calls.as_ref(), callback_task_id);
    let finish_reason = match run.status {
        NativeRunStatus::Succeeded => "stop",
        NativeRunStatus::Incomplete => "length",
        NativeRunStatus::Waiting if tool_calls.is_some() => "tool_calls",
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into());
        }
    };
    let finish_reason = if tool_calls.is_some() {
        "tool_calls"
    } else {
        finish_reason
    };
    Ok(OpenAiChatCompletionResponse {
        id: completion_id,
        object: "chat.completion",
        created: run.created_at.unix_timestamp(),
        model,
        choices: vec![OpenAiChatCompletionChoice {
            index: 0,
            message: OpenAiChatMessage {
                role: "assistant",
                content: if tool_calls.is_some() {
                    run.answer
                } else {
                    Some(run.answer.unwrap_or_default())
                },
                tool_calls,
            },
            finish_reason,
        }],
        usage: Some(openai_usage(run.usage.as_ref())),
    })
}

pub(super) async fn collect_blocking_native_response(
    invocation: compatibility_interface::CompatibilityTypedStreamInvocation,
    model: String,
    previous_response_id: Option<String>,
    recorder: Option<&ClientTrajectoryRecorder>,
    projection_mode: compat_sse::ResponsesProjectionMode,
) -> Result<OpenAiResponsesObject, OpenAiRouteError> {
    let (mut events, completion) = invocation.into_parts();
    // The typed turn already applies the resume sequence boundary. Keep the
    // completion owner alive even if the blocking HTTP caller disconnects.
    let completion = tokio::spawn(completion.complete());
    let mut items = None;
    let mut selector = compat_sse::ResponsesProjectionSelector::new(projection_mode);
    // A blocking response is written once, after the whole turn is collected;
    // the tool deliveries are projected together with that single body.
    let mut deliveries = Vec::new();
    while let Some(event) = events.recv().await {
        let (run, envelope, delivery) = event.into_parts();
        if let Some(recorder) = recorder {
            recorder.bind_run(run.id, None);
            if envelope.source == control_plane::ports::RuntimeEventSource::Provider {
                if let Some(node_run_id) = envelope.node_run_id {
                    recorder.link_llm_node(run.id, node_run_id);
                }
            }
        }
        collect_selected_blocking_response_event(&mut items, &mut selector, run.id, &envelope);
        deliveries.extend(delivery);
    }
    let terminal = completion
        .await
        .map_err(|error| {
            native::NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_completion_failed",
                error.to_string(),
            )
        })?
        .map_err(|error| {
            native::NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_completion_failed",
                format!("{error:?}"),
            )
        })?;
    let (terminal, receipt) = terminal.into_parts();
    let run = match terminal {
        interface_runtime::InterfaceStreamTerminal::Completed(output) => output.0,
        interface_runtime::InterfaceStreamTerminal::Failed(failure) => {
            let error = &failure.error().0;
            return Err(native::NativeApiError::new(
                error.status,
                error.code,
                error.message.clone(),
            )
            .into());
        }
        interface_runtime::InterfaceStreamTerminal::Rejected { classification } => {
            return Err(native::NativeApiError::new(
                StatusCode::FORBIDDEN,
                "interface_rejected",
                classification.to_string(),
            )
            .into());
        }
        interface_runtime::InterfaceStreamTerminal::Cancelled => {
            return Err(native::NativeApiError::new(
                StatusCode::CONFLICT,
                "run_cancelled",
                "published run cancelled",
            )
            .into());
        }
    };
    if let Some(recorder) = recorder {
        recorder.bind_run(run.id, None);
    }
    if selector.selection_missing()
        && !matches!(
            run.status,
            NativeRunStatus::Failed | NativeRunStatus::Cancelled
        )
    {
        return Err(native::NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "llm_input_selection_missing",
            "host input selection is unavailable",
        )
        .into());
    }
    let response =
        to_openai_responses_response_with_native_items(run, model, previous_response_id, items)?;
    let _receipt = receipt.projected();
    for delivery in deliveries {
        delivery.projected();
    }
    Ok(response)
}

pub(super) fn collect_selected_blocking_response_event(
    items: &mut Option<Vec<Value>>,
    selector: &mut compat_sse::ResponsesProjectionSelector,
    run_id: Uuid,
    event: &control_plane::ports::RuntimeEventEnvelope,
) {
    selector.observe(run_id, event);
    if selector.native_event(event) {
        items.get_or_insert_with(Vec::new);
        collect_blocking_response_output_item(items, &event.event_type, &event.payload);
    }
}

pub(super) fn collect_blocking_response_output_item(
    items: &mut Option<Vec<Value>>,
    event_type: &str,
    payload: &Value,
) {
    if !matches!(
        event_type,
        "provider_output_item_added" | "provider_output_item_done"
    ) {
        return;
    }
    // None retains canonical projection. Some(empty) records native item mode
    // even when no completed formal item is available, preventing synthesis.
    let items = items.get_or_insert_with(Vec::new);
    if event_type == "provider_output_item_done" {
        if let Some(item) = payload.get("item") {
            items.push(item.clone());
        }
    }
}

pub(in super::super) fn native_response_id(run: &NativeRunResult) -> String {
    run.metadata
        .get("response_round_id")
        .and_then(Value::as_str)
        .map(|round| format!("resp_{round}"))
        .unwrap_or_else(|| response_id_from_run_id(run.id))
}

#[cfg(test)]
pub(in super::super) fn to_openai_responses_response(
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
) -> Result<OpenAiResponsesObject, OpenAiRouteError> {
    to_openai_responses_response_with_native_items(run, model, previous_response_id, None)
}

pub(super) fn to_openai_responses_response_with_native_items(
    run: NativeRunResult,
    model: String,
    previous_response_id: Option<String>,
    native_items: Option<Vec<Value>>,
) -> Result<OpenAiResponsesObject, OpenAiRouteError> {
    let native_items = native_items.or_else(|| {
        run.metadata
            .pointer("/responses_round/output")
            .and_then(Value::as_array)
            .cloned()
    });
    let function_call_items = if native_items.is_none() {
        openai_response_function_call_items(run.tool_calls.as_ref())
    } else {
        None
    };
    let has_tool_calls = function_call_items.is_some()
        || native_items.as_ref().is_some_and(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("function_call" | "custom_tool_call")
                )
            })
        });
    let (status, incomplete_details) = match run.status {
        NativeRunStatus::Succeeded => ("completed", None),
        NativeRunStatus::Incomplete => (
            "incomplete",
            Some(OpenAiResponsesIncompleteDetails {
                reason: "max_output_tokens",
            }),
        ),
        NativeRunStatus::Waiting if has_tool_calls => ("completed", None),
        NativeRunStatus::Waiting => return Err(OpenAiRouteError::RequiredAction),
        NativeRunStatus::Created
        | NativeRunStatus::Queued
        | NativeRunStatus::Running
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled => {
            return Err(native::blocking_run_projection_error(&run).into());
        }
    };
    let output_text = if let Some(items) = &native_items {
        items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flatten()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<String>()
    } else if function_call_items.is_some() {
        String::new()
    } else {
        run.answer.clone().unwrap_or_default()
    };
    let output = native_items
        .or(function_call_items)
        .unwrap_or_else(|| vec![openai_response_message_item(&run, &output_text, status)]);
    Ok(OpenAiResponsesObject {
        id: native_response_id(&run),
        object: "response",
        created_at: run.created_at.unix_timestamp(),
        status,
        model,
        output,
        output_text,
        usage: openai_responses_usage(run.usage.as_ref()),
        incomplete_details,
        previous_response_id,
    })
}

pub(super) fn openai_response_message_item(
    run: &NativeRunResult,
    output_text: &str,
    status: &'static str,
) -> Value {
    json!({
        "id": format!("msg_{}", run.id),
        "type": "message",
        "status": status,
        "role": "assistant",
        "content": [
            {
                "type": "output_text",
                "text": output_text,
                "annotations": []
            }
        ]
    })
}

pub(super) fn openai_response_function_call_items(
    tool_calls: Option<&Value>,
) -> Option<Vec<Value>> {
    let calls = external_llm_tool_calls(tool_calls)?;
    let mapped =
        control_plane::application_public_api::compat::openai::projection::function_call_items(
            calls,
        );
    (!mapped.is_empty()).then_some(mapped)
}

pub(super) fn openai_tool_calls(
    tool_calls: Option<&Value>,
    callback_task_id: Option<Uuid>,
) -> Option<Vec<OpenAiToolCall>> {
    let calls = external_llm_tool_calls(tool_calls)?;
    let mapped = calls
        .iter()
        .filter_map(|call| {
            let name = call.get("name").and_then(Value::as_str)?;
            let original_id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("tool_call")
                .to_string();
            let id = callback_task_id
                .map(|callback_task_id| {
                    encode_openai_callback_tool_call_id(callback_task_id, &original_id)
                })
                .unwrap_or(original_id);
            let arguments = call.get("arguments").cloned().unwrap_or_else(|| json!({}));
            Some(OpenAiToolCall {
                id,
                call_type: "function",
                function: OpenAiToolCallFunction {
                    name: name.to_string(),
                    arguments: openai_arguments_string(arguments),
                },
            })
        })
        .collect::<Vec<_>>();
    (!mapped.is_empty()).then_some(mapped)
}

pub(super) fn openai_arguments_string(arguments: Value) -> String {
    match arguments {
        Value::String(value) => value,
        value => value.to_string(),
    }
}

pub(super) fn callback_task_id_from_required_action(run: &NativeRunResult) -> Option<Uuid> {
    run.required_action
        .as_ref()
        .and_then(|action| action.payload.get("callback_task_id"))
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

pub(in super::super) fn openai_usage(
    usage: Option<&control_plane::application_public_api::native::NativeUsage>,
) -> OpenAiUsage {
    let Some(usage) = usage else {
        return OpenAiUsage::default();
    };
    OpenAiUsage {
        prompt_tokens: usage.prompt_tokens.unwrap_or_default(),
        completion_tokens: usage.completion_tokens.unwrap_or_default(),
        total_tokens: usage.total_tokens.unwrap_or_default(),
        prompt_tokens_details: usage
            .cache_read_tokens
            .or(usage.input_cache_hit_tokens)
            .map(|cached_tokens| OpenAiPromptTokensDetails { cached_tokens }),
        completion_tokens_details: usage
            .reasoning_tokens
            .map(|reasoning_tokens| OpenAiCompletionTokensDetails { reasoning_tokens }),
    }
}

pub(in super::super) fn openai_responses_usage(
    usage: Option<&control_plane::application_public_api::native::NativeUsage>,
) -> OpenAiResponsesUsage {
    let Some(usage) = usage else {
        return OpenAiResponsesUsage::default();
    };
    OpenAiResponsesUsage {
        input_tokens: usage.prompt_tokens.unwrap_or_default(),
        input_tokens_details: Some(OpenAiResponsesInputTokensDetails {
            cached_tokens: usage
                .cache_read_tokens
                .or(usage.input_cache_hit_tokens)
                .unwrap_or_default(),
        }),
        output_tokens: usage.completion_tokens.unwrap_or_default(),
        output_tokens_details: Some(OpenAiResponsesOutputTokensDetails {
            reasoning_tokens: usage.reasoning_tokens.unwrap_or_default(),
        }),
        total_tokens: usage.total_tokens.unwrap_or_default(),
    }
}
