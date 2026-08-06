use super::*;

const LLM_ROUTING_COUNTER_TTL: time::Duration = time::Duration::hours(1);

pub(super) async fn llm_request_runtimes(
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<Vec<CompiledLlmRuntime>> {
    let request_count = llm_request_count(node);
    let candidates = llm_route_candidate_runtimes(runtime);
    let mut request_runtimes = Vec::with_capacity(request_count);
    for attempt_index in 0..request_count {
        let target_index = llm_target_index(
            runtime.routing.as_ref(),
            candidates.len(),
            attempt_index,
            runtime_context,
        )
        .await?;
        request_runtimes.push(candidates[target_index].clone());
    }
    Ok(request_runtimes)
}

pub(super) struct ResolvedLlmRequestRuntime {
    pub(super) runtime: CompiledLlmRuntime,
    pub(super) route: Result<ResolvedProviderRoute>,
}

pub(super) async fn resolve_llm_request_runtime<I>(
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
    attempt_index: usize,
) -> Result<ResolvedLlmRequestRuntime>
where
    I: ProviderInvoker + ?Sized,
{
    let candidates = llm_route_candidate_runtimes(runtime);
    let mut compatible = Vec::with_capacity(candidates.len());
    let mut missing = BTreeSet::new();

    for candidate in candidates {
        match invoker.resolve_llm_route(&candidate).await {
            Ok(route) => {
                let candidate_missing = missing_routing_capabilities(
                    &route.runtime_capabilities,
                    required_capabilities,
                );
                if candidate_missing.is_empty() {
                    compatible.push(ResolvedLlmRequestRuntime {
                        runtime: candidate,
                        route: Ok(route),
                    });
                } else {
                    missing.extend(candidate_missing);
                }
            }
            Err(error) => compatible.push(ResolvedLlmRequestRuntime {
                runtime: candidate,
                route: Err(error),
            }),
        }
    }

    if compatible.is_empty() {
        return Err(semantic_route_error("llm_route", &missing));
    }

    let target_index = llm_target_index(
        runtime.routing.as_ref(),
        compatible.len(),
        attempt_index,
        runtime_context,
    )
    .await?;
    Ok(compatible.swap_remove(target_index))
}

pub(super) fn llm_request_count(node: &CompiledNode) -> usize {
    if node
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
    }
}

fn llm_route_candidate_runtimes(runtime: &CompiledLlmRuntime) -> Vec<CompiledLlmRuntime> {
    let Some(routing) = runtime.routing.as_ref() else {
        return vec![runtime.clone()];
    };
    if routing.routing_mode != LlmRoutingMode::FailoverQueue || routing.queue_targets.is_empty() {
        return vec![runtime.clone()];
    }

    routing
        .queue_targets
        .iter()
        .map(|target| {
            let mut candidate = runtime.clone();
            candidate.provider_instance_id = target.provider_instance_id.clone();
            candidate.provider_instance_display_name =
                target.provider_instance_display_name.clone();
            candidate.provider_code = target.provider_code.clone();
            candidate.protocol = target.protocol.clone();
            candidate.model = target.upstream_model_id.clone();
            candidate
        })
        .collect()
}

