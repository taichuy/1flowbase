use super::*;

const LLM_ROUTING_COUNTER_TTL: time::Duration = time::Duration::hours(1);

pub(super) async fn llm_request_runtimes(
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
) -> Result<Vec<CompiledLlmRuntime>> {
    let request_count = if node
        .config
        .get("retry_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        1 + node
            .config
            .get("max_retries")
            .and_then(Value::as_u64)
            .unwrap_or(1) as usize
    } else {
        1
    };

    let Some(routing) = runtime.routing.as_ref() else {
        ensure_route_supports_required_capabilities(
            &runtime.provider_instance_id,
            &BTreeSet::new(),
            required_capabilities,
        )?;
        return Ok(vec![runtime.clone(); request_count]);
    };
    if routing.routing_mode != LlmRoutingMode::FailoverQueue || routing.queue_targets.is_empty() {
        let declared_capabilities = routing
            .fixed_model_target
            .as_ref()
            .and_then(|target| target.get("runtime_capabilities"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        ensure_route_supports_required_capabilities(
            &runtime.provider_instance_id,
            &declared_capabilities,
            required_capabilities,
        )?;
        return Ok(vec![runtime.clone(); request_count]);
    }

    let compatible_targets = compatible_queue_targets(routing, required_capabilities)?;

    let mut request_runtimes = Vec::with_capacity(request_count);
    for attempt_index in 0..request_count {
        let target_index = match routing.distribution_rule {
            crate::compiled_plan::LlmDistributionRule::RoundRobin
                if compatible_targets.len() > 1 =>
            {
                let distribution_key = routing
                    .distribution_key
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        anyhow!("round_robin llm routing is missing distribution_key")
                    })?;
                let counter = runtime_context
                    .next_llm_routing_counter(distribution_key, Some(LLM_ROUTING_COUNTER_TTL))
                    .await?;
                (counter - 1).rem_euclid(compatible_targets.len() as i64) as usize
            }
            crate::compiled_plan::LlmDistributionRule::RetryRoundRobin
            | crate::compiled_plan::LlmDistributionRule::None
                if compatible_targets.len() > 1 =>
            {
                attempt_index % compatible_targets.len()
            }
            _ => 0,
        };
        let target = compatible_targets[target_index];
        let mut request_runtime = runtime.clone();
        request_runtime.provider_instance_id = target.provider_instance_id.clone();
        request_runtime.provider_instance_display_name =
            target.provider_instance_display_name.clone();
        request_runtime.provider_code = target.provider_code.clone();
        request_runtime.protocol = target.protocol.clone();
        request_runtime.model = target.upstream_model_id.clone();
        request_runtimes.push(request_runtime);
    }

    Ok(request_runtimes)
}

fn compatible_queue_targets<'a>(
    routing: &'a crate::compiled_plan::CompiledLlmRouting,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
) -> Result<Vec<&'a crate::compiled_plan::CompiledLlmRouteTarget>> {
    let compatible_targets = routing
        .queue_targets
        .iter()
        .filter(|target| {
            missing_routing_capabilities(&target.runtime_capabilities, required_capabilities)
                .is_empty()
        })
        .collect::<Vec<_>>();
    if compatible_targets.is_empty() {
        let missing = routing
            .queue_targets
            .iter()
            .flat_map(|target| {
                missing_routing_capabilities(&target.runtime_capabilities, required_capabilities)
            })
            .collect::<BTreeSet<_>>();
        return Err(semantic_route_error("failover_queue", &missing));
    }
    Ok(compatible_targets)
}

fn ensure_route_supports_required_capabilities(
    route_id: &str,
    declared_capabilities: &BTreeSet<String>,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
) -> Result<()> {
    let missing = missing_routing_capabilities(declared_capabilities, required_capabilities);
    if missing.is_empty() {
        return Ok(());
    }
    Err(semantic_route_error(route_id, &missing))
}

fn missing_routing_capabilities(
    declared_capabilities: &BTreeSet<String>,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
) -> BTreeSet<String> {
    required_capabilities
        .iter()
        .filter(|capability| {
            matches!(
                capability,
                ProviderInvocationCapability::MessageBlocksReasoningHistoryV1
                    | ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1
            )
        })
        .map(|capability| capability.manifest_capability_name().to_string())
        .filter(|capability| !declared_capabilities.contains(capability))
        .collect()
}

fn semantic_route_error(route_id: &str, missing: &BTreeSet<String>) -> anyhow::Error {
    plugin_framework::PluginFrameworkError::runtime(
        ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::SemanticCapabilityUnsupported,
            "no LLM route accepts the request's canonical message-block semantics",
        )
        .with_provider_details(json!({
            "route_id": route_id,
            "missing_capabilities": missing,
        })),
    )
    .into()
}

pub(super) struct AttemptMetricInput<'a> {
    pub(super) attempt_index: usize,
    pub(super) retry_reason: Option<&'a str>,
    pub(super) runtime: &'a CompiledLlmRuntime,
    pub(super) status: &'a str,
    pub(super) failed_after_first_token: bool,
    pub(super) error_payload: Option<&'a Value>,
    pub(super) usage: &'a ProviderUsage,
    pub(super) event_count: usize,
    pub(super) started_at: OffsetDateTime,
    pub(super) first_token_at: Option<OffsetDateTime>,
    pub(super) finished_at: OffsetDateTime,
    pub(super) time_to_first_token_ms: Option<u64>,
}

