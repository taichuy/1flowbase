use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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

#[derive(Clone)]
pub struct ResolvedProviderRoute {
    pub runtime_capabilities: BTreeSet<String>,
    invocation_pin: Arc<dyn Any + Send + Sync>,
}

impl ResolvedProviderRoute {
    pub fn new<T>(runtime_capabilities: BTreeSet<String>, invocation_pin: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self {
            runtime_capabilities,
            invocation_pin: Arc::new(invocation_pin),
        }
    }

    pub fn invocation_pin<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync,
    {
        self.invocation_pin.downcast_ref::<T>()
    }
}

#[async_trait]
pub trait ProviderInvoker: Send + Sync {
    async fn resolve_llm_route(
        &self,
        _runtime: &CompiledLlmRuntime,
    ) -> Result<ResolvedProviderRoute> {
        Ok(ResolvedProviderRoute::new(BTreeSet::new(), ()))
    }

    async fn invoke_resolved_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _resolved_route: ResolvedProviderRoute,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.invoke_llm(runtime, input).await
    }

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

mod llm_executor;
pub use llm_executor::execute_llm_node;
pub(super) use llm_executor::execute_llm_node_provider_round;

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
