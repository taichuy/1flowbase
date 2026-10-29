use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use extension_contracts::{
    error::ExtensionContractError,
    provider_contract::{
        NativePromptBlock, ProviderCanonicalBlockLocator, ProviderCompactError,
        ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensError,
        ProviderCountTokensInput, ProviderCountTokensResult, ProviderFinishReason,
        ProviderGenerateProjectionError, ProviderGenerateTranslationReceipt,
        ProviderInvocationCapability, ProviderInvocationInput, ProviderInvocationResult,
        ProviderMessage, ProviderMessageRole, ProviderOutputProtocolFailure,
        ProviderProjectionErrorCode, ProviderRuntimeError, ProviderRuntimeErrorKind,
        ProviderStreamEvent, ProviderToolCall, ProviderUsage, ProviderWireOperation,
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
        CompiledEdge, CompiledEdgeIndex, CompiledLlmRuntime, CompiledNode, CompiledPlan,
        CompiledPluginRuntime, LlmRoutingMode,
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

pub mod ai_native_recovery;
pub mod branching;
pub mod communication;
pub mod container;
#[path = "execution_engine/llm_operations/compact_operation.rs"]
mod compact_operation;
mod http_request;
pub mod internal_compaction;
#[path = "execution_engine/llm_operations/llm_callbacks.rs"]
mod llm_callbacks;
mod llm_context;
mod llm_error_payloads;
pub(crate) mod llm_final_content;
mod native_inference_recovery;
pub use native_inference_recovery::{
    recover_native_inference_with_runtime_context_and_lifecycle,
    validate_native_inference_recovery_scope,
};
#[path = "execution_engine/llm_operations/llm_invocation.rs"]
mod llm_invocation;
pub(crate) mod llm_metrics;
#[path = "execution_engine/llm_operations/llm_node_outputs.rs"]
mod llm_node_outputs;
#[path = "execution_engine/llm_operations/llm_parameters.rs"]
mod llm_parameters;
#[path = "execution_engine/llm_operations/route_preflight.rs"]
mod llm_route_preflight;
mod node_failure_policy;
mod provider_routing;
mod run_input;
#[cfg(test)]
mod tests;
pub(crate) mod variable_aggregator;
mod variable_assignment;
mod visible_internal_llm_tools;

use branching::*;
use compact_operation::execute_compact_consumer;
pub use http_request::{
    execute_http_request_node, execute_http_request_node_with_provider_invoker,
    HttpRequestClientLease, HttpRequestClientLeaseReleaser, HttpRequestNodeExecution,
    HttpResponseFilePersistInput, HttpResponseFilePersister,
};
pub use llm_callbacks::pending_llm_tool_callback_requires_ephemeral_provider_continuation;
use llm_callbacks::*;
use llm_context::*;
use llm_error_payloads::*;
pub use llm_final_content::billable_provider_output;
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
use run_input::{
    frozen_runtime_internal_provider_names, start_node_execution_input,
    synchronize_runtime_global_variables,
};
pub use run_input::{normalize_plan_variable_pool, ExecutionRuntimeContext};
use variable_aggregator::{execute_variable_aggregator_node, variable_aggregator_input_payload};
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
    runtime_plugin_id: Option<String>,
    retry_partition: Option<ai_native_recovery::ProviderRetryPartition>,
    invocation_pin: Arc<dyn Any + Send + Sync>,
}

pub struct ResolvedMainLlmRouteCandidate {
    pub runtime: CompiledLlmRuntime,
    pub route: ResolvedProviderRoute,
}

pub struct ResolvedMainLlmRouting {
    pub candidates: Vec<ResolvedMainLlmRouteCandidate>,
    pub distribution_rule: crate::compiled_plan::LlmDistributionRule,
    pub distribution_key: Option<String>,
}

impl ResolvedProviderRoute {
    pub fn new<T>(runtime_capabilities: BTreeSet<String>, invocation_pin: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self {
            runtime_capabilities,
            runtime_plugin_id: None,
            retry_partition: None,
            invocation_pin: Arc::new(invocation_pin),
        }
    }

    pub fn with_runtime_plugin_id(mut self, plugin_id: impl Into<String>) -> Self {
        self.runtime_plugin_id = Some(plugin_id.into());
        self
    }

    pub fn runtime_plugin_id(&self) -> Option<&str> {
        self.runtime_plugin_id.as_deref()
    }

    pub fn with_retry_partition(
        mut self,
        retry_partition: ai_native_recovery::ProviderRetryPartition,
    ) -> Self {
        self.retry_partition = Some(retry_partition);
        self
    }

    pub fn retry_partition(&self) -> Option<&ai_native_recovery::ProviderRetryPartition> {
        self.retry_partition.as_ref()
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
    async fn provider_distribution_registry_fingerprint(&self) -> Result<String> {
        Ok("builtin-provider-distribution-registry/v1".to_string())
    }

    async fn select_provider_distribution(
        &self,
        _plugin_id: &str,
        _invocation: extension_contracts::ProviderDistributionInvocation,
    ) -> Result<extension_contracts::ProviderDistributionSelectionReceipt> {
        Err(anyhow!("provider distribution runtime is not configured"))
    }

    async fn pipeline_provider_input(
        &self,
        input: ProviderInvocationInput,
    ) -> std::result::Result<
        crate::provider_input_pipeline::ProviderInputPipelineOutput,
        crate::provider_input_pipeline::ProviderInputPipelineError,
    > {
        Ok(crate::provider_input_pipeline::ProviderInputPipelineOutput::unchanged(input))
    }

    async fn resolve_main_llm_routing(
        &self,
        _runtime: &CompiledLlmRuntime,
        _node_id: &str,
    ) -> Result<Option<ResolvedMainLlmRouting>> {
        Ok(None)
    }

    async fn get_session_llm_provider(
        &self,
        _runtime: &CompiledLlmRuntime,
        _node_id: &str,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    /// Atomically remembers the first selected target; returns the existing winner on conflict.
    async fn bind_session_llm_provider(
        &self,
        _runtime: &CompiledLlmRuntime,
        _node_id: &str,
        selected_provider_instance_id: &str,
    ) -> Result<String> {
        Ok(selected_provider_instance_id.to_owned())
    }

    async fn resolve_llm_route(
        &self,
        _runtime: &CompiledLlmRuntime,
    ) -> Result<ResolvedProviderRoute> {
        Ok(ResolvedProviderRoute::new(BTreeSet::new(), ()))
    }

    /// Host preparation after target selection, before recovery directives and output validation.
    /// The returned representation is authoritative for this attempt, not Provider metadata.
    fn prepare_selected_llm_input(
        &self,
        _runtime: &CompiledLlmRuntime,
        _resolved_route: &ResolvedProviderRoute,
        input: &mut ProviderInvocationInput,
    ) -> Result<ai_native_recovery::RecoveryInputMode> {
        Ok(
            if input.native_transport.is_some()
                || input
                    .required_capabilities
                    .contains(&ProviderInvocationCapability::ResponsesNativePassthrough)
            {
                ai_native_recovery::RecoveryInputMode::NativeOpaque
            } else {
                ai_native_recovery::RecoveryInputMode::SemanticMapped
            },
        )
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

    async fn acquire_http_node_client(
        &self,
        network_egress: &domain::HttpNodeNetworkEgress,
        _timeout: std::time::Duration,
        _verify_ssl: bool,
    ) -> Result<Option<HttpRequestClientLease>> {
        anyhow::ensure!(
            !matches!(network_egress, domain::HttpNodeNetworkEgress::Proxy { .. }),
            "configured HTTP node proxy resolver is unavailable"
        );
        Ok(None)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeInternalToolRegistration {
    pub registration_id: String,
    pub provider_name: String,
    pub provider_tool: Value,
    pub owner: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeInternalToolOutput {
    pub content: Value,
    pub is_error: bool,
    pub event: Value,
}

#[async_trait]
pub trait RuntimeInternalToolInvoker: Send + Sync {
    fn registrations_for_node(&self, node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration>;

    async fn invoke_runtime_internal_tool(
        &self,
        node: &CompiledNode,
        registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput>;
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

    async fn invoke_notification_node(
        &self,
        _node: &CompiledNode,
        _resolved_inputs: &Map<String, Value>,
    ) -> Result<NativeNotificationInvocationOutput> {
        Err(anyhow!("notification runtime is not configured"))
    }

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
    /// Atomic scoped state transition; ordinary flows retain their existing hooks.
    async fn commit_container_transition(
        &self,
        _transition: &container::ContainerTransition<'_>,
    ) -> Result<()> { Ok(()) }

    async fn begin_node(&self, node: &CompiledNode, input_payload: &Value) -> Result<()>;

    /// The communication owner keeps this LLM running until final seal; this operation
    /// restores its active execution context without restarting a completed graph node.
    async fn activate_retained_communication_node(&self, _node: &CompiledNode) -> Result<()> {
        Ok(())
    }

    async fn communication_node_receipt(&self, _node: &CompiledNode) -> Result<Option<uuid::Uuid>> {
        Ok(None)
    }

    async fn runtime_internal_tool_started(
        &self,
        _node: &CompiledNode,
        _tool_call: &Value,
        _call_usage: &Value,
    ) -> Result<()> {
        Ok(())
    }

    async fn runtime_internal_tool_finished(
        &self,
        _node: &CompiledNode,
        _tool_call: &Value,
        _call_usage: &Value,
        _tool_result: &Value,
        _duration_ms: u64,
    ) -> Result<()> {
        Ok(())
    }

    async fn complete_node(&self, _trace: &NodeExecutionTrace) -> Result<()> {
        Ok(())
    }
}

pub(crate) struct NoopExecutionLifecycle;

#[async_trait]
impl ExecutionLifecycle for NoopExecutionLifecycle {
    async fn begin_node(&self, _node: &CompiledNode, _input_payload: &Value) -> Result<()> {
        Ok(())
    }
}

async fn complete_latest_node_trace(
    lifecycle: &dyn ExecutionLifecycle,
    node_traces: &[NodeExecutionTrace],
) -> Result<()> {
    let trace = node_traces
        .last()
        .ok_or_else(|| anyhow!("node execution completed without a trace"))?;
    lifecycle.complete_node(trace).await
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

#[derive(Default)]
pub(super) struct LlmRoundMetricsAccumulator {
    attempts: Vec<Value>,
    usage: ProviderUsage,
    next_round_index: usize,
}

impl LlmRoundMetricsAccumulator {
    pub(super) fn absorb(&mut self, execution: &LlmNodeExecution) {
        let Some(attempts) = execution
            .metrics_payload
            .get("attempts")
            .and_then(Value::as_array)
        else {
            return;
        };
        let inferred_round_index = self.next_round_index;
        for attempt in attempts {
            let mut attempt = attempt.clone();
            let round_index = attempt
                .get("provider_round_index")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(inferred_round_index);
            self.next_round_index = self.next_round_index.max(round_index.saturating_add(1));
            if let Some(attempt) = attempt.as_object_mut() {
                attempt.insert("provider_round_index".to_string(), json!(round_index));
                attempt.insert("attempt_index".to_string(), json!(self.attempts.len()));
            }
            if let Some(usage) = attempt
                .get("usage")
                .cloned()
                .and_then(|usage| serde_json::from_value::<ProviderUsage>(usage).ok())
            {
                add_provider_usage(&mut self.usage, &usage);
            }
            self.attempts.push(attempt);
        }
    }

    pub(super) fn apply(&self, execution: &mut LlmNodeExecution) {
        let Some(metrics) = execution.metrics_payload.as_object_mut() else {
            return;
        };
        let mut usage = self.usage.clone();
        if usage.total_tokens.is_none() {
            usage.total_tokens = usage.total_tokens();
        }
        metrics.insert("attempts".to_string(), Value::Array(self.attempts.clone()));
        metrics.insert(
            "usage".to_string(),
            serde_json::to_value(usage).unwrap_or(Value::Null),
        );
    }
}

fn add_provider_usage(target: &mut ProviderUsage, source: &ProviderUsage) {
    fn add(target: &mut Option<u64>, source: Option<u64>) {
        if let Some(source) = source {
            *target = Some(target.unwrap_or_default().saturating_add(source));
        }
    }

    add(&mut target.input_tokens, source.input_tokens);
    add(&mut target.output_tokens, source.output_tokens);
    add(&mut target.reasoning_tokens, source.reasoning_tokens);
    add(&mut target.cache_read_tokens, source.cache_read_tokens);
    add(&mut target.cache_write_tokens, source.cache_write_tokens);
    if let Some(buckets) = &source.cache_write_by_ttl_seconds {
        let totals = target
            .cache_write_by_ttl_seconds
            .get_or_insert_with(Default::default);
        for (ttl, quantity) in buckets {
            let total = totals.entry(ttl.clone()).or_default();
            *total = total.saturating_add(*quantity);
        }
    }
    add(
        &mut target.input_cache_hit_tokens,
        source.input_cache_hit_tokens,
    );
    add(
        &mut target.input_cache_miss_tokens,
        source.input_cache_miss_tokens,
    );
    add(&mut target.total_tokens, source.total_tokens);
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
pub struct NativeNotificationInvocationOutput {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
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

    // Keep the graph future on the heap across start/resume wrappers while awaiting in place.
    Box::pin(execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        &NoopExecutionLifecycle,
    ))
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

    Box::pin(execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        &NoopExecutionLifecycle,
    ))
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

    Box::pin(execute_from(
        plan,
        0,
        variable_pool,
        None,
        &runtime_context,
        invoker,
        lifecycle,
    ))
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
    if checkpoint.container_state.is_some() {
        let execution_id = container::waiting_execution_id(checkpoint, waiting_node_id)?;
        return container::resume_scoped_container_with_runtime_context_and_lifecycle(plan, checkpoint, execution_id, resume_payload, runtime_context, invoker, lifecycle).await;
    }
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
                    return Box::pin(execute_from(
                        plan,
                        checkpoint.next_node_index,
                        variable_pool,
                        Some(checkpoint.active_node_ids.iter().cloned().collect()),
                        &runtime_context,
                        invoker,
                        lifecycle,
                    ))
                    .await;
                }
                VisibleInternalLlmToolResume::Waiting(wait) => {
                    let wait = *wait;
                    let checkpoint_variable_pool = wait.checkpoint_variable_pool;
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                            node_id: wait.node_id.clone(),
                            node_alias: wait.node_alias.clone(),
                            callback_kind: LLM_TOOL_CALLBACK_KIND.to_string(),
                            request_payload: wait.request_payload,
                        }),
                        variable_pool: checkpoint_variable_pool.clone(),
                        checkpoint_snapshot: Some(CheckpointSnapshot {
                            container_state: None,
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
        return Box::pin(execute_from(
            plan,
            checkpoint.next_node_index,
            variable_pool,
            Some(checkpoint.active_node_ids.iter().cloned().collect()),
            &runtime_context,
            invoker,
            lifecycle,
        ))
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

    Box::pin(execute_from(
        plan,
        checkpoint.next_node_index,
        variable_pool,
        Some(checkpoint.active_node_ids.iter().cloned().collect()),
        &runtime_context,
        invoker,
        lifecycle,
    ))
    .await
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

#[path = "execution_engine/llm_operations/llm_executor.rs"]
mod llm_executor;
pub use llm_executor::execute_llm_node;
pub(super) use llm_executor::execute_llm_node_provider_round;
#[cfg(test)]
pub(crate) use llm_executor::full_jitter_delay_ms;

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

mod scheduler;
use scheduler::execute_from;
