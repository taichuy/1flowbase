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
            &node.node_id,
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
    node_id: &str,
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
    if let Some(main_routing) = invoker.resolve_main_llm_routing(runtime, node_id).await? {
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
        let selection = super::provider_routing::select_session_provider_target(
            runtime,
            node_id,
            super::provider_routing::ProviderDistributionPolicy {
                rule: &main_routing.distribution_rule,
                key: main_routing.distribution_key.as_deref(),
            },
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

    let mut candidates = llm_route_candidate_runtimes(runtime);
    let default_rule = runtime
        .routing
        .as_ref()
        .map(|routing| {
            matches!(
                routing.distribution_rule,
                crate::compiled_plan::LlmDistributionRule::None
            )
        })
        .unwrap_or(true);
    if default_rule {
        if let Some(owner) = invoker.get_session_llm_provider(runtime, node_id).await? {
            candidates.retain(|candidate| candidate.provider_instance_id == owner);
            if candidates.is_empty() {
                return Err(extension_contracts::ExtensionContractError::runtime(
                    ProviderRuntimeError::new(
                        ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                        "the session's original Provider instance is no longer eligible",
                    ),
                )
                .into());
            }
        }
    }
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

    let selection = super::provider_routing::select_session_provider_target(
        runtime,
        node_id,
        super::provider_routing::ProviderDistributionPolicy {
            rule: runtime
                .routing
                .as_ref()
                .map(|routing| &routing.distribution_rule)
                .unwrap_or(&crate::compiled_plan::LlmDistributionRule::None),
            key: runtime
                .routing
                .as_ref()
                .and_then(|routing| routing.distribution_key.as_deref()),
        },
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

const MAX_ROUTE_PROJECTION_CAUSES: usize = 8;

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
