use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use plugin_framework::{
    error::PluginFrameworkError,
    provider_contract::{
        NativePromptBlock, ProviderCompactError, ProviderCompactProfile, ProviderCompactResult,
        ProviderCountTokensError, ProviderCountTokensInput, ProviderCountTokensResult,
        ProviderFinishReason, ProviderInvocationCapability, ProviderInvocationInput,
        ProviderInvocationResult, ProviderMessage, ProviderMessageRole, ProviderRuntimeError,
        ProviderRuntimeErrorKind, ProviderStreamEvent, ProviderToolCall, ProviderUsage,
        ProviderWireOperation,
    },
};
use serde_json::{json, Map, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    answer_projection::{answer_segments_value_from_text, ANSWER_SEGMENTS_KEY},
    binding_runtime::{
        render_templated_bindings, resolve_answer_node_inputs, resolve_node_inputs,
        BindingResolutionIssue,
    },
    compiled_plan::{
        CompiledEdge, CompiledLlmRuntime, CompiledNode, CompiledPlan, CompiledPluginRuntime,
        LlmRoutingMode,
    },
    execution_state::{
        compact_operation_receipt_from_traces, count_tokens_receipt_from_traces,
        CheckpointSnapshot, CompactOperationReceipt, CountTokensReceipt, ExecutionIncompleteReason,
        ExecutionStopReason, FlowDebugExecutionOutcome, NativeOperationTerminal,
        NodeExecutionFailure, NodeExecutionTrace, PendingCallbackTask, PendingHumanInput,
    },
    node_errors::build_node_type_not_implemented_error_payload,
    output_schema::value_is_llm_context_messages,
    payload_builder::{
        is_reserved_payload_key, BuiltNodePayloads, PublicOutputContract, RawNodeExecutionResult,
    },
};

pub use crate::code_runtime::{
    execute_code_node, CodeInvocationOutput, CodeInvoker, ConsoleLogEntry, QuickJsCodeInvoker,
};

pub mod branching;
mod compact_operation;
mod http_request;
mod llm_callbacks;
mod llm_context;
mod llm_error_payloads;
mod llm_final_content;
mod llm_invocation;
mod llm_metrics;
mod llm_node_outputs;
mod llm_parameters;
mod node_failure_policy;
mod run_input;
#[cfg(test)]
mod tests;
mod variable_assignment;
mod visible_internal_llm_tools;

use branching::*;
use compact_operation::execute_compact_consumer;
pub use http_request::{
    execute_http_request_node, HttpRequestNodeExecution, HttpResponseFilePersistInput,
    HttpResponseFilePersister,
};
pub use llm_callbacks::pending_llm_tool_callback_requires_ephemeral_provider_continuation;
use llm_callbacks::*;
use llm_context::*;
use llm_error_payloads::*;
use llm_final_content::*;
use llm_invocation::*;
use llm_metrics::*;
use llm_node_outputs::*;
pub use llm_node_outputs::{
    canonicalize_provider_output_tool_call_names, canonicalize_provider_stream_event_tool_call_name,
};
use llm_parameters::*;
use node_failure_policy::{apply_node_error_policy, NodeErrorPolicyApplication};
pub(crate) use run_input::materialize_start_builtin_defaults;
pub use run_input::{normalize_plan_variable_pool, ExecutionRuntimeContext};
use run_input::{start_node_execution_input, synchronize_runtime_global_variables};
pub(crate) use variable_assignment::execute_variable_assignment_node;
use visible_internal_llm_tools::*;

const LLM_TOOL_CALLBACK_KIND: &str = "llm_tool_calls";
const LLM_TOOL_CALLBACK_STATE_KEY: &str = "__llm_tool_callback";
const RESPONSES_WEBSOCKET_TRANSPORT: &str = "responses_websocket";

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInvocationOutput {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
    pub first_token_at: Option<OffsetDateTime>,
    pub time_to_first_token_ms: Option<u64>,
}

#[async_trait]
pub trait ProviderInvoker: Send + Sync {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput>;

