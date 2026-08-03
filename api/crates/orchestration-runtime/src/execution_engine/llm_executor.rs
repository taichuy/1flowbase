use super::*;

pub async fn execute_llm_node<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &mut Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    execute_llm_node_with_visible_internal_tools(
        plan,
        node,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
}

pub(super) async fn execute_llm_node_provider_round<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + ?Sized,
{
    let runtime = node.llm_runtime.as_ref().ok_or_else(|| {
        anyhow!(
            "compiled llm node is missing runtime metadata: {}",
            node.node_id
        )
    })?;
    if runtime_context.operation() == domain::AiNativeOperation::CountTokens {
        let attempt_runtimes = llm_request_runtimes(node, runtime, runtime_context).await?;
        let selected_runtime = attempt_runtimes
            .first()
            .ok_or_else(|| anyhow!("CountTokens LLM consumer has no selected runtime"))?;
        return execute_count_tokens_consumer(
            plan,
            node,
            selected_runtime,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }
    if matches!(
        runtime_context.operation(),
        domain::AiNativeOperation::Compact(_)
    ) {
        let attempt_runtimes = llm_request_runtimes(node, runtime, runtime_context).await?;
        let selected_runtime = attempt_runtimes
            .first()
            .ok_or_else(|| anyhow!("Compact LLM consumer has no selected runtime"))?;
        return execute_compact_consumer(
            plan,
            node,
            selected_runtime,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }
    let mut routing_probe = match build_provider_invocation(
        plan,
        node,
        runtime,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
    {
        Ok(invocation) => Some(invocation),
        Err(error_payload) => {
            return build_failed_llm_execution(
                node,
                runtime,
                error_payload,
                LlmFailureProjection::NoNodeOutput,
                None,
                build_llm_metrics_payload(
                    runtime,
                    ProviderUsage::default(),
                    Some(ProviderFinishReason::Error),
                    0,
                    Vec::new(),
                    None,
                    None,
                ),
                Vec::new(),
                LlmDebugInvocation {
                    messages: &[],
                    context: None,
                },
            );
        }
    };
    let required_capabilities = routing_probe
        .as_ref()
        .map(|invocation| invocation.input.required_capabilities.clone())
        .unwrap_or_default();
    let request_count = llm_request_count(node);
    let retry_enabled = node
        .config
        .get("retry_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let retry_interval_ms = node
        .config
        .get("retry_interval_ms")
        .and_then(Value::as_u64)
        .unwrap_or(500);
    let mut attempt_metrics = Vec::new();
    let mut failed_attempts = Vec::new();
    let mut retry_reason: Option<String> = None;

    for attempt_index in 0..request_count {
        let resolved_attempt = match resolve_llm_request_runtime(
            runtime,
            runtime_context,
            invoker,
            &required_capabilities,
            attempt_index,
        )
        .await
        {
            Ok(attempt) => attempt,
            Err(error) => {
                let provider_error = provider_runtime_error_from_anyhow(&error);
                let error_payload = build_provider_error_payload(runtime, &provider_error);
                return build_failed_llm_execution(
                    node,
                    runtime,
                    error_payload,
                    LlmFailureProjection::NoNodeOutput,
                    None,
                    build_llm_metrics_payload(
                        runtime,
                        ProviderUsage::default(),
                        Some(ProviderFinishReason::Error),
                        0,
                        attempt_metrics,
                        None,
                        None,
                    ),
                    Vec::new(),
                    LlmDebugInvocation {
                        messages: &[],
                        context: None,
                    },
                );
            }
        };
        let attempt_runtime = &resolved_attempt.runtime;
        let resolved_route = match resolved_attempt.route {
            Ok(route) => route,
            Err(error) => {
                let attempt_started_at = OffsetDateTime::now_utc();
                let provider_error = provider_runtime_error_from_anyhow(&error);
                let mut error_payload =
                    build_provider_error_payload(attempt_runtime, &provider_error);
                error_payload["failed_after_first_token"] = Value::Bool(false);
                let attempt = build_attempt_metric(AttemptMetricInput {
                    attempt_index,
                    retry_reason: retry_reason.as_deref(),
                    runtime: attempt_runtime,
                    status: "failed",
                    failed_after_first_token: false,
                    error_payload: Some(&error_payload),
                    usage: &ProviderUsage::default(),
                    event_count: 0,
                    started_at: attempt_started_at,
                    first_token_at: None,
                    finished_at: OffsetDateTime::now_utc(),
                    time_to_first_token_ms: None,
                });
                attempt_metrics.push(attempt.clone());
                failed_attempts.push(attempt);
                if retry_enabled
                    && provider_error_allows_retry(&provider_error)
                    && attempt_index + 1 < request_count
                {
                    retry_reason = error_payload
                        .get("error_code")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    if retry_interval_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_interval_ms))
                            .await;
                    }
                    continue;
                }
                return build_failed_llm_execution(
                    node,
                    attempt_runtime,
                    error_payload,
                    LlmFailureProjection::NoNodeOutput,
                    Some(recoverable_provider_error_message(&provider_error)),
                    build_llm_metrics_payload(
                        attempt_runtime,
                        ProviderUsage::default(),
                        Some(ProviderFinishReason::Error),
                        0,
                        attempt_metrics,
                        None,
                        None,
                    ),
                    Vec::new(),
                    LlmDebugInvocation {
                        messages: &[],
                        context: None,
                    },
                );
            }
        };
        let route_matches_probe = routing_probe.is_some()
            && attempt_runtime.provider_instance_id == runtime.provider_instance_id
            && attempt_runtime.provider_code == runtime.provider_code
            && attempt_runtime.protocol == runtime.protocol
            && attempt_runtime.model == runtime.model;
        let mut invocation = if route_matches_probe {
            routing_probe
                .take()
                .expect("the routing probe is consumed by at most one matching route")
        } else {
            match build_provider_invocation(
                plan,
                node,
                attempt_runtime,
                resolved_inputs,
                rendered_templates,
                variable_pool,
                runtime_context,
                invoker,
            )
            .await
            {
                Ok(invocation) => invocation,
                Err(error_payload) => {
                    return build_failed_llm_execution(
                        node,
                        attempt_runtime,
                        error_payload,
                        LlmFailureProjection::NoNodeOutput,
                        None,
                        build_llm_metrics_payload(
                            attempt_runtime,
                            ProviderUsage::default(),
                            Some(ProviderFinishReason::Error),
                            0,
                            attempt_metrics,
                            None,
                            None,
                        ),
                        Vec::new(),
                        LlmDebugInvocation {
                            messages: &[],
                            context: None,
                        },
                    );
                }
            }
        };
        inject_visible_internal_llm_tool_media_content_blocks(&mut invocation.input, variable_pool)
            .await;
        let invocation_messages = build_llm_debug_invocation_messages(
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            &invocation.input,
        );
        if invocation.input.messages.is_empty()
            && !invocation.input.required_capabilities.contains(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
            )
        {
            let attempt_finished_at = OffsetDateTime::now_utc();
            let error_payload = build_empty_prompt_messages_error_payload(attempt_runtime);
            let attempt = build_attempt_metric(AttemptMetricInput {
                attempt_index,
                retry_reason: retry_reason.as_deref(),
                runtime: attempt_runtime,
                status: "failed",
                failed_after_first_token: false,
                error_payload: Some(&error_payload),
                usage: &ProviderUsage::default(),
                event_count: 0,
                started_at: attempt_finished_at,
                first_token_at: None,
                finished_at: attempt_finished_at,
                time_to_first_token_ms: None,
            });
            attempt_metrics.push(attempt);

            return build_failed_llm_execution(
                node,
                attempt_runtime,
                error_payload,
                LlmFailureProjection::NoNodeOutput,
                None,
                build_llm_metrics_payload(
                    attempt_runtime,
                    ProviderUsage::default(),
                    Some(ProviderFinishReason::Error),
                    0,
                    attempt_metrics,
                    None,
                    None,
                ),
                Vec::new(),
                LlmDebugInvocation {
                    messages: &invocation_messages,
                    context: Some(&invocation.debug_context),
                },
            );
        }
        let tool_prompt_transcript =
            llm_tool_prompt_transcript(node, variable_pool, &invocation.input);
        let invocation_tools = invocation.input.tools.clone();
        let native_responses_passthrough = invocation.input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
        );
        let attempt_started_at = OffsetDateTime::now_utc();
        let mut output = match invoker
            .invoke_resolved_llm(attempt_runtime, resolved_route, invocation.input)
            .await
        {
            Ok(output) => output,
            Err(error) => {
                let attempt_finished_at = OffsetDateTime::now_utc();
                let provider_error = provider_runtime_error_from_anyhow(&error);
                let mut error_payload =
                    build_provider_error_payload(attempt_runtime, &provider_error);
                error_payload["failed_after_first_token"] = Value::Bool(false);
                let recoverable_error_message = recoverable_provider_error_message(&provider_error);
                let attempt = build_attempt_metric(AttemptMetricInput {
                    attempt_index,
                    retry_reason: retry_reason.as_deref(),
                    runtime: attempt_runtime,
                    status: "failed",
                    failed_after_first_token: false,
                    error_payload: Some(&error_payload),
                    usage: &ProviderUsage::default(),
                    event_count: 0,
                    started_at: attempt_started_at,
                    first_token_at: None,
                    finished_at: attempt_finished_at,
                    time_to_first_token_ms: None,
                });
                attempt_metrics.push(attempt.clone());
                failed_attempts.push(attempt);
                if retry_enabled
                    && provider_error_allows_retry(&provider_error)
                    && attempt_index + 1 < request_count
                {
                    retry_reason = error_payload
                        .get("error_code")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    if retry_interval_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_interval_ms))
                            .await;
                    }
                    continue;
                }

                return build_failed_llm_execution(
                    node,
                    attempt_runtime,
                    error_payload,
                    LlmFailureProjection::NoNodeOutput,
                    Some(recoverable_error_message),
                    build_llm_metrics_payload(
                        attempt_runtime,
                        ProviderUsage::default(),
                        Some(ProviderFinishReason::Error),
                        0,
                        attempt_metrics,
                        None,
                        None,
                    ),
                    Vec::new(),
                    LlmDebugInvocation {
                        messages: &invocation_messages,
                        context: Some(&invocation.debug_context),
                    },
                );
            }
        };
        let attempt_finished_at = OffsetDateTime::now_utc();
        canonicalize_provider_output_tool_call_names(&mut output, &invocation_tools);

        let usage = collect_usage(&output.events, &output.result.usage);
        let finish_reason = output
            .result
            .finish_reason
            .clone()
            .or_else(|| finish_reason_from_events(&output.events));
        let final_content = resolve_final_llm_content(
            output.result.final_content.clone(),
            collect_dify_style_deltas(&output.events),
        );
        let stream_provider_error = first_provider_error(&output.events).cloned();
        let invalid_tool_call_error = if stream_provider_error.is_none() {
            invalid_tool_call_finish_error(finish_reason.as_ref(), &output.result)
        } else {
            None
        };
        let terminal_finish_error = (stream_provider_error.is_none()
            && invalid_tool_call_error.is_none()
            && matches!(finish_reason, Some(ProviderFinishReason::Error)))
        .then(|| {
            ProviderRuntimeError::normalize(
                "invoke",
                "provider invocation finished with error",
                None,
            )
        });
        let failure_projection = if invalid_tool_call_error.is_some() {
            LlmFailureProjection::LegacyTerminalFallback
        } else if terminal_finish_error.is_some()
            && content_delta_seen_before_terminal_failure(&output.events, finish_reason.as_ref())
        {
            LlmFailureProjection::FailedNodeOutput
        } else {
            LlmFailureProjection::NoNodeOutput
        };
        let provider_error = stream_provider_error
            .or(invalid_tool_call_error)
            .or(terminal_finish_error);
        let failed_after_first_token = provider_error.is_some()
            && content_delta_seen_before_terminal_failure(&output.events, finish_reason.as_ref());
        let recoverable_error_message = provider_error
            .as_ref()
            .map(recoverable_provider_error_message);
        let mut error_payload = provider_error
            .as_ref()
            .map(|error| build_provider_error_payload(attempt_runtime, error))
            .or_else(|| {
                (!has_valid_provider_output(
                    final_content.as_deref(),
                    &output.result,
                    native_responses_passthrough,
                ))
                .then(|| build_empty_provider_response_error_payload(attempt_runtime))
            });
        if failure_projection == LlmFailureProjection::LegacyTerminalFallback {
            if let (Some(error_payload), Some(message)) =
                (&mut error_payload, recoverable_error_message.as_deref())
            {
                // Invalid tool-call completion is generated by this runtime,
                // rather than copied from an upstream provider response.
                error_payload["message"] = Value::String(message.to_string());
            }
        }
        if let Some(error_payload) = &mut error_payload {
            error_payload["failed_after_first_token"] = Value::Bool(failed_after_first_token);
        }
        let attempt_status = match error_payload
            .as_ref()
            .and_then(|payload| payload.get("error_code"))
            .and_then(Value::as_str)
        {
            Some("empty_response") => "empty_response",
            Some(_) => "failed",
            None => "succeeded",
        };
        let attempt = build_attempt_metric(AttemptMetricInput {
            attempt_index,
            retry_reason: retry_reason.as_deref(),
            runtime: attempt_runtime,
            status: attempt_status,
            failed_after_first_token,
            error_payload: error_payload.as_ref(),
            usage: &usage,
            event_count: output.events.len(),
            started_at: attempt_started_at,
            first_token_at: output.first_token_at,
            finished_at: attempt_finished_at,
            time_to_first_token_ms: output.time_to_first_token_ms,
        });
        attempt_metrics.push(attempt.clone());

        if let Some(error_payload) = &error_payload {
            failed_attempts.push(attempt);
            if retry_enabled
                && !failed_after_first_token
                && provider_error
                    .as_ref()
                    .is_none_or(provider_error_allows_retry)
                && attempt_index + 1 < request_count
            {
                retry_reason = error_payload
                    .get("error_code")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if retry_interval_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(retry_interval_ms)).await;
                }
                continue;
            }
            return build_failed_llm_execution(
                node,
                attempt_runtime,
                error_payload.clone(),
                failure_projection,
                recoverable_error_message,
                build_llm_metrics_payload(
                    attempt_runtime,
                    usage,
                    finish_reason,
                    output.events.len(),
                    attempt_metrics,
                    output.first_token_at,
                    output.time_to_first_token_ms,
                ),
                output.events,
                LlmDebugInvocation {
                    messages: &invocation_messages,
                    context: Some(&invocation.debug_context),
                },
            );
        }

        let mut execution = build_successful_llm_execution(
            node,
            attempt_runtime,
            &output.result,
            final_content,
            native_responses_passthrough,
            build_llm_metrics_payload(
                attempt_runtime,
                usage,
                finish_reason.clone(),
                output.events.len(),
                attempt_metrics,
                output.first_token_at,
                output.time_to_first_token_ms,
            ),
            output.events,
            LlmDebugInvocation {
                messages: &invocation_messages,
                context: Some(&invocation.debug_context),
            },
        )?;
        execution.pending_callback = build_llm_tool_callback_wait(
            node,
            variable_pool,
            &execution.output_payload,
            &tool_prompt_transcript,
        )?;
        return Ok(execution);
    }

    let error_payload = json!({
        "error_code": "provider_unavailable",
        "message": "all llm node requests failed",
        "attempts": failed_attempts,
    });
    build_failed_llm_execution(
        node,
        runtime,
        error_payload,
        LlmFailureProjection::NoNodeOutput,
        None,
        build_llm_metrics_payload(
            runtime,
            ProviderUsage::default(),
            Some(ProviderFinishReason::Error),
            0,
            attempt_metrics,
            None,
            None,
        ),
        Vec::new(),
        LlmDebugInvocation {
            messages: &[],
            context: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
async fn execute_count_tokens_consumer<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + ?Sized,
{
    let invocation = match build_provider_invocation(
        plan,
        node,
        runtime,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
    {
        Ok(invocation) => invocation,
        Err(error_payload) => {
            return Ok(LlmNodeExecution {
                output_payload: json!({}),
                error_payload: Some(error_payload),
                metrics_payload: json!({ "operation": "count_tokens" }),
                debug_payload: json!({}),
                provider_events: Vec::new(),
                pending_callback: None,
                failure_projection: LlmFailureProjection::NoNodeOutput,
                recoverable_error_message: None,
            });
        }
    };
    let input = ProviderCountTokensInput::from_invocation(invocation.input);
    match invoker.count_tokens(runtime, input).await {
        Ok(result) => {
            let receipt = CountTokensReceipt::new(result)?;
            Ok(LlmNodeExecution {
                output_payload: receipt.as_payload()?,
                error_payload: None,
                metrics_payload: json!({
                    "operation": "count_tokens",
                    "provider_instance_id": runtime.provider_instance_id,
                    "provider_code": runtime.provider_code,
                    "model": runtime.model,
                }),
                debug_payload: json!({}),
                provider_events: Vec::new(),
                pending_callback: None,
                failure_projection: LlmFailureProjection::NoNodeOutput,
                recoverable_error_message: None,
            })
        }
        Err(error) => Ok(LlmNodeExecution {
            output_payload: json!({}),
            error_payload: Some(build_provider_error_payload(
                runtime,
                &provider_runtime_error_from_anyhow(&error),
            )),
            metrics_payload: json!({ "operation": "count_tokens", "error": true }),
            debug_payload: json!({}),
            provider_events: Vec::new(),
            pending_callback: None,
            failure_projection: LlmFailureProjection::NoNodeOutput,
            recoverable_error_message: None,
        }),
    }
}
