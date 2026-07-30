use super::*;

pub(super) struct VisibleInternalLlmToolBranchContext<'a, I: ?Sized> {
    pub(super) plan: &'a CompiledPlan,
    pub(super) runtime_context: &'a ExecutionRuntimeContext,
    pub(super) invoker: &'a I,
    pub(super) main_node_id: &'a str,
    pub(super) tool_call: &'a Value,
    pub(super) tool: &'a VisibleInternalLlmTool,
}

pub(super) struct VisibleInternalLlmToolNodeContext<'a, I: ?Sized> {
    pub(super) plan: &'a CompiledPlan,
    pub(super) node: &'a CompiledNode,
    pub(super) resolved_inputs: &'a Map<String, Value>,
    pub(super) rendered_templates: &'a Map<String, Value>,
    pub(super) runtime_context: &'a ExecutionRuntimeContext,
    pub(super) invoker: &'a I,
}

pub(super) async fn execute_visible_internal_llm_tool_call<I>(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    main_node_id: &str,
    tool_call: &Value,
    tool: &VisibleInternalLlmTool,
) -> Result<VisibleInternalLlmToolBranchExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut tool_variable_pool = variable_pool.clone();
    let inherited_context =
        visible_internal_llm_tool_inherited_context(variable_pool, main_node_id);
    tool_variable_pool.insert(
        VISIBLE_INTERNAL_LLM_TOOL_VARIABLE.to_string(),
        json!({
            "tool_call_id": tool_call_id(tool_call),
            "tool_name": tool.name,
            "arguments": tool_call
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({})),
            "tool_mode": tool.tool_mode.as_str(),
            "external_tool_policy": tool.external_tool_policy.as_str(),
            "external_callback_policy": tool.external_callback_policy.as_str(),
            "execution_mode": tool.execution_mode.as_str(),
            "preconditions": visible_internal_llm_tool_preconditions_value(&tool.preconditions),
            "context": inherited_context,
        }),
    );

    execute_visible_internal_llm_tool_branch(
        plan,
        tool_variable_pool,
        runtime_context,
        invoker,
        main_node_id,
        tool_call,
        tool,
    )
    .await
}

