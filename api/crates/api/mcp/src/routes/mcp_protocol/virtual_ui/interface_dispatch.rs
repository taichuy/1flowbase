use super::*;

pub(super) async fn call(request: McpCallRequest<'_>) -> Result<VirtualToolOutcome, ApiError> {
    call_inner(request, None).await
}

pub(in super::super) async fn call_inner(
    request: McpCallRequest<'_>,
    resource_read_uri: Option<&str>,
) -> Result<VirtualToolOutcome, ApiError> {
    let McpCallRequest {
        dependencies,
        interface_catalog,
        interface_dispatch,
        headers,
        actor,
        catalog,
        scope,
        arguments,
        assistant_client,
    } = request;
    let Some(tool_id) = string_argument(arguments, "tool_id") else {
        return Ok(VirtualToolOutcome::invalid("Invalid tool_id"));
    };
    let allow_assistant_client = match assistant_client {
        Some(client) => client.has_frontstage_capability_bundle().await,
        None => false,
    };
    let Some((instance, tool)) = visible_tool(catalog, scope, tool_id, allow_assistant_client)
    else {
        return Ok(VirtualToolOutcome::invalid("Tool not visible"));
    };
    let description = description_tool(catalog, scope, tool, allow_assistant_client);
    // Resource reads still pass the canonical description guard using current discovery.
    let mut read_arguments;
    let arguments = if resource_read_uri.is_some()
        && domain::mcp_management::mcp_des_id_required(&tool.input_mapping)
    {
        let Some(description) = description else {
            return Ok(VirtualToolOutcome::invalid("Description tool not visible"));
        };
        read_arguments = arguments.clone();
        read_arguments["des_id"] = json!(description.des_id);
        &read_arguments
    } else {
        arguments
    };
    if let Err(message) = validate_tool_call_controls(arguments, tool, description) {
        return Ok(VirtualToolOutcome::invalid(message));
    }
    let augmented;
    let arguments = match file_params::merge_call_file(tool, arguments) {
        Ok(Some(merged)) => {
            augmented = merged;
            &augmented
        }
        Ok(None) => arguments,
        Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
    };
    // A public locator retains only caller arguments, never hidden/default bindings.
    let replay_arguments = arguments
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolved_arguments = domain::mcp_management::input_defaults::resolve_call_defaults(
        arguments,
        &tool.input_mapping,
    );
    let arguments = &resolved_arguments;
    let inline_chars = match result_delivery::tool_inline_limit(arguments, tool) {
        Ok(limit) => limit,
        Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
    };
    let selection =
        match result_delivery::ResultSelection::parse(arguments, tool.response_fields.as_deref()) {
            Ok(selection) => selection,
            Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
        };
    let tool_arguments = arguments
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match &tool.execution_target {
        domain::McpToolExecutionTarget::AssistantClient { capability_code } => {
            let Some(client) = assistant_client else {
                return Ok(VirtualToolOutcome::invalid("Tool not visible"));
            };
            if !client.has_declared_capability(capability_code).await {
                return Ok(VirtualToolOutcome::invalid("Tool not visible"));
            }
            match client
                .invoke_capability(capability_code, tool_arguments)
                .await
            {
                Ok((value, is_error)) if is_error => Ok(VirtualToolOutcome::failed(
                    "Assistant client capability failed",
                    value,
                )),
                Ok((value, false)) => Ok(VirtualToolOutcome::Success(
                    result_delivery::deliver_result(
                        &dependencies.result_delivery,
                        actor,
                        result_delivery::CompletedOperation::Write {
                            operation_id: &tool.tool_id,
                        },
                        value,
                        inline_chars,
                        &selection,
                        tool,
                    )
                    .await,
                )),
                Err(error) => Ok(VirtualToolOutcome::failed(
                    "Assistant client unavailable",
                    json!({"reason": error.to_string(), "retry_original": false}),
                )),
                Ok((_, true)) => unreachable!(),
            }
        }
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let media_mapping = match result_delivery::media::mapping(&tool.output_mapping) {
                Ok(mapping) => mapping,
                Err(reason) => return Ok(media_mapping_error(reason)),
            };
            let interface = match interface_catalog.bindable_interface(interface_id) {
                Ok(interface) => interface,
                Err(error) => return Ok(interface_error(&error.0)),
            };
            let operation = if matches!(interface.method.as_str(), "GET" | "HEAD" | "OPTIONS") {
                result_delivery::CompletedOperation::Read {
                    operation_id: interface_id,
                }
            } else {
                result_delivery::CompletedOperation::Write {
                    operation_id: interface_id,
                }
            };
            let body = McpDebugExecuteBody {
                interface_id: interface_id.clone(),
                debug_response_mode: McpDebugResponseMode::ToolResult,
                mcp_arguments: tool_arguments,
                input_mapping: tool.input_mapping.clone(),
                output_mapping: tool.output_mapping.clone(),
            };
            match crate::resource_import::with_selected_scope(
                scope.instance_ids().to_vec(),
                interface_dispatch.execute(
                    headers.clone(),
                    interface,
                    body,
                    McpServerBoundInputs {
                        workspace_id: instance.workspace_id,
                    },
                ),
            )
            .await
            {
                Ok(value) => {
                    let (detail, media_content) = match media_mapping {
                        Some(mapping) => match result_delivery::media::project(&mapping, &value) {
                            Ok(mut result) => {
                                if mapping.resource_read && resource_read_uri.is_none() {
                                    if let Err(reason) = resources::wrap_content(
                                        &mut result.content,
                                        actor,
                                        instance,
                                        tool,
                                        &replay_arguments,
                                    ) {
                                        return Ok(media_mapping_error(reason));
                                    }
                                }
                                (
                                    result.structured_content.expect("media metadata"),
                                    result.content,
                                )
                            }
                            Err(reason) => return Ok(media_mapping_error(reason)),
                        },
                        None => (value, json!([])),
                    };
                    let delivered = result_delivery::deliver_result(
                        &dependencies.result_delivery,
                        actor,
                        operation,
                        detail,
                        inline_chars,
                        &selection,
                        tool,
                    )
                    .await;
                    Ok(VirtualToolOutcome::Success(
                        result_delivery::media::retain_native_content(delivered, &media_content),
                    ))
                }
                Err(debug_execute::McpDebugExecuteError::Api(error)) => Ok(interface_error(&error)),
                Err(debug_execute::McpDebugExecuteError::TargetResponse(response)) => {
                    Ok(target_interface_failure(response).await)
                }
            }
        }
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => {
            let service = McpManagementService::new(dependencies.store.clone());
            let availability = service
                .upstream_proxy_availability_for_actor(
                    actor,
                    *upstream_connection_id,
                    remote_tool_name,
                )
                .await?;
            if availability != domain::McpToolAvailabilityStatus::Available {
                return Ok(VirtualToolOutcome::failed(
                    "Upstream tool unavailable",
                    json!({"availability_status": availability.as_str()}),
                ));
            }
            let connection = service
                .get_upstream_connection_for_actor(actor, *upstream_connection_id)
                .await?;
            let secret = service
                .upstream_secret_for_actor(
                    actor,
                    *upstream_connection_id,
                    &dependencies.provider_secret_master_key,
                )
                .await?;
            let remote_arguments = match map_proxy_arguments(&tool_arguments, &tool.input_mapping) {
                Ok(arguments) => arguments,
                Err(error) => {
                    return Ok(VirtualToolOutcome::Error {
                        code: -32602,
                        message: "Invalid tool arguments",
                        data: Some(json!({"reason": error.to_string()})),
                    })
                }
            };
            let client = match McpStreamableHttpClient::connect(&connection, secret.as_ref()).await
            {
                Ok(client) => client,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Upstream MCP connection failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let upstream = match client.call_tool(remote_tool_name, remote_arguments).await {
                Ok(result) => result,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Upstream MCP tools/call failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let mapped = match map_proxy_result(&upstream, &tool.output_mapping) {
                Ok(result) => result,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Tool result mapping failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let value = match serde_json::to_value(mapped) {
                Ok(value) => value,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Tool result serialization failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let has_media = value
                .get("content")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.get("type").and_then(Value::as_str) != Some("text"))
                });
            let detail = value.get("structuredContent").cloned().unwrap_or_else(|| {
                if has_media {
                    json!({"content": value.get("content").and_then(Value::as_array).map(|items|
                        items.iter().filter(|item| item.get("type").and_then(Value::as_str) == Some("text")).cloned().collect::<Vec<_>>()).unwrap_or_default()})
                } else { value.clone() }
            });
            if selection.is_default()
                && !result_delivery::exceeds_inline_limit(&detail, inline_chars)
            {
                Ok(VirtualToolOutcome::Success(value))
            } else {
                let delivered = result_delivery::deliver_result(
                    &dependencies.result_delivery,
                    actor,
                    result_delivery::CompletedOperation::Write {
                        operation_id: &tool.tool_id,
                    },
                    detail,
                    inline_chars,
                    &selection,
                    tool,
                )
                .await;
                let mut delivered =
                    result_delivery::media::retain_content(delivered, &value["content"]);
                if let Some(is_error) = value.get("isError") {
                    delivered["isError"] = is_error.clone();
                }
                Ok(VirtualToolOutcome::Success(delivered))
            }
        }
    }
}

