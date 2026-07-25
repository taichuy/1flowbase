use super::*;

pub(super) async fn execute_compact_consumer<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + ?Sized,
{
    let input = match build_provider_invocation(
        plan,
        node,
        runtime,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
    ) {
        Ok(invocation) => invocation.input,
        Err(error_payload) => return Ok(failed_compact_execution(error_payload)),
    };
    let expected_profile = input
        .profile
        .ok_or_else(|| anyhow!("canonical Compact invocation is missing its profile"))?;

    match invoker.compact(runtime, input).await {
        Ok(result) if result.satisfies_profile(expected_profile) => {
            let receipt = CompactResponseReceipt::from_provider_result(result)?;
            Ok(LlmNodeExecution {
                output_payload: receipt.as_payload()?,
                error_payload: None,
                metrics_payload: json!({
                    "operation": "compact",
                    "profile": expected_profile,
                    "provider_instance_id": runtime.provider_instance_id,
                    "provider_code": runtime.provider_code,
                    "model": runtime.model,
                }),
                debug_payload: json!({}),
                provider_events: Vec::new(),
                pending_callback: None,
                failure_projection: LlmFailureProjection::NoNodeOutput,
                recoverable_error_message: None,
            })
        }
        Ok(result) => Ok(failed_compact_execution(json!({
            "error_code": "provider_compact_contract_mismatch",
            "message": "provider Compact result did not match the requested operation and profile",
            "expected_profile": expected_profile,
            "actual_operation": result.operation(),
            "actual_profile": result.profile(),
        }))),
        Err(error) => Ok(failed_compact_execution(json!({
            "error_code": "provider_compact_failed",
            "message": error.to_string(),
            "provider_code": runtime.provider_code,
            "model": runtime.model,
        }))),
    }
}

fn failed_compact_execution(error_payload: Value) -> LlmNodeExecution {
    LlmNodeExecution {
        output_payload: json!({}),
        error_payload: Some(error_payload),
        metrics_payload: json!({ "operation": "compact", "error": true }),
        debug_payload: json!({}),
        provider_events: Vec::new(),
        pending_callback: None,
        failure_projection: LlmFailureProjection::NoNodeOutput,
        recoverable_error_message: None,
    }
}
