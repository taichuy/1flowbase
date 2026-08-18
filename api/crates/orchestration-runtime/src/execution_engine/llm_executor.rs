use super::visible_internal_llm_tools::payloads::{output_tool_calls, tool_call_id};
use super::*;

// The execution boundary keeps plan, node, variable, runtime, invoker, and lifecycle ownership explicit.
#[allow(clippy::too_many_arguments)]
pub async fn execute_llm_node<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &mut Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let registrations = runtime_context.runtime_internal_tool_registrations(node);
    if registrations.is_empty() {
        return execute_llm_node_with_visible_internal_tools(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }
    let internal_invoker = runtime_context
        .runtime_internal_tool_invoker
        .as_ref()
        .ok_or_else(|| anyhow!("runtime internal tool registrations have no invoker"))?;
    let mut llm_variable_pool = variable_pool.clone();
    let mut internal_events = Vec::new();
    let mut round_metrics = LlmRoundMetricsAccumulator::default();

    loop {
        let mut execution = execute_llm_node_with_visible_internal_tools(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            &mut llm_variable_pool,
            runtime_context,
            invoker,
        )
        .await?;
        round_metrics.absorb(&execution);
        if execution.error_payload.is_some() {
            round_metrics.apply(&mut execution);
            attach_runtime_internal_tool_events(&mut execution, &internal_events);
            return Ok(execution);
        }
        let Some(tool_calls) = output_tool_calls(&execution.output_payload) else {
            round_metrics.apply(&mut execution);
            attach_runtime_internal_tool_events(&mut execution, &internal_events);
            return Ok(execution);
        };
        let internal_calls = tool_calls
            .iter()
            .filter_map(|call| {
                let name = call
                    .get("name")
                    .or_else(|| {
                        call.get("function")
                            .and_then(|function| function.get("name"))
                    })
                    .and_then(Value::as_str)?;
                registrations
                    .iter()
                    .find(|registration| registration.provider_name == name)
                    .map(|registration| (call.clone(), registration.clone()))
            })
            .collect::<Vec<_>>();
        if internal_calls.is_empty() {
            round_metrics.apply(&mut execution);
            attach_runtime_internal_tool_events(&mut execution, &internal_events);
            return Ok(execution);
        }
        let mut callback_wait = execution.pending_callback.take().ok_or_else(|| {
            anyhow!(
                "runtime internal tool call is missing callback checkpoint for {}",
                node.node_id
            )
        })?;
        let call_usage = execution
            .metrics_payload
            .get("usage")
            .cloned()
            .unwrap_or_else(|| json!({}));
        llm_variable_pool = std::mem::take(&mut callback_wait.checkpoint_variable_pool);
        let internal_ids = internal_calls
            .iter()
            .map(|(call, _)| tool_call_id(call))
            .collect::<BTreeSet<_>>();
        let external_calls = tool_calls
            .iter()
            .filter(|call| !internal_ids.contains(&tool_call_id(call)))
            .cloned()
            .collect::<Vec<_>>();
        let mut results = Vec::with_capacity(internal_calls.len());
        for (call, registration) in &internal_calls {
            let call_id = tool_call_id(call);
            let arguments = call
                .get("arguments")
                .or_else(|| {
                    call.get("function")
                        .and_then(|function| function.get("arguments"))
                })
                .cloned()
                .unwrap_or_else(|| json!({}));
            let arguments = match arguments {
                Value::String(value) => serde_json::from_str(&value).unwrap_or_else(|_| json!({})),
                value => value,
            };
            let started_at = OffsetDateTime::now_utc();
            let timer = std::time::Instant::now();
            lifecycle
                .runtime_internal_tool_started(node, call, &call_usage)
                .await?;
            let mut output = internal_invoker
                .invoke_runtime_internal_tool(node, registration, arguments)
                .await?;
            let finished_at = OffsetDateTime::now_utc();
            let duration_ms = u64::try_from(timer.elapsed().as_millis()).unwrap_or(u64::MAX);
            let is_error = output.is_error;
            enrich_runtime_internal_tool_event(
                &mut output.event,
                &call_id,
                registration,
                is_error,
                started_at,
                finished_at,
                duration_ms,
            );
            internal_events.push(output.event);
            let tool_result = json!({
                "tool_call_id": call_id,
                "name": registration.provider_name,
                "content": output.content,
                "is_error": is_error,
                "callback_status": "returned",
                "execution_status": if is_error { "failed" } else { "succeeded" },
                "execution_kind": "host_internal",
                "registration_id": registration.registration_id,
                "registration_owner": registration.owner,
                "started_at": started_at,
                "finished_at": finished_at,
                "duration_ms": duration_ms,
            });
            lifecycle
                .runtime_internal_tool_finished(node, call, &call_usage, &tool_result, duration_ms)
                .await?;
            results.push(tool_result);
        }
        apply_mixed_llm_tool_callback_results(
            &mut llm_variable_pool,
            &node.node_id,
            &results,
            &external_calls,
        )?;
        if !external_calls.is_empty() {
            refresh_llm_tool_callback_wait_from_checkpoint(&mut callback_wait, llm_variable_pool)?;
            if let Some(output) = execution.output_payload.as_object_mut() {
                output.insert("tool_calls".to_string(), Value::Array(external_calls));
            }
            execution.pending_callback = Some(callback_wait);
            round_metrics.apply(&mut execution);
            attach_runtime_internal_tool_events(&mut execution, &internal_events);
            return Ok(execution);
        }
    }
}