async fn execute_visible_internal_llm_tool_branch<I>(
    plan: &CompiledPlan,
    variable_pool: Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    main_node_id: &str,
    tool_call: &Value,
    tool: &VisibleInternalLlmTool,
) -> Result<VisibleInternalLlmToolBranchExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut route_events = vec![visible_internal_llm_tool_route_event(
        "visible_internal_llm_tool_started",
        main_node_id,
        tool_call,
        tool,
        json!({}),
    )];

    if !plan.nodes.contains_key(&tool.target_node_id) {
        let error_payload = json!({
            "error_code": "visible_internal_llm_tool_failed",
            "message": "visible internal LLM tool branch entry node was not found",
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
        return Ok(VisibleInternalLlmToolBranchExecution::Failed {
            error_payload,
            route_events,
        });
    }

    let active_node_ids = tool.start_node_ids();

    continue_visible_internal_llm_tool_branch(
        VisibleInternalLlmToolBranchContext {
            plan,
            runtime_context,
            invoker,
            main_node_id,
            tool_call,
            tool,
        },
        variable_pool,
        active_node_ids,
        String::new(),
        Vec::new(),
        route_events,
    )
    .await
}

pub(super) async fn continue_visible_internal_llm_tool_branch<I>(
    context: VisibleInternalLlmToolBranchContext<'_, I>,
    mut variable_pool: Map<String, Value>,
    mut active_node_ids: BTreeSet<String>,
    mut branch_text: String,
    mut provider_events: Vec<ProviderStreamEvent>,
    mut route_events: Vec<Value>,
) -> Result<VisibleInternalLlmToolBranchExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let plan = context.plan;
    let runtime_context = context.runtime_context;
    let invoker = context.invoker;
    let main_node_id = context.main_node_id;
    let tool_call = context.tool_call;
    let tool = context.tool;

    for node_id in &plan.topological_order {
        if !active_node_ids.contains(node_id) {
            continue;
        }
        let Some(node) = plan.nodes.get(node_id) else {
            continue;
        };
        if let Some(panel_execution) = execute_bounded_parallel_panel(
            BoundedParallelPanelContext {
                plan,
                runtime_context,
                invoker,
                main_node_id,
                tool_call,
                tool,
            },
            node_id,
            &mut variable_pool,
            &mut active_node_ids,
        )
        .await?
        {
            match panel_execution {
                BoundedParallelPanelExecution::Completed(output) => {
                    branch_text = output.branch_text;
                    provider_events.extend(output.provider_events);
                    route_events.extend(output.route_events);
                    continue;
                }
                BoundedParallelPanelExecution::Waiting {
                    wait,
                    branch_text,
                    route_events,
                } => {
                    return Ok(VisibleInternalLlmToolBranchExecution::Waiting {
                        wait,
                        branch_text,
                        route_events,
                    });
                }
                BoundedParallelPanelExecution::Failed {
                    error_payload,
                    route_events,
                } => {
                    return Ok(VisibleInternalLlmToolBranchExecution::Failed {
                        error_payload,
                        route_events,
                    });
                }
            }
        }
        let resolved_inputs = match resolve_node_inputs(node, &variable_pool) {
            Ok(inputs) => inputs,
            Err(error) => {
                let error_payload = visible_internal_llm_tool_node_error(
                    tool_call,
                    tool,
                    node,
                    "visible internal LLM tool input resolution failed",
                    Some(error.to_string()),
                    None,
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
                return Ok(VisibleInternalLlmToolBranchExecution::Failed {
                    error_payload,
                    route_events,
                });
            }
        };
        let rendered_templates = render_templated_bindings(node, &resolved_inputs);
        let node_output = match execute_visible_internal_llm_tool_node(
            VisibleInternalLlmToolNodeContext {
                plan,
                node,
                resolved_inputs: &resolved_inputs,
                rendered_templates: &rendered_templates,
                runtime_context,
                invoker,
            },
            &mut variable_pool,
            &mut provider_events,
        )
        .await?
        {
            VisibleInternalLlmToolNodeExecution::Completed(output) => output,
            VisibleInternalLlmToolNodeExecution::Waiting(wait) => {
                let branch_text = format!(
                    "{}{}",
                    branch_text,
                    wait.node_trace
                        .as_ref()
                        .map(|trace| visible_internal_llm_tool_output_text(&trace.output_payload))
                        .unwrap_or_default()
                );
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
                return Ok(VisibleInternalLlmToolBranchExecution::Waiting {
                    wait,
                    branch_text,
                    route_events,
                });
            }
            VisibleInternalLlmToolNodeExecution::Failed(error_payload) => {
                let error_payload = visible_internal_llm_tool_node_error(
                    tool_call,
                    tool,
                    node,
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
                return Ok(VisibleInternalLlmToolBranchExecution::Failed {
                    error_payload,
                    route_events,
                });
            }
        };

        branch_text = visible_internal_llm_tool_output_text(&node_output.output_payload);
        if node.node_type == "llm" {
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
        }
        variable_pool.insert(
            node.node_id.clone(),
            project_node_variable_payload(node, &node_output.output_payload)?,
        );
        if node.node_type == TOOL_RESULT_NODE_TYPE {
            variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE);
            return Ok(VisibleInternalLlmToolBranchExecution::Completed(
                VisibleInternalLlmToolOutput {
                    text: branch_text,
                    provider_events,
                    route_events,
                    variable_pool,
                },
            ));
        }
        activate_downstream_nodes(plan, &mut active_node_ids, node, None);
    }

    variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE);
    Ok(VisibleInternalLlmToolBranchExecution::Completed(
        VisibleInternalLlmToolOutput {
            text: branch_text,
            provider_events,
            route_events,
            variable_pool,
        },
    ))
}

