use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::provider_contract::{
    ProviderInvocationInput, ProviderMessage, ProviderMessageRole, ProviderProjectionErrorCode,
    ProviderProjectionFidelity,
};
use serde_json::{json, Value};

use crate::{
    compiled_plan::{
        CompiledLlmRouteTarget, CompiledLlmRouting, CompiledLlmRuntime, LlmDistributionRule,
        LlmRoutingMode,
    },
    execution_engine::{
        llm_metrics::{
            attach_generate_projection_receipt, bounded_generate_projection_receipt,
            preflight_llm_route_candidate, resolve_llm_request_runtime, LlmRoutePreflightCause,
        },
        ExecutionRuntimeContext, ProviderInvocationOutput, ProviderInvoker, ResolvedProviderRoute,
    },
};

struct Issue1743CapabilityResolver {
    capabilities: BTreeMap<String, BTreeSet<String>>,
}

struct Issue1743AffinityResolver;

#[async_trait]
impl ProviderInvoker for Issue1743AffinityResolver {
    async fn resolve_llm_route(
        &self,
        runtime: &CompiledLlmRuntime,
    ) -> Result<ResolvedProviderRoute> {
        if runtime.provider_instance_id == "provider-wrong-affinity" {
            return Err(plugin_framework::PluginFrameworkError::runtime(
                plugin_framework::provider_contract::ProviderRuntimeError::new(
                    plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderAffinityMismatch,
                    "typed affinity mismatch fixture",
                ),
            )
            .into());
        }
        Ok(ResolvedProviderRoute::new(
            BTreeSet::from(["native_continuation_supported".to_string()]),
            runtime.provider_instance_id.clone(),
        ))
    }

    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("issue_1743 route projection fixtures do not invoke a Provider")
    }
}

#[async_trait]
impl ProviderInvoker for Issue1743CapabilityResolver {
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
        unreachable!("issue_1743 route projection fixtures do not invoke a Provider")
    }
}

fn issue_1743_canonical_probe(content: &str, content_blocks: Value) -> ProviderInvocationInput {
    let mut input = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: content.to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(content_blocks),
        }],
        ..ProviderInvocationInput::default()
    };
    input
        .synchronize_required_capabilities()
        .expect("issue_1743 canonical probe must be valid");
    input
}

fn issue_1743_target(id: &str) -> CompiledLlmRouteTarget {
    CompiledLlmRouteTarget {
        provider_instance_id: id.to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "synthetic".to_string(),
        upstream_model_id: format!("{id}-model"),
    }
}

fn issue_1743_failover_runtime(targets: Vec<CompiledLlmRouteTarget>) -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: "provider-template".to_string(),
        provider_instance_display_name: String::new(),
        provider_code: "fixture_provider".to_string(),
        protocol: "synthetic".to_string(),
        model: "template-model".to_string(),
        routing: Some(CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: Some("issue-1743-queue".to_string()),
            queue_snapshot_id: Some("issue-1743-snapshot".to_string()),
            queue_targets: targets,
            distribution_rule: LlmDistributionRule::RetryRoundRobin,
            distribution_key: None,
            context_policy: json!({}),
            stream_policy: json!({}),
        }),
    }
}