fn attach_runtime_internal_tool_events(execution: &mut LlmNodeExecution, events: &[Value]) {
    if events.is_empty() {
        return;
    }
    let debug = execution
        .debug_payload
        .as_object_mut()
        .expect("LLM debug payload is canonical object");
    debug.insert(
        "runtime_internal_tool_events".to_string(),
        Value::Array(events.to_vec()),
    );
    if let Some(metrics) = execution.metrics_payload.as_object_mut() {
        metrics.insert("internal_tool_call_count".to_string(), json!(events.len()));
    }
}

fn enrich_runtime_internal_tool_event(
    event: &mut Value,
    tool_call_id: &str,
    registration: &RuntimeInternalToolRegistration,
    is_error: bool,
    started_at: OffsetDateTime,
    finished_at: OffsetDateTime,
    duration_ms: u64,
) {
    let Some(event) = event.as_object_mut() else {
        return;
    };
    event.insert("tool_call_id".to_string(), json!(tool_call_id));
    event.insert(
        "registration_id".to_string(),
        json!(registration.registration_id),
    );
    event.insert(
        "provider_name".to_string(),
        json!(registration.provider_name),
    );
    event.insert("owner".to_string(), registration.owner.clone());
    event.insert("execution_kind".to_string(), json!("host_internal"));
    event.insert("callback_status".to_string(), json!("returned"));
    event.insert(
        "execution_status".to_string(),
        json!(if is_error { "failed" } else { "succeeded" }),
    );
    event.insert("is_error".to_string(), json!(is_error));
    event.insert("started_at".to_string(), json!(started_at));
    event.insert("finished_at".to_string(), json!(finished_at));
    event.insert("duration_ms".to_string(), json!(duration_ms));
}

