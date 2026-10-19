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
        _node_id: &str,
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
        "test-node",
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
        "test-node",
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
        "test-node",
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
        "test-node",
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
    let required = BTreeSet::from([ProviderInvocationCapability::MessageBlocksReasoningHistoryV1]);

    assert!(
        resolve_llm_request_runtime(
            "test-node",
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
        "test-node",
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
        "test-node",
        &runtime,
        &ExecutionRuntimeContext::default(),
        &resolver,
        &BTreeSet::from([ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1]),
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
