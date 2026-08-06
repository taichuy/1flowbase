use futures_util::future::join_all;

use super::*;

const MAX_FUSION_PANEL_CONCURRENCY: usize = 4;

pub(super) struct BoundedParallelPanelOutput {
    pub(super) branch_text: String,
    pub(super) provider_events: Vec<ProviderStreamEvent>,
    pub(super) route_events: Vec<Value>,
}

pub(super) enum BoundedParallelPanelExecution {
    Completed(BoundedParallelPanelOutput),
    Waiting {
        wait: Box<LlmToolCallbackWait>,
        branch_text: String,
        route_events: Vec<Value>,
    },
    Failed {
        error_payload: Value,
        route_events: Vec<Value>,
    },
}

pub(super) struct BoundedParallelPanelContext<'a, I: ?Sized> {
    pub(super) plan: &'a CompiledPlan,
    pub(super) runtime_context: &'a ExecutionRuntimeContext,
    pub(super) invoker: &'a I,
    pub(super) main_node_id: &'a str,
    pub(super) tool_call: &'a Value,
    pub(super) tool: &'a VisibleInternalLlmTool,
}

pub(super) async fn execute_bounded_parallel_panel<I>(
    context: BoundedParallelPanelContext<'_, I>,
    current_node_id: &str,
    variable_pool: &mut Map<String, Value>,
    active_node_ids: &mut BTreeSet<String>,
) -> Result<Option<BoundedParallelPanelExecution>>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let plan = context.plan;
    let runtime_context = context.runtime_context;
    let invoker = context.invoker;
    let main_node_id = context.main_node_id;
    let tool_call = context.tool_call;
    let tool = context.tool;

    if tool.execution_mode != VisibleInternalLlmToolExecutionMode::BoundedParallelPanel {
        return Ok(None);
    }
    if plan
        .nodes
        .get(current_node_id)
        .map(|node| node.node_type.as_str())
        != Some("llm")
    {
        return Ok(None);
    }

    let panel_nodes = plan
        .topological_order
        .iter()
        .filter(|node_id| active_node_ids.contains(*node_id))
        .filter_map(|node_id| plan.nodes.get(node_id))
        .filter(|node| node.node_type == "llm")
        .take(MAX_FUSION_PANEL_CONCURRENCY)
        .cloned()
        .collect::<Vec<_>>();
    if panel_nodes.len() < 2 {
        return Ok(None);
    }

    let executions = join_all(panel_nodes.into_iter().map(|node| {
        let mut panel_variable_pool = variable_pool.clone();
        async move {
            let resolved_inputs = match if node.node_type == "variable_aggregator" {
                variable_aggregator_input_payload(&node)
            } else {
                resolve_node_inputs(&node, &panel_variable_pool)
            } {
                Ok(inputs) => inputs,
                Err(error) => {
                    let error_payload = visible_internal_llm_tool_node_error(
                        tool_call,
                        tool,
                        &node,
                        "visible internal LLM tool input resolution failed",
                        Some(error.to_string()),
                        None,
                    );
                    return Ok((
                        node,
                        VisibleInternalLlmToolNodeExecution::Failed(error_payload),
                        Vec::new(),
                    ));
                }
            };
            let rendered_templates = render_templated_bindings(&node, &resolved_inputs);
            let mut provider_events = Vec::new();
            let execution = execute_visible_internal_llm_tool_node(
                VisibleInternalLlmToolNodeContext {
                    plan,
                    node: &node,
                    resolved_inputs: &resolved_inputs,
                    rendered_templates: &rendered_templates,
                    runtime_context,
                    invoker,
                },
                &mut panel_variable_pool,
                &mut provider_events,
            )
            .await?;

            Ok::<_, anyhow::Error>((node, execution, provider_events))
        }
    }))
    .await;

    let mut branch_text = String::new();
    let mut provider_events = Vec::new();
    let mut route_events = Vec::new();

    for execution in executions {
        let (node, execution, node_provider_events) = match execution {
            Ok(output) => output,
            Err(error) => {
                let error_payload = json!({
                    "error_code": "visible_internal_llm_tool_failed",
                    "message": "visible internal LLM tool parallel panel execution failed",
                    "runtime_message": error.to_string(),
                    "tool_call_id": tool_call_id(tool_call),
                    "tool_name": tool.name,
                    "target_node_id": tool.target_node_id,
                });
                route_events.push(visible_internal_llm_tool_route_event(
                    "visible_internal_llm_tool_failed",
                    main_node_id,
                    tool_call,
                    tool,
                    json!({ "error_payload": error_payload }),
                ));
                return Ok(Some(BoundedParallelPanelExecution::Failed {
                    error_payload,
                    route_events,
                }));
            }
        };
        provider_events.extend(node_provider_events);
        active_node_ids.remove(&node.node_id);

        match execution {
            VisibleInternalLlmToolNodeExecution::Completed(node_output) => {
                branch_text = visible_internal_llm_tool_output_text(&node_output.output_payload);
                route_events.push(visible_internal_llm_tool_route_event(
                    "visible_internal_llm_tool_completed",
                    main_node_id,
                    tool_call,
                    tool,
                    json!({
                        "route_node_id": node.node_id,
                        "node_id": node.node_id,
                        "node_alias": node.alias,
                        "node_type": node.node_type,
                        "input_payload": node_output.input_payload.clone(),
                        "output_payload": node_output.output_payload.clone(),
                        "provider_route": node_output.output_payload
                            .get("provider_route")
                            .cloned()
                            .unwrap_or(Value::Null),
                        "metrics_payload": node_output.metrics_payload.clone(),
                        "debug_payload": node_output.debug_payload.clone(),
                        "content": branch_text.clone(),
                    }),
                ));
                variable_pool.insert(
                    node.node_id.clone(),
                    project_node_variable_payload(&node, &node_output.output_payload)?,
                );
                activate_downstream_nodes(plan, active_node_ids, &node, None);
            }
            VisibleInternalLlmToolNodeExecution::Waiting(wait) => {
                let waiting_branch_text = wait
                    .node_trace
                    .as_ref()
                    .map(|trace| visible_internal_llm_tool_output_text(&trace.output_payload))
                    .unwrap_or_default();
                route_events.push(visible_internal_llm_tool_route_event(
                    "visible_internal_llm_tool_waiting_callback",
                    main_node_id,
                    tool_call,
                    tool,
                    json!({
                        "waiting_node_id": wait.node_id,
                        "waiting_node_alias": wait.node_alias,
                        "request_payload": wait.request_payload,
                    }),
                ));
                return Ok(Some(BoundedParallelPanelExecution::Waiting {
                    wait,
                    branch_text: waiting_branch_text,
                    route_events,
                }));
            }
            VisibleInternalLlmToolNodeExecution::Failed(error_payload) => {
                let error_payload = visible_internal_llm_tool_node_error(
                    tool_call,
                    tool,
                    &node,
                    "visible internal LLM tool branch node failed",
                    None,
                    Some(error_payload),
                );
                route_events.push(visible_internal_llm_tool_route_event(
                    "visible_internal_llm_tool_failed",
                    main_node_id,
                    tool_call,
                    tool,
                    json!({
                        "route_node_id": node.node_id,
                        "node_id": node.node_id,
                        "node_alias": node.alias,
                        "node_type": node.node_type,
                        "error_payload": error_payload,
                    }),
                ));
                return Ok(Some(BoundedParallelPanelExecution::Failed {
                    error_payload,
                    route_events,
                }));
            }
        }
    }

    Ok(Some(BoundedParallelPanelExecution::Completed(
        BoundedParallelPanelOutput {
            branch_text,
            provider_events,
            route_events,
        },
    )))
}