pub(crate) async fn execute_llm_node_provider_round<I>(
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
        let attempt_runtimes =
            llm_request_runtimes(node, runtime, runtime_context, invoker).await?;
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
        let attempt_runtimes =
            llm_request_runtimes(node, runtime, runtime_context, invoker).await?;
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
    let routing_probe = match build_provider_invocation(
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
    let required_capabilities = routing_probe.input.required_capabilities.clone();
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
    let mut retry_feedback: Option<String> = None;

    for attempt_index in 0..request_count {
        let resolved_attempt = match resolve_llm_request_runtime(
            runtime,
            runtime_context,
            invoker,
            &required_capabilities,
            Some(&routing_probe.input),
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
        let generate_projection_receipt = resolved_attempt.generate_projection_receipt.clone();
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
                    plugin_id: None,
                    reasoning_effort: None,
                    status: "failed",
                    failed_after_first_token: false,
                    error_payload: Some(&error_payload),
                    generate_projection_receipt: generate_projection_receipt.as_ref(),
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
        let runtime_plugin_id = resolved_route.runtime_plugin_id().map(str::to_string);
        // Candidate preflight may produce a lossy provider-bound projection. Invocation always
        // starts again from the canonical node inputs so the routing copy can never reach a Provider.
        let mut invocation = match build_provider_invocation(
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
        };
        if let Some(feedback) = retry_feedback.as_ref() {
            invocation.input.messages.push(ProviderMessage {
                role: ProviderMessageRole::User,
                content: feedback.clone(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            });
        }
        let reasoning_effort = invocation
            .input
            .model_parameters
            .get("reasoning_effort")
            .and_then(Value::as_str)
            .or_else(|| {
                invocation
                    .input
                    .model_parameters
                    .get("reasoning")
                    .and_then(|value| value.get("effort"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string);
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
                plugin_id: runtime_plugin_id.as_deref(),
                reasoning_effort: reasoning_effort.as_deref(),
                status: "failed",
                failed_after_first_token: false,
                error_payload: Some(&error_payload),
                generate_projection_receipt: generate_projection_receipt.as_ref(),
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
        invocation.input.trace_context.insert(
            "provider_attempt_index".to_string(),
            attempt_index.to_string(),
        );
        invocation.input.trace_context.insert(
            "provider_invocation_id".to_string(),
            uuid::Uuid::now_v7().to_string(),
        );
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
                    plugin_id: runtime_plugin_id.as_deref(),
                    reasoning_effort: reasoning_effort.as_deref(),
                    status: "failed",
                    failed_after_first_token: false,
                    error_payload: Some(&error_payload),
                    generate_projection_receipt: generate_projection_receipt.as_ref(),
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
        let provider_observability = take_provider_observability_metadata(&mut output.result);

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
        let output_protocol_failure =
            first_provider_output_protocol_failure(&output.events).cloned();
        let invalid_tool_call_error =
            if stream_provider_error.is_none() && output_protocol_failure.is_none() {
                invalid_tool_call_finish_error(finish_reason.as_ref(), &output.result)
            } else {
                None
            };
        let invalid_finish_reason_error = if stream_provider_error.is_none()
            && output_protocol_failure.is_none()
            && invalid_tool_call_error.is_none()
        {
            invalid_finish_reason_error(finish_reason.as_ref(), &output.result)
        } else {
            None
        };
        let retryable_invalid_finish_reason = invalid_finish_reason_error.is_some();
        let reasoning_only_output_error = if stream_provider_error.is_none()
            && output_protocol_failure.is_none()
            && invalid_tool_call_error.is_none()
            && invalid_finish_reason_error.is_none()
        {
            reasoning_only_provider_output_error(
                final_content.as_deref(),
                &output.result,
                native_responses_passthrough,
            )
        } else {
            None
        };
        let retryable_reasoning_only_output = reasoning_only_output_error.is_some();
        let terminal_finish_error = (stream_provider_error.is_none()
            && output_protocol_failure.is_none()
            && invalid_tool_call_error.is_none()
            && invalid_finish_reason_error.is_none()
            && reasoning_only_output_error.is_none()
            && matches!(finish_reason, Some(ProviderFinishReason::Error)))
        .then(|| {
            ProviderRuntimeError::normalize(
                "invoke",
                "provider invocation finished with error",
                None,
            )
        });
        let failure_projection = if output_protocol_failure.is_some() {
            LlmFailureProjection::NoNodeOutput
        } else if invalid_tool_call_error.is_some() {
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
            .or(invalid_finish_reason_error)
            .or(reasoning_only_output_error)
            .or(terminal_finish_error);
        let failed_after_first_token = (provider_error.is_some()
            || output_protocol_failure.is_some())
            && content_delta_seen_before_terminal_failure(&output.events, finish_reason.as_ref());
        let recoverable_error_message = output_protocol_failure
            .as_ref()
            .map(|failure| failure.message.clone())
            .or_else(|| {
                provider_error
                    .as_ref()
                    .map(recoverable_provider_error_message)
            });
        let mut error_payload = output_protocol_failure
            .as_ref()
            .map(|failure| build_output_protocol_failure_payload(attempt_runtime, failure))
            .or_else(|| {
                provider_error
                    .as_ref()
                    .map(|error| build_provider_error_payload(attempt_runtime, error))
            })
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
        let mut attempt = build_attempt_metric(AttemptMetricInput {
            attempt_index,
            retry_reason: retry_reason.as_deref(),
            runtime: attempt_runtime,
            plugin_id: runtime_plugin_id.as_deref(),
            reasoning_effort: reasoning_effort.as_deref(),
            status: attempt_status,
            failed_after_first_token,
            error_payload: error_payload.as_ref(),
            generate_projection_receipt: generate_projection_receipt.as_ref(),
            usage: &usage,
            event_count: output.events.len(),
            started_at: attempt_started_at,
            first_token_at: output.first_token_at,
            finished_at: attempt_finished_at,
            time_to_first_token_ms: output.time_to_first_token_ms,
        });
        attach_provider_stream_timing(&mut attempt, provider_observability.stream_timing.as_ref());
        attach_provider_billing(&mut attempt, provider_observability.billing.as_ref());
        attempt_metrics.push(attempt.clone());

        if let Some(error_payload) = &error_payload {
            failed_attempts.push(attempt);
            if retry_enabled
                && (output_protocol_failure.is_some()
                    || retryable_invalid_finish_reason
                    || retryable_reasoning_only_output
                    || !failed_after_first_token)
                && provider_error
                    .as_ref()
                    .is_none_or(provider_error_allows_retry)
                && attempt_index + 1 < request_count
            {
                retry_reason = error_payload
                    .get("error_code")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(failure) = output_protocol_failure.as_ref() {
                    retry_feedback = Some(failure.retry_feedback.clone());
                }
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