pub(super) async fn execute_visible_internal_llm_tool_node<I>(
    context: VisibleInternalLlmToolNodeContext<'_, I>,
    variable_pool: &mut Map<String, Value>,
    provider_events: &mut Vec<ProviderStreamEvent>,
) -> Result<VisibleInternalLlmToolNodeExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let VisibleInternalLlmToolNodeContext {
        plan,
        node,
        resolved_inputs,
        rendered_templates,
        runtime_context,
        invoker,
    } = context;
    match node.node_type.as_str() {
        "llm" => {
            let resolved_inputs =
                visible_internal_llm_tool_llm_resolved_inputs(resolved_inputs, variable_pool);
            if let Some(error_payload) =
                visible_internal_llm_tool_precondition_error(variable_pool).await
            {
                return Ok(VisibleInternalLlmToolNodeExecution::Failed(error_payload));
            }
            let mut execution = execute_llm_node_provider_round(
                plan,
                node,
                &resolved_inputs,
                rendered_templates,
                variable_pool,
                runtime_context,
                invoker,
            )
            .await?;
            provider_events.extend(execution.provider_events.clone());
            if let Some(mut error_payload) = execution.error_payload.take() {
                if let Some(message) = execution.recoverable_error_message.take() {
                    error_payload["message"] = Value::String(message);
                }
                return Ok(VisibleInternalLlmToolNodeExecution::Failed(error_payload));
            }
            if let Some(mut wait) = execution.pending_callback.take() {
                if visible_internal_llm_tool_blocks_external_callback(variable_pool) {
                    return Ok(VisibleInternalLlmToolNodeExecution::Failed(json!({
                        "error_code": "visible_internal_llm_tool_external_callback_forbidden",
                        "message": "visible internal LLM tool external callback is forbidden by the mounted tool policy",
                        "node_id": node.node_id,
                        "request_payload": wait.request_payload,
                    })));
                }
                wait.node_trace = Some(NodeExecutionTrace {
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: execution.output_payload.clone(),
                    error_payload: None,
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: execution.provider_events,
                });
                return Ok(VisibleInternalLlmToolNodeExecution::Waiting(Box::new(wait)));
            }

            Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
                VisibleInternalLlmToolNodeOutput {
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: execution.output_payload,
                    metrics_payload: Some(execution.metrics_payload),
                    debug_payload: Some(execution.debug_payload),
                },
            )))
        }
        "template_transform" | "answer" | TOOL_RESULT_NODE_TYPE => {
            let output_key = first_output_key(node);
            let output_value = rendered_templates
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
            Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
                VisibleInternalLlmToolNodeOutput::from_output_payload(template_output_payload(
                    node,
                    output_key,
                    output_value,
                    variable_pool,
                )),
            )))
        }
        "code" => {
            let execution = execute_code_node(plan, node, resolved_inputs, invoker).await?;
            if let Some(error_payload) = execution.error_payload {
                return Ok(VisibleInternalLlmToolNodeExecution::Failed(error_payload));
            }

            Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
                VisibleInternalLlmToolNodeOutput {
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: execution.output_payload,
                    metrics_payload: None,
                    debug_payload: Some(execution.debug_payload),
                },
            )))
        }
        "http_request" => {
            let execution =
                execute_http_request_node(node, resolved_inputs, variable_pool, None).await?;
            if let Some(error_payload) = execution.error_payload {
                return Ok(VisibleInternalLlmToolNodeExecution::Failed(error_payload));
            }

            Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
                VisibleInternalLlmToolNodeOutput::from_output_payload(execution.output_payload),
            )))
        }
        "plugin_node" => {
            let execution =
                execute_capability_plugin_node(node, resolved_inputs, rendered_templates, invoker)
                    .await?;
            if let Some(error_payload) = execution.error_payload {
                return Ok(VisibleInternalLlmToolNodeExecution::Failed(error_payload));
            }

            Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
                VisibleInternalLlmToolNodeOutput {
                    input_payload: Value::Object(resolved_inputs.clone()),
                    output_payload: execution.output_payload,
                    metrics_payload: Some(execution.metrics_payload),
                    debug_payload: Some(execution.debug_payload),
                },
            )))
        }
        "variable_assigner" => Ok(VisibleInternalLlmToolNodeExecution::Completed(Box::new(
            VisibleInternalLlmToolNodeOutput::from_output_payload(
                execute_variable_assignment_node(node, resolved_inputs, variable_pool)?,
            ),
        ))),
        unsupported => Ok(VisibleInternalLlmToolNodeExecution::Failed(json!({
            "message": format!("visible internal LLM tool branch node type {unsupported} is not supported"),
        }))),
    }
}

pub(super) fn visible_internal_llm_tool_preconditions_from_variable_pool(
    variable_pool: &Map<String, Value>,
) -> Vec<VisibleInternalLlmToolPrecondition> {
    variable_pool
        .get(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE)
        .and_then(|tool| tool.get("preconditions"))
        .map(|preconditions| {
            visible_internal_llm_tool_preconditions_from_value(Some(preconditions))
        })
        .unwrap_or_default()
}
