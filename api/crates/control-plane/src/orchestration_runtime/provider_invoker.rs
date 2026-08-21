use super::*;
use plugin_framework::provider_contract::{
    ProviderMessageRole, ProviderOutputItemPhase, ProviderRuntimeError, ProviderRuntimeErrorKind,
};
use plugin_framework::{
    provider_contract::ProviderCountTokensFallbackReason,
    provider_count_tokens_estimator::estimate_provider_count_tokens,
};
use sha2::{Digest, Sha256};

use super::canonical_stream::{
    CanonicalBlockId, CanonicalCallId, CanonicalContentKind, CanonicalItemId, CanonicalStreamEvent,
    CanonicalStreamState, CanonicalStreamTransitionError,
};
use crate::installed_provider_package::load_installed_provider_package;

mod failover_queue;
mod main_instance_routing;
mod protocol_context;
pub(super) use failover_queue::freeze_failover_queue_routes;

const PROVIDER_LIVE_EVENT_LANE_CAPACITY: usize = 32;

const VISIBLE_INTERNAL_LLM_MEDIA_TOOLS_CONTEXT_KEY: &str = "visible_internal_llm_media_tools";

fn billing_invocation_id(
    flow_run_id: Uuid,
    node_id: Option<&str>,
    input: &ProviderInvocationInput,
) -> Uuid {
    if let Some(invocation_id) = input
        .trace_context
        .get("provider_invocation_id")
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        return invocation_id;
    }
    let attempt = input
        .trace_context
        .get("provider_attempt_index")
        .map(String::as_str)
        .unwrap_or("0");
    let digest = Sha256::digest(format!(
        "{flow_run_id}:{}:{}:{}:{attempt}",
        node_id.unwrap_or("unknown"),
        input.provider_instance_id,
        input.model
    ));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(bytes)
}

fn collected_provider_usage(
    events: &[ProviderStreamEvent],
    result: &plugin_framework::provider_contract::ProviderUsage,
) -> plugin_framework::provider_contract::ProviderUsage {
    fn add(target: &mut Option<u64>, value: Option<u64>) {
        if let Some(value) = value {
            *target = Some(target.unwrap_or_default().saturating_add(value));
        }
    }
    fn delta(
        target: &mut plugin_framework::provider_contract::ProviderUsage,
        value: &plugin_framework::provider_contract::ProviderUsage,
    ) {
        add(&mut target.input_tokens, value.input_tokens);
        add(
            &mut target.input_cache_hit_tokens,
            value.input_cache_hit_tokens,
        );
        add(
            &mut target.input_cache_miss_tokens,
            value.input_cache_miss_tokens,
        );
        add(&mut target.output_tokens, value.output_tokens);
        add(&mut target.reasoning_tokens, value.reasoning_tokens);
        add(&mut target.cache_read_tokens, value.cache_read_tokens);
        add(&mut target.cache_write_tokens, value.cache_write_tokens);
        add(&mut target.total_tokens, value.total_tokens);
    }
    let mut usage = result.clone();
    for event in events {
        match event {
            ProviderStreamEvent::UsageSnapshot { usage: snapshot } => usage = snapshot.clone(),
            ProviderStreamEvent::UsageDelta { usage: value } => delta(&mut usage, value),
            _ => {}
        }
    }
    usage
}

/// Fail-closed billing conflict for the narrow case where the provider produced
/// billable output but reported no usage. Carries the stream evidence so the
/// surfaced failure stays diagnosable.
fn provider_usage_unavailable_conflict(
    output: &crate::ports::ProviderRuntimeInvocationOutput,
) -> plugin_framework::PluginFrameworkError {
    plugin_framework::PluginFrameworkError::runtime(
        ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse,
            "provider_usage_unavailable",
        )
        .with_provider_details(json!({
            "reason": "billable provider output arrived without provider-reported usage",
            "billable_output": true,
            "finish_reason": serde_json::to_value(&output.result.finish_reason)
                .unwrap_or(Value::Null),
            "stream_termination": output
                .result
                .provider_metadata
                .get("stream_termination")
                .cloned()
                .unwrap_or(Value::Null),
        })),
    )
}

fn provider_tool_structure_receipt(tools: &[Value]) -> Value {
    Value::Array(
        tools
            .iter()
            .enumerate()
            .map(|(index, tool)| {
                let function = tool.get("function");
                json!({
                    "index": index,
                    "type": tool.get("type").and_then(Value::as_str),
                    "name": function
                        .and_then(|value| value.get("name"))
                        .or_else(|| tool.get("name"))
                        .and_then(Value::as_str),
                    "has_parameters": function
                        .and_then(|value| value.get("parameters"))
                        .or_else(|| tool.get("input_schema"))
                        .or_else(|| tool.get("inputSchema"))
                        .is_some(),
                })
            })
            .collect(),
    )
}

#[derive(Clone)]
struct RuntimeProviderInvocationPin {
    instance: domain::ModelProviderInstanceRecord,
    installation: domain::LocalPluginInstallationRecord,
    package: ProviderPackage,
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::ProviderInvoker for RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    async fn pipeline_provider_input(
        &self,
        mut input: ProviderInvocationInput,
    ) -> std::result::Result<
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineOutput,
        orchestration_runtime::provider_input_pipeline::ProviderInputPipelineError,
    > {
        if self.continuation_affinity().is_some() {
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
        }
        self.runtime.pipeline_provider_input(input).await
    }

    async fn acquire_http_node_client(
        &self,
        timeout: std::time::Duration,
        verify_ssl: bool,
    ) -> Result<Option<orchestration_runtime::execution_engine::HttpRequestClientLease>> {
        self.runtime
            .acquire_http_node_client(self.workspace_id, timeout, verify_ssl)
            .await
    }

    async fn compact(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: ProviderInvocationInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCompactResult> {
        self.apply_provider_transport(runtime, &mut input)?;
        let instance = self.resolve_llm_instance(runtime).await?;
        let installation = self.ready_installation(instance.installation_id).await?;
        let package = load_installed_provider_package(&installation)?;
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            &package,
            &installation,
            &instance,
        )
        .await?;

        self.runtime.compact(&installation, input).await
    }

    async fn count_tokens(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        mut input: plugin_framework::provider_contract::ProviderCountTokensInput,
    ) -> Result<plugin_framework::provider_contract::ProviderCountTokensResult> {
        let attempted = async {
            let instance = self.resolve_llm_instance(runtime).await?;
            let installation = self.ready_installation(instance.installation_id).await?;
            let package = load_installed_provider_package(&installation)?;
            input.set_provider_config(
                build_provider_runtime_config(
                    &self.repository,
                    &self.runtime,
                    &self.provider_secret_master_key,
                    &package,
                    &installation,
                    &instance,
                )
                .await?,
            );
            self.runtime
                .count_tokens(&installation, input.clone())
                .await
        }
        .await;

        match attempted {
            Ok(result) => Ok(result),
            Err(_) => Ok(match estimate_provider_count_tokens(input.as_invocation()) {
                Ok(mut result) => {
                    result.fallback_reason =
                        Some(ProviderCountTokensFallbackReason::PluginUnavailable);
                    result
                }
                Err(_) => plugin_framework::provider_contract::ProviderCountTokensResult::fallback_zero(),
            }),
        }
    }

    async fn resolve_protocol_context_locator(&self, locator: &Value) -> Result<Option<Value>> {
        self.open_protocol_context_locator_value(locator).await
    }