#[test]
fn issue_1743_lossy_candidate_is_accepted_without_mutating_canonical_probe() {
    let canonical = issue_1743_canonical_probe(
        "visible answer",
        json!([
            {"type": "reasoning", "text": "private reasoning"},
            {"type": "text", "text": "visible answer"}
        ]),
    );
    let snapshot = canonical.clone();

    let accepted = preflight_llm_route_candidate(
        &BTreeSet::new(),
        &canonical.required_capabilities,
        Some(&canonical),
    )
    .expect("ordinary visible text permits lossy reasoning projection")
    .expect("Generate preflight must carry a receipt");

    assert_eq!(
        accepted.receipt.fidelity,
        Some(ProviderProjectionFidelity::Lossy)
    );
    let diagnostic = bounded_generate_projection_receipt(&accepted.receipt);
    assert_eq!(diagnostic["fidelity"], json!("lossy"));
    assert_eq!(diagnostic["provenance"]["omitted_count"], json!(1));
    assert_eq!(
        diagnostic["provenance"]["omitted_blocks"][0],
        json!({"message_index": 0, "block_index": 0, "block_kind": "reasoning"})
    );
    let mut attempt = json!({"status": "succeeded"});
    attach_generate_projection_receipt(&mut attempt, Some(&accepted.receipt));
    assert_eq!(attempt["provider_generate_projection"], diagnostic);
    let mut oversized = accepted.receipt.clone();
    let omitted_locator = oversized
        .provenance
        .as_ref()
        .expect("lossy receipt must carry provenance")
        .omitted_blocks[0]
        .clone();
    oversized
        .provenance
        .as_mut()
        .expect("lossy receipt must carry provenance")
        .omitted_blocks = vec![omitted_locator; 20];
    let oversized_provenance = oversized
        .provenance
        .as_mut()
        .expect("lossy receipt must carry provenance");
    oversized_provenance.omitted_block_count = 20;
    oversized_provenance.capped = true;
    let bounded = bounded_generate_projection_receipt(&oversized);
    assert_eq!(bounded["provenance"]["omitted_count"], json!(20));
    assert_eq!(
        bounded["provenance"]["omitted_blocks"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(bounded["provenance"]["locators_capped"], json!(true));
    assert_eq!(canonical, snapshot);
}

#[test]
fn issue_1743_reasoning_only_candidate_is_rejected_with_typed_cause() {
    let canonical = issue_1743_canonical_probe(
        "",
        json!([{"type": "reasoning", "text": "private reasoning"}]),
    );

    let cause = preflight_llm_route_candidate(
        &BTreeSet::new(),
        &canonical.required_capabilities,
        Some(&canonical),
    )
    .expect_err("reasoning-only history cannot be erased during routing");

    assert!(matches!(
        cause,
        LlmRoutePreflightCause::Unsupported {
            code: ProviderProjectionErrorCode::ReasoningOnlyMessageUnsupported,
            receipt,
            ..
        } if receipt.error_code == Some(ProviderProjectionErrorCode::ReasoningOnlyMessageUnsupported)
    ));
}

#[test]
fn issue_1743_exact_synthetic_candidate_accepts_explicit_reasoning_block_capabilities() {
    let canonical = issue_1743_canonical_probe(
        "visible answer",
        json!([
            {"type": "reasoning", "text": "private reasoning"},
            {"type": "reasoning_redacted", "data": "opaque-redaction"},
            {"type": "text", "text": "visible answer"}
        ]),
    );
    let declared = BTreeSet::from([
        "message_blocks.reasoning_history.v1".to_string(),
        "message_blocks.redacted_reasoning_history.v1".to_string(),
    ]);

    let accepted = preflight_llm_route_candidate(
        &declared,
        &canonical.required_capabilities,
        Some(&canonical),
    )
    .expect("explicit reasoning block capabilities must preserve the canonical history")
    .expect("Generate preflight must carry a receipt");

    assert_eq!(
        accepted.receipt.fidelity,
        Some(ProviderProjectionFidelity::Exact)
    );
    let diagnostic = bounded_generate_projection_receipt(&accepted.receipt);
    assert_eq!(diagnostic["fidelity"], json!("exact"));
    assert_eq!(diagnostic["provenance"]["preserved_count"], json!(3));
    let mut attempt = json!({"status": "succeeded"});
    attach_generate_projection_receipt(&mut attempt, Some(&accepted.receipt));
    assert_eq!(attempt["provider_generate_projection"], diagnostic);
}

#[tokio::test]
async fn issue_1743_two_candidate_failover_preflights_from_one_canonical_source() {
    let runtime = issue_1743_failover_runtime(vec![
        issue_1743_target("provider-lossy"),
        issue_1743_target("provider-exact"),
    ]);
    let resolver = Issue1743CapabilityResolver {
        capabilities: BTreeMap::from([(
            "provider-exact".to_string(),
            BTreeSet::from(["message_blocks.reasoning_history.v1".to_string()]),
        )]),
    };
    let canonical = issue_1743_canonical_probe(
        "",
        json!([{"type": "reasoning", "text": "private reasoning"}]),
    );
    let snapshot = canonical.clone();

    let selected = resolve_llm_request_runtime(
        &runtime,
        &ExecutionRuntimeContext::default(),
        &resolver,
        &canonical.required_capabilities,
        Some(&canonical),
        0,
    )
    .await
    .expect("the exact backup remains eligible after the lossy route rejects reasoning-only");

    assert_eq!(selected.runtime.provider_instance_id, "provider-exact");
    assert_eq!(
        selected
            .generate_projection_receipt
            .expect("selected Generate route must retain its typed receipt")
            .fidelity,
        Some(ProviderProjectionFidelity::Exact)
    );
    assert_eq!(canonical, snapshot);
}

#[tokio::test]
async fn issue_1743_typed_affinity_mismatch_is_ineligible_but_matching_backup_routes() {
    let runtime = issue_1743_failover_runtime(vec![
        issue_1743_target("provider-wrong-affinity"),
        issue_1743_target("provider-affinity-owner"),
    ]);
    let mut canonical = issue_1743_canonical_probe(
        "tool delta",
        json!([{"type": "text", "text": "tool delta"}]),
    );
    canonical.required_capabilities.insert(
        plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
    );

    let selected = resolve_llm_request_runtime(
        &runtime,
        &ExecutionRuntimeContext::default(),
        &Issue1743AffinityResolver,
        &canonical.required_capabilities,
        Some(&canonical),
        0,
    )
    .await
    .expect("typed affinity mismatch must be skipped in favor of the legal owner");

    assert_eq!(
        selected.runtime.provider_instance_id,
        "provider-affinity-owner"
    );
}

#[tokio::test]
async fn issue_1743_native_continuation_capability_is_a_hard_route_requirement() {
    let runtime = issue_1743_failover_runtime(vec![issue_1743_target("provider-no-native")]);
    let resolver = Issue1743CapabilityResolver {
        capabilities: BTreeMap::new(),
    };
    let mut canonical = issue_1743_canonical_probe(
        "tool delta",
        json!([{"type": "text", "text": "tool delta"}]),
    );
    canonical.required_capabilities.insert(
        plugin_framework::provider_contract::ProviderInvocationCapability::NativeContinuationSupported,
    );

    let error = match resolve_llm_request_runtime(
        &runtime,
        &ExecutionRuntimeContext::default(),
        &resolver,
        &canonical.required_capabilities,
        Some(&canonical),
        0,
    )
    .await
    {
        Ok(_) => panic!("a route without native continuation capability must be ineligible"),
        Err(error) => error,
    };

    let runtime_error = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .and_then(|error| match error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => Some(error),
            _ => None,
        })
        .expect("hard capability rejection must remain typed");
    assert_eq!(
        runtime_error.kind,
        plugin_framework::provider_contract::ProviderRuntimeErrorKind::SemanticCapabilityUnsupported
    );
}

#[tokio::test]
async fn issue_1743_reasoning_only_route_error_exposes_only_bounded_typed_projection_cause() {
    let runtime = issue_1743_failover_runtime(vec![issue_1743_target("provider-no-reasoning")]);
    let resolver = Issue1743CapabilityResolver {
        capabilities: BTreeMap::new(),
    };
    let mut canonical = issue_1743_canonical_probe(
        "",
        json!([
            {
                "type": "reasoning",
                "text": "RAW_BODY_CANARY",
                "signature": "SIGNATURE_CANARY"
            },
            {
                "type": "reasoning_redacted",
                "data": "REDACTED_DATA_CANARY"
            }
        ]),
    );
    canonical
        .run_context
        .insert("cursor".to_string(), json!("CURSOR_CANARY"));

    let error = match resolve_llm_request_runtime(
        &runtime,
        &ExecutionRuntimeContext::default(),
        &resolver,
        &canonical.required_capabilities,
        Some(&canonical),
        0,
    )
    .await
    {
        Ok(_) => panic!("reasoning-only route must fail closed"),
        Err(error) => error,
    };
    let runtime_error = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .and_then(|error| match error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => Some(error),
            _ => None,
        })
        .expect("semantic rejection must remain typed");
    let details = runtime_error
        .provider_details
        .as_ref()
        .expect("semantic rejection must carry safe route diagnostics");

    assert_eq!(details["route_id"], json!("llm_route"));
    assert_eq!(
        details["projection"]["causes"][0]["error_code"],
        json!("reasoning_only_message_unsupported")
    );
    assert_eq!(
        details["projection"]["causes"][0]["block"],
        json!({"message_index": 0, "block_index": 0, "block_kind": "reasoning"})
    );
    let encoded = details.to_string();
    for canary in [
        "RAW_BODY_CANARY",
        "SIGNATURE_CANARY",
        "REDACTED_DATA_CANARY",
        "CURSOR_CANARY",
    ] {
        assert!(!encoded.contains(canary));
    }
}