async fn llm_target_index(
    routing: Option<&crate::compiled_plan::CompiledLlmRouting>,
    target_count: usize,
    attempt_index: usize,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<usize> {
    let distribution_rule = routing
        .map(|routing| routing.distribution_rule)
        .unwrap_or_default();
    Ok(match distribution_rule {
        crate::compiled_plan::LlmDistributionRule::RoundRobin if target_count > 1 => {
            let distribution_key = routing
                .and_then(|routing| routing.distribution_key.as_deref())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("round_robin llm routing is missing distribution_key"))?;
            let counter = runtime_context
                .next_llm_routing_counter(distribution_key, Some(LLM_ROUTING_COUNTER_TTL))
                .await?;
            (counter - 1).rem_euclid(target_count as i64) as usize
        }
        crate::compiled_plan::LlmDistributionRule::RetryRoundRobin
        | crate::compiled_plan::LlmDistributionRule::None
            if target_count > 1 =>
        {
            attempt_index % target_count
        }
        _ => 0,
    })
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
    pub(super) reasoning_effort: Option<&'a str>,
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
        "reasoning_effort": input.reasoning_effort,
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

pub(super) fn attach_provider_stream_timing(attempt: &mut Value, timing: Option<&Value>) {
    let (Some(attempt), Some(timing)) = (attempt.as_object_mut(), timing) else {
        return;
    };
    attempt.insert("provider_stream_timing".to_string(), timing.clone());
}

pub(super) fn take_provider_stream_timing(result: &mut ProviderInvocationResult) -> Option<Value> {
    let metadata = result.provider_metadata.as_object_mut()?;
    let timing = metadata.remove("_1flowbase_runtime_stream_timing");
    if let Some(upstream_metadata) = metadata.remove("_1flowbase_upstream_provider_metadata") {
        result.provider_metadata = upstream_metadata;
    }
    timing
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
mod provider_stream_timing_tests {
    use super::*;

    #[test]
    fn timing_receipt_contains_only_safe_event_facts() {
        let timing = json!([{
            "sequence": 1,
            "event_kind": "text_delta",
            "size_bytes": 42,
            "ingress_ms": 120,
            "runtime_append_ms": 121
        }]);
        let mut attempt = json!({ "attempt_index": 0 });

        attach_provider_stream_timing(&mut attempt, Some(&timing));

        assert_eq!(attempt["provider_stream_timing"], timing);
        assert!(!attempt.to_string().contains("sensitive answer text"));
    }

    #[test]
    fn timing_wrapper_restores_upstream_provider_metadata_exactly() {
        let upstream = json!({
            "response_id": "response-1",
            "_1flowbase_runtime_stream_timing": "upstream-owned-value"
        });
        let timing = json!([{
            "sequence": 1,
            "event_kind": "finish",
            "size_bytes": 32,
            "ingress_ms": 200,
            "runtime_append_ms": 201
        }]);
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "_1flowbase_runtime_stream_timing": timing,
                "_1flowbase_upstream_provider_metadata": upstream
            }),
            ..ProviderInvocationResult::default()
        };

        let receipt = take_provider_stream_timing(&mut result);

        assert_eq!(receipt, Some(timing));
        assert_eq!(result.provider_metadata, upstream);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct CapabilityResolver {
        capabilities: BTreeMap<String, BTreeSet<String>>,
    }

    struct MutableCapabilityResolver {
        capabilities: Arc<Mutex<BTreeSet<String>>>,
    }

    #[async_trait]
    impl ProviderInvoker for CapabilityResolver {
        async fn resolve_llm_route(
            &self,
            runtime: &CompiledLlmRuntime,
        ) -> Result<ResolvedProviderRoute> {
            Ok(ResolvedProviderRoute::new(
                self.capabilities
                    .get(&runtime.provider_instance_id)
                    .cloned()
                    .unwrap_or_default(),
                runtime.provider_instance_id.clone(),
            ))
        }

        async fn invoke_llm(
            &self,
            _runtime: &CompiledLlmRuntime,
            _input: ProviderInvocationInput,
        ) -> Result<ProviderInvocationOutput> {
            unreachable!("route resolution tests do not invoke a Provider")
        }
    }

    #[async_trait]
    impl ProviderInvoker for MutableCapabilityResolver {
        async fn resolve_llm_route(
            &self,
            _runtime: &CompiledLlmRuntime,
        ) -> Result<ResolvedProviderRoute> {
            Ok(ResolvedProviderRoute::new(
                self.capabilities.lock().unwrap().clone(),
                (),
            ))
        }

        async fn invoke_llm(
            &self,
            _runtime: &CompiledLlmRuntime,
            _input: ProviderInvocationInput,
        ) -> Result<ProviderInvocationOutput> {
            unreachable!("route resolution tests do not invoke a Provider")
        }
    }

    fn target(id: &str) -> crate::compiled_plan::CompiledLlmRouteTarget {
        crate::compiled_plan::CompiledLlmRouteTarget {
            provider_instance_id: id.to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            upstream_model_id: format!("{id}-model"),
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

    fn runtime(routing: crate::compiled_plan::CompiledLlmRouting) -> CompiledLlmRuntime {
        CompiledLlmRuntime {
            provider_instance_id: "provider-incompatible".to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "provider-incompatible-model".to_string(),
            routing: Some(routing),
        }
    }

    #[tokio::test]
    async fn root_1534_live_capability_resolution_skips_an_incompatible_primary() {
        let runtime = runtime(failover_routing(vec![
            target("provider-incompatible"),
            target("provider-compatible"),
        ]));
        let resolver = CapabilityResolver {
            capabilities: BTreeMap::from([(
                "provider-compatible".to_string(),
                BTreeSet::from(["message_blocks.reasoning_history.v1".to_string()]),
            )]),
        };

        let selected = resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &resolver,
            &BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]),
            0,
        )
        .await
        .expect("the compatible backup should remain eligible");

        assert_eq!(selected.runtime.provider_instance_id, "provider-compatible");
        assert!(selected.route.is_ok());
    }

    #[tokio::test]
    async fn root_1534_live_capability_resolution_ignores_stale_compiled_capabilities() {
        let runtime = CompiledLlmRuntime {
            provider_instance_id: "provider-current".to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "current-model".to_string(),
            routing: Some(crate::compiled_plan::CompiledLlmRouting {
                routing_mode: LlmRoutingMode::FixedModel,
                fixed_model_target: Some(json!({
                    "provider_instance_id": "provider-current",
                    "provider_code": "fixture_provider",
                    "protocol": "openai_compatible",
                    "upstream_model_id": "current-model",
                    "runtime_capabilities": []
                })),
                queue_template_id: None,
                queue_snapshot_id: None,
                queue_targets: Vec::new(),
                distribution_rule: crate::compiled_plan::LlmDistributionRule::None,
                distribution_key: None,
                context_policy: json!({}),
                stream_policy: json!({}),
            }),
        };
        let resolver = CapabilityResolver {
            capabilities: BTreeMap::from([(
                "provider-current".to_string(),
                BTreeSet::from(["message_blocks.reasoning_history.v1".to_string()]),
            )]),
        };

        let selected = resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &resolver,
            &BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]),
            0,
        )
        .await
        .expect("the current installation should override a stale published capability copy");

        assert_eq!(selected.runtime.provider_instance_id, "provider-current");
    }

    #[tokio::test]
    async fn root_1534_live_capability_resolution_rejects_stale_fixed_support_claim() {
        let runtime = CompiledLlmRuntime {
            provider_instance_id: "provider-current".to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "current-model".to_string(),
            routing: Some(crate::compiled_plan::CompiledLlmRouting {
                routing_mode: LlmRoutingMode::FixedModel,
                fixed_model_target: Some(json!({
                    "provider_instance_id": "provider-current",
                    "provider_code": "fixture_provider",
                    "protocol": "openai_compatible",
                    "upstream_model_id": "current-model",
                    "runtime_capabilities": ["message_blocks.reasoning_history.v1"]
                })),
                queue_template_id: None,
                queue_snapshot_id: None,
                queue_targets: Vec::new(),
                distribution_rule: crate::compiled_plan::LlmDistributionRule::None,
                distribution_key: None,
                context_policy: json!({}),
                stream_policy: json!({}),
            }),
        };
        let resolver = CapabilityResolver {
            capabilities: BTreeMap::new(),
        };

        let error = match resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &resolver,
            &BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]),
            0,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("a stale compiled capability claim must not authorize invocation"),
        };
        let framework_error = error
            .downcast_ref::<plugin_framework::PluginFrameworkError>()
            .expect("semantic rejection should preserve the typed framework error");

        assert!(matches!(
            framework_error,
            plugin_framework::PluginFrameworkError::RuntimeContract { error }
                if error.kind == ProviderRuntimeErrorKind::SemanticCapabilityUnsupported
        ));
    }

    #[tokio::test]
    async fn root_1534_same_compiled_plan_observes_a_provider_capability_upgrade() {
        let runtime = CompiledLlmRuntime {
            provider_instance_id: "provider-current".to_string(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "current-model".to_string(),
            routing: None,
        };
        let capabilities = Arc::new(Mutex::new(BTreeSet::new()));
        let resolver = MutableCapabilityResolver {
            capabilities: capabilities.clone(),
        };
        let required =
            BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]);

        assert!(
            resolve_llm_request_runtime(
                &runtime,
                &ExecutionRuntimeContext::default(),
                &resolver,
                &required,
                0,
            )
            .await
            .is_err(),
            "the old Provider generation should be incompatible"
        );
        capabilities
            .lock()
            .unwrap()
            .insert("message_blocks.reasoning_history.v1".to_string());

        let selected = resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &resolver,
            &required,
            0,
        )
        .await
        .expect("the same compiled plan should observe the upgraded Provider generation");

        assert_eq!(selected.runtime.provider_instance_id, "provider-current");
    }

    #[tokio::test]
    async fn root_1534_live_capability_resolution_rejects_all_incompatible_routes() {
        let runtime = runtime(failover_routing(vec![target("provider-reasoning-only")]));
        let resolver = CapabilityResolver {
            capabilities: BTreeMap::from([(
                "provider-reasoning-only".to_string(),
                BTreeSet::from(["message_blocks.reasoning_history.v1".to_string()]),
            )]),
        };

        let error = match resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &resolver,
            &BTreeSet::from([
                ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
            ]),
            0,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("redacted reasoning must not route to an incompatible Provider"),
        };
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