pub(super) fn build_attempt_metric(input: AttemptMetricInput<'_>) -> Value {
    json!({
        "attempt_index": input.attempt_index,
        "is_retry": input.attempt_index > 0,
        "retry_reason": input.retry_reason,
        "provider_instance_id": input.runtime.provider_instance_id,
        "provider_instance_display_name": input.runtime.provider_instance_display_name,
        "provider_code": input.runtime.provider_code,
        "protocol": input.runtime.protocol,
        "upstream_model_id": input.runtime.model,
        "model": input.runtime.model,
        "status": input.status,
        "failed_after_first_token": input.failed_after_first_token,
        "event_count": input.event_count,
        "started_at": offset_datetime_json(Some(input.started_at)),
        "first_token_at": offset_datetime_json(input.first_token_at),
        "finished_at": offset_datetime_json(Some(input.finished_at)),
        "time_to_first_token_ms": input.time_to_first_token_ms,
        "usage": serde_json::to_value(input.usage).unwrap_or(Value::Null),
        "error_code": input.error_payload
            .and_then(|payload| payload.get("error_code"))
            .cloned()
            .unwrap_or(Value::Null),
        "error_message_ref": input.error_payload
            .and_then(|payload| payload.get("message"))
            .and_then(Value::as_str)
            .map(|message| format!("runtime_artifact:inline:error:{message}"))
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

pub(super) fn build_llm_metrics_payload(
    runtime: &CompiledLlmRuntime,
    usage: ProviderUsage,
    finish_reason: Option<ProviderFinishReason>,
    event_count: usize,
    attempts: Vec<Value>,
    first_token_at: Option<OffsetDateTime>,
    time_to_first_token_ms: Option<u64>,
) -> Value {
    json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_instance_display_name": runtime.provider_instance_display_name,
        "provider_code": runtime.provider_code,
        "protocol": runtime.protocol,
        "model": runtime.model,
        "event_count": event_count,
        "first_token_at": offset_datetime_json(first_token_at),
        "time_to_first_token_ms": time_to_first_token_ms,
        "route": build_llm_route_payload(runtime),
        "usage": serde_json::to_value(&usage).unwrap_or(Value::Null),
        "finish_reason": finish_reason
            .as_ref()
            .map(|reason| serde_json::to_value(reason).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
        "queue_snapshot_id": runtime
            .routing
            .as_ref()
            .and_then(|routing| routing.queue_snapshot_id.clone())
            .map(Value::String)
            .unwrap_or(Value::Null),
        "attempts": attempts,
    })
}

pub(super) fn offset_datetime_json(value: Option<OffsetDateTime>) -> Value {
    value
        .and_then(|datetime| datetime.format(&Rfc3339).ok())
        .map(Value::String)
        .unwrap_or(Value::Null)
}

pub(super) fn build_llm_route_payload(runtime: &CompiledLlmRuntime) -> Value {
    match runtime.routing.as_ref() {
        Some(routing) => json!({
            "routing_mode": routing.routing_mode,
            "fixed_model_target": routing.fixed_model_target,
            "queue_template_id": routing.queue_template_id,
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
        None => json!({
            "routing_mode": "fixed_model",
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: &str, capabilities: &[&str]) -> crate::compiled_plan::CompiledLlmRouteTarget {
        crate::compiled_plan::CompiledLlmRouteTarget {
            provider_instance_id: id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            upstream_model_id: format!("{id}-model"),
            runtime_capabilities: capabilities
                .iter()
                .map(|capability| (*capability).to_string())
                .collect(),
        }
    }

    fn failover_routing(
        queue_targets: Vec<crate::compiled_plan::CompiledLlmRouteTarget>,
    ) -> crate::compiled_plan::CompiledLlmRouting {
        crate::compiled_plan::CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: Some("queue-template-1".to_string()),
            queue_snapshot_id: Some("queue-snapshot-1".to_string()),
            queue_targets,
            distribution_rule: crate::compiled_plan::LlmDistributionRule::RetryRoundRobin,
            distribution_key: None,
            context_policy: json!({}),
            stream_policy: json!({}),
        }
    }

    #[test]
    fn root_1534_filters_incompatible_primary_before_failover_selection() {
        let routing = failover_routing(vec![
            target("provider-incompatible", &[]),
            target(
                "provider-compatible",
                &["message_blocks.reasoning_history.v1"],
            ),
        ]);

        let compatible = compatible_queue_targets(
            &routing,
            &BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]),
        )
        .expect("the compatible backup should remain eligible");

        assert_eq!(compatible.len(), 1);
        assert_eq!(compatible[0].provider_instance_id, "provider-compatible");
    }

    #[test]
    fn root_1534_rejects_all_incompatible_routes_with_typed_semantic_error() {
        let routing = failover_routing(vec![target(
            "provider-reasoning-only",
            &["message_blocks.reasoning_history.v1"],
        )]);

        let error = compatible_queue_targets(
            &routing,
            &BTreeSet::from([
                ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
            ]),
        )
        .expect_err("redacted reasoning must not route to an incompatible Provider");
        let framework_error = error
            .downcast_ref::<plugin_framework::PluginFrameworkError>()
            .expect("semantic rejection should preserve the typed framework error");

        assert!(matches!(
            framework_error,
            plugin_framework::PluginFrameworkError::RuntimeContract { error }
                if error.kind == ProviderRuntimeErrorKind::SemanticCapabilityUnsupported
        ));
    }
}