    async fn count_tokens(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensResult> {
        bail!("provider CountTokens is not supported by this invoker")
    }

    async fn compact(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderCompactResult> {
        bail!("provider Compact is not supported by this invoker")
    }

    /// Resolves a durable-safe protocol-context locator immediately before Provider invocation.
    /// Implementations return `Ok(None)` for ordinary JSON values and must never log the resolved
    /// raw value.
    async fn resolve_protocol_context_locator(&self, _locator: &Value) -> Result<Option<Value>> {
        Ok(None)
    }
}

#[async_trait]
pub trait LlmRoutingCounterStore: Send + Sync {
    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        ttl: Option<time::Duration>,
    ) -> Result<i64>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityInvocationOutput {
    pub output_payload: Value,
}

#[async_trait]
pub trait CapabilityInvoker: Send + Sync {
    async fn invoke_capability_node(
        &self,
        runtime: &CompiledPluginRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CapabilityInvocationOutput>;

    async fn invoke_data_model_node(
        &self,
        _node: &CompiledNode,
        _resolved_inputs: &Map<String, Value>,
    ) -> Result<DataModelInvocationOutput> {
        Err(anyhow!("data model runtime is not configured"))
    }

    async fn invoke_native_sql_node(
        &self,
        _node: &CompiledNode,
        _sql: &str,
    ) -> Result<NativeSqlInvocationOutput> {
        Err(anyhow!("native SQL runtime is not configured"))
    }
}

pub(crate) fn resolved_native_sql(resolved_inputs: &Map<String, Value>) -> Result<&str> {
    resolved_inputs
        .get("sql")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("SQL node is missing resolved bindings.sql"))
}

#[async_trait]
pub trait ExecutionLifecycle: Send + Sync {
    async fn begin_node(&self, node: &CompiledNode, input_payload: &Value) -> Result<()>;
}

struct NoopExecutionLifecycle;

#[async_trait]
impl ExecutionLifecycle for NoopExecutionLifecycle {
    async fn begin_node(&self, _node: &CompiledNode, _input_payload: &Value) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LlmFailureProjection {
    NoNodeOutput,
    FailedNodeOutput,
    LegacyTerminalFallback,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmNodeExecution {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
    pub provider_events: Vec<ProviderStreamEvent>,
    pub pending_callback: Option<LlmToolCallbackWait>,
    pub(super) failure_projection: LlmFailureProjection,
    pub(super) recoverable_error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityNodeExecution {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataModelInvocationOutput {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
    pub pending_callback: Option<DataModelCallback>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlInvocationOutput {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataModelCallback {
    pub callback_kind: String,
    pub request_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmToolCallbackWait {
    pub node_id: String,
    pub node_alias: String,
    pub request_payload: Value,
    pub checkpoint_variable_pool: Map<String, Value>,
    pub node_trace: Option<NodeExecutionTrace>,
}

pub async fn start_flow_debug_run<I>(
    plan: &CompiledPlan,
    input_payload: &Value,
    invoker: &I,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let variable_pool = input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;
    let runtime_context = ExecutionRuntimeContext::from_plan_input(plan, &variable_pool)?;

    execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        &NoopExecutionLifecycle,
    )
    .await
}

pub async fn start_flow_debug_run_with_runtime_context<I>(
    plan: &CompiledPlan,
    input_payload: &Value,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let variable_pool = input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;

    execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        &NoopExecutionLifecycle,
    )
    .await
}

pub async fn start_flow_debug_run_with_runtime_context_and_lifecycle<I>(
    plan: &CompiledPlan,
    input_payload: &Value,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let variable_pool = input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;

    execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        lifecycle,
    )
    .await
}

pub async fn resume_flow_debug_run<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    waiting_node_id: &str,
    resume_payload: &Value,
    invoker: &I,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let runtime_context =
        ExecutionRuntimeContext::from_plan_input(plan, &checkpoint.variable_pool)?;
    resume_flow_debug_run_with_runtime_context(
        plan,
        checkpoint,
        waiting_node_id,
        resume_payload,
        runtime_context,
        invoker,
    )
    .await
}

pub async fn resume_flow_debug_run_with_runtime_context<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    waiting_node_id: &str,
    resume_payload: &Value,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    resume_flow_debug_run_with_runtime_context_and_lifecycle(
        plan,
        checkpoint,
        waiting_node_id,
        resume_payload,
        runtime_context,
        invoker,
        &NoopExecutionLifecycle,
    )
    .await
}

pub async fn resume_flow_debug_run_with_runtime_context_and_lifecycle<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    waiting_node_id: &str,
    resume_payload: &Value,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let waiting_node = plan
        .nodes
        .get(waiting_node_id)
        .ok_or_else(|| anyhow!("waiting node not found: {waiting_node_id}"))?;
    let mut variable_pool = checkpoint.variable_pool.clone();

    if pending_llm_tool_callback_state(&variable_pool, waiting_node_id).is_some() {
        append_llm_tool_result_messages(&mut variable_pool, waiting_node_id, resume_payload)?;
        if has_visible_internal_llm_tool_callback_state(&variable_pool) {
            match resume_visible_internal_llm_tool_callback(
                plan,
                waiting_node_id,
                variable_pool,
                &runtime_context,
                invoker,
            )
            .await?
            {
                VisibleInternalLlmToolResume::Ready(variable_pool) => {
                    return execute_from(
                        plan,
                        checkpoint.next_node_index,
                        variable_pool,
                        Some(checkpoint.active_node_ids.iter().cloned().collect()),
                        &runtime_context,
                        invoker,
                        lifecycle,
                    )
                    .await;
                }
                VisibleInternalLlmToolResume::Waiting(wait) => {
                    let wait = *wait;
                    let checkpoint_variable_pool = wait.checkpoint_variable_pool.clone();
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                            node_id: wait.node_id.clone(),
                            node_alias: wait.node_alias.clone(),
                            callback_kind: LLM_TOOL_CALLBACK_KIND.to_string(),
                            request_payload: wait.request_payload,
                        }),
                        variable_pool: checkpoint_variable_pool.clone(),
                        checkpoint_snapshot: Some(CheckpointSnapshot {
                            next_node_index: checkpoint.next_node_index,
                            variable_pool: checkpoint_variable_pool,
                            active_node_ids: checkpoint.active_node_ids.clone(),
                        }),
                        operation_terminal: None,
                        node_traces: wait.node_trace.into_iter().collect(),
                    });
                }
                VisibleInternalLlmToolResume::Failed {
                    node_id,
                    node_alias,
                    execution,
                } => {
                    let execution = *execution;
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
                            node_id,
                            node_alias,
                            error_payload: execution.error_payload.clone().unwrap_or_else(|| {
                                json!({
                                    "error_code": "visible_internal_llm_tool_failed",
                                    "message": "visible internal LLM tool branch node failed"
                                })
                            }),
                        }),
                        variable_pool: Map::new(),
                        checkpoint_snapshot: None,
                        operation_terminal: None,
                        node_traces: Vec::new(),
                    });
                }
            }
        }
        return execute_from(
            plan,
            checkpoint.next_node_index,
            variable_pool,
            Some(checkpoint.active_node_ids.iter().cloned().collect()),
            &runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    }

    let patch = resume_payload
        .as_object()
        .ok_or_else(|| anyhow!("resume payload must be an object"))?;
    let allowed_output_keys = waiting_node
        .outputs
        .iter()
        .map(|output| output.key.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for key in patch.keys() {
        if !allowed_output_keys.contains(key.as_str()) {
            return Err(anyhow!(
                "resume payload key {key} is not a public output for {waiting_node_id}"
            ));
        }
    }
    variable_pool.insert(waiting_node_id.to_string(), Value::Object(patch.clone()));

    execute_from(
        plan,
        checkpoint.next_node_index,
        variable_pool,
        Some(checkpoint.active_node_ids.iter().cloned().collect()),
        &runtime_context,
        invoker,
        lifecycle,
    )
    .await
}

