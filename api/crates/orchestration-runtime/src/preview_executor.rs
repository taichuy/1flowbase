use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::{
    binding_runtime::{render_templated_bindings, resolve_node_inputs},
    compiled_plan::CompiledPlan,
    execution_engine::{
        execute_code_node, execute_http_request_node_with_provider_invoker, execute_llm_node,
        execute_variable_assignment_node, materialize_start_builtin_defaults, resolved_native_sql,
        start_flow_debug_run_with_runtime_context_and_lifecycle,
        variable_aggregator::{
            execute_variable_aggregator_node, variable_aggregator_input_payload,
        },
        CapabilityInvoker, CodeInvoker, ExecutionLifecycle, ExecutionRuntimeContext,
        HttpResponseFilePersister, LlmRoutingCounterStore, NoopExecutionLifecycle, ProviderInvoker,
    },
    node_errors::build_node_type_not_implemented_error_payload,
};

pub struct NodePreviewOutcome {
    pub target_node_id: String,
    pub resolved_inputs: Map<String, Value>,
    pub rendered_templates: Map<String, Value>,
    pub output_contract: Vec<Value>,
    pub node_output: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
    pub provider_events: Vec<extension_contracts::provider_contract::ProviderStreamEvent>,
}

impl NodePreviewOutcome {
    pub fn as_payload(&self) -> Value {
        json!({
            "target_node_id": self.target_node_id,
            "resolved_inputs": self.resolved_inputs,
            "rendered_templates": self.rendered_templates,
            "output_contract": self.output_contract,
            "node_output": self.node_output,
            "error_payload": self.error_payload,
            "metrics_payload": self.metrics_payload,
            "debug_payload": self.debug_payload,
            "provider_events": self.provider_events,
        })
    }

    pub fn is_failed(&self) -> bool {
        self.error_payload.is_some()
    }
}

fn start_preview_output(resolved_inputs: &Map<String, Value>) -> Value {
    let mut output = resolved_inputs.clone();

    materialize_start_preview_defaults(&mut output);

    Value::Object(output)
}

fn materialize_start_preview_defaults(start_payload: &mut Map<String, Value>) {
    start_payload
        .entry("query".to_string())
        .or_insert_with(|| Value::String(String::new()));
    materialize_start_builtin_defaults(start_payload);
}

fn materialize_start_nodes_in_variable_pool(
    plan: &CompiledPlan,
    variable_pool: &mut Map<String, Value>,
) {
    for (node_id, node) in &plan.nodes {
        if node.node_type != "start" {
            continue;
        }

        let start_payload = variable_pool
            .entry(node_id.clone())
            .or_insert_with(|| Value::Object(Map::new()));

        if let Some(start_payload) = start_payload.as_object_mut() {
            materialize_start_preview_defaults(start_payload);
        }
    }
}

pub async fn run_node_preview<I>(
    plan: &CompiledPlan,
    target_node_id: &str,
    input_payload: &Value,
    invoker: &I,
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    run_node_preview_with_http_file_persister(plan, target_node_id, input_payload, invoker, None)
        .await
}

pub async fn run_node_preview_with_http_file_persister<I>(
    plan: &CompiledPlan,
    target_node_id: &str,
    input_payload: &Value,
    invoker: &I,
    http_file_persister: Option<&dyn HttpResponseFilePersister>,
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut variable_pool = input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;
    materialize_start_nodes_in_variable_pool(plan, &mut variable_pool);
    let runtime_context = ExecutionRuntimeContext::from_plan_input(plan, &variable_pool)?;
    run_node_preview_with_prepared_context(
        plan,
        target_node_id,
        variable_pool,
        runtime_context,
        invoker,
        http_file_persister,
        &NoopExecutionLifecycle,
    )
    .await
}

