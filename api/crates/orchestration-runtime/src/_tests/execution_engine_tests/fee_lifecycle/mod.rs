use super::*;

struct FeeFailureInvoker;
impl_noop_code_invoker!(FeeFailureInvoker);
#[async_trait]
impl ProviderInvoker for FeeFailureInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Err(ExtensionContractError::runtime(ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse, "billing_finalize_failed",
        ).with_provider_details(json!({
            "_1flowbase_billing":{"billing_status":"reconciliation_failed","billing_error_code":"billing_finalize_failed","total_cost":"0.0775","cache_write_tokens":5000},
            "_1flowbase_user_account":"billing-user",
            "usage":{"input_tokens":5000,"cache_write_tokens":5000,"cache_write_by_ttl_seconds":{"300":3000,"3600":2000}}
        }))).into())
    }
}
#[async_trait]
impl CapabilityInvoker for FeeFailureInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("fee failure fixture has no capability node")
    }
}

// AC4/5: the real executor emits failed attempts with self-contained fee and usage facts.
#[tokio::test]
async fn fee_failure_attempt_retains_usage_cost_and_account_snapshot() {
    let mut plan = base_plan();
    let config = &mut plan.nodes.get_mut("node-llm").unwrap().config;
    config["retry_enabled"] = json!(true);
    config["max_retries"] = json!(2);
    config["retry_interval_ms"] = json!(0);
    let outcome = start_flow_debug_run(
        &plan,
        &json!({"node-start":{"query":"fixture"}}),
        &FeeFailureInvoker,
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    let metrics = &outcome.node_traces[1].metrics_payload;
    assert_eq!(metrics["attempts"].as_array().unwrap().len(), 1);
    let attempt = &metrics["attempts"][0];
    assert_eq!(attempt["status"], "failed");
    assert_eq!(
        attempt["billing"]["billing_status"],
        "reconciliation_failed"
    );
    assert_eq!(attempt["billing"]["total_cost"], "0.0775");
    assert_eq!(attempt["usage"]["cache_write_tokens"], 5000);
    assert_eq!(attempt["usage"]["cache_write_by_ttl_seconds"]["300"], 3000);
    assert_eq!(attempt["user_account"], "billing-user");
    assert_eq!(metrics["usage"]["cache_write_tokens"], 5000);
}
