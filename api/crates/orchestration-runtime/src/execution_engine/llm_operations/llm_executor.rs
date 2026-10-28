use super::visible_internal_llm_tools::payloads::{output_tool_calls, tool_call_id};
use super::*;
use crate::execution_engine::ai_native_recovery::{
    decide_outer_replay_with_default_bucket, AiNativeRecoveryLedger, AiNativeRecoveryReceipt,
    OuterReplayDecision, ProviderRetryPartition, RecoveryInputMode,
};
use extension_contracts::provider_contract::PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY;

const AUTOMATIC_TRANSPORT_RETRY_LIMIT: usize = 1;
const AUTOMATIC_TRANSPORT_RETRY_BASE_MS: u64 = 200;
const AUTOMATIC_TRANSPORT_RETRY_CAP_MS: u64 = 1_000;

fn unix_millis(now: OffsetDateTime) -> i64 {
    i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}

fn recovery_absolute_deadline(input: &ProviderInvocationInput, now: OffsetDateTime) -> i64 {
    input
        .run_context
        .get("task_deadline_unix_ms")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| unix_millis(now + time::Duration::minutes(30)))
}

fn fallback_retry_partition(runtime: &CompiledLlmRuntime) -> ProviderRetryPartition {
    ProviderRetryPartition::new(
        runtime.provider_instance_id.clone(),
        format!("{}:{}", runtime.provider_code, runtime.protocol),
        "host-unresolved-credential-owner",
    )
    .expect("compiled Provider identities must fit retry partition bounds")
}

fn attach_ai_native_recovery_receipt(attempt: &mut Value, receipt: &AiNativeRecoveryReceipt) {
    attempt["ai_native_recovery"] = serde_json::to_value(receipt).unwrap_or(Value::Null);
}

/// Parse the typed recovery receipt a Provider attached to a failed invocation.
///
/// `Ok(None)` means the Provider reported no recovery detail at all; `Err` means
/// a receipt was present but could not be trusted. The two are kept apart so the
/// audit trail never rewrites a refused receipt into a missing one, and so only
/// the explicit `Ok(Some(..))` case can ever reach the replay decision.
fn parse_provider_recovery_receipt(
    provider_error: &ProviderRuntimeError,
) -> Result<Option<extension_contracts::provider_contract::ProviderRecoveryReceipt>, String> {
    provider_error
        .provider_details
        .as_ref()
        .map_or(Ok(None), |details| {
            extension_contracts::provider_contract::recovery_receipt_from_details(details)
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LlmRetryClass {
    AutomaticTransport,
    Configured,
}

pub(crate) fn full_jitter_delay_ms(retry_ordinal: usize, random_sample: u64) -> u64 {
    let exponent = u32::try_from(retry_ordinal).unwrap_or(u32::MAX).min(62);
    let upper_bound = AUTOMATIC_TRANSPORT_RETRY_BASE_MS
        .saturating_mul(1_u64 << exponent)
        .min(AUTOMATIC_TRANSPORT_RETRY_CAP_MS);
    random_sample % (upper_bound + 1)
}

#[cfg(not(test))]
fn automatic_transport_retry_delay_ms(retry_ordinal: usize) -> u64 {
    full_jitter_delay_ms(retry_ordinal, rand::random())
}

#[cfg(test)]
fn automatic_transport_retry_delay_ms(_retry_ordinal: usize) -> u64 {
    0
}

async fn wait_before_llm_retry(
    retry_class: LlmRetryClass,
    automatic_transport_retry_ordinal: usize,
    configured_retry_interval_ms: u64,
) {
    let delay_ms = match retry_class {
        LlmRetryClass::AutomaticTransport => {
            automatic_transport_retry_delay_ms(automatic_transport_retry_ordinal)
        }
        LlmRetryClass::Configured => configured_retry_interval_ms,
    };
    if delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }
}

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
    let Some(communication) = &runtime_context.communication else {
        return execute_llm_node_tools(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    };
    if !communication.is_main(node).await {
        return execute_llm_node_tools(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    }
    let mut pool = variable_pool.clone();
    let mut metrics = LlmRoundMetricsAccumulator::default();
    let mut tool_events = Vec::new();
    loop {
        let mut execution = execute_llm_node_tools(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            &mut pool,
            runtime_context,
            invoker,
            lifecycle,
        )
        .await?;
        metrics.absorb(&execution);
        if let Some(events) = execution
            .debug_payload
            .get("runtime_internal_tool_events")
            .and_then(Value::as_array)
        {
            tool_events.extend(events.iter().cloned());
        }
        if execution.error_payload.is_none()
            && execution.pending_callback.is_none()
            && communication.has_new_inputs().await?
        {
            pool = communication.retained_pool(node, &pool).await?;
            continue;
        }
        metrics.apply(&mut execution);
        attach_runtime_internal_tool_events(&mut execution, &tool_events);
        *variable_pool = pool;
        return Ok(execution);
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_llm_node_tools<I>(
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
            if let Some(communication) = &runtime_context.communication {
                communication.check_active().await?;
            }
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
        if let Some(communication) = &runtime_context.communication {
            communication
                .retain_pool(node, &llm_variable_pool, "after_tools")
                .await?;
        }
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

#[cfg(test)]
#[path = "../../_tests/llm_recovery_deadline.rs"]
mod recovery_deadline_tests;

#[path = "llm_executor/provider_round.rs"]
mod provider_round;

pub(crate) use provider_round::execute_llm_node_provider_round;