pub async fn run_node_preview_with_http_file_persister_and_counter_store<I>(
    plan: &CompiledPlan,
    target_node_id: &str,
    input_payload: &Value,
    invoker: &I,
    http_file_persister: Option<&dyn HttpResponseFilePersister>,
    llm_routing_counter_store: Option<Arc<dyn LlmRoutingCounterStore>>,
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    run_node_preview_with_http_file_persister_and_counter_store_and_lifecycle(
        plan,
        target_node_id,
        input_payload,
        invoker,
        http_file_persister,
        llm_routing_counter_store,
        &NoopExecutionLifecycle,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_node_preview_with_http_file_persister_and_counter_store_and_lifecycle<I>(
    plan: &CompiledPlan,
    target_node_id: &str,
    input_payload: &Value,
    invoker: &I,
    http_file_persister: Option<&dyn HttpResponseFilePersister>,
    llm_routing_counter_store: Option<Arc<dyn LlmRoutingCounterStore>>,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut variable_pool = input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;
    materialize_start_nodes_in_variable_pool(plan, &mut variable_pool);
    let runtime_context = match llm_routing_counter_store {
        Some(store) => ExecutionRuntimeContext::from_plan_input(plan, &variable_pool)?
            .with_llm_routing_counter_store(store),
        None => ExecutionRuntimeContext::from_plan_input(plan, &variable_pool)?,
    };
    run_node_preview_with_prepared_context(
        plan,
        target_node_id,
        variable_pool,
        runtime_context,
        invoker,
        http_file_persister,
        lifecycle,
    )
    .await
}

async fn run_node_preview_with_prepared_context<I>(
    plan: &CompiledPlan,
    target_node_id: &str,
    mut variable_pool: Map<String, Value>,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    http_file_persister: Option<&dyn HttpResponseFilePersister>,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let node = plan
        .nodes
        .get(target_node_id)
        .ok_or_else(|| anyhow!("target node not found: {target_node_id}"))?;
    let mut resolved_inputs = if node.node_type == "start" {
        variable_pool
            .get(target_node_id)
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default()
    } else if node.node_type == "variable_aggregator" {
        variable_aggregator_input_payload(node)?
    } else {
        resolve_node_inputs(node, &variable_pool)?
    };
    if !matches!(
        node.node_type.as_str(),
        "template_transform" | "workflow_start" | "workflow_end"
    ) {
        lifecycle
            .begin_node(node, &Value::Object(resolved_inputs.clone()))
            .await?;
    }
    let rendered_templates = render_templated_bindings(node, &resolved_inputs);
    let output_contract = node
        .outputs
        .iter()
        .map(|output| {
            json!({
                "key": output.key,
                "title": output.title,
                "value_type": output.value_type,
            })
        })
        .collect();

    let (node_output, error_payload, metrics_payload, debug_payload, provider_events) = if node
        .node_type
        == "start"
    {
        (
            start_preview_output(&resolved_inputs),
            None,
            json!({ "preview_mode": true }),
            json!({}),
            Vec::new(),
        )
    } else if node.node_type == "llm" {
        let execution = execute_llm_node(
            plan,
            node,
            &resolved_inputs,
            &rendered_templates,
            &mut variable_pool,
            &runtime_context,
            invoker,
            &NoopExecutionLifecycle,
        )
        .await?;
        (
            execution.output_payload,
            execution.error_payload,
            execution.metrics_payload,
            execution.debug_payload,
            execution.provider_events,
        )
    } else if node.node_type == "code" {
        let execution = execute_code_node(plan, node, &resolved_inputs, invoker).await?;
        (
            execution.output_payload,
            execution.error_payload,
            execution.metrics_payload,
            execution.debug_payload,
            Vec::new(),
        )
    } else if node.node_type == "variable_assigner" {
        let execution =
            execute_variable_assignment_node(node, &resolved_inputs, &mut variable_pool)?;
        (
            execution,
            None,
            json!({ "preview_mode": true }),
            json!({}),
            Vec::new(),
        )
    } else if node.node_type == "variable_aggregator" {
        let execution = execute_variable_aggregator_node(node, &variable_pool)?;
        (
            execution.output_payload,
            execution.error_payload,
            json!({ "preview_mode": true }),
            execution.debug_payload,
            Vec::new(),
        )
    } else if node.node_type == "http_request" {
        let execution = execute_http_request_node_with_provider_invoker(
            node,
            &resolved_inputs,
            &variable_pool,
            http_file_persister,
            Some(invoker),
        )
        .await?;
        (
            execution.output_payload,
            execution.error_payload,
            execution.metrics_payload,
            execution.debug_payload,
            Vec::new(),
        )
    } else if matches!(
        node.node_type.as_str(),
        "data_model_get"
            | "data_model_list"
            | "data_model_create"
            | "data_model_update"
            | "data_model_delete"
    ) {
        let execution = invoker
            .invoke_data_model_node(node, &resolved_inputs)
            .await?;
        // Preview is a single invocation, so it cannot own a callback checkpoint.
        // Preserve the invoker's policy decision; never turn a confirmation request into a write.
        let error_payload = execution.error_payload.or_else(|| {
            execution.pending_callback.as_ref().map(|callback| json!({
                "error_code": "node_preview_callback_not_supported",
                "message": "Single node preview cannot resume a pending callback; use a workflow debug run",
                "callback_kind": callback.callback_kind,
            }))
        });
        (
            execution.output_payload,
            error_payload,
            execution.metrics_payload,
            execution.debug_payload,
            Vec::new(),
        )
    } else if matches!(
        node.node_type.as_str(),
        "template_transform" | "workflow_start" | "workflow_end"
    ) {
        // Use the graph executor's existing projection and boundary materialization,
        // with exactly one active target and the caller's explicit variable pool.
        let mut target_plan = plan.clone();
        target_plan.topological_order = vec![node.node_id.clone()];
        target_plan.edges.clear();
        target_plan
            .nodes
            .retain(|node_id, _| node_id == target_node_id);
        let execution = start_flow_debug_run_with_runtime_context_and_lifecycle(
            &target_plan,
            &Value::Object(variable_pool),
            runtime_context,
            invoker,
            &NoopExecutionLifecycle,
        )
        .await?;
        let trace = execution
            .node_traces
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("preview target did not execute: {target_node_id}"))?;
        lifecycle.begin_node(node, &trace.input_payload).await?;
        resolved_inputs = trace
            .input_payload
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow!("preview target input must be an object"))?;
        (
            trace.output_payload,
            trace.error_payload,
            trace.metrics_payload,
            trace.debug_payload,
            trace.provider_events,
        )
    } else if node.node_type == "sql" {
        let execution = invoker
            .invoke_native_sql_node(node, resolved_native_sql(&resolved_inputs)?)
            .await?;
        (
            execution.output_payload,
            execution.error_payload,
            execution.metrics_payload,
            execution.debug_payload,
            Vec::new(),
        )
    } else {
        let error_payload = Some(build_node_type_not_implemented_error_payload(
            &node.node_type,
            "preview",
        ));
        (
            json!({}),
            error_payload,
            json!({ "preview_mode": true }),
            json!({}),
            Vec::new(),
        )
    };

    Ok(NodePreviewOutcome {
        target_node_id: node.node_id.clone(),
        resolved_inputs,
        rendered_templates,
        output_contract,
        node_output,
        error_payload,
        metrics_payload,
        debug_payload,
        provider_events,
    })
}