async fn execute_from<I>(
    plan: &CompiledPlan,
    next_node_index: usize,
    mut variable_pool: Map<String, Value>,
    active_node_ids: Option<BTreeSet<String>>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    normalize_plan_variable_pool(plan, &mut variable_pool);
    synchronize_runtime_global_variables(plan, &mut variable_pool, runtime_context);
    let mut node_traces = Vec::new();
    let mut pending_failure: Option<NodeExecutionFailure> = None;
    let mut active_node_ids = active_node_ids.unwrap_or_else(|| initial_active_node_ids(plan));
    let mounted_llm_target_node_ids = visible_internal_llm_tool_target_node_ids(plan);

    for (index, node_id) in plan
        .topological_order
        .iter()
        .enumerate()
        .skip(next_node_index)
    {
        let node = plan
            .nodes
            .get(node_id)
            .ok_or_else(|| anyhow!("compiled node missing: {node_id}"))?;

        if !active_node_ids.contains(node_id) {
            continue;
        }
        if mounted_llm_target_node_ids.contains(node_id) {
            continue;
        }

        let (resolved_inputs, answer_binding_error_payload) =
            match resolve_node_inputs(node, &variable_pool) {
                Ok(inputs) => (inputs, None),
                Err(_) if node.node_type == "answer" => {
                    let resolution = resolve_answer_node_inputs(node, &variable_pool);
                    let error_payload = (!resolution.issues.is_empty()).then(|| {
                        build_answer_binding_resolution_error_payload(node, &resolution.issues)
                    });
                    (resolution.resolved_inputs, error_payload)
                }
                Err(error) => {
                    lifecycle.begin_node(node, &json!({})).await?;
                    let error_payload = build_binding_resolution_error_payload(&error);
                    node_traces.push(NodeExecutionTrace {
                        node_id: node.node_id.clone(),
                        node_type: node.node_type.clone(),
                        node_alias: node.alias.clone(),
                        input_payload: json!({}),
                        output_payload: json!({}),
                        error_payload: Some(error_payload.clone()),
                        metrics_payload: json!({ "preview_mode": true }),
                        debug_payload: json!({}),
                        provider_events: Vec::new(),
                    });
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
                            node_id: node.node_id.clone(),
                            node_alias: node.alias.clone(),
                            error_payload,
                        }),
                        variable_pool,
                        checkpoint_snapshot: None,
                        operation_terminal: None,
                        node_traces,
                    });
                }
            };
        let rendered_templates = render_templated_bindings(node, &resolved_inputs);
        let node_input_payload = if matches!(node.node_type.as_str(), "start" | "workflow_start") {
            start_node_execution_input(&variable_pool, &node.node_id)
        } else {
            Value::Object(resolved_inputs.clone())
        };
        lifecycle.begin_node(node, &node_input_payload).await?;
        let mut selected_source_handle: Option<String> = None;

        match node.node_type.as_str() {
            "start" | "workflow_start" => {
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: node_input_payload,
                    output_payload: json!({}),
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
            }
            "workflow_end" => {
                let output_payload =
                    project_node_variable_payload(node, &Value::Object(resolved_inputs.clone()))?;
                variable_pool.insert(node.node_id.clone(), output_payload.clone());
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload,
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
            }
            "if_else" => {
                selected_source_handle = select_if_else_source_handle(node, &variable_pool)?;
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: json!({}),
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({
                        "selected_source_handle": selected_source_handle.clone(),
                    }),
                    provider_events: Vec::new(),
                });
            }
            "llm" => {
                let execution = execute_llm_node(
                    plan,
                    node,
                    &resolved_inputs,
                    &rendered_templates,
                    &mut variable_pool,
                    runtime_context,
                    invoker,
                )
                .await?;
                let trace = NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: execution.provider_events.clone(),
                };
                node_traces.push(trace);

                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: execution.failure_projection,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }

                if let Some(wait) = execution.pending_callback {
                    if let Some(trace) = wait.node_trace.clone() {
                        node_traces.push(trace);
                    }
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                            node_id: wait.node_id.clone(),
                            node_alias: wait.node_alias.clone(),
                            callback_kind: LLM_TOOL_CALLBACK_KIND.to_string(),
                            request_payload: wait.request_payload,
                        }),
                        variable_pool,
                        checkpoint_snapshot: Some(CheckpointSnapshot {
                            next_node_index: index,
                            variable_pool: wait.checkpoint_variable_pool,
                            active_node_ids: checkpoint_active_node_ids(&active_node_ids),
                        }),
                        operation_terminal: None,
                        node_traces,
                    });
                }

                variable_pool.insert(node.node_id.clone(), execution.output_payload);
            }
            "plugin_node" => {
                let execution = execute_capability_plugin_node(
                    node,
                    &resolved_inputs,
                    &rendered_templates,
                    invoker,
                )
                .await?;
                let trace = NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: Vec::new(),
                };
                node_traces.push(trace);

                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }

                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &execution.output_payload)?,
                );
            }
            "sql" => {
                let execution = invoker
                    .invoke_native_sql_node(node, resolved_native_sql(&resolved_inputs)?)
                    .await?;
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: node_input_payload,
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload,
                    debug_payload: execution.debug_payload,
                    provider_events: Vec::new(),
                });
                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &execution.output_payload)?,
                );
            }
            "data_model_list" | "data_model_get" | "data_model_create" | "data_model_update"
            | "data_model_delete" => {
                let execution = invoker
                    .invoke_data_model_node(node, &resolved_inputs)
                    .await?;
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: Vec::new(),
                });

                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }

                if let Some(callback) = execution.pending_callback {
                    activate_downstream_nodes(plan, &mut active_node_ids, node, None);
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                            node_id: node.node_id.clone(),
                            node_alias: node.alias.clone(),
                            callback_kind: callback.callback_kind,
                            request_payload: callback.request_payload,
                        }),
                        variable_pool: variable_pool.clone(),
                        checkpoint_snapshot: Some(CheckpointSnapshot {
                            next_node_index: index + 1,
                            variable_pool,
                            active_node_ids: checkpoint_active_node_ids(&active_node_ids),
                        }),
                        operation_terminal: None,
                        node_traces,
                    });
                }

                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &execution.output_payload)?,
                );
            }
            "variable_assigner" => {
                let output_payload =
                    execute_variable_assignment_node(node, &resolved_inputs, &mut variable_pool)?;
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &output_payload)?,
                );
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload,
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
            }
            "template_transform" | "answer" | "tool_result" => {
                let output_key = first_output_key(node);
                let output_value =
                    rendered_templates
                        .values()
                        .next()
                        .cloned()
                        .unwrap_or_else(|| {
                            resolved_inputs
                                .values()
                                .next()
                                .cloned()
                                .unwrap_or(Value::Null)
                        });
                let output_payload =
                    template_output_payload(node, output_key, output_value, &variable_pool);
                let output_payload = answer_output_payload_with_error(
                    output_payload,
                    answer_binding_error_payload.as_ref(),
                );
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &output_payload)?,
                );
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload,
                    error_payload: answer_binding_error_payload.clone(),
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
                if pending_failure.is_none() {
                    if let Some(error_payload) = answer_binding_error_payload {
                        pending_failure = Some(NodeExecutionFailure {
                            node_id: node.node_id.clone(),
                            node_alias: node.alias.clone(),
                            error_payload,
                        });
                    }
                }
            }
            "human_input" => {
                let prompt = rendered_templates
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or("请提供人工输入")
                    .to_string();
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: json!({}),
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true, "waiting": "human_input" }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
                activate_downstream_nodes(plan, &mut active_node_ids, node, None);
                return Ok(FlowDebugExecutionOutcome {
                    stop_reason: ExecutionStopReason::WaitingHuman(PendingHumanInput {
                        node_id: node.node_id.clone(),
                        node_alias: node.alias.clone(),
                        prompt,
                    }),
                    variable_pool: variable_pool.clone(),
                    checkpoint_snapshot: Some(CheckpointSnapshot {
                        next_node_index: index + 1,
                        variable_pool,
                        active_node_ids: checkpoint_active_node_ids(&active_node_ids),
                    }),
                    operation_terminal: None,
                    node_traces,
                });
            }
            "tool" => {
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: json!({}),
                    error_payload: None,
                    metrics_payload: json!({ "preview_mode": true, "waiting": node.node_type }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
                activate_downstream_nodes(plan, &mut active_node_ids, node, None);
                return Ok(FlowDebugExecutionOutcome {
                    stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                        node_id: node.node_id.clone(),
                        node_alias: node.alias.clone(),
                        callback_kind: node.node_type.clone(),
                        request_payload: Value::Object(resolved_inputs),
                    }),
                    variable_pool: variable_pool.clone(),
                    checkpoint_snapshot: Some(CheckpointSnapshot {
                        next_node_index: index + 1,
                        variable_pool,
                        active_node_ids: checkpoint_active_node_ids(&active_node_ids),
                    }),
                    operation_terminal: None,
                    node_traces,
                });
            }
            "http_request" => {
                let execution = execute_http_request_node(
                    node,
                    &resolved_inputs,
                    &variable_pool,
                    runtime_context.http_response_file_persister.as_deref(),
                )
                .await?;
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: Vec::new(),
                });

                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }

                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &execution.output_payload)?,
                );
            }
            "code" => {
                let execution = execute_code_node(plan, node, &resolved_inputs, invoker).await?;
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: Vec::new(),
                });

                if let Some(error_payload) = execution.error_payload {
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &execution.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })? {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                    continue;
                }

                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(node, &execution.output_payload)?,
                );
            }
            other => {
                let error_payload = build_node_type_not_implemented_error_payload(other, "preview");
                node_traces.push(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs),
                    output_payload: json!({}),
                    error_payload: Some(error_payload.clone()),
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
                return Ok(FlowDebugExecutionOutcome {
                    stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
                        node_id: node.node_id.clone(),
                        node_alias: node.alias.clone(),
                        error_payload,
                    }),
                    variable_pool,
                    checkpoint_snapshot: None,
                    operation_terminal: None,
                    node_traces,
                });
            }
        }
        // CountTokens and Compact terminate at the LLM node selected by the
        // workflow. Generate-only downstream nodes (for example Answer) must
        // not reinterpret their typed provider terminal as generated text.
        if node.node_type != "llm"
            || matches!(
                runtime_context.operation(),
                domain::AiNativeOperation::Generate(_)
            )
        {
            activate_downstream_nodes(
                plan,
                &mut active_node_ids,
                node,
                selected_source_handle.as_deref(),
            );
        }
    }

    if let Some(failure) = pending_failure {
        return Ok(FlowDebugExecutionOutcome {
            stop_reason: ExecutionStopReason::Failed(failure),
            variable_pool,
            checkpoint_snapshot: None,
            operation_terminal: None,
            node_traces,
        });
    }

    let operation_terminal = match runtime_context.operation() {
        domain::AiNativeOperation::Generate(_) => None,
        domain::AiNativeOperation::CountTokens => Some(NativeOperationTerminal::CountTokens(
            count_tokens_receipt_from_traces(&node_traces)?,
        )),
        domain::AiNativeOperation::Compact(_) => Some(NativeOperationTerminal::Compact(
            compact_operation_receipt_from_traces(&node_traces)?,
        )),
    };

    Ok(FlowDebugExecutionOutcome {
        stop_reason: successful_flow_stop_reason(&node_traces),
        variable_pool,
        checkpoint_snapshot: None,
        operation_terminal,
        node_traces,
    })
}

