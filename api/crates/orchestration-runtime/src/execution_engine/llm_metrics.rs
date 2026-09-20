use super::*;

pub(super) async fn llm_request_runtimes<I>(
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<Vec<CompiledLlmRuntime>>
where
    I: ProviderInvoker + ?Sized,
{
    let request_count = llm_request_count(node);
    let mut request_runtimes = Vec::with_capacity(request_count);
    for attempt_index in 0..request_count {
        let resolved = resolve_llm_request_runtime(
            runtime,
            runtime_context,
            invoker,
            &BTreeSet::new(),
            None,
            attempt_index,
        )
        .await?;
        request_runtimes.push(resolved.runtime);
    }
    Ok(request_runtimes)
}

pub(crate) struct ResolvedLlmRequestRuntime {
    pub(crate) runtime: CompiledLlmRuntime,
    pub(crate) route: Result<ResolvedProviderRoute>,
    pub(crate) generate_projection_receipt: Option<ProviderGenerateTranslationReceipt>,
    pub(crate) distribution_selection_receipt:
        Option<extension_contracts::ProviderDistributionSelectionReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LlmRoutePreflightCause {
    Unsupported {
        code: ProviderProjectionErrorCode,
        block: ProviderCanonicalBlockLocator,
        receipt: Box<ProviderGenerateTranslationReceipt>,
    },
    InvalidCanonicalContract {
        message: String,
    },
    MissingReasoningCapabilities {
        capabilities: BTreeSet<String>,
        receipt: Box<ProviderGenerateTranslationReceipt>,
    },
}

impl LlmRoutePreflightCause {
    fn missing_capabilities(&self) -> BTreeSet<String> {
        match self {
            Self::Unsupported { receipt, .. } => receipt
                .provenance
                .iter()
                .flat_map(|provenance| provenance.omitted_blocks.iter())
                .filter_map(|block| match block.block_kind {
                    extension_contracts::provider_contract::ProviderCanonicalBlockKind::Reasoning => {
                        Some(
                            ProviderInvocationCapability::MessageBlocksReasoningHistoryV1
                                .manifest_capability_name()
                                .to_string(),
                        )
                    }
                    extension_contracts::provider_contract::ProviderCanonicalBlockKind::RedactedReasoning => {
                        Some(
                            ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1
                                .manifest_capability_name()
                                .to_string(),
                        )
                    }
                    _ => None,
                })
                .collect(),
            Self::MissingReasoningCapabilities { capabilities, .. } => capabilities.clone(),
            Self::InvalidCanonicalContract { .. } => {
                BTreeSet::from(["invalid_canonical_contract".to_string()])
            }
        }
    }

    fn bounded_diagnostic(&self) -> Value {
        match self {
            Self::Unsupported {
                code,
                block,
                receipt,
            } => json!({
                "cause": "unsupported",
                "error_code": code,
                "block": block,
                "receipt": bounded_generate_projection_receipt(receipt),
            }),
            Self::InvalidCanonicalContract { .. } => json!({
                "cause": "invalid_canonical_contract",
            }),
            Self::MissingReasoningCapabilities {
                capabilities,
                receipt,
            } => json!({
                "cause": "missing_capabilities",
                "missing_capabilities": capabilities,
                "receipt": bounded_generate_projection_receipt(receipt),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedLlmRoutePreflight {
    pub(crate) receipt: ProviderGenerateTranslationReceipt,
}

pub(crate) async fn resolve_llm_request_runtime<I>(
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
    canonical_generate_probe: Option<&ProviderInvocationInput>,
    attempt_index: usize,
) -> Result<ResolvedLlmRequestRuntime>
where
    I: ProviderInvoker + ?Sized,
{
    if let Some(main_routing) = invoker.resolve_main_llm_routing(runtime).await? {
        let mut compatible = Vec::with_capacity(main_routing.candidates.len());
        let mut missing = BTreeSet::new();
        let mut projection_causes = Vec::new();
        for candidate in main_routing.candidates {
            match preflight_llm_route_candidate(
                &candidate.route.runtime_capabilities,
                required_capabilities,
                canonical_generate_probe,
            ) {
                Ok(preflight) => compatible.push((candidate, preflight)),
                Err(cause) => {
                    missing.extend(cause.missing_capabilities());
                    projection_causes.push(cause);
                }
            }
        }
        if compatible.is_empty() {
            return Err(semantic_route_error(
                "main_instance",
                &missing,
                &projection_causes,
            ));
        }
        let selection = llm_target_selection(
            &main_routing.distribution_rule,
            main_routing.distribution_key.as_deref(),
            &compatible
                .iter()
                .map(|(candidate, _)| candidate.runtime.provider_instance_id.clone())
                .collect::<Vec<_>>(),
            attempt_index,
            runtime_context,
            invoker,
        )
        .await?;
        let (selected, preflight) = compatible.swap_remove(selection.target_index);
        return Ok(ResolvedLlmRequestRuntime {
            runtime: selected.runtime,
            route: Ok(selected.route),
            generate_projection_receipt: preflight.map(|preflight| preflight.receipt),
            distribution_selection_receipt: Some(selection.receipt),
        });
    }

    let candidates = llm_route_candidate_runtimes(runtime);
    let mut compatible = Vec::with_capacity(candidates.len());
    let mut missing = BTreeSet::new();
    let mut projection_causes = Vec::new();

    for candidate in candidates {
        match invoker.resolve_llm_route(&candidate).await {
            Ok(route) => {
                match preflight_llm_route_candidate(
                    &route.runtime_capabilities,
                    required_capabilities,
                    canonical_generate_probe,
                ) {
                    Ok(preflight) => compatible.push(ResolvedLlmRequestRuntime {
                        runtime: candidate,
                        route: Ok(route),
                        generate_projection_receipt: preflight.map(|preflight| preflight.receipt),
                        distribution_selection_receipt: None,
                    }),
                    Err(cause) => {
                        missing.extend(cause.missing_capabilities());
                        projection_causes.push(cause);
                    }
                }
            }
            Err(error)
                if provider_runtime_error_from_anyhow(&error).kind
                    == ProviderRuntimeErrorKind::ProviderAffinityMismatch =>
            {
                continue;
            }
            Err(error) => compatible.push(ResolvedLlmRequestRuntime {
                runtime: candidate,
                route: Err(error),
                generate_projection_receipt: None,
                distribution_selection_receipt: None,
            }),
        }
    }

    if compatible.is_empty() {
        return Err(semantic_route_error(
            "llm_route",
            &missing,
            &projection_causes,
        ));
    }

    let selection = llm_target_selection(
        runtime
            .routing
            .as_ref()
            .map(|routing| &routing.distribution_rule)
            .unwrap_or(&crate::compiled_plan::LlmDistributionRule::None),
        runtime
            .routing
            .as_ref()
            .and_then(|routing| routing.distribution_key.as_deref()),
        &compatible
            .iter()
            .map(|candidate| candidate.runtime.provider_instance_id.clone())
            .collect::<Vec<_>>(),
        attempt_index,
        runtime_context,
        invoker,
    )
    .await?;
    let mut selected = compatible.swap_remove(selection.target_index);
    selected.distribution_selection_receipt = Some(selection.receipt);
    Ok(selected)
}

pub(crate) fn preflight_llm_route_candidate(
    declared_capabilities: &BTreeSet<String>,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
    canonical_generate_probe: Option<&ProviderInvocationInput>,
) -> std::result::Result<Option<AcceptedLlmRoutePreflight>, LlmRoutePreflightCause> {
    let Some(canonical_generate_probe) = canonical_generate_probe else {
        let capabilities =
            missing_routing_capabilities(declared_capabilities, required_capabilities);
        return if capabilities.is_empty() {
            Ok(None)
        } else {
            Err(LlmRoutePreflightCause::MissingReasoningCapabilities {
                capabilities,
                receipt: Box::default(),
            })
        };
    };
    let declared_capability_list = declared_capabilities.iter().cloned().collect::<Vec<_>>();
    let projection = canonical_generate_probe
        .project_current_provider_generate(&declared_capability_list)
        .map_err(|error| match error {
            ProviderGenerateProjectionError::Unsupported {
                code,
                block,
                receipt,
            } => LlmRoutePreflightCause::Unsupported {
                code,
                block,
                receipt: Box::new(receipt),
            },
            ProviderGenerateProjectionError::InvalidContract { message } => {
                LlmRoutePreflightCause::InvalidCanonicalContract { message }
            }
        })?;
    let capabilities = missing_routing_capabilities(
        declared_capabilities,
        &projection.provider_bound_input.required_capabilities,
    );
    if capabilities.is_empty() {
        Ok(Some(AcceptedLlmRoutePreflight {
            receipt: projection.receipt,
        }))
    } else {
        Err(LlmRoutePreflightCause::MissingReasoningCapabilities {
            capabilities,
            receipt: Box::new(projection.receipt),
        })
    }
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
    if routing.routing_mode != LlmRoutingMode::FailoverQueue
        || routing.queue_template_id.is_none()
        || routing.queue_targets.is_empty()
    {
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

async fn llm_target_selection(
    distribution_rule: &crate::compiled_plan::LlmDistributionRule,
    distribution_key: Option<&str>,
    target_ids: &[String],
    attempt_index: usize,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &(impl ProviderInvoker + ?Sized),
) -> Result<super::provider_routing::ProviderDistributionSelection> {
    super::provider_routing::select_provider_target(
        distribution_rule,
        distribution_key,
        target_ids,
        attempt_index,
        runtime_context,
        invoker,
    )
    .await
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
                    | ProviderInvocationCapability::NativeContinuationSupported
            )
        })
        .map(|capability| capability.manifest_capability_name().to_string())
        .filter(|capability| !declared_capabilities.contains(capability))
        .collect()
}

const MAX_PROJECTION_LOCATORS: usize = 16;
const MAX_ROUTE_PROJECTION_CAUSES: usize = 8;

pub(crate) fn bounded_generate_projection_receipt(
    receipt: &ProviderGenerateTranslationReceipt,
) -> Value {
    let provenance = receipt.provenance.as_ref().map(|provenance| {
        json!({
            "source": provenance.source,
            "preserved_count": provenance.preserved_block_count,
            "omitted_count": provenance.omitted_block_count,
            "locators_capped": provenance.capped,
            "preserved_blocks": provenance.preserved_blocks
                .iter()
                .take(MAX_PROJECTION_LOCATORS)
                .collect::<Vec<_>>(),
            "omitted_blocks": provenance.omitted_blocks
                .iter()
                .take(MAX_PROJECTION_LOCATORS)
                .collect::<Vec<_>>(),
        })
    });
    json!({
        "fidelity": receipt.fidelity,
        "loss_codes": &receipt.loss_codes,
        "error_code": receipt.error_code,
        "provenance": provenance,
    })
}

fn semantic_route_error(
    route_id: &str,
    missing: &BTreeSet<String>,
    causes: &[LlmRoutePreflightCause],
) -> anyhow::Error {
    let cause_count = causes.len();
    extension_contracts::ExtensionContractError::runtime(
        ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::SemanticCapabilityUnsupported,
            "no LLM route accepts the request's canonical message-block semantics",
        )
        .with_provider_details(json!({
            "route_id": route_id,
            "missing_capabilities": missing,
            "projection": {
                "cause_count": cause_count,
                "causes_capped": cause_count > MAX_ROUTE_PROJECTION_CAUSES,
                "causes": causes.iter()
                    .take(MAX_ROUTE_PROJECTION_CAUSES)
                    .map(LlmRoutePreflightCause::bounded_diagnostic)
                    .collect::<Vec<_>>(),
            },
        })),
    )
    .into()
}

pub(super) struct AttemptMetricInput<'a> {
    pub(super) attempt_index: usize,
    pub(super) retry_reason: Option<&'a str>,
    pub(super) runtime: &'a CompiledLlmRuntime,
    pub(super) plugin_id: Option<&'a str>,
    pub(super) reasoning_effort: Option<&'a str>,
    pub(super) status: &'a str,
    pub(super) failed_after_first_token: bool,
    pub(super) error_payload: Option<&'a Value>,
    pub(super) generate_projection_receipt: Option<&'a ProviderGenerateTranslationReceipt>,
    pub(super) usage: &'a ProviderUsage,
    pub(super) event_count: usize,
    pub(super) started_at: OffsetDateTime,
    pub(super) first_token_at: Option<OffsetDateTime>,
    pub(super) finished_at: OffsetDateTime,
    pub(super) time_to_first_token_ms: Option<u64>,
}

pub(super) fn build_attempt_metric(input: AttemptMetricInput<'_>) -> Value {
    let mut attempt = json!({
        "attempt_index": input.attempt_index,
        "is_retry": input.attempt_index > 0,
        "retry_reason": input.retry_reason,
        "provider_instance_id": input.runtime.provider_instance_id,
        "provider_instance_display_name": input.runtime.provider_instance_display_name,
        "provider_code": input.runtime.provider_code,
        "plugin_id": input.plugin_id,
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
    });
    for key in [
        "1flowbase_provider_recovery_diagnostics",
        "1flowbase_provider_recovery",
    ] {
        if let Some(value) = input.error_payload.and_then(|payload| payload.get(key)) {
            // Already bounded by the provider-error projection owner.
            attempt[key] = value.clone();
        }
    }
    attach_generate_projection_receipt(&mut attempt, input.generate_projection_receipt);
    attempt
}

pub(crate) fn attach_generate_projection_receipt(
    attempt: &mut Value,
    receipt: Option<&ProviderGenerateTranslationReceipt>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    attempt.insert(
        "provider_generate_projection".to_string(),
        receipt
            .map(bounded_generate_projection_receipt)
            .unwrap_or(Value::Null),
    );
}

pub(super) fn attach_distribution_selection_receipt(
    attempt: &mut Value,
    receipt: Option<&extension_contracts::ProviderDistributionSelectionReceipt>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    attempt.insert(
        "provider_distribution_selection".to_string(),
        receipt
            .and_then(|receipt| serde_json::to_value(receipt).ok())
            .unwrap_or(Value::Null),
    );
}

pub(super) fn attach_provider_stream_timing(attempt: &mut Value, timing: Option<&Value>) {
    let (Some(attempt), Some(timing)) = (attempt.as_object_mut(), timing) else {
        return;
    };
    attempt.insert("provider_stream_timing".to_string(), timing.clone());
}

const RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY: &str = "1flowbase_runtime_provider_stages";
const GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY: &str = "1flowbase_gateway_provider_stages";

fn bounded_stage_value(value: Option<&Value>, field: &str) -> Option<u64> {
    value
        .filter(|receipt| receipt.get("schema_version").and_then(Value::as_u64) == Some(1))
        .and_then(|receipt| receipt.get(field))
        .and_then(Value::as_u64)
        .filter(|duration| *duration <= 24 * 60 * 60 * 1_000)
}

fn stage_receipt(owner: &str, duration_ms: Option<u64>, unavailable: &str) -> Value {
    json!({
        "owner": owner,
        "duration_ms": duration_ms,
        "availability": if duration_ms.is_some() { "observed" } else { "unavailable" },
        "unavailable_reason": duration_ms.is_none().then_some(unavailable),
    })
}

pub(super) fn attach_provider_timing_receipt(
    attempt: &mut Value,
    observability: &ProviderObservabilityMetadata,
    attempt_index: usize,
    attempt_status: &str,
    error_payload: Option<&Value>,
) {
    let Some(attempt) = attempt.as_object_mut() else {
        return;
    };
    let runtime = observability.runtime_stages.as_ref();
    let gateway = observability.gateway_stages.as_ref();
    let provider = observability.provider_timing.as_ref();
    let connection = observability.transport_session.as_ref();
    let connect_ms = provider.and_then(|receipt| receipt.connect_ms);
    let termination_kind = provider
        .map(|receipt| match receipt.termination_kind {
            extension_contracts::ProviderInvocationTerminationKind::Completed => "completed",
            extension_contracts::ProviderInvocationTerminationKind::UpstreamError => {
                "upstream_error"
            }
            extension_contracts::ProviderInvocationTerminationKind::TransportError => {
                "transport_error"
            }
            extension_contracts::ProviderInvocationTerminationKind::Deadline => "deadline",
        })
        .unwrap_or_else(|| {
            match error_payload
                .and_then(|payload| payload.get("error_code"))
                .and_then(Value::as_str)
            {
                Some("provider_transport_unavailable" | "endpoint_unreachable") => {
                    "transport_error"
                }
                Some("deadline_exceeded" | "provider_invocation_timeout") => "deadline",
                Some("empty_response") => "empty_response",
                Some(_) => "upstream_error",
                None if attempt_status == "succeeded" => "completed",
                None => attempt_status,
            }
        });
    let connect_unavailable = if connection.is_some_and(|receipt| receipt.reused) {
        "reused_connection_no_connect"
    } else {
        "provider_receipt_unavailable"
    };
    let connection = connection.map(|receipt| {
        json!({
            "cold": !receipt.reused,
            "reused": receipt.reused,
            "generation": receipt.generation,
            "connection_age_ms": receipt.connection_age_ms,
            "ttl_remaining_ms": receipt.ttl_remaining_ms,
            "physical_state": receipt.physical_state,
            "close_reason": receipt.close_reason,
            "close_acknowledged": receipt.close_acknowledged,
        })
    });
    attempt.insert(
        "provider_timing_receipt".to_string(),
        json!({
            "schema_version": 1,
            "attempt_index": attempt_index,
            "stages": {
                "ingress": stage_receipt("ai_native", bounded_stage_value(gateway, "ingress_ms"), "no_provider_event_ingress_observed"),
                "mapping": stage_receipt("runtime_host", bounded_stage_value(runtime, "mapping_ms"), "runtime_receipt_unavailable"),
                "flow": stage_receipt("ai_native", bounded_stage_value(gateway, "flow_ms"), "gateway_receipt_unavailable"),
                "queue": stage_receipt("runtime_host", bounded_stage_value(runtime, "queue_ms"), "runtime_receipt_unavailable"),
                "connect": stage_receipt("provider", connect_ms, connect_unavailable),
                "upstream": stage_receipt("provider", provider.map(|receipt| receipt.upstream_ms), "provider_receipt_unavailable"),
                "flush": stage_receipt("ai_native", bounded_stage_value(gateway, "flush_ms"), "no_stream_event_flush_observed"),
            },
            "connection": connection,
            "termination_kind": termination_kind,
        }),
    );
}

#[derive(Default)]
pub(super) struct ProviderObservabilityMetadata {
    pub(super) user_account: Option<Value>,
    pub(super) stream_timing: Option<Value>,
    pub(super) billing: Option<Value>,
    pub(super) provider_timing: Option<extension_contracts::ProviderInvocationTimingReceipt>,
    pub(super) transport_session: Option<extension_contracts::ProviderTransportSessionReceipt>,
    pub(super) runtime_stages: Option<Value>,
    pub(super) gateway_stages: Option<Value>,
}

pub(super) fn attach_provider_billing(attempt: &mut Value, billing: Option<&Value>) {
    let (Some(attempt), Some(billing)) = (attempt.as_object_mut(), billing) else {
        return;
    };
    attempt.insert("billing".to_string(), billing.clone());
}

pub(super) fn take_provider_observability_metadata(
    result: &mut ProviderInvocationResult,
) -> ProviderObservabilityMetadata {
    let mut extracted = ProviderObservabilityMetadata::default();
    while let Some(metadata) = result.provider_metadata.as_object_mut() {
        if let Some(value) =
            metadata.remove(extension_contracts::PROVIDER_INVOCATION_TIMING_RECEIPT_METADATA_KEY)
        {
            extracted.provider_timing = serde_json::from_value(value)
                .ok()
                .filter(
                    |receipt: &extension_contracts::ProviderInvocationTimingReceipt| {
                        receipt.validate().is_ok()
                    },
                )
                .or(extracted.provider_timing);
        }
        if let Some(value) =
            metadata.remove(extension_contracts::PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY)
        {
            extracted.transport_session = serde_json::from_value(value)
                .ok()
                .filter(
                    |receipt: &extension_contracts::ProviderTransportSessionReceipt| {
                        receipt.validate().is_ok()
                    },
                )
                .or(extracted.transport_session);
        }
        extracted.runtime_stages = metadata
            .remove(RUNTIME_PROVIDER_STAGE_TIMING_METADATA_KEY)
            .or(extracted.runtime_stages);
        extracted.gateway_stages = metadata
            .remove(GATEWAY_PROVIDER_STAGE_TIMING_METADATA_KEY)
            .or(extracted.gateway_stages);
        let is_wrapper = metadata.contains_key("_1flowbase_upstream_provider_metadata")
            && (metadata.contains_key("_1flowbase_runtime_stream_timing")
                || metadata.contains_key("_1flowbase_billing")
                || metadata.contains_key("_1flowbase_user_account"));
        if !is_wrapper {
            break;
        }
        extracted.stream_timing = metadata
            .remove("_1flowbase_runtime_stream_timing")
            .or(extracted.stream_timing);
        extracted.billing = metadata.remove("_1flowbase_billing").or(extracted.billing);
        extracted.user_account = metadata
            .remove("_1flowbase_user_account")
            .or(extracted.user_account);
        let Some(upstream_metadata) = metadata.remove("_1flowbase_upstream_provider_metadata")
        else {
            break;
        };
        result.provider_metadata = upstream_metadata;
    }
    extracted
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
            "distribution_rule": routing.distribution_rule,
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

        let receipt = take_provider_observability_metadata(&mut result).stream_timing;

        assert_eq!(receipt, Some(timing));
        assert_eq!(result.provider_metadata, upstream);
    }

    #[test]
    fn provider_observability_wrapper_extracts_billing_and_restores_upstream_metadata() {
        let upstream = json!({"response_id": "response-1"});
        let billing = json!({
            "pricing_provider_code": "openai",
            "pricing_model_id": "gpt-x",
            "total_cost": "0.00125",
            "currency_code": "USD"
        });
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "_1flowbase_billing": billing,
                "_1flowbase_user_account": "billing-user",
                "_1flowbase_upstream_provider_metadata": upstream
            }),
            ..ProviderInvocationResult::default()
        };

        let observability = take_provider_observability_metadata(&mut result);

        assert_eq!(observability.billing, Some(billing));
        assert_eq!(observability.user_account, Some(json!("billing-user")));
        assert_eq!(result.provider_metadata, upstream);
    }

    #[test]
    fn c2_clocked_receipts_project_safe_complete_stage_schema() {
        let mut result = ProviderInvocationResult {
            provider_metadata: json!({
                "1flowbase_gateway_provider_stages": {
                    "schema_version": 1,
                    "ingress_ms": 37,
                    "flow_ms": 11,
                    "flush_ms": 2
                },
                "1flowbase_runtime_provider_stages": {
                    "schema_version": 1,
                    "mapping_ms": 3,
                    "queue_ms": 5
                },
                "1flowbase_provider_invocation_timing": {
                    "schema_version": 1,
                    "connect_ms": 17,
                    "upstream_ms": 241,
                    "termination_kind": "completed"
                },
                "1flowbase_physical_transport_session": {
                    "generation": 7,
                    "reused": false,
                    "physical_state": "ready",
                    "connection_age_ms": 260,
                    "ttl_remaining_ms": 3_000,
                },
                "response_id": "must-remain-upstream-only"
            }),
            ..ProviderInvocationResult::default()
        };
        let observability = take_provider_observability_metadata(&mut result);
        let mut attempt = json!({"attempt_index": 2});

        attach_provider_timing_receipt(&mut attempt, &observability, 2, "succeeded", None);

        let receipt = &attempt["provider_timing_receipt"];
        assert_eq!(receipt["attempt_index"], 2);
        assert_eq!(receipt["stages"]["ingress"]["duration_ms"], 37);
        assert_eq!(receipt["stages"]["mapping"]["duration_ms"], 3);
        assert_eq!(receipt["stages"]["flow"]["duration_ms"], 11);
        assert_eq!(receipt["stages"]["queue"]["duration_ms"], 5);
        assert_eq!(receipt["stages"]["connect"]["duration_ms"], 17);
        assert_eq!(receipt["stages"]["upstream"]["duration_ms"], 241);
        assert_eq!(receipt["stages"]["flush"]["duration_ms"], 2);
        assert_eq!(receipt["connection"]["cold"], true);
        assert_eq!(receipt["connection"]["generation"], 7);
        assert_eq!(receipt["termination_kind"], "completed");
        let serialized = serde_json::to_string(receipt).unwrap();
        for forbidden in [
            "credential",
            "api_key",
            "prompt",
            "tool_output",
            "encrypted_content",
            "cursor",
            "response_id",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert_eq!(
            result.provider_metadata["response_id"],
            "must-remain-upstream-only"
        );
    }

    #[test]
    fn c2_failed_attempt_derives_transport_termination_without_fabricating_stages() {
        let observability = ProviderObservabilityMetadata::default();
        let mut attempt = json!({"attempt_index": 1});

        attach_provider_timing_receipt(
            &mut attempt,
            &observability,
            1,
            "failed",
            Some(&json!({"error_code": "provider_transport_unavailable"})),
        );

        let receipt = &attempt["provider_timing_receipt"];
        assert_eq!(receipt["termination_kind"], "transport_error");
        assert_eq!(receipt["connection"], Value::Null);
        assert_eq!(receipt["stages"]["connect"]["duration_ms"], Value::Null);
        assert_eq!(receipt["stages"]["connect"]["availability"], "unavailable");
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

    struct DynamicMainResolver;

    #[async_trait]
    impl ProviderInvoker for DynamicMainResolver {
        async fn resolve_main_llm_routing(
            &self,
            runtime: &CompiledLlmRuntime,
        ) -> Result<Option<ResolvedMainLlmRouting>> {
            let candidates = ["provider-current-a", "provider-current-b"]
                .into_iter()
                .map(|provider_instance_id| ResolvedMainLlmRouteCandidate {
                    runtime: CompiledLlmRuntime {
                        provider_instance_id: provider_instance_id.to_string(),
                        provider_instance_display_name: String::new(),
                        provider_code: runtime.provider_code.clone(),
                        protocol: "openai_compatible".to_string(),
                        model: runtime.model.clone(),
                        routing: None,
                    },
                    route: ResolvedProviderRoute::new(BTreeSet::new(), ()),
                })
                .collect();
            Ok(Some(ResolvedMainLlmRouting {
                candidates,
                distribution_rule: crate::compiled_plan::LlmDistributionRule::RetryRoundRobin,
                distribution_key: None,
            }))
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
    async fn main_instance_distribution_is_resolved_live_for_each_attempt() {
        let runtime = CompiledLlmRuntime {
            provider_instance_id: String::new(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: String::new(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        };

        let selected = resolve_llm_request_runtime(
            &runtime,
            &ExecutionRuntimeContext::default(),
            &DynamicMainResolver,
            &BTreeSet::new(),
            None,
            1,
        )
        .await
        .expect("the current main distribution should select the retry target");

        assert_eq!(selected.runtime.provider_instance_id, "provider-current-b");
    }

    #[tokio::test]
    async fn count_tokens_and_compact_runtime_selection_uses_current_main_instance() {
        let runtime = CompiledLlmRuntime {
            provider_instance_id: String::new(),
            provider_instance_display_name: String::new(),
            provider_code: "fixture_provider".to_string(),
            protocol: String::new(),
            model: "gpt-5.4-mini".to_string(),
            routing: None,
        };
        let node = CompiledNode {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            alias: "llm".to_string(),
            container_id: None,
            dependency_node_ids: Vec::new(),
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: Some(runtime.clone()),
            code_runtime: None,
        };

        let selected = llm_request_runtimes(
            &node,
            &runtime,
            &ExecutionRuntimeContext::default(),
            &DynamicMainResolver,
        )
        .await
        .expect("native LLM operations should resolve the current main instance");

        assert_eq!(selected[0].provider_instance_id, "provider-current-a");
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
            None,
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
            None,
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
            None,
            0,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("a stale compiled capability claim must not authorize invocation"),
        };
        let framework_error = error
            .downcast_ref::<extension_contracts::ExtensionContractError>()
            .expect("semantic rejection should preserve the typed framework error");

        assert!(matches!(
            framework_error,
            extension_contracts::ExtensionContractError::RuntimeContract { error }
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
                None,
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
            None,
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
            None,
            0,
        )
        .await
        {
            Err(error) => error,
            Ok(_) => panic!("redacted reasoning must not route to an incompatible Provider"),
        };
        let framework_error = error
            .downcast_ref::<extension_contracts::ExtensionContractError>()
            .expect("semantic rejection should preserve the typed framework error");

        assert!(matches!(
            framework_error,
            extension_contracts::ExtensionContractError::RuntimeContract { error }
                if error.kind == ProviderRuntimeErrorKind::SemanticCapabilityUnsupported
        ));
    }
}
