use super::*;

#[derive(Clone, Default)]
struct SessionInvoker {
    owner: Arc<Mutex<Option<String>>>,
    calls: Arc<Mutex<Vec<String>>>,
    fail: bool,
    barrier: Option<Arc<tokio::sync::Barrier>>,
}

impl_noop_code_invoker!(SessionInvoker);

#[async_trait]
impl ProviderInvoker for SessionInvoker {
    async fn get_session_llm_provider(
        &self,
        _: &CompiledLlmRuntime,
        _: &str,
    ) -> Result<Option<String>> {
        Ok(self.owner.lock().unwrap().clone())
    }

    async fn bind_session_llm_provider(
        &self,
        _: &CompiledLlmRuntime,
        _: &str,
        selected: &str,
    ) -> Result<String> {
        if let Some(barrier) = &self.barrier {
            barrier.wait().await;
        }
        Ok(self
            .owner
            .lock()
            .unwrap()
            .get_or_insert_with(|| selected.to_owned())
            .clone())
    }

    async fn select_provider_distribution(
        &self,
        plugin: &str,
        invocation: extension_contracts::ProviderDistributionInvocation,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        CanonicalDynamicDistributionInvoker
            .select_provider_distribution(plugin, invocation)
            .await
    }

    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls
            .lock()
            .unwrap()
            .push(runtime.provider_instance_id.clone());
        if self.fail {
            return Err(ExtensionContractError::runtime(ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderTransportUnavailable,
                "original owner failed",
            ))
            .into());
        }
        Ok(final_provider_output(format!(
            "winner:{}",
            runtime.provider_instance_id
        )))
    }
}

#[async_trait]
impl CapabilityInvoker for SessionInvoker {
    async fn invoke_capability_node(
        &self,
        _: &CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("session fixture has no capability node")
    }
}

fn session_plan(rule: LlmDistributionRule, targets: &[&str]) -> CompiledPlan {
    let mut plan = base_plan();
    plan.nodes.get_mut("node-llm").unwrap().llm_runtime =
        Some(model_group_llm_runtime(rule, targets));
    plan
}

async fn run_session(
    plan: &CompiledPlan,
    invoker: &SessionInvoker,
) -> crate::execution_state::FlowDebugExecutionOutcome {
    start_flow_debug_run(plan, &json!({"node-start":{"query":"hello"}}), invoker)
        .await
        .unwrap()
}

#[tokio::test]
async fn provider_session_none_session_keeps_first_owner_after_candidate_reordering_and_failure() {
    let invoker = SessionInvoker::default();
    run_session(
        &session_plan(LlmDistributionRule::None, &["provider-a", "provider-b"]),
        &invoker,
    )
    .await;
    run_session(
        &session_plan(LlmDistributionRule::None, &["provider-b", "provider-a"]),
        &invoker,
    )
    .await;
    let failing = SessionInvoker {
        fail: true,
        ..invoker.clone()
    };
    let plan = session_plan(LlmDistributionRule::None, &["provider-b", "provider-a"]);
    let outcome = run_session(&plan, &failing).await;
    assert_eq!(
        invoker.calls.lock().unwrap().as_slice(),
        ["provider-a", "provider-a", "provider-a"]
    );
    let trace = outcome
        .node_traces
        .iter()
        .find(|t| t.node_id == "node-llm")
        .unwrap();
    assert!(trace
        .error_payload
        .as_ref()
        .unwrap()
        .to_string()
        .contains("original owner failed"));
    assert_eq!(invoker.owner.lock().unwrap().as_deref(), Some("provider-a"));
}

#[tokio::test]
async fn provider_session_none_session_missing_original_owner_fails_without_invoking_remaining_target(
) {
    let invoker = SessionInvoker::default();
    run_session(
        &session_plan(LlmDistributionRule::None, &["provider-a", "provider-b"]),
        &invoker,
    )
    .await;
    let outcome = run_session(
        &session_plan(LlmDistributionRule::None, &["provider-b"]),
        &invoker,
    )
    .await;
    assert_eq!(invoker.calls.lock().unwrap().as_slice(), ["provider-a"]);
    assert!(outcome
        .node_traces
        .iter()
        .find(|t| t.node_id == "node-llm")
        .unwrap()
        .error_payload
        .is_some());
    assert_eq!(invoker.owner.lock().unwrap().as_deref(), Some("provider-a"));
}

#[tokio::test]
async fn provider_session_explicit_rules_select_their_target_without_overwriting_original_owner() {
    for rule in [
        LlmDistributionRule::RetryRoundRobin,
        LlmDistributionRule::RoundRobin,
        LlmDistributionRule::Dynamic {
            rule_id: "fixture".into(),
            rule_version: "1".into(),
            contract_version: extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1.into(),
            config: BTreeMap::new(),
        },
    ] {
        let invoker = SessionInvoker::default();
        run_session(
            &session_plan(LlmDistributionRule::None, &["provider-a", "provider-b"]),
            &invoker,
        )
        .await;
        let plan = session_plan(rule, &["provider-b", "provider-a"]);
        let context = ExecutionRuntimeContext::from_plan_input(&plan, &serde_json::Map::new())
            .unwrap()
            .with_llm_routing_counter_store(Arc::new(RecordingRoutingCounterStore::default()));
        start_flow_debug_run_with_runtime_context(
            &plan,
            &json!({"node-start":{"query":"hello"}}),
            context,
            &invoker,
        )
        .await
        .unwrap();
        run_session(
            &session_plan(LlmDistributionRule::None, &["provider-b", "provider-a"]),
            &invoker,
        )
        .await;
        assert_eq!(
            invoker.calls.lock().unwrap().as_slice(),
            ["provider-a", "provider-b", "provider-a"]
        );
        assert_eq!(invoker.owner.lock().unwrap().as_deref(), Some("provider-a"));
    }
}

#[tokio::test]
async fn provider_session_concurrent_first_requests_invoke_only_the_atomic_binding_winner() {
    let invoker = SessionInvoker {
        barrier: Some(Arc::new(tokio::sync::Barrier::new(2))),
        ..Default::default()
    };
    let first = session_plan(LlmDistributionRule::None, &["provider-a", "provider-b"]);
    let second = session_plan(LlmDistributionRule::None, &["provider-b", "provider-a"]);
    tokio::join!(
        run_session(&first, &invoker),
        run_session(&second, &invoker)
    );
    let winner = invoker.owner.lock().unwrap().clone().unwrap();
    let calls = invoker.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call == &winner));
}