#[tokio::test]
async fn issue_1743_invalid_canonical_route_error_retains_typed_nonempty_cause() {
    let runtime = issue_1743_failover_runtime(vec![issue_1743_target("provider-invalid")]);
    let resolver = Issue1743CapabilityResolver {
        capabilities: BTreeMap::new(),
    };
    let canonical = ProviderInvocationInput {
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::Assistant,
            content: String::new(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(
                json!([{"type": "unknown_private_block", "data": "RAW_BODY_CANARY"}]),
            ),
        }],
        ..ProviderInvocationInput::default()
    };

    let error = match resolve_llm_request_runtime(
        &runtime,
        &ExecutionRuntimeContext::default(),
        &resolver,
        &BTreeSet::new(),
        Some(&canonical),
        0,
    )
    .await
    {
        Ok(_) => panic!("invalid canonical contract must reject every route"),
        Err(error) => error,
    };
    let details = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .and_then(|error| match error {
            plugin_framework::PluginFrameworkError::RuntimeContract { error } => {
                error.provider_details.as_ref()
            }
            _ => None,
        })
        .expect("invalid canonical rejection must retain typed details");

    assert_eq!(
        details["missing_capabilities"],
        json!(["invalid_canonical_contract"])
    );
    assert_eq!(
        details["projection"]["causes"][0]["cause"],
        json!("invalid_canonical_contract")
    );
    assert!(!details.to_string().contains("RAW_BODY_CANARY"));
}
