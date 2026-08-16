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
            preflight_llm_route_candidate, resolve_llm_request_runtime, LlmRoutePreflightCause,
        },
        ExecutionRuntimeContext, ProviderInvocationOutput, ProviderInvoker, ResolvedProviderRoute,
    },
};

struct Issue1743CapabilityResolver {
    capabilities: BTreeMap<String, BTreeSet<String>>,
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