pub(super) async fn target_interface_failure(response: Response) -> VirtualToolOutcome {
    let status = response.status();
    let payload = if matches!(
        status,
        StatusCode::BAD_REQUEST
            | StatusCode::CONFLICT
            | StatusCode::UNPROCESSABLE_ENTITY
            | StatusCode::UNAUTHORIZED
            | StatusCode::FORBIDDEN
    ) {
        to_bytes(response.into_body(), TARGET_INTERFACE_ERROR_BODY_LIMIT)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    } else {
        None
    };
    let target_code = payload
        .as_ref()
        .and_then(|payload| payload.get("code"))
        .and_then(Value::as_str)
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 128
                && code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
        .map(str::to_owned);
    let mut outcome = target_failure(status, target_code.clone());
    // `details` is an explicit public diagnostic contract owned by the target API.
    // Never forward message, arbitrary response fields, auth details or internal/server errors.
    if target_code.is_some() && matches!(status, StatusCode::BAD_REQUEST | StatusCode::CONFLICT) {
        if let Some(details) = payload
            .as_ref()
            .and_then(|payload| payload.get("details"))
            .filter(|details| details.is_object() && details.to_string().len() <= 4096)
        {
            if let VirtualToolOutcome::Error {
                data: Some(Value::Object(data)),
                ..
            } = &mut outcome
            {
                data.insert("target_details".to_owned(), details.clone());
            }
        }
    }
    outcome
}

pub(super) fn target_failure(
    status: StatusCode,
    target_code: Option<String>,
) -> VirtualToolOutcome {
    let category = match status {
        StatusCode::UNAUTHORIZED => "target_authentication",
        StatusCode::FORBIDDEN => "target_authorization",
        _ => "target_interface",
    };
    let mut data = serde_json::Map::from_iter([
        ("category".to_string(), json!(category)),
        ("http_status".to_string(), json!(status.as_u16())),
        ("outcome".to_string(), json!("failed")),
        ("retry_original".to_string(), json!(false)),
    ]);
    if let Some(code) = target_code {
        data.insert("target_code".to_string(), json!(code));
    }
    VirtualToolOutcome::failed("Tool execution failed", Value::Object(data))
}