    async fn resolve_main_llm_routing(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<Option<orchestration_runtime::execution_engine::ResolvedMainLlmRouting>> {
        if !runtime.targets_main_instance() {
            return Ok(None);
        }
        let mut routing = self.resolve_current_main_llm_routing(runtime).await?;
        if self.continuation_affinity().is_some() {
            routing.candidates.retain(|candidate| {
                self.ensure_continuation_route(
                    &candidate.runtime,
                    &candidate.route.runtime_capabilities,
                )
                .is_ok()
            });
            if routing.candidates.is_empty() {
                return Err(provider_transport_error(
                    ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                    "main Provider routing has no legal continuation owner",
                ));
            }
        }
        Ok(Some(routing))
    }

    async fn resolve_llm_route(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<orchestration_runtime::execution_engine::ResolvedProviderRoute> {
        let route = self.resolve_registered_llm_route(runtime).await?;
        if let Some(affinity) = self.continuation_affinity() {
            self.ensure_provider_affinity(runtime, affinity)?;
        }
        Ok(route)
    }

    async fn invoke_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        let resolved_route = self.resolve_llm_route(runtime).await?;
        self.invoke_resolved_llm(runtime, resolved_route, input)
            .await
    }

    async fn invoke_resolved_llm(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        resolved_route: orchestration_runtime::execution_engine::ResolvedProviderRoute,
        mut input: ProviderInvocationInput,
    ) -> Result<orchestration_runtime::execution_engine::ProviderInvocationOutput> {
        self.ensure_continuation_route(runtime, &resolved_route.runtime_capabilities)?;
        self.apply_provider_transport(runtime, &mut input)?;
        let pin = resolved_route
            .invocation_pin::<RuntimeProviderInvocationPin>()
            .ok_or(ControlPlaneError::InvalidInput("resolved_provider_route"))?
            .clone();
        let instance = pin.instance;
        let installation = pin.installation;
        let package = pin.package;
        adapt_or_ensure_model_supports_content_blocks(
            &self.repository,
            &instance,
            &package,
            &runtime.model,
            &mut input,
        )
        .await?;

        let runtime_config_started = std::time::Instant::now();
        input.provider_config = build_provider_runtime_config(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            &package,
            &installation,
            &instance,
        )
        .await?;
        tracing::debug!(
            runtime_config_ms = runtime_config_started.elapsed().as_millis() as u64,
            "runtime config finished"
        );

        let canonical_tool_registry = input.tools.clone();
        let actual_provider_code = input.provider_code.clone();
        let tool_structure = provider_tool_structure_receipt(&input.tools);
        tracing::debug!(
            flow_run_id = ?self.flow_run_id,
            provider_instance_id = %instance.id,
            provider_code = %input.provider_code,
            protocol = %input.protocol,
            model = %input.model,
            tool_count = input.tools.len(),
            tool_structure = %tool_structure,
            "AI Gateway provider tool structure"
        );
        let configured_model = instance
            .configured_models
            .iter()
            .find(|model| model.model_id == runtime.model);
        let effective_context_window = input
            .model_parameters
            .get("requested_context_window")
            .and_then(Value::as_u64)
            .or_else(|| configured_model.and_then(|model| model.context_window_override_tokens))
            .or_else(|| {
                package
                    .predefined_models
                    .iter()
                    .find(|model| model.model_id == runtime.model)
                    .and_then(|model| model.context_window)
            });
        let provider_invoke_started_at = OffsetDateTime::now_utc();
        let provider_invoke_started = std::time::Instant::now();
        let first_token_timing = Arc::new(Mutex::new(None::<FirstTokenTiming>));
        let provider_stream_timing = Arc::new(Mutex::new(Vec::<Value>::new()));
        let mut required_forward_handle = None;
        let mut diagnostic_forward_handle = None;
        let active_node = self
            .flow_execution_context
            .as_ref()
            .and_then(|context| context.active_node.lock().ok()?.clone());
        let active_node = active_node.or_else(|| {
            Some(RuntimeActiveNode {
                node_id: self.active_node_id.clone()?,
                node_run_id: self.active_node_run_id?,
            })
        });
        let billing_node_id = active_node.as_ref().map(|node| node.node_id.clone());
        if let (Some(active_node), Some(stream), Some(flow_run_id)) = (
            active_node.as_ref(),
            self.runtime_event_stream.as_ref(),
            self.flow_run_id,
        ) {
            match estimate_provider_count_tokens(&input) {
                Ok(estimate) => {
                    let mut context_snapshot = debug_stream_events::context_snapshot(
                        &active_node.node_id,
                        active_node.node_run_id,
                        &estimate,
                        effective_context_window,
                    );
                    match runtime_event_persister::persist_runtime_event_payload(
                        &self.repository,
                        flow_run_id,
                        &context_snapshot,
                    )
                    .await
                    {
                        Ok(()) => {
                            context_snapshot.persist_required = false;
                            context_snapshot.durability = RuntimeEventDurability::Ephemeral;
                        }
                        Err(error) => {
                            tracing::warn!(
                                flow_run_id = %flow_run_id,
                                node_id = %active_node.node_id,
                                error = %error,
                                "failed to persist AI Gateway context snapshot"
                            );
                        }
                    }
                    append_provider_runtime_event(stream, flow_run_id, context_snapshot).await;
                }
                Err(error) => {
                    tracing::warn!(
                        flow_run_id = %flow_run_id,
                        node_id = %active_node.node_id,
                        error = %error,
                        "AI Gateway context estimate unavailable"
                    );
                }
            }
        }
        let presentation_source_node_id = input.trace_context.get("node_id").cloned();
        let live_provider_events = if let Some(RuntimeActiveNode {
            node_id,
            node_run_id,
        }) = active_node
        {
            let live_sender = self.live_provider_events.clone();
            let runtime_event_stream = self.runtime_event_stream.clone();
            let flow_run_id = self.flow_run_id;
            let first_token_timing_for_task = first_token_timing.clone();
            let provider_stream_timing_for_task = provider_stream_timing.clone();
            let canonical_tool_registry_for_task = canonical_tool_registry.clone();
            let answer_presentation = answer_presentation_source_is_active(
                presentation_source_node_id.as_deref(),
                &node_id,
            )
            .then(|| self.answer_presentation.clone())
            .flatten();
            let (required_sender, mut required_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let (diagnostic_sender, mut diagnostic_receiver) =
                mpsc::channel::<ProviderStreamEvent>(PROVIDER_LIVE_EVENT_LANE_CAPACITY);
            let diagnostic_node_id = node_id.clone();
            required_forward_handle = Some(tokio::spawn(async move {
                let mut canonical_writer = RuntimeCanonicalStreamWriter::new(node_id.clone());
                let mut ingress_sequence = 0_u64;
                while let Some(mut event) = required_receiver.recv().await {
                    ingress_sequence += 1;
                    let ingress_ms = provider_invoke_started.elapsed().as_millis() as u64;
                    let event_kind = provider_stream_event_kind(&event);
                    let size_bytes = serde_json::to_vec(&event).map_or(0, |payload| payload.len());
                    orchestration_runtime::execution_engine::canonicalize_provider_stream_event_tool_call_name(
                            &mut event,
                            &canonical_tool_registry_for_task,
                        );
                    record_first_token_timing(
                        &first_token_timing_for_task,
                        &event,
                        provider_invoke_started_at,
                        provider_invoke_started,
                    );
                    let canonical_deltas = canonical_writer.write(&event)?;
                    project_canonical_provider_deltas(
                        runtime_event_stream.as_ref(),
                        flow_run_id,
                        answer_presentation.as_ref(),
                        &node_id,
                        node_run_id,
                        &canonical_deltas,
                    )
                    .await;
                    if let (Some(stream), Some(flow_run_id)) = (&runtime_event_stream, flow_run_id)
                    {
                        let runtime_events = match &event {
                            ProviderStreamEvent::TextDelta { .. }
                            | ProviderStreamEvent::ReasoningDelta { .. }
                            | ProviderStreamEvent::Finish { .. }
                            | ProviderStreamEvent::Error { .. } => Vec::new(),
                            ProviderStreamEvent::ReasoningSignatureDelta { signature } => {
                                vec![debug_stream_events::reasoning_signature_delta(
                                    &node_id,
                                    node_run_id,
                                    signature.clone(),
                                )]
                            }
                            ProviderStreamEvent::OutputItem {
                                phase,
                                output_index,
                                item,
                            } => vec![match phase {
                                ProviderOutputItemPhase::Added => {
                                    debug_stream_events::provider_output_item_added(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                                ProviderOutputItemPhase::Done => {
                                    debug_stream_events::provider_output_item_done(
                                        &node_id,
                                        node_run_id,
                                        *output_index,
                                        item.clone(),
                                    )
                                }
                            }],
                            ProviderStreamEvent::UsageSnapshot { usage } => {
                                vec![debug_stream_events::usage_snapshot(
                                    &node_id,
                                    node_run_id,
                                    usage,
                                )]
                            }
                            _ => Vec::new(),
                        };
                        for runtime_event in runtime_events {
                            let event_type = runtime_event.event_type.clone();
                            let source = runtime_event.source;
                            let mut stream_event = runtime_event;
                            if debug_stream_events::is_answer_presentation_delta_payload(
                                &stream_event.payload,
                            ) {
                                stream_event.persist_required = false;
                            }
                            match stream.append(flow_run_id, stream_event).await {
                                Ok(_) => {}
                                Err(error) => {
                                    if is_expected_runtime_event_stream_closed_error(&error) {
                                        tracing::debug!(
                                            flow_run_id = %flow_run_id,
                                            event_type = %event_type,
                                            source = ?source,
                                            error = %error,
                                            "provider runtime event append skipped because stream is already closed"
                                        );
                                    } else {
                                        tracing::warn!(
                                            flow_run_id = %flow_run_id,
                                            event_type = %event_type,
                                            source = ?source,
                                            error = %error,
                                            "failed to append provider runtime event"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(mut timeline) = provider_stream_timing_for_task.lock() {
                        timeline.push(json!({
                            "sequence": ingress_sequence,
                            "event_kind": event_kind,
                            "size_bytes": size_bytes,
                            "ingress_ms": ingress_ms,
                            "runtime_append_ms": provider_invoke_started.elapsed().as_millis() as u64,
                        }));
                    }
                    if let Some(sender) = &live_sender {
                        sender
                            .send(LiveProviderStreamEvent {
                                node_id: node_id.clone(),
                                node_run_id,
                                event: event.clone(),
                            })
                            .await
                            .map_err(|_| anyhow!("required live provider event writer closed"))?;
                    }
                }
                let completion_deltas = canonical_writer.complete()?;
                project_canonical_provider_deltas(
                    runtime_event_stream.as_ref(),
                    flow_run_id,
                    answer_presentation.as_ref(),
                    &node_id,
                    node_run_id,
                    &completion_deltas,
                )
                .await;
                Ok::<_, anyhow::Error>(canonical_writer.into_state())
            }));
            let diagnostic_stream = self.runtime_event_stream.clone();
            let diagnostic_flow_run_id = self.flow_run_id;
            diagnostic_forward_handle = Some(tokio::spawn(async move {
                while let Some(event) = diagnostic_receiver.recv().await {
                    let ProviderStreamEvent::NativeEvent { protocol, event } = event else {
                        continue;
                    };
                    if let (Some(stream), Some(flow_run_id)) =
                        (&diagnostic_stream, diagnostic_flow_run_id)
                    {
                        let _ = stream
                            .append(
                                flow_run_id,
                                debug_stream_events::provider_native_event(
                                    &diagnostic_node_id,
                                    node_run_id,
                                    protocol,
                                    event,
                                ),
                            )
                            .await;
                    }
                }
            }));
            Some(crate::ports::ProviderLiveEventSenders {
                required: required_sender,
                diagnostic: diagnostic_sender,
            })
        } else {
            None
        };

        let billing_started_at = OffsetDateTime::now_utc();
        let pricing_provider_code = configured_model
            .map(|model| model.pricing_provider_code.as_str())
            .unwrap_or(domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE);
        let pricing_model_id = configured_model
            .map(|model| model.pricing_model_id.as_str())
            .unwrap_or(domain::DEFAULT_MODEL_PRICING_MODEL_ID);
        let billing = if self
            .repository
            .model_billing_enabled_at(self.workspace_id)
            .await?
            .is_some_and(|enabled_at| billing_started_at >= enabled_at)
        {
            let actor = self
                .flow_execution_context
                .as_ref()
                .map(|context| &context.data_model.actor)
                .ok_or(ControlPlaneError::Conflict(
                    "billing_actor_context_required",
                ))?;
            let candidates = if let Some(cache) = &self.model_pricing_cache_store {
                let key = crate::billing::pricing_rules_cache_key(
                    pricing_provider_code,
                    pricing_model_id,
                );
                match cache.get_json(&key).await? {
                    Some(value) => match serde_json::from_value(value) {
                        Ok(rules) => rules,
                        Err(_) => {
                            cache.delete(&key).await?;
                            let rules = self
                                .repository
                                .model_billing_list_pricing_rules(
                                    pricing_provider_code,
                                    pricing_model_id,
                                )
                                .await?;
                            cache
                                .set_json(
                                    &key,
                                    serde_json::to_value(&rules)?,
                                    Some(time::Duration::minutes(5)),
                                )
                                .await?;
                            rules
                        }
                    },
                    None => {
                        let rules = self
                            .repository
                            .model_billing_list_pricing_rules(
                                pricing_provider_code,
                                pricing_model_id,
                            )
                            .await?;
                        cache
                            .set_json(
                                &key,
                                serde_json::to_value(&rules)?,
                                Some(time::Duration::minutes(5)),
                            )
                            .await?;
                        rules
                    }
                }
            } else {
                self.repository
                    .model_billing_match_pricing_rules(
                        pricing_provider_code,
                        pricing_model_id,
                        billing_started_at,
                    )
                    .await?
            };
            let rule = crate::billing::choose_pricing_rule_for(
                pricing_provider_code,
                pricing_model_id,
                candidates,
                billing_started_at,
            )?
            .ok_or(ControlPlaneError::Conflict("pricing_rule_not_configured"))?;
            let input_tokens = estimate_provider_count_tokens(&input)
                .map(|estimate| estimate.input_tokens)
                .unwrap_or(0);
            let maximum_output_tokens = input
                .model_parameters
                .get("max_output_tokens")
                .or_else(|| input.model_parameters.get("max_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let estimate = crate::billing::rate_token_usage(
                &rule,
                &crate::billing::TokenUsage {
                    input_tokens: i64::try_from(input_tokens).unwrap_or(i64::MAX),
                    input_cache_hit_tokens: 0,
                    input_cache_miss_tokens: Some(i64::try_from(input_tokens).unwrap_or(i64::MAX)),
                    output_tokens: i64::try_from(maximum_output_tokens).unwrap_or(i64::MAX),
                },
            )?;
            let flow_run_id = self
                .flow_run_id
                .ok_or(ControlPlaneError::Conflict("billing_flow_run_required"))?;
            let invocation_id =
                billing_invocation_id(flow_run_id, billing_node_id.as_deref(), &input);
            let reservation = self
                .repository
                .model_billing_reserve_credit(&crate::ports::ReserveCreditInput {
                    workspace_id: self.workspace_id,
                    user_id: actor.user_id,
                    amount: estimate.total_cost.to_string(),
                    flow_run_id: Some(flow_run_id),
                    provider_invocation_id: invocation_id,
                    pricing_rule_id: rule.id,
                    charge_enabled_default: !actor.is_root,
                    reservation_expires_at: billing_started_at + time::Duration::minutes(15),
                })
                .await?;
            Some((rule, reservation, invocation_id, flow_run_id))
        } else {
            None
        };

        let billing_heartbeat = billing.as_ref().map(|(_, reservation, _, _)| {
            let repository = self.repository.clone();
            let billing_session_id = reservation.billing_session_id;
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    match repository
                        .model_billing_heartbeat_credit_reservation(
                            billing_session_id,
                            OffsetDateTime::now_utc() + time::Duration::minutes(15),
                        )
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(error) => tracing::warn!(
                            billing_session_id = %billing_session_id,
                            error = %error,
                            "model billing reservation heartbeat failed"
                        ),
                    }
                }
            })
        });

        let native_responses_passthrough = input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
        );
        let invocation_result = self
            .runtime
            .invoke_stream_with_network_egress(
                &installation,
                input,
                live_provider_events,
                self.workspace_id,
                domain::NetworkEgressConsumerSelector::ModelProviderInstance {
                    instance_id: instance.id,
                },
            )
            .await;
        if let Some(handle) = billing_heartbeat {
            handle.abort();
        }
        tracing::debug!(
            provider_invoke_ms = provider_invoke_started.elapsed().as_millis() as u64,
            "provider invoke finished"
        );
        let canonical_stream_state = if let Some(handle) = required_forward_handle {
            Some(handle.await.map_err(|error| {
                anyhow!("provider live event forwarding task panicked: {error}")
            })??)
        } else {
            None
        };
        if let Some(handle) = diagnostic_forward_handle {
            handle.await.map_err(|error| {
                anyhow!("provider diagnostic event forwarding task panicked: {error}")
            })?;
        }
        let (mut invocation_output, mut invocation_error) = match invocation_result {
            Ok(output) => (Some(output), None),
            Err(error) => (None, Some(error)),
        };
        // A native Responses turn may already have emitted a provider-owned continuation
        // before billing can classify the final usage. Preserve that one-shot state even when
        // fail-closed billing must reject the turn; the client still needs the continuation to
        // submit the provider's next approval/input turn.
        if let Some(output) = invocation_output.as_ref() {
            self.stage_provider_continuation(runtime, output.result.response_id.as_deref())
                .await?;
        }
        if let Some((rule, reservation, invocation_id, flow_run_id)) = billing {
            let usage = invocation_output.as_ref().map_or_else(
                || {
                    canonical_stream_state
                        .as_ref()
                        .map(|state| state.accumulated().usage().value().clone())
                        .unwrap_or_default()
                },
                |output| collected_provider_usage(&output.events, &output.result.usage),
            );
            let has_usage = usage.input_tokens.is_some()
                || usage.input_cache_miss_tokens.is_some()
                || usage.input_cache_hit_tokens.is_some()
                || usage.output_tokens.is_some();
            if !has_usage {
                self.repository
                    .model_billing_release_credit(
                        reservation.billing_session_id,
                        if invocation_error.is_some() {
                            "provider_invocation_failed_without_usage"
                        } else {
                            "provider_usage_unavailable"
                        },
                    )
                    .await?;
                if let Some(error) = invocation_error.take() {
                    return Err(error);
                }
                if let Some(output) = invocation_output.as_ref() {
                    if orchestration_runtime::execution_engine::billable_provider_output(
                        &output.events,
                        &output.result,
                        native_responses_passthrough,
                    ) {
                        return Err(provider_usage_unavailable_conflict(output).into());
                    }
                }
                // No usage and no billable output: the reservation is already
                // released, so hand the transport-Ok stream back to the executor
                // and let classification surface the upstream evidence.
            } else {
                let rated = crate::billing::rate_token_usage(
                    &rule,
                    &crate::billing::TokenUsage {
                        input_tokens: i64::try_from(usage.input_tokens.unwrap_or(0))
                            .unwrap_or(i64::MAX),
                        input_cache_hit_tokens: i64::try_from(
                            usage.input_cache_hit_tokens.unwrap_or(0),
                        )
                        .unwrap_or(i64::MAX),
                        input_cache_miss_tokens: usage
                            .input_cache_miss_tokens
                            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                        output_tokens: i64::try_from(usage.output_tokens.unwrap_or(0))
                            .unwrap_or(i64::MAX),
                    },
                )?;
                let price_snapshot = json!({
                    "pricing_rule_id": rule.id,
                    "pricing_provider_code": rule.provider_code,
                    "pricing_model_id": rule.upstream_model_id,
                    "provider_code": actual_provider_code,
                    "upstream_model_id": runtime.model,
                    "currency_code": rule.currency_code,
                    "request_started_at": billing_started_at,
                    "input_token_unit_size": rated.applied_rates.input.unit_size,
                    "input_token_unit_price": rated.applied_rates.input.unit_price.to_string(),
                    "output_token_unit_size": rated.applied_rates.output.unit_size,
                    "output_token_unit_price": rated.applied_rates.output.unit_price.to_string(),
                    "cache_hit_token_unit_size": rated.applied_rates.cache_hit.unit_size,
                    "cache_hit_token_unit_price": rated.applied_rates.cache_hit.unit_price.to_string(),
                    "rating_policy_enabled": rule.rating_policy_enabled,
                    "rating_policy": rule.rating_policy.clone(),
                    "rating_policy_match": rated.rating_policy_match.clone(),
                });
                let usage_snapshot = json!({
                    "usage_source": "provider_reported",
                    "ordinary_input_tokens": rated.ordinary_input_tokens,
                    "input_cache_hit_tokens": rated.cache_hit_tokens,
                    "output_tokens": rated.output_tokens,
                    "raw_usage": usage,
                });
                let active_node = self
                    .flow_execution_context
                    .as_ref()
                    .and_then(|context| context.active_node.lock().ok()?.clone());
                let finalized = self
                .repository
                .finalize_model_billing(&crate::ports::FinalizeModelBillingInput {
                    usage: crate::ports::AppendUsageLedgerInput {
                    flow_run_id,
                    node_run_id: active_node.as_ref().map(|node| node.node_run_id),
                    span_id: None,
                    failover_attempt_id: None,
                    provider_instance_id: Uuid::parse_str(&instance.id.to_string()).ok(),
                    gateway_route_id: None,
                    model_id: Some(runtime.model.clone()),
                    upstream_model_id: Some(runtime.model.clone()),
                    upstream_request_id: invocation_output
                        .as_ref()
                        .and_then(|output| output.result.response_id.clone()),
                    input_tokens: usage
                        .input_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cached_input_tokens: usage
                        .input_cache_hit_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    output_tokens: usage
                        .output_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    reasoning_output_tokens: usage
                        .reasoning_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    total_tokens: usage
                        .total_tokens()
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    input_cache_hit_tokens: usage
                        .input_cache_hit_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    input_cache_miss_tokens: usage
                        .input_cache_miss_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cache_read_tokens: usage
                        .cache_read_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    cache_write_tokens: usage
                        .cache_write_tokens
                        .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    price_snapshot: Some(price_snapshot.clone()),
                    cost_snapshot: Some(
                        json!({"total_cost":rated.total_cost.to_string(),"currency_code":"USD"}),
                    ),
                    usage_status: domain::UsageLedgerStatus::Recorded,
                    raw_usage: serde_json::to_value(&usage)?,
                    normalized_usage: usage_snapshot.clone(),
                    },
                    cost: crate::ports::AppendCostLedgerInput {
                        flow_run_id: Some(flow_run_id),
                        span_id: None,
                        usage_ledger_id: None,
                        billing_session_id: None,
                        workspace_id: self.workspace_id,
                        provider_instance_id: Some(instance.id),
                        provider_account_id: None,
                        gateway_route_id: None,
                        model_id: Some(runtime.model.clone()),
                        upstream_model_id: Some(runtime.model.clone()),
                        price_snapshot: price_snapshot.clone(),
                        raw_cost: Some(rated.total_cost.to_string()),
                        normalized_cost: Some(rated.total_cost.to_string()),
                        settlement_currency: Some("USD".to_string()),
                        cost_source: "local_token_pricing".to_string(),
                        cost_status: "rated".to_string(),
                    },
                    settlement: crate::ports::SettleCreditInput {
                        billing_session_id: reservation.billing_session_id,
                        actual_amount: rated.total_cost.to_string(),
                        cost_ledger_id: None,
                        usage_ledger_id: None,
                        price_snapshot: price_snapshot.clone(),
                        usage_snapshot: usage_snapshot.clone(),
                    },
                })
                .await;
                if let Some(output) = invocation_output.as_mut() {
                    let provider_metadata = std::mem::take(&mut output.result.provider_metadata);
                    let billing_metadata = match finalized {
                        Ok(finalized) => json!({
                            "provider_invocation_id": invocation_id,
                            "billing_session_id": reservation.billing_session_id,
                            "pricing_rule_id": rule.id,
                            "usage_ledger_id": finalized.usage.id,
                            "cost_ledger_id": finalized.cost.id,
                            "pricing_provider_code": rule.provider_code,
                            "pricing_model_id": rule.upstream_model_id,
                            "total_cost": rated.total_cost.to_string(),
                            "currency_code": "USD",
                            "charge_skipped": reservation.charge_skipped,
                            "billing_status": "settled",
                        }),
                        Err(error) => {
                            tracing::error!(
                                provider_invocation_id = %invocation_id,
                                billing_session_id = %reservation.billing_session_id,
                                error = %error,
                                "model billing finalization failed after provider response"
                            );
                            json!({
                                "provider_invocation_id": invocation_id,
                                "billing_session_id": reservation.billing_session_id,
                                "pricing_rule_id": rule.id,
                                "pricing_provider_code": rule.provider_code,
                                "pricing_model_id": rule.upstream_model_id,
                                "total_cost": rated.total_cost.to_string(),
                                "currency_code": "USD",
                                "charge_skipped": reservation.charge_skipped,
                                "billing_status": "reconciliation_failed",
                                "billing_error_code": "billing_finalize_failed",
                            })
                        }
                    };
                    output.result.provider_metadata = json!({
                        "_1flowbase_billing": billing_metadata,
                        "_1flowbase_upstream_provider_metadata": provider_metadata,
                    });
                }
            }
        }
        if let Some(error) = invocation_error {
            return Err(error);
        }
        let mut invocation_output = invocation_output
            .ok_or_else(|| anyhow!("provider invocation completed without output or error"))?;
        let runtime_stream_timing = provider_stream_timing
            .lock()
            .map_err(|_| anyhow!("provider stream timing lock is poisoned"))?
            .clone();
        if !runtime_stream_timing.is_empty() {
            let provider_metadata = std::mem::take(&mut invocation_output.result.provider_metadata);
            invocation_output.result.provider_metadata = json!({
                "_1flowbase_runtime_stream_timing": runtime_stream_timing,
                "_1flowbase_upstream_provider_metadata": provider_metadata,
            });
        }
        let captured_first_token_timing = first_token_timing.lock().ok().and_then(|timing| *timing);
        let mut output = orchestration_runtime::execution_engine::ProviderInvocationOutput {
            events: invocation_output.events,
            result: invocation_output.result,
            first_token_at: captured_first_token_timing.map(|timing| timing.first_token_at),
            time_to_first_token_ms: captured_first_token_timing
                .map(|timing| timing.time_to_first_token_ms),
        };
        orchestration_runtime::execution_engine::canonicalize_provider_output_tool_call_names(
            &mut output,
            &canonical_tool_registry,
        );
        Ok(output)
    }
}

fn provider_stream_event_kind(event: &ProviderStreamEvent) -> &'static str {
    match event {
        ProviderStreamEvent::NativeEvent { .. } => "native_event",
        ProviderStreamEvent::TextDelta { .. } => "text_delta",
        ProviderStreamEvent::ReasoningDelta { .. } => "reasoning_delta",
        ProviderStreamEvent::ReasoningSignatureDelta { .. } => "reasoning_signature_delta",
        ProviderStreamEvent::ToolCallDelta { .. } => "tool_call_delta",
        ProviderStreamEvent::ToolCallCommit { .. } => "tool_call_commit",
        ProviderStreamEvent::McpCallDelta { .. } => "mcp_call_delta",
        ProviderStreamEvent::McpCallCommit { .. } => "mcp_call_commit",
        ProviderStreamEvent::OutputItem { .. } => "output_item",
        ProviderStreamEvent::UsageDelta { .. } => "usage_delta",
        ProviderStreamEvent::UsageSnapshot { .. } => "usage_snapshot",
        ProviderStreamEvent::Finish { .. } => "finish",
        ProviderStreamEvent::Error { .. } => "error",
        ProviderStreamEvent::OutputProtocolFailure { .. } => "output_protocol_failure",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CanonicalProviderDelta {
    pub(super) kind: CanonicalContentKind,
    pub(super) text: String,
}

pub(super) struct RuntimeCanonicalStreamWriter {
    item_id: CanonicalItemId,
    text_block_id: CanonicalBlockId,
    reasoning_block_id: CanonicalBlockId,
    state: CanonicalStreamState,
    think_tag_splitter: ThinkTagStreamSplitter,
}

impl RuntimeCanonicalStreamWriter {
    pub(super) fn new(item_id: impl Into<String>) -> Self {
        let item_id = CanonicalItemId::new(item_id);
        Self {
            text_block_id: CanonicalBlockId::new(item_id.clone(), "text"),
            reasoning_block_id: CanonicalBlockId::new(item_id.clone(), "reasoning"),
            item_id,
            state: CanonicalStreamState::default(),
            think_tag_splitter: ThinkTagStreamSplitter::default(),
        }
    }

    pub(super) fn write(
        &mut self,
        event: &ProviderStreamEvent,
    ) -> Result<Vec<CanonicalProviderDelta>> {
        if self.state.terminal().is_some() {
            return Err(CanonicalStreamTransitionError::StreamAlreadyTerminal.into());
        }

        match event {
            ProviderStreamEvent::TextDelta { delta } => {
                let parts = self.think_tag_splitter.split(delta);
                self.append_content_parts(parts)
            }
            ProviderStreamEvent::ReasoningDelta { delta } => {
                self.append_content_parts(vec![DebugDeltaPart {
                    kind: DebugDeltaKind::Reasoning,
                    text: delta.clone(),
                }])
            }
            ProviderStreamEvent::ReasoningSignatureDelta { .. } => Ok(Vec::new()),
            ProviderStreamEvent::ToolCallDelta { call_id, delta } => {
                if let Some(arguments) = tool_argument_delta(delta) {
                    self.state.apply(CanonicalStreamEvent::ToolArgumentsDelta {
                        call_id: CanonicalCallId::new(self.item_id.clone(), call_id.clone()),
                        delta: arguments.to_string(),
                    })?;
                }
                Ok(Vec::new())
            }
            ProviderStreamEvent::UsageDelta { usage } => {
                self.state.apply(CanonicalStreamEvent::UsageDelta {
                    usage: usage.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::UsageSnapshot { usage } => {
                self.state.apply(CanonicalStreamEvent::UsageSnapshot {
                    usage: usage.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::OutputItem {
                phase,
                output_index,
                item,
            } => {
                self.state.apply(CanonicalStreamEvent::OutputItem {
                    phase: *phase,
                    output_index: *output_index,
                    item: item.clone(),
                })?;
                Ok(Vec::new())
            }
            ProviderStreamEvent::Finish { reason } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Finish {
                    reason: reason.clone(),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::Error { error } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Fail {
                    error: error.clone(),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::OutputProtocolFailure { failure } => {
                let deltas = self.flush_pending_content()?;
                self.state.apply(CanonicalStreamEvent::Fail {
                    error: ProviderRuntimeError::new(
                        ProviderRuntimeErrorKind::ProviderInvalidResponse,
                        failure.message.clone(),
                    )
                    .with_provider_details(failure.provider_details.clone()),
                })?;
                Ok(deltas)
            }
            ProviderStreamEvent::NativeEvent { .. }
            | ProviderStreamEvent::ToolCallCommit { .. }
            | ProviderStreamEvent::McpCallDelta { .. }
            | ProviderStreamEvent::McpCallCommit { .. } => Ok(Vec::new()),
        }
    }

    pub(super) fn complete(&mut self) -> Result<Vec<CanonicalProviderDelta>> {
        if self.state.terminal().is_some() {
            return Ok(Vec::new());
        }
        self.flush_pending_content()
    }

    pub(super) fn state(&self) -> &CanonicalStreamState {
        &self.state
    }

    pub(super) fn into_state(self) -> CanonicalStreamState {
        self.state
    }

    fn flush_pending_content(&mut self) -> Result<Vec<CanonicalProviderDelta>> {
        let parts = self.think_tag_splitter.finish();
        self.append_content_parts(parts)
    }

    fn append_content_parts(
        &mut self,
        parts: Vec<DebugDeltaPart>,
    ) -> Result<Vec<CanonicalProviderDelta>> {
        let mut deltas = Vec::with_capacity(parts.len());
        for part in parts {
            let (kind, event) = match part.kind {
                DebugDeltaKind::Text => (
                    CanonicalContentKind::Text,
                    CanonicalStreamEvent::TextDelta {
                        block_id: self.text_block_id.clone(),
                        delta: part.text.clone(),
                    },
                ),
                DebugDeltaKind::Reasoning => (
                    CanonicalContentKind::Reasoning,
                    CanonicalStreamEvent::ReasoningDelta {
                        block_id: self.reasoning_block_id.clone(),
                        delta: part.text.clone(),
                    },
                ),
            };
            self.state.apply(event)?;
            deltas.push(CanonicalProviderDelta {
                kind,
                text: part.text,
            });
        }
        Ok(deltas)
    }
}

fn tool_argument_delta(delta: &Value) -> Option<&str> {
    delta
        .pointer("/function/arguments")
        .or_else(|| delta.get("arguments"))
        .and_then(Value::as_str)
}

fn record_first_token_timing(
    first_token_timing: &Arc<Mutex<Option<FirstTokenTiming>>>,
    event: &ProviderStreamEvent,
    provider_invoke_started_at: OffsetDateTime,
    provider_invoke_started: std::time::Instant,
) {
    if !matches!(
        event,
        ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. }
    ) {
        return;
    }

    let Ok(mut timing) = first_token_timing.lock() else {
        return;
    };
    if timing.is_some() {
        return;
    }
    let elapsed = provider_invoke_started.elapsed();
    *timing = Some(FirstTokenTiming {
        first_token_at: provider_invoke_started_at + elapsed,
        time_to_first_token_ms: elapsed.as_millis() as u64,
    });
}

pub(super) fn is_expected_runtime_event_stream_closed_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.contains("runtime event stream is closed")
        || message.contains("runtime event stream is not open")
}

fn answer_presentation_source_is_active(trace_node_id: Option<&str>, active_node_id: &str) -> bool {
    trace_node_id == Some(active_node_id)
}

async fn project_canonical_provider_deltas(
    stream: Option<&Arc<dyn RuntimeEventStream>>,
    flow_run_id: Option<Uuid>,
    answer_presentation: Option<
        &Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>,
    >,
    node_id: &str,
    node_run_id: Uuid,
    deltas: &[CanonicalProviderDelta],
) {
    for delta in deltas {
        let canonical_event = match delta.kind {
            CanonicalContentKind::Text => ProviderStreamEvent::TextDelta {
                delta: delta.text.clone(),
            },
            CanonicalContentKind::Reasoning => ProviderStreamEvent::ReasoningDelta {
                delta: delta.text.clone(),
            },
        };
        if let Some(cursor) = answer_presentation {
            let presentation_events =
                cursor
                    .lock()
                    .await
                    .push_provider_event(node_id, node_run_id, &canonical_event);
            if let (Some(stream), Some(flow_run_id)) = (stream, flow_run_id) {
                for presentation_event in presentation_events {
                    append_provider_runtime_event(stream, flow_run_id, presentation_event).await;
                }
            }
        }

        let (Some(stream), Some(flow_run_id)) = (stream, flow_run_id) else {
            continue;
        };
        let debug_event = match delta.kind {
            CanonicalContentKind::Text => {
                debug_stream_events::text_delta(node_id, node_run_id, delta.text.clone())
            }
            CanonicalContentKind::Reasoning => {
                debug_stream_events::reasoning_delta(node_id, node_run_id, delta.text.clone())
            }
        };
        append_provider_runtime_event(stream, flow_run_id, debug_event).await;
    }
}

async fn append_provider_runtime_event(
    stream: &Arc<dyn RuntimeEventStream>,
    flow_run_id: Uuid,
    event: crate::ports::RuntimeEventPayload,
) {
    let event_type = event.event_type.clone();
    let source = event.source;
    if let Err(error) = stream.append(flow_run_id, event).await {
        if is_expected_runtime_event_stream_closed_error(&error) {
            tracing::debug!(
                flow_run_id = %flow_run_id,
                event_type = %event_type,
                source = ?source,
                error = %error,
                "provider runtime event append skipped because stream is already closed"
            );
        } else {
            tracing::warn!(
                flow_run_id = %flow_run_id,
                event_type = %event_type,
                source = ?source,
                error = %error,
                "failed to append provider runtime event"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DebugDeltaKind {
    Text,
    Reasoning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DebugDeltaPart {
    pub(super) kind: DebugDeltaKind,
    pub(super) text: String,
}

#[derive(Debug, Default)]
pub(super) struct ThinkTagStreamSplitter {
    inside_think: bool,
    pending: String,
}

impl ThinkTagStreamSplitter {
    pub(super) fn split(&mut self, delta: &str) -> Vec<DebugDeltaPart> {
        self.pending.push_str(delta);
        let mut parts = Vec::new();

        loop {
            let tag = if self.inside_think {
                "</think>"
            } else {
                "<think>"
            };

            if let Some(tag_index) = self.pending.find(tag) {
                let text = self.pending[..tag_index].to_string();
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
                self.pending.drain(..tag_index + tag.len());
                self.inside_think = !self.inside_think;
                continue;
            }

            let keep_len = partial_tag_prefix_len(&self.pending, tag);
            let emit_len = self.pending.len().saturating_sub(keep_len);
            if emit_len > 0 {
                let text = self.pending[..emit_len].to_string();
                self.pending.drain(..emit_len);
                push_debug_delta_part(
                    &mut parts,
                    if self.inside_think {
                        DebugDeltaKind::Reasoning
                    } else {
                        DebugDeltaKind::Text
                    },
                    text,
                );
            }
            break;
        }

        parts
    }

    pub(super) fn finish(&mut self) -> Vec<DebugDeltaPart> {
        let text = std::mem::take(&mut self.pending);
        let mut parts = Vec::new();
        push_debug_delta_part(
            &mut parts,
            if self.inside_think {
                DebugDeltaKind::Reasoning
            } else {
                DebugDeltaKind::Text
            },
            text,
        );
        parts
    }
}

fn push_debug_delta_part(parts: &mut Vec<DebugDeltaPart>, kind: DebugDeltaKind, text: String) {
    if text.is_empty() {
        return;
    }

    if let Some(previous) = parts.last_mut().filter(|part| part.kind == kind) {
        previous.text.push_str(&text);
        return;
    }

    parts.push(DebugDeltaPart { kind, text });
}

fn partial_tag_prefix_len(buffer: &str, tag: &str) -> usize {
    let max_len = buffer.len().min(tag.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|length| {
            let start = buffer.len() - length;
            buffer.is_char_boundary(start) && tag.starts_with(&buffer[start..])
        })
        .unwrap_or(0)
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    fn continuation_affinity(&self) -> Option<&crate::ports::ProviderTransportAffinity> {
        self.provider_continuation
            .as_ref()
            .map(crate::ports::ProviderContinuation::affinity)
            .or_else(|| {
                self.provider_transport_payload
                    .as_ref()
                    .and_then(crate::ports::ProviderTransportPayload::affinity)
            })
    }

    fn ensure_continuation_route(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        runtime_capabilities: &std::collections::BTreeSet<String>,
    ) -> Result<()> {
        let Some(affinity) = self.continuation_affinity() else {
            return Ok(());
        };
        self.ensure_provider_affinity(runtime, affinity)?;
        let capability = plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported
            .manifest_capability_name();
        if runtime_capabilities.contains(capability) {
            Ok(())
        } else {
            Err(provider_transport_error(
                ProviderRuntimeErrorKind::SemanticCapabilityUnsupported,
                "selected LLM Provider does not support native continuation",
            ))
        }
    }

    pub(super) fn apply_provider_transport(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: &mut ProviderInvocationInput,
    ) -> Result<()> {
        if let Some(continuation) = self.provider_continuation.as_ref() {
            self.ensure_provider_affinity(runtime, continuation.affinity())?;
            input.previous_response_id = Some(continuation.response_id().to_string());
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
        }
        let native_transport_capability =
            plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough;
        let Some(payload) = self.provider_transport_payload.as_ref() else {
            if input
                .required_capabilities
                .contains(&native_transport_capability)
            {
                return Err(anyhow!("ephemeral_transport_missing"));
            }
            return Ok(());
        };

        if let Some(affinity) = payload.affinity() {
            self.ensure_provider_affinity(runtime, affinity)?;
            input.required_capabilities.insert(
                plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
            );
            // The sealed native request already owns the continuation delta. Canonical history is
            // retained for routing, then removed at the selected Provider boundary to prevent a
            // second full-history wire representation from accompanying the native transport.
            input.messages.clear();
            input.system.clear();
            input.tools.clear();
            input.mcp_bindings.clear();
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksReasoningHistoryV1,
            );
            input.required_capabilities.remove(
                &plugin_framework::provider_contract::ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1,
            );
        }

        input
            .required_capabilities
            .insert(native_transport_capability);
        let protocol = match payload.protocol() {
            crate::ports::ProviderTransportProtocol::OpenAiResponses => "openai_responses",
        };
        input.native_transport = Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: protocol.to_string(),
                wire_body: payload.wire_body().clone(),
                digest: payload.digest().to_string(),
                size_bytes: payload.size_bytes() as u64,
            },
        );
        Ok(())
    }

    fn ensure_provider_affinity(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        affinity: &crate::ports::ProviderTransportAffinity,
    ) -> Result<()> {
        if affinity.matches(
            &runtime.provider_instance_id,
            &runtime.provider_code,
            &runtime.protocol,
            &runtime.model,
        ) {
            return Ok(());
        }
        Err(plugin_framework::PluginFrameworkError::runtime(
            plugin_framework::provider_contract::ProviderRuntimeError::new(
                plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                "selected LLM Provider does not own the opaque continuation",
            ),
        )
        .into())
    }

    async fn ready_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::LocalPluginInstallationRecord> {
        match (
            self.api_node_id.as_deref(),
            self.provider_install_root.as_deref(),
        ) {
            (Some(node_id), Some(install_root)) => {
                ready_current_node_plugin_installation(
                    &self.repository,
                    node_id,
                    install_root,
                    installation_id,
                )
                .await
            }
            _ => Err(ControlPlaneError::Conflict("plugin_node_context_required").into()),
        }
    }
}

fn provider_transport_error(
    kind: ProviderRuntimeErrorKind,
    message: &'static str,
) -> anyhow::Error {
    plugin_framework::PluginFrameworkError::runtime(ProviderRuntimeError::new(kind, message)).into()
}

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository + PluginRepository + Clone + Send + Sync,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    pub(super) fn for_flow_run(&self, flow_run_id: Uuid) -> Self {
        Self {
            repository: self.repository.clone(),
            runtime: self.runtime.clone(),
            workspace_id: self.workspace_id,
            provider_secret_master_key: self.provider_secret_master_key.clone(),
            live_provider_events: self.live_provider_events.clone(),
            runtime_event_stream: self.runtime_event_stream.clone(),
            flow_run_id: Some(flow_run_id),
            active_node_id: self.active_node_id.clone(),
            active_node_run_id: self.active_node_run_id,
            api_node_id: self.api_node_id.clone(),
            provider_install_root: self.provider_install_root.clone(),
            flow_execution_context: self.flow_execution_context.clone(),
            answer_presentation: self.answer_presentation.clone(),
            provider_transport_payload: self.provider_transport_payload.clone(),
            provider_transport_store: self.provider_transport_store.clone(),
            provider_continuation: self.provider_continuation.clone(),
            model_pricing_cache_store: self.model_pricing_cache_store.clone(),
        }
    }

    pub(super) fn with_flow_execution_context(
        &self,
        context: Arc<RuntimeFlowExecutionContext>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.flow_execution_context = Some(context);
        invoker
    }

    pub(super) fn with_answer_presentation(
        &self,
        answer_presentation: Arc<tokio::sync::Mutex<answer_presentation::AnswerPresentationCursor>>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.answer_presentation = Some(answer_presentation);
        invoker
    }

    pub(super) fn with_provider_transport_payload(
        &self,
        provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.provider_transport_payload = provider_transport_payload;
        invoker
    }

    pub(super) fn with_provider_continuation(
        &self,
        provider_continuation: Option<crate::ports::ProviderContinuation>,
    ) -> Self {
        let mut invoker = self.clone();
        invoker.provider_continuation = provider_continuation;
        invoker
    }

    async fn stage_provider_continuation(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        response_id: Option<&str>,
    ) -> Result<()> {
        let Some(response_id) = response_id.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };
        let (Some(store), Some(flow_run_id)) =
            (self.provider_transport_store.as_ref(), self.flow_run_id)
        else {
            return Ok(());
        };
        // A first native Responses turn can expose only the Provider response id; the opaque
        // request payload and an already-claimed continuation are both absent on that path.
        // Keep the id available for the later MCP approval turn instead of treating those
        // absent inputs as proof that no continuation exists.
        let continuation = crate::ports::ProviderContinuation::new(
            response_id,
            crate::ports::ProviderTransportAffinity::new(
                &runtime.provider_instance_id,
                &runtime.provider_code,
                &runtime.protocol,
                &runtime.model,
            ),
        )?;
        store
            .put_continuation(
                crate::ports::ProviderContinuationSlotId::for_flow_run(flow_run_id),
                continuation,
            )
            .await
            .map_err(|_| {
                anyhow::Error::from(plugin_framework::PluginFrameworkError::runtime(
                    plugin_framework::provider_contract::ProviderRuntimeError::new(
                        plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderTransportUnavailable,
                        "opaque Provider continuation could not be staged",
                    ),
                ))
            })
    }

    pub(super) async fn resolve_llm_instance(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        let provider_instance_id = Uuid::parse_str(&runtime.provider_instance_id)
            .map_err(|_| ControlPlaneError::InvalidInput("source_instance_id"))?;
        let instance = self
            .repository
            .get_instance(self.workspace_id, provider_instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        if instance.provider_code != runtime.provider_code {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        if instance.status != domain::ModelProviderInstanceStatus::Ready {
            return Err(ControlPlaneError::Conflict("provider_instance_not_ready").into());
        }
        if !instance.included_in_main {
            return Err(ControlPlaneError::Conflict("provider_instance_not_in_main").into());
        }
        let installation = self
            .repository
            .get_installation(instance.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned {
            return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
        }
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        if !instance.enabled_model_ids.is_empty()
            && !instance
                .enabled_model_ids
                .iter()
                .any(|model_id| model_id == &runtime.model)
        {
            return Err(ControlPlaneError::InvalidInput("model").into());
        }

        Ok(instance)
    }
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CapabilityInvoker
    for RuntimeProviderInvoker<R, H>
where
    R: ModelDefinitionRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync,
    H: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone + Send + Sync,
{
    async fn invoke_capability_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CapabilityInvocationOutput> {
        let installation = self.ready_installation(runtime.installation_id).await?;
        let assigned = self
            .repository
            .list_assignments(self.workspace_id)
            .await?
            .into_iter()
            .any(|assignment| assignment.installation_id == installation.id);
        if !assigned
            || matches!(
                installation.desired_state,
                domain::PluginDesiredState::Disabled
            )
        {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }
        if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let plugin_id = installation.plugin_id.clone();
        let credit_commands_allowed = installation.trust_level == "verified_official";

        let mut output = self
            .runtime
            .execute_node(ExecuteCapabilityNodeInput {
                installation,
                contribution_code: runtime.contribution_code.clone(),
                config_payload,
                input_payload,
            })
            .await?;

        if let Some(command_value) = output
            .output_payload
            .as_object_mut()
            .and_then(|payload| payload.remove("_1flowbase_credit_command"))
        {
            if !credit_commands_allowed {
                return Err(ControlPlaneError::PermissionDenied(
                    "plugin_credit_command_requires_verified_official_plugin",
                )
                .into());
            }
            let request: crate::ports::PluginCreditCommandRequest =
                serde_json::from_value(command_value)
                    .map_err(|_| ControlPlaneError::InvalidInput("plugin_credit_command"))?;
            let result = self
                .repository
                .execute_plugin_credit_command(
                    self.workspace_id,
                    &plugin_id,
                    &output.granted_credit_permissions,
                    request,
                )
                .await?;
            output
                .output_payload
                .as_object_mut()
                .expect("credit command was extracted from an object payload")
                .insert(
                    "_1flowbase_credit_result".to_string(),
                    serde_json::to_value(result)?,
                );
        }

        Ok(
            orchestration_runtime::execution_engine::CapabilityInvocationOutput {
                output_payload: output.output_payload,
            },
        )
    }

    async fn invoke_data_model_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        resolved_inputs: &serde_json::Map<String, Value>,
    ) -> Result<orchestration_runtime::execution_engine::DataModelInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("data model flow execution context is not configured"))?;
        let active_node = context
            .active_node
            .lock()
            .map_err(|_| anyhow!("active runtime node lock is poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("data model active runtime node is not set"))?;
        if active_node.node_id != node.node_id {
            return Err(anyhow!(
                "data model active runtime node mismatch: expected {}, got {}",
                node.node_id,
                active_node.node_id
            ));
        }

        let execution = data_model_runtime::execute_data_model_node(
            self.repository.clone(),
            context.data_model.runtime_engine.clone(),
            &context.data_model.actor,
            node,
            resolved_inputs,
            &data_model_runtime::DataModelRunContext {
                workspace_id: self.workspace_id,
                application_id: context.data_model.application_id,
                draft_id: context.data_model.draft_id,
                flow_run_id: context.data_model.flow_run_id,
                node_run_id: active_node.node_run_id,
            },
        )
        .await;

        let (pending_callback, debug_payload) = match execution.waiting_confirmation {
            Some(confirmation) => {
                let request_payload = json!({
                    "kind": "data_model_side_effect_confirmation",
                    "actor_user_id": context.data_model.actor.user_id,
                    "node_id": node.node_id,
                    "run_id": context.data_model.flow_run_id,
                    "payload_hash": confirmation.payload_hash,
                    "idempotency_key": confirmation.idempotency_key,
                    "expires_at": confirmation.expires_at,
                    "request_payload": confirmation.request_payload,
                });
                (
                    Some(orchestration_runtime::execution_engine::DataModelCallback {
                        callback_kind: "data_model_side_effect_confirmation".to_string(),
                        request_payload,
                    }),
                    json!({
                        "side_effect_policy": "confirm_each_run",
                        "idempotency_key": confirmation.idempotency_key,
                        "payload_hash": confirmation.payload_hash,
                        "expires_at": confirmation.expires_at,
                    }),
                )
            }
            None => (None, json!({})),
        };

        Ok(
            orchestration_runtime::execution_engine::DataModelInvocationOutput {
                output_payload: execution.output_payload,
                error_payload: execution.error_payload,
                metrics_payload: execution.metrics_payload,
                debug_payload,
                pending_callback,
            },
        )
    }

    async fn invoke_native_sql_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        sql: &str,
    ) -> Result<orchestration_runtime::execution_engine::NativeSqlInvocationOutput> {
        let context = self
            .flow_execution_context
            .as_ref()
            .ok_or_else(|| anyhow!("native SQL flow execution context is not configured"))?;
        let data_source_instance_id = node
            .config
            .get("data_source_instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("SQL node is missing data_source_instance_id"))?;
        match context
            .data_model
            .runtime_engine
            .execute_native_sql(self.workspace_id, data_source_instance_id, sql)
            .await
        {
            Ok(output) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: serde_json::to_value(output)?,
                    error_payload: None,
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
            Err(error) => Ok(
                orchestration_runtime::execution_engine::NativeSqlInvocationOutput {
                    output_payload: json!({ "results": [] }),
                    error_payload: Some(native_sql_error_payload(&error)),
                    metrics_payload: json!({}),
                    debug_payload: json!({
                        "data_source_instance_id": data_source_instance_id,
                        "sql": sql,
                    }),
                },
            ),
        }
    }
}

fn native_sql_error_payload(error: &anyhow::Error) -> Value {
    if let Some(runtime_error) = error.downcast_ref::<plugin_framework::ProviderRuntimeError>() {
        return native_sql_provider_error_payload(runtime_error);
    }
    if let Some(framework_error) = error.downcast_ref::<plugin_framework::PluginFrameworkError>() {
        return match framework_error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => {
                native_sql_provider_error_payload(error)
            }
            plugin_framework::PluginFrameworkError::InvalidProviderContract { message } => {
                let code = if message.starts_with("data_source_capability_not_supported") {
                    "data_source_capability_not_supported"
                } else {
                    "invalid_native_sql_result_contract"
                };
                json!({
                    "kind": "contract_error",
                    "code": code,
                    "message": message,
                })
            }
            _ => json!({
                "kind": "transport_error",
                "code": "data_source_transport_error",
                "message": framework_error.to_string(),
            }),
        };
    }
    json!({
        "kind": "transport_error",
        "code": "data_source_transport_error",
        "message": error.to_string(),
    })
}

fn native_sql_provider_error_payload(error: &plugin_framework::ProviderRuntimeError) -> Value {
    let code = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("code"))
        .and_then(Value::as_str)
        .or(error.provider_summary.as_deref())
        .unwrap_or("data_source_error");
    let kind = if code == "outcome_unknown" {
        "outcome_unknown"
    } else if error.kind == plugin_framework::ProviderRuntimeErrorKind::ProviderUpstreamError {
        "source_error"
    } else {
        "transport_error"
    };
    let detail = error
        .provider_details
        .as_ref()
        .and_then(|details| details.get("detail"))
        .cloned()
        .or_else(|| error.provider_details.clone());
    json!({
        "kind": kind,
        "code": code,
        "message": error.message,
        "detail": detail,
    })
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::CodeInvoker for RuntimeProviderInvoker<R, H>
where
    R: Clone + Send + Sync,
    H: Clone + Send + Sync,
{
    async fn invoke_code_node(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<orchestration_runtime::execution_engine::CodeInvocationOutput> {
        let input_payload = self.expose_protocol_contexts_to_code(input_payload).await?;
        orchestration_runtime::execution_engine::CodeInvoker::invoke_code_node(
            &orchestration_runtime::execution_engine::QuickJsCodeInvoker::default(),
            runtime,
            config_payload,
            input_payload,
        )
        .await
    }

    async fn protect_protocol_context_output(
        &self,
        output: &mut orchestration_runtime::execution_engine::CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        self.protect_code_protocol_context_output(output, selected_output_paths)
            .await
    }

    async fn protect_protocol_context_logs(
        &self,
        console_logs: &mut Vec<orchestration_runtime::execution_engine::ConsoleLogEntry>,
    ) -> Result<()> {
        self.protect_code_console_logs(console_logs).await
    }
}

mod config;
use config::build_provider_runtime_config;

mod media;
use media::adapt_or_ensure_model_supports_content_blocks;
#[cfg(test)]
pub(super) use media::textualize_media_content_blocks_for_text_model;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/canonical_writer_tests.rs"]
mod canonical_writer_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/provider_invoker/continuation_claim_tests.rs"]
mod continuation_claim_tests;

#[cfg(test)]
#[path = "../_tests/orchestration_runtime/support.rs"]
pub(crate) mod test_support;