fn successful_flow_stop_reason(node_traces: &[NodeExecutionTrace]) -> ExecutionStopReason {
    let reached_output_limit = node_traces.iter().any(|trace| {
        trace.node_type == "llm"
            && trace
                .output_payload
                .get("finish_reason")
                .and_then(Value::as_str)
                == Some("length")
    });

    if reached_output_limit {
        ExecutionStopReason::Incomplete(ExecutionIncompleteReason::OutputLimit)
    } else {
        ExecutionStopReason::Completed
    }
}

pub async fn execute_llm_node<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &mut Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    execute_llm_node_with_visible_internal_tools(
        plan,
        node,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
}

pub(super) async fn execute_llm_node_provider_round<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<LlmNodeExecution>
where
    I: ProviderInvoker + ?Sized,
{
    let runtime = node.llm_runtime.as_ref().ok_or_else(|| {
        anyhow!(
            "compiled llm node is missing runtime metadata: {}",
            node.node_id
        )
    })?;
    if runtime_context.operation() == domain::AiNativeOperation::CountTokens {
        let attempt_runtimes =
            llm_request_runtimes(node, runtime, runtime_context, &BTreeSet::new()).await?;
        let selected_runtime = attempt_runtimes
            .first()
            .ok_or_else(|| anyhow!("CountTokens LLM consumer has no selected runtime"))?;
        return execute_count_tokens_consumer(
            plan,
            node,
            selected_runtime,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }
    if matches!(
        runtime_context.operation(),
        domain::AiNativeOperation::Compact(_)
    ) {
        let attempt_runtimes =
            llm_request_runtimes(node, runtime, runtime_context, &BTreeSet::new()).await?;
        let selected_runtime = attempt_runtimes
            .first()
            .ok_or_else(|| anyhow!("Compact LLM consumer has no selected runtime"))?;
        return execute_compact_consumer(
            plan,
            node,
            selected_runtime,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }
    let mut routing_probe = match build_provider_invocation(
        plan,
        node,
        runtime,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
    {
        Ok(invocation) => Some(invocation),
        Err(error_payload) => {
            return build_failed_llm_execution(
                node,
                runtime,
                error_payload,
                LlmFailureProjection::NoNodeOutput,
                None,
                build_llm_metrics_payload(
                    runtime,
                    ProviderUsage::default(),
                    Some(ProviderFinishReason::Error),
                    0,
                    Vec::new(),
                    None,
                    None,
                ),
                Vec::new(),
                LlmDebugInvocation {
                    messages: &[],
                    context: None,
                },
            );
        }
    };
    let required_capabilities = routing_probe
        .as_ref()
        .map(|invocation| invocation.input.required_capabilities.clone())
        .unwrap_or_default();
    let attempt_runtimes =
        match llm_request_runtimes(node, runtime, runtime_context, &required_capabilities).await {
            Ok(runtimes) => runtimes,
            Err(error) => {
                let provider_error = provider_runtime_error_from_anyhow(&error);
                let error_payload = build_provider_error_payload(runtime, &provider_error);
                return build_failed_llm_execution(
                    node,
                    runtime,
                    error_payload,
                    LlmFailureProjection::NoNodeOutput,
                    None,
                    build_llm_metrics_payload(
                        runtime,
                        ProviderUsage::default(),
                        Some(ProviderFinishReason::Error),
                        0,
                        Vec::new(),
                        None,
                        None,
                    ),
                    Vec::new(),
                    LlmDebugInvocation {
                        messages: &[],
                        context: None,
                    },
                );
            }
        };
    let retry_enabled = node
        .config
        .get("retry_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let retry_interval_ms = node
        .config
        .get("retry_interval_ms")
        .and_then(Value::as_u64)
        .unwrap_or(500);
    let mut attempt_metrics = Vec::new();
    let mut failed_attempts = Vec::new();
    let mut retry_reason: Option<String> = None;

    for (attempt_index, attempt_runtime) in attempt_runtimes.iter().enumerate() {
        let route_matches_probe = routing_probe.is_some()
            && attempt_runtime.provider_instance_id == runtime.provider_instance_id
            && attempt_runtime.provider_code == runtime.provider_code
            && attempt_runtime.protocol == runtime.protocol
            && attempt_runtime.model == runtime.model;
        let mut invocation = if route_matches_probe {
            routing_probe
                .take()
                .expect("the routing probe is consumed by at most one matching route")
        } else {
            match build_provider_invocation(
                plan,
                node,
                attempt_runtime,
                resolved_inputs,
                rendered_templates,
                variable_pool,
                runtime_context,
                invoker,
            )
            .await
            {
                Ok(invocation) => invocation,
                Err(error_payload) => {
                    return build_failed_llm_execution(
                        node,
                        attempt_runtime,
                        error_payload,
                        LlmFailureProjection::NoNodeOutput,
                        None,
                        build_llm_metrics_payload(
                            attempt_runtime,
                            ProviderUsage::default(),
                            Some(ProviderFinishReason::Error),
                            0,
                            attempt_metrics,
                            None,
                            None,
                        ),
                        Vec::new(),
                        LlmDebugInvocation {
                            messages: &[],
                            context: None,
                        },
                    );
                }
            }
        };
        inject_visible_internal_llm_tool_media_content_blocks(&mut invocation.input, variable_pool)
            .await;
        let invocation_messages = build_llm_debug_invocation_messages(
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            &invocation.input,
        );
        if invocation.input.messages.is_empty()
            && !invocation.input.required_capabilities.contains(
                &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
            )
        {
            let attempt_finished_at = OffsetDateTime::now_utc();
            let error_payload = build_empty_prompt_messages_error_payload(attempt_runtime);
            let attempt = build_attempt_metric(AttemptMetricInput {
                attempt_index,
                retry_reason: retry_reason.as_deref(),
                runtime: attempt_runtime,
                status: "failed",
                failed_after_first_token: false,
                error_payload: Some(&error_payload),
                usage: &ProviderUsage::default(),
                event_count: 0,
                started_at: attempt_finished_at,
                first_token_at: None,
                finished_at: attempt_finished_at,
                time_to_first_token_ms: None,
            });
            attempt_metrics.push(attempt);

            return build_failed_llm_execution(
                node,
                attempt_runtime,
                error_payload,
                LlmFailureProjection::NoNodeOutput,
                None,
                build_llm_metrics_payload(
                    attempt_runtime,
                    ProviderUsage::default(),
                    Some(ProviderFinishReason::Error),
                    0,
                    attempt_metrics,
                    None,
                    None,
                ),
                Vec::new(),
                LlmDebugInvocation {
                    messages: &invocation_messages,
                    context: Some(&invocation.debug_context),
                },
            );
        }
        let tool_prompt_transcript =
            llm_tool_prompt_transcript(node, variable_pool, &invocation.input);
        let invocation_tools = invocation.input.tools.clone();
        let native_responses_passthrough = invocation.input.required_capabilities.contains(
            &plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough,
        );
        let attempt_started_at = OffsetDateTime::now_utc();
        let mut output = match invoker.invoke_llm(attempt_runtime, invocation.input).await {
            Ok(output) => output,
            Err(error) => {
                let attempt_finished_at = OffsetDateTime::now_utc();
                let provider_error = provider_runtime_error_from_anyhow(&error);
                let mut error_payload =
                    build_provider_error_payload(attempt_runtime, &provider_error);
                error_payload["failed_after_first_token"] = Value::Bool(false);
                let recoverable_error_message = recoverable_provider_error_message(&provider_error);
                let attempt = build_attempt_metric(AttemptMetricInput {
                    attempt_index,
                    retry_reason: retry_reason.as_deref(),
                    runtime: attempt_runtime,
                    status: "failed",
                    failed_after_first_token: false,
                    error_payload: Some(&error_payload),
                    usage: &ProviderUsage::default(),
                    event_count: 0,
                    started_at: attempt_started_at,
                    first_token_at: None,
                    finished_at: attempt_finished_at,
                    time_to_first_token_ms: None,
                });
                attempt_metrics.push(attempt.clone());
                failed_attempts.push(attempt);
                if retry_enabled
                    && provider_error_allows_retry(&provider_error)
                    && attempt_index + 1 < attempt_runtimes.len()
                {
                    retry_reason = error_payload
                        .get("error_code")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    if retry_interval_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(retry_interval_ms))
                            .await;
                    }
                    continue;
                }

                return build_failed_llm_execution(
                    node,
                    attempt_runtime,
                    error_payload,
                    LlmFailureProjection::NoNodeOutput,
                    Some(recoverable_error_message),
                    build_llm_metrics_payload(
                        attempt_runtime,
                        ProviderUsage::default(),
                        Some(ProviderFinishReason::Error),
                        0,
                        attempt_metrics,
                        None,
                        None,
                    ),
                    Vec::new(),
                    LlmDebugInvocation {
                        messages: &invocation_messages,
                        context: Some(&invocation.debug_context),
                    },
                );
            }
        };
        let attempt_finished_at = OffsetDateTime::now_utc();
        canonicalize_provider_output_tool_call_names(&mut output, &invocation_tools);

        let usage = collect_usage(&output.events, &output.result.usage);
        let finish_reason = output
            .result
            .finish_reason
            .clone()
            .or_else(|| finish_reason_from_events(&output.events));
        let final_content = resolve_final_llm_content(
            output.result.final_content.clone(),
            collect_dify_style_deltas(&output.events),
        );
        let stream_provider_error = first_provider_error(&output.events).cloned();
        let invalid_tool_call_error = if stream_provider_error.is_none() {
            invalid_tool_call_finish_error(finish_reason.as_ref(), &output.result)
        } else {
            None
        };
        let terminal_finish_error = (stream_provider_error.is_none()
            && invalid_tool_call_error.is_none()
            && matches!(finish_reason, Some(ProviderFinishReason::Error)))
        .then(|| {
            ProviderRuntimeError::normalize(
                "invoke",
                "provider invocation finished with error",
                None,
            )
        });
        let failure_projection = if invalid_tool_call_error.is_some() {
            LlmFailureProjection::LegacyTerminalFallback
        } else if terminal_finish_error.is_some()
            && content_delta_seen_before_terminal_failure(&output.events, finish_reason.as_ref())
        {
            LlmFailureProjection::FailedNodeOutput
        } else {
            LlmFailureProjection::NoNodeOutput
        };
        let provider_error = stream_provider_error
            .or(invalid_tool_call_error)
            .or(terminal_finish_error);
        let failed_after_first_token = provider_error.is_some()
            && content_delta_seen_before_terminal_failure(&output.events, finish_reason.as_ref());
        let recoverable_error_message = provider_error
            .as_ref()
            .map(recoverable_provider_error_message);
        let mut error_payload = provider_error
            .as_ref()
            .map(|error| build_provider_error_payload(attempt_runtime, error))
            .or_else(|| {
                (!has_valid_provider_output(
                    final_content.as_deref(),
                    &output.result,
                    native_responses_passthrough,
                ))
                .then(|| build_empty_provider_response_error_payload(attempt_runtime))
            });
        if failure_projection == LlmFailureProjection::LegacyTerminalFallback {
            if let (Some(error_payload), Some(message)) =
                (&mut error_payload, recoverable_error_message.as_deref())
            {
                // Invalid tool-call completion is generated by this runtime,
                // rather than copied from an upstream provider response.
                error_payload["message"] = Value::String(message.to_string());
            }
        }
        if let Some(error_payload) = &mut error_payload {
            error_payload["failed_after_first_token"] = Value::Bool(failed_after_first_token);
        }
        let attempt_status = match error_payload
            .as_ref()
            .and_then(|payload| payload.get("error_code"))
            .and_then(Value::as_str)
        {
            Some("empty_response") => "empty_response",
            Some(_) => "failed",
            None => "succeeded",
        };
        let attempt = build_attempt_metric(AttemptMetricInput {
            attempt_index,
            retry_reason: retry_reason.as_deref(),
            runtime: attempt_runtime,
            status: attempt_status,
            failed_after_first_token,
            error_payload: error_payload.as_ref(),
            usage: &usage,
            event_count: output.events.len(),
            started_at: attempt_started_at,
            first_token_at: output.first_token_at,
            finished_at: attempt_finished_at,
            time_to_first_token_ms: output.time_to_first_token_ms,
        });
        attempt_metrics.push(attempt.clone());

        if let Some(error_payload) = &error_payload {
            failed_attempts.push(attempt);
            if retry_enabled
                && !failed_after_first_token
                && provider_error
                    .as_ref()
                    .is_none_or(provider_error_allows_retry)
                && attempt_index + 1 < attempt_runtimes.len()
            {
                retry_reason = error_payload
                    .get("error_code")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if retry_interval_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(retry_interval_ms)).await;
                }
                continue;
            }
            return build_failed_llm_execution(
                node,
                attempt_runtime,
                error_payload.clone(),
                failure_projection,
                recoverable_error_message,
                build_llm_metrics_payload(
                    attempt_runtime,
                    usage,
                    finish_reason,
                    output.events.len(),
                    attempt_metrics,
                    output.first_token_at,
                    output.time_to_first_token_ms,
                ),
                output.events,
                LlmDebugInvocation {
                    messages: &invocation_messages,
                    context: Some(&invocation.debug_context),
                },
            );
        }

        let mut execution = build_successful_llm_execution(
            node,
            attempt_runtime,
            &output.result,
            final_content,
            native_responses_passthrough,
            build_llm_metrics_payload(
                attempt_runtime,
                usage,
                finish_reason.clone(),
                output.events.len(),
                attempt_metrics,
                output.first_token_at,
                output.time_to_first_token_ms,
            ),
            output.events,
            LlmDebugInvocation {
                messages: &invocation_messages,
                context: Some(&invocation.debug_context),
            },
        )?;
        execution.pending_callback = build_llm_tool_callback_wait(
            node,
            variable_pool,
            &execution.output_payload,
            &tool_prompt_transcript,
        )?;
        return Ok(execution);
    }

    let error_payload = json!({
        "error_code": "provider_unavailable",
        "message": "all llm node requests failed",
        "attempts": failed_attempts,
    });
    build_failed_llm_execution(
        node,
        runtime,
        error_payload,
        LlmFailureProjection::NoNodeOutput,
        None,
        build_llm_metrics_payload(
            runtime,
            ProviderUsage::default(),
            Some(ProviderFinishReason::Error),
            0,
            attempt_metrics,
            None,
            None,
        ),
        Vec::new(),
        LlmDebugInvocation {
            messages: &[],
            context: None,
        },
    )
}

