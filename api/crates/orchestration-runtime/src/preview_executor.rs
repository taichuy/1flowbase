use std::{collections::BTreeSet, sync::Arc};

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};

use crate::{
    binding_runtime::{render_templated_bindings, resolve_node_inputs},
    compiled_plan::CompiledPlan,
    execution_engine::{
        branching::{
            activate_downstream_nodes, initial_active_node_ids, select_if_else_source_handle,
        },
        execute_code_node, execute_http_request_node, execute_llm_node,
        execute_variable_assignment_node, materialize_start_builtin_defaults, CapabilityInvoker,
        CodeInvoker, ExecutionRuntimeContext, HttpResponseFilePersister, LlmRoutingCounterStore,
        ProviderInvoker,
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
    pub provider_events: Vec<plugin_framework::provider_contract::ProviderStreamEvent>,
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

fn collect_preview_execution_scope(plan: &CompiledPlan, target_node_id: &str) -> BTreeSet<String> {
    let mut scope = BTreeSet::new();
    let mut pending = vec![target_node_id.to_string()];

    while let Some(node_id) = pending.pop() {
        if !scope.insert(node_id.clone()) {
            continue;
        }
        if let Some(node) = plan.nodes.get(&node_id) {
            pending.extend(node.dependency_node_ids.iter().cloned());
        }
    }

    scope
}

fn replay_deterministic_upstream_state(
    plan: &CompiledPlan,
    target_node_id: &str,
    variable_pool: &mut Map<String, Value>,
) -> Result<()> {
    let execution_scope = collect_preview_execution_scope(plan, target_node_id);
    let mut active_node_ids = initial_active_node_ids(plan);

    for node_id in &plan.topological_order {
        if node_id == target_node_id {
            break;
        }
        if !execution_scope.contains(node_id) || !active_node_ids.contains(node_id) {
            continue;
        }
        let Some(node) = plan.nodes.get(node_id) else {
            continue;
        };

        let selected_source_handle = match node.node_type.as_str() {
            "if_else" => select_if_else_source_handle(node, variable_pool)?,
            "variable_assigner" => {
                let resolved_inputs = resolve_node_inputs(node, variable_pool)?;
                let output_payload =
                    execute_variable_assignment_node(node, &resolved_inputs, variable_pool)?;
                variable_pool.insert(node.node_id.clone(), output_payload);
                None
            }
            _ => None,
        };
        activate_downstream_nodes(
            plan,
            &mut active_node_ids,
            node,
            selected_source_handle.as_deref(),
        );
    }

    if !active_node_ids.contains(target_node_id) {
        return Err(anyhow!(
            "target node is inactive for supplied preview input: {target_node_id}"
        ));
    }

    Ok(())
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
) -> Result<NodePreviewOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    replay_deterministic_upstream_state(plan, target_node_id, &mut variable_pool)?;
    let node = plan
        .nodes
        .get(target_node_id)
        .ok_or_else(|| anyhow!("target node not found: {target_node_id}"))?;
    let resolved_inputs = if node.node_type == "start" {
        variable_pool
            .get(target_node_id)
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default()
    } else {
        resolve_node_inputs(node, &variable_pool)?
    };
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
        let execution = execute_code_node(node, &resolved_inputs, invoker).await?;
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
    } else if node.node_type == "http_request" {
        let execution =
            execute_http_request_node(node, &resolved_inputs, &variable_pool, http_file_persister)
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