#[allow(clippy::too_many_arguments)]
async fn execute_count_tokens_consumer<I>(
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
    let invocation = match build_provider_invocation(
        plan,
        node,
        runtime,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
        invoker,
    )
    .await
    {
        Ok(invocation) => invocation,
        Err(error_payload) => {
            return Ok(LlmNodeExecution {
                output_payload: json!({}),
                error_payload: Some(error_payload),
                metrics_payload: json!({ "operation": "count_tokens" }),
                debug_payload: json!({}),
                provider_events: Vec::new(),
                pending_callback: None,
                failure_projection: LlmFailureProjection::NoNodeOutput,
                recoverable_error_message: None,
            });
        }
    };
    let input = ProviderCountTokensInput {
        operation: ProviderWireOperation::CountTokens,
        contract_version: invocation.input.contract_version,
        provider_instance_id: invocation.input.provider_instance_id,
        provider_code: invocation.input.provider_code,
        protocol: invocation.input.protocol,
        model: invocation.input.model,
        provider_config: Value::Null,
        messages: invocation.input.messages,
        system: invocation.input.system,
        request_context: invocation.input.request_context,
        required_capabilities: invocation.input.required_capabilities,
        client_protocol_envelope: invocation.input.client_protocol_envelope,
    };
    match invoker.count_tokens(runtime, input).await {
        Ok(result) => {
            let receipt = CountTokensReceipt::new(result)?;
            Ok(LlmNodeExecution {
                output_payload: receipt.as_payload()?,
                error_payload: None,
                metrics_payload: json!({
                    "operation": "count_tokens",
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
        Err(error) => Ok(LlmNodeExecution {
            output_payload: json!({}),
            error_payload: Some(build_provider_error_payload(
                runtime,
                &provider_runtime_error_from_anyhow(&error),
            )),
            metrics_payload: json!({ "operation": "count_tokens", "error": true }),
            debug_payload: json!({}),
            provider_events: Vec::new(),
            pending_callback: None,
            failure_projection: LlmFailureProjection::NoNodeOutput,
            recoverable_error_message: None,
        }),
    }
}

pub async fn execute_capability_plugin_node<I>(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    _rendered_templates: &Map<String, Value>,
    invoker: &I,
) -> Result<CapabilityNodeExecution>
where
    I: CapabilityInvoker + ?Sized,
{
    let runtime = node.plugin_runtime.as_ref().ok_or_else(|| {
        anyhow!(
            "compiled plugin node is missing runtime metadata: {}",
            node.node_id
        )
    })?;
    let config_payload = node.config.clone();
    let input_payload = Value::Object(resolved_inputs.clone());

    match invoker
        .invoke_capability_node(runtime, config_payload, input_payload)
        .await
    {
        Ok(output) => {
            let raw = RawNodeExecutionResult {
                executor_output: object_from_value(output.output_payload)?,
                metrics_facts: object_from_value(json!({
                    "plugin_id": runtime.plugin_id,
                    "plugin_version": runtime.plugin_version,
                    "plugin_unique_identifier": runtime.plugin_unique_identifier,
                    "package_id": runtime.package_id,
                    "contribution_code": runtime.contribution_code,
                    "node_shell": runtime.node_shell,
                    "schema_version": runtime.schema_version,
                    "contribution_checksum": runtime.contribution_checksum,
                    "compiled_contribution_hash": runtime.compiled_contribution_hash,
                    "side_effect_policy": runtime.side_effect_policy,
                }))?,
                error_facts: Map::new(),
                debug_facts: Map::new(),
                provider_events: Vec::new(),
            };
            let built = build_plugin_node_payloads(node, raw)?;

            Ok(CapabilityNodeExecution {
                output_payload: built.output_payload,
                error_payload: None,
                metrics_payload: built.metrics_payload,
                debug_payload: built.debug_payload,
            })
        }
        Err(error) => {
            let raw = RawNodeExecutionResult {
                executor_output: object_from_value(json!({ first_output_key(node): Value::Null }))?,
                metrics_facts: object_from_value(json!({
                    "plugin_id": runtime.plugin_id,
                    "plugin_version": runtime.plugin_version,
                    "plugin_unique_identifier": runtime.plugin_unique_identifier,
                    "package_id": runtime.package_id,
                    "contribution_code": runtime.contribution_code,
                    "node_shell": runtime.node_shell,
                    "schema_version": runtime.schema_version,
                    "contribution_checksum": runtime.contribution_checksum,
                    "compiled_contribution_hash": runtime.compiled_contribution_hash,
                    "side_effect_policy": runtime.side_effect_policy,
                    "error": true,
                }))?,
                error_facts: object_from_value(json!({
                    "message": error.to_string(),
                }))?,
                debug_facts: Map::new(),
                provider_events: Vec::new(),
            };
            let built = build_plugin_node_payloads(node, raw)?;

            Ok(CapabilityNodeExecution {
                output_payload: built.output_payload,
                error_payload: Some(built.error_payload),
                metrics_payload: built.metrics_payload,
                debug_payload: built.debug_payload,
            })
        }
    }
}

fn build_plugin_node_payloads(
    node: &CompiledNode,
    raw: RawNodeExecutionResult,
) -> Result<BuiltNodePayloads> {
    for key in raw.executor_output.keys() {
        if is_reserved_payload_key(key) {
            return Err(anyhow!(
                "reserved plugin output key `{key}` cannot be returned by capability node executor"
            ));
        }
    }

    PublicOutputContract::from_compiled_outputs(&node.outputs)?.build_node_payloads(raw)
}
