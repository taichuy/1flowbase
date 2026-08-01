use super::*;

mod branch_execution;
mod callback_state;
mod media_context;
mod panel_execution;
mod payloads;
mod registry;
mod types;
use self::branch_execution::{
    continue_visible_internal_llm_tool_branch, execute_visible_internal_llm_tool_call,
    execute_visible_internal_llm_tool_node,
    visible_internal_llm_tool_preconditions_from_variable_pool,
    VisibleInternalLlmToolBranchContext, VisibleInternalLlmToolNodeContext,
};
pub(super) use self::callback_state::has_visible_internal_llm_tool_callback_state;
use self::callback_state::{
    insert_visible_internal_llm_tool_callback_state, visible_internal_llm_tool_callback_state,
    VisibleInternalLlmToolCallbackStateInput,
};
pub(super) use self::media_context::{
    inject_visible_internal_llm_tool_media_content_blocks,
    project_visible_internal_llm_tool_shared_messages,
    visible_internal_llm_tool_blocks_external_tools,
};
use self::media_context::{
    visible_internal_llm_tool_blocks_external_callback, visible_internal_llm_tool_execution_mode,
    visible_internal_llm_tool_external_callback_policy,
    visible_internal_llm_tool_external_tool_policy, visible_internal_llm_tool_inherited_context,
    visible_internal_llm_tool_llm_resolved_inputs, visible_internal_llm_tool_mode,
    visible_internal_llm_tool_precondition_error,
};
use self::panel_execution::{
    execute_bounded_parallel_panel, BoundedParallelPanelContext, BoundedParallelPanelExecution,
};
use self::payloads::*;
use self::registry::visible_internal_llm_tools;
pub(super) use self::registry::{
    is_visible_internal_llm_tool_source_handle, visible_internal_llm_media_tool_context,
    visible_internal_llm_node_has_media_tool, visible_internal_llm_provider_tools,
    visible_internal_llm_tool_target_node_ids,
};
use self::types::*;

const VISIBLE_INTERNAL_LLM_TOOL_TYPE: &str = "visible_internal_llm_tool";
const VISIBLE_INTERNAL_LLM_TOOL_VARIABLE: &str = "visible_internal_llm_tool";
const VISIBLE_INTERNAL_LLM_TOOL_SOURCE_HANDLE_PREFIX: &str = "visible_internal_llm_tool:";
const VISIBLE_INTERNAL_LLM_TOOL_CALLBACK_STATE_KEY: &str = "__visible_internal_llm_tool_callback";
const MAX_VISIBLE_INTERNAL_LLM_TOOL_ROUNDS: usize = 8;
const TOOL_RESULT_NODE_TYPE: &str = "tool_result";
const TOOL_MODE_AGENT: &str = "agent";
const TOOL_MODE_FUSION: &str = "fusion";
const EXTERNAL_TOOL_POLICY_FORBIDDEN: &str = "forbidden";
const EXTERNAL_TOOL_POLICY_INHERITED: &str = "inherited";
const EXTERNAL_CALLBACK_POLICY_FORBIDDEN: &str = "forbidden";
const EXTERNAL_CALLBACK_POLICY_INHERITED: &str = "inherited";
const EXECUTION_MODE_SEQUENTIAL_RESUME: &str = "sequential_resume";
const EXECUTION_MODE_BOUNDED_PARALLEL_PANEL: &str = "bounded_parallel_panel";
const VISIBLE_INTERNAL_LLM_TOOL_PRECONDITION_MEDIA_CONTENT_AVAILABLE: &str =
    "media_content_available";

pub(super) enum VisibleInternalLlmToolResume {
    Ready(Map<String, Value>),
    Waiting(Box<LlmToolCallbackWait>),
    Failed {
        node_id: String,
        node_alias: String,
        execution: Box<LlmNodeExecution>,
    },
}
struct VisibleInternalLlmToolRemainingContext<'a, I: ?Sized> {
    plan: &'a CompiledPlan,
    variable_pool: &'a Map<String, Value>,
    runtime_context: &'a ExecutionRuntimeContext,
    invoker: &'a I,
    main_node_id: &'a str,
}

fn commit_conversation_namespace(
    variable_pool: &mut Map<String, Value>,
    branch_variable_pool: &Map<String, Value>,
) {
    if let Some(conversation) = branch_variable_pool.get("conversation") {
        variable_pool.insert("conversation".to_string(), conversation.clone());
    }
}

pub(super) async fn execute_llm_node_with_visible_internal_tools<I>(
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
    let tools = visible_internal_llm_tools(node);
    if tools.is_empty() {
        return execute_llm_node_provider_round(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
            invoker,
        )
        .await;
    }

    let mut llm_variable_pool = variable_pool.clone();
    let mut visible_transcript =
        pending_llm_tool_callback_visible_internal_transcript(node, variable_pool)
            .unwrap_or_default();
    let mut route_events = pending_llm_tool_callback_visible_internal_events(node, variable_pool);
    let mut provider_events = Vec::new();

    for round_index in 0..MAX_VISIBLE_INTERNAL_LLM_TOOL_ROUNDS {
        let mut execution = execute_llm_node_provider_round(
            plan,
            node,
            resolved_inputs,
            rendered_templates,
            &llm_variable_pool,
            runtime_context,
            invoker,
        )
        .await?;
        provider_events.extend(execution.provider_events.clone());

        if execution.error_payload.is_some() {
            if !provider_events.is_empty() {
                execution.provider_events = provider_events;
            }
            return Ok(execution);
        }

        let Some(tool_calls) = output_tool_calls(&execution.output_payload) else {
            append_output_text(&mut visible_transcript, &execution.output_payload);
            return Ok(execution_with_visible_transcript(
                execution,
                visible_transcript,
                provider_events,
                route_events,
            ));
        };

        let internal_tool_calls = visible_internal_tool_calls(&tool_calls, &tools);
        if internal_tool_calls.is_empty() {
            if !provider_events.is_empty() {
                execution.provider_events = provider_events;
            }
            attach_visible_internal_llm_tool_events(&mut execution, &route_events);
            return Ok(execution);
        }
        let mut callback_wait = execution.pending_callback.take().ok_or_else(|| {
            anyhow!(
                "provider tool-call result is missing canonical callback transcript for {}",
                node.node_id
            )
        })?;
        let internal_call_ids = internal_tool_calls
            .iter()
            .map(|(tool_call, _)| tool_call_id(tool_call))
            .collect::<BTreeSet<_>>();
        let external_tool_calls = tool_calls
            .iter()
            .filter(|tool_call| !internal_call_ids.contains(&tool_call_id(tool_call)))
            .cloned()
            .collect::<Vec<_>>();
        if internal_tool_calls.len() != tool_calls.len() {
            if visible_internal_media_tool_calls_are_repeated_after_route(
                &internal_tool_calls,
                &route_events,
            ) {
                for (tool_call, tool) in &internal_tool_calls {
                    route_events.push(visible_internal_llm_tool_route_event(
                        "visible_internal_llm_tool_ignored_repeated_media_call",
                        &node.node_id,
                        tool_call,
                        tool,
                        json!({}),
                    ));
                }
                remove_visible_internal_tool_calls(
                    &mut execution.output_payload,
                    &internal_tool_calls,
                );
                let mut checkpoint_variable_pool =
                    std::mem::take(&mut callback_wait.checkpoint_variable_pool);
                discard_llm_tool_callback_calls(
                    &mut checkpoint_variable_pool,
                    &node.node_id,
                    &internal_call_ids,
                    &external_tool_calls,
                )?;
                refresh_llm_tool_callback_wait_from_checkpoint(
                    &mut callback_wait,
                    checkpoint_variable_pool,
                )?;
                execution.pending_callback = Some(callback_wait);
                if !provider_events.is_empty() {
                    execution.provider_events = provider_events;
                }
                attach_visible_internal_llm_tool_events(&mut execution, &route_events);
                return Ok(execution);
            }

            // Mixed round: run hidden internal calls inline, splice their results
            // into the pending history, and hand only the external calls to the
            // normal client callback wait.
            append_output_text(&mut visible_transcript, &execution.output_payload);
            llm_variable_pool = std::mem::take(&mut callback_wait.checkpoint_variable_pool);
            set_pending_llm_tool_callback_visible_internal_transcript(
                &mut llm_variable_pool,
                &node.node_id,
                visible_transcript.clone(),
            )?;

            let mut internal_tool_results = Vec::new();
            for (tool_call, tool) in &internal_tool_calls {
                match execute_visible_internal_llm_tool_call(
                    plan,
                    &llm_variable_pool,
                    runtime_context,
                    invoker,
                    &node.node_id,
                    tool_call,
                    tool,
                )
                .await?
                {
                    VisibleInternalLlmToolBranchExecution::Completed(output) => {
                        llm_variable_pool = output.variable_pool;
                        commit_conversation_namespace(variable_pool, &llm_variable_pool);
                        provider_events.extend(output.provider_events);
                        route_events.extend(output.route_events);
                        visible_transcript.push_str(&output.text);
                        internal_tool_results.push(visible_internal_llm_tool_result(
                            tool_call,
                            &tool.name,
                            output.text,
                        ));
                    }
                    VisibleInternalLlmToolBranchExecution::Waiting {
                        route_events: waiting_route_events,
                        ..
                    } => {
                        route_events.extend(waiting_route_events);
                        let error_payload = json!({
                            "error_code": "visible_internal_llm_tool_mixed_round_callback_unavailable",
                            "message": "visible internal LLM tool branch requires an external tool callback and cannot run alongside client tool calls; call this tool again in its own round",
                        });
                        route_events.push(visible_internal_llm_tool_route_event(
                            "visible_internal_llm_tool_failed",
                            &node.node_id,
                            tool_call,
                            tool,
                            json!({ "error_payload": error_payload }),
                        ));
                        let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                            tool_call,
                            &tool.name,
                            &error_payload,
                        ) else {
                            return visible_internal_llm_tool_failure(
                                node,
                                provider_events,
                                error_payload,
                                route_events,
                            );
                        };
                        internal_tool_results.push(tool_result);
                    }
                    VisibleInternalLlmToolBranchExecution::Failed {
                        error_payload,
                        route_events: failed_route_events,
                    } => {
                        route_events.extend(failed_route_events);
                        let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                            tool_call,
                            &tool.name,
                            &error_payload,
                        ) else {
                            return visible_internal_llm_tool_failure(
                                node,
                                provider_events,
                                error_payload,
                                route_events,
                            );
                        };
                        internal_tool_results.push(tool_result);
                    }
                }
            }

            apply_mixed_llm_tool_callback_results(
                &mut llm_variable_pool,
                &node.node_id,
                &internal_tool_results,
                &external_tool_calls,
            )?;
            set_pending_llm_tool_callback_visible_internal_transcript(
                &mut llm_variable_pool,
                &node.node_id,
                visible_transcript.clone(),
            )?;
            set_pending_llm_tool_callback_visible_internal_events(
                &mut llm_variable_pool,
                &node.node_id,
                route_events.clone(),
            )?;

            remove_visible_internal_tool_calls(&mut execution.output_payload, &internal_tool_calls);
            if let Some(output) = execution.output_payload.as_object_mut() {
                output.insert(
                    "text".to_string(),
                    Value::String(visible_transcript.clone()),
                );
            }
            refresh_llm_tool_callback_wait_from_checkpoint(&mut callback_wait, llm_variable_pool)?;
            let mut pending_execution = execution_with_visible_transcript(
                execution,
                visible_transcript,
                provider_events,
                route_events,
            );
            pending_execution.pending_callback = Some(callback_wait);
            return Ok(pending_execution);
        }

        append_output_text(&mut visible_transcript, &execution.output_payload);
        llm_variable_pool = callback_wait.checkpoint_variable_pool;
        set_pending_llm_tool_callback_visible_internal_transcript(
            &mut llm_variable_pool,
            &node.node_id,
            visible_transcript.clone(),
        )?;
        let mut tool_results = Vec::new();
        for (index, (tool_call, tool)) in internal_tool_calls.iter().enumerate() {
            let target_output = match execute_visible_internal_llm_tool_call(
                plan,
                &llm_variable_pool,
                runtime_context,
                invoker,
                &node.node_id,
                tool_call,
                tool,
            )
            .await?
            {
                VisibleInternalLlmToolBranchExecution::Completed(output) => output,
                VisibleInternalLlmToolBranchExecution::Waiting {
                    mut wait,
                    branch_text,
                    route_events: waiting_route_events,
                } => {
                    route_events.extend(waiting_route_events);
                    let remaining_tool_calls = internal_tool_calls
                        .iter()
                        .skip(index + 1)
                        .map(|(tool_call, tool)| VisibleInternalLlmToolPendingCall {
                            tool_call: tool_call.clone(),
                            tool: (*tool).clone(),
                        })
                        .collect::<Vec<_>>();
                    insert_visible_internal_llm_tool_callback_state(
                        &mut wait.checkpoint_variable_pool,
                        VisibleInternalLlmToolCallbackStateInput {
                            main_node_id: &node.node_id,
                            tool_call,
                            tool_name: &tool.name,
                            target_node_id: &tool.target_node_id,
                            main_visible_transcript: &visible_transcript,
                            branch_text: &branch_text,
                            route_events: &route_events,
                            completed_tool_results: &tool_results,
                            remaining_tool_calls: &remaining_tool_calls,
                        },
                    );
                    let pending_visible_transcript = format!("{visible_transcript}{branch_text}");
                    let mut pending_execution = execution_with_visible_transcript(
                        execution,
                        pending_visible_transcript,
                        provider_events,
                        route_events,
                    );
                    pending_execution.pending_callback = Some(*wait);
                    return Ok(pending_execution);
                }
                VisibleInternalLlmToolBranchExecution::Failed {
                    error_payload,
                    route_events: failed_route_events,
                } => {
                    route_events.extend(failed_route_events);
                    if let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                        tool_call,
                        &tool.name,
                        &error_payload,
                    ) {
                        tool_results.push(tool_result);
                        continue;
                    }
                    return visible_internal_llm_tool_failure(
                        node,
                        provider_events,
                        error_payload,
                        route_events,
                    );
                }
            };
            llm_variable_pool = target_output.variable_pool;
            commit_conversation_namespace(variable_pool, &llm_variable_pool);
            provider_events.extend(target_output.provider_events);
            route_events.extend(target_output.route_events);
            visible_transcript.push_str(&target_output.text);
            tool_results.push(visible_internal_llm_tool_result(
                tool_call,
                &tool.name,
                target_output.text,
            ));
        }

        append_llm_tool_result_messages(
            &mut llm_variable_pool,
            &node.node_id,
            &json!({ "tool_results": tool_results }),
        )?;
        set_pending_llm_tool_callback_visible_internal_transcript(
            &mut llm_variable_pool,
            &node.node_id,
            visible_transcript.clone(),
        )?;
        set_pending_llm_tool_callback_visible_internal_events(
            &mut llm_variable_pool,
            &node.node_id,
            route_events.clone(),
        )?;

        if round_index + 1 == MAX_VISIBLE_INTERNAL_LLM_TOOL_ROUNDS {
            return visible_internal_llm_tool_failure(
                node,
                provider_events,
                json!({
                    "error_code": "visible_internal_llm_tool_round_limit",
                    "message": "visible internal LLM tool execution exceeded the maximum callback rounds",
                }),
                route_events,
            );
        }
    }

    visible_internal_llm_tool_failure(
        node,
        provider_events,
        json!({
            "error_code": "visible_internal_llm_tool_round_limit",
            "message": "visible internal LLM tool execution exceeded the maximum callback rounds",
        }),
        route_events,
    )
}
async fn execute_remaining_visible_internal_llm_tool_calls<I>(
    context: VisibleInternalLlmToolRemainingContext<'_, I>,
    mut completed_tool_results: Vec<Value>,
    mut visible_transcript: String,
    pending_calls: Vec<VisibleInternalLlmToolPendingCall>,
) -> Result<VisibleInternalLlmToolRemainingExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let plan = context.plan;
    let mut variable_pool = context.variable_pool.clone();
    let runtime_context = context.runtime_context;
    let invoker = context.invoker;
    let main_node_id = context.main_node_id;

    let mut provider_events = Vec::new();
    let mut route_events = Vec::new();
    for (index, pending_call) in pending_calls.iter().enumerate() {
        match execute_visible_internal_llm_tool_call(
            plan,
            &variable_pool,
            runtime_context,
            invoker,
            main_node_id,
            &pending_call.tool_call,
            &pending_call.tool,
        )
        .await?
        {
            VisibleInternalLlmToolBranchExecution::Completed(output) => {
                variable_pool = output.variable_pool;
                provider_events.extend(output.provider_events);
                route_events.extend(output.route_events);
                visible_transcript.push_str(&output.text);
                completed_tool_results.push(visible_internal_llm_tool_result(
                    &pending_call.tool_call,
                    &pending_call.tool.name,
                    output.text,
                ));
            }
            VisibleInternalLlmToolBranchExecution::Waiting {
                mut wait,
                branch_text,
                route_events: waiting_route_events,
            } => {
                route_events.extend(waiting_route_events);
                let remaining_tool_calls = pending_calls
                    .iter()
                    .skip(index + 1)
                    .cloned()
                    .collect::<Vec<_>>();
                insert_visible_internal_llm_tool_callback_state(
                    &mut wait.checkpoint_variable_pool,
                    VisibleInternalLlmToolCallbackStateInput {
                        main_node_id,
                        tool_call: &pending_call.tool_call,
                        tool_name: &pending_call.tool.name,
                        target_node_id: &pending_call.tool.target_node_id,
                        main_visible_transcript: &visible_transcript,
                        branch_text: &branch_text,
                        route_events: &route_events,
                        completed_tool_results: &completed_tool_results,
                        remaining_tool_calls: &remaining_tool_calls,
                    },
                );
                return Ok(VisibleInternalLlmToolRemainingExecution::Waiting(wait));
            }
            VisibleInternalLlmToolBranchExecution::Failed {
                error_payload,
                route_events: failed_route_events,
            } => {
                route_events.extend(failed_route_events);
                if let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                    &pending_call.tool_call,
                    &pending_call.tool.name,
                    &error_payload,
                ) {
                    completed_tool_results.push(tool_result);
                    continue;
                }
                return Ok(VisibleInternalLlmToolRemainingExecution::Failed {
                    error_payload,
                    provider_events,
                    route_events,
                });
            }
        }
    }

    Ok(VisibleInternalLlmToolRemainingExecution::Completed {
        tool_results: completed_tool_results,
        visible_transcript,
        provider_events,
        route_events,
        variable_pool,
    })
}

pub(super) async fn resume_visible_internal_llm_tool_callback<I>(
    plan: &CompiledPlan,
    waiting_node_id: &str,
    mut variable_pool: Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<VisibleInternalLlmToolResume>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let state = visible_internal_llm_tool_callback_state(&variable_pool)?;
    let node = plan.nodes.get(waiting_node_id).ok_or_else(|| {
        anyhow!("visible internal llm tool waiting node not found: {waiting_node_id}")
    })?;
    let resolved_inputs = resolve_node_inputs(node, &variable_pool).map_err(|error| {
        anyhow!("visible internal llm tool waiting node input resolution failed: {error}")
    })?;
    let rendered_templates = render_templated_bindings(node, &resolved_inputs);
    let mut provider_events = Vec::new();
    let mut route_events = state.route_events.clone();

    match execute_visible_internal_llm_tool_node(
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
        VisibleInternalLlmToolNodeExecution::Waiting(mut wait) => {
            let output_text = wait
                .node_trace
                .as_ref()
                .map(|trace| visible_internal_llm_tool_output_text(&trace.output_payload))
                .unwrap_or_default();
            let branch_text = format!("{}{}", state.branch_text, output_text);
            route_events.push(visible_internal_llm_tool_route_event(
                "visible_internal_llm_tool_waiting_callback",
                &state.main_node_id,
                &state.tool_call,
                &VisibleInternalLlmTool {
                    name: state.tool_name.clone(),
                    description: None,
                    target_node_id: state.target_node_id.clone(),
                    target_node_ids: vec![state.target_node_id.clone()],
                    input_schema: None,
                    tool_mode: visible_internal_llm_tool_mode(&variable_pool),
                    external_tool_policy: visible_internal_llm_tool_external_tool_policy(
                        &variable_pool,
                    ),
                    external_callback_policy: visible_internal_llm_tool_external_callback_policy(
                        &variable_pool,
                    ),
                    execution_mode: visible_internal_llm_tool_execution_mode(&variable_pool),
                    preconditions: visible_internal_llm_tool_preconditions_from_variable_pool(
                        &variable_pool,
                    ),
                },
                json!({
                    "waiting_node_id": wait.node_id,
                    "waiting_node_alias": wait.node_alias,
                    "request_payload": wait.request_payload,
                }),
            ));
            insert_visible_internal_llm_tool_callback_state(
                &mut wait.checkpoint_variable_pool,
                VisibleInternalLlmToolCallbackStateInput {
                    main_node_id: &state.main_node_id,
                    tool_call: &state.tool_call,
                    tool_name: &state.tool_name,
                    target_node_id: &state.target_node_id,
                    main_visible_transcript: &state.main_visible_transcript,
                    branch_text: &branch_text,
                    route_events: &route_events,
                    completed_tool_results: &state.completed_tool_results,
                    remaining_tool_calls: &state.remaining_tool_calls,
                },
            );
            Ok(VisibleInternalLlmToolResume::Waiting(wait))
        }
        VisibleInternalLlmToolNodeExecution::Completed(node_output) => {
            let branch_text = format!(
                "{}{}",
                state.branch_text,
                visible_internal_llm_tool_output_text(&node_output.output_payload)
            );
            let tool = VisibleInternalLlmTool {
                name: state.tool_name.clone(),
                description: None,
                target_node_id: state.target_node_id.clone(),
                target_node_ids: vec![state.target_node_id.clone()],
                input_schema: None,
                tool_mode: visible_internal_llm_tool_mode(&variable_pool),
                external_tool_policy: visible_internal_llm_tool_external_tool_policy(
                    &variable_pool,
                ),
                external_callback_policy: visible_internal_llm_tool_external_callback_policy(
                    &variable_pool,
                ),
                execution_mode: visible_internal_llm_tool_execution_mode(&variable_pool),
                preconditions: visible_internal_llm_tool_preconditions_from_variable_pool(
                    &variable_pool,
                ),
            };
            variable_pool.insert(
                node.node_id.clone(),
                project_node_variable_payload(node, &node_output.output_payload)?,
            );
            route_events.push(visible_internal_llm_tool_route_event(
                "visible_internal_llm_tool_completed",
                &state.main_node_id,
                &state.tool_call,
                &tool,
                json!({
                    "route_node_id": node.node_id,
                    "node_id": node.node_id,
                    "node_alias": node.alias,
                    "node_type": node.node_type,
                    "provider_route": node_output.output_payload
                        .get("provider_route")
                        .cloned()
                        .unwrap_or(Value::Null),
                    "metrics_payload": node_output.metrics_payload.clone(),
                    "debug_payload": node_output.debug_payload.clone(),
                    "content": branch_text.clone(),
                }),
            ));

            let mut active_node_ids = BTreeSet::new();
            activate_downstream_nodes(plan, &mut active_node_ids, node, None);
            let branch_execution = if active_node_ids.is_empty() {
                let mut completed_variable_pool = variable_pool.clone();
                completed_variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_VARIABLE);
                VisibleInternalLlmToolBranchExecution::Completed(VisibleInternalLlmToolOutput {
                    text: branch_text,
                    provider_events,
                    route_events,
                    variable_pool: completed_variable_pool,
                })
            } else {
                continue_visible_internal_llm_tool_branch(
                    VisibleInternalLlmToolBranchContext {
                        plan,
                        runtime_context,
                        invoker,
                        main_node_id: &state.main_node_id,
                        tool_call: &state.tool_call,
                        tool: &tool,
                    },
                    variable_pool.clone(),
                    active_node_ids,
                    branch_text,
                    provider_events,
                    route_events,
                )
                .await?
            };

            let branch_output = match branch_execution {
                VisibleInternalLlmToolBranchExecution::Completed(output) => output,
                VisibleInternalLlmToolBranchExecution::Waiting {
                    mut wait,
                    branch_text,
                    route_events,
                } => {
                    insert_visible_internal_llm_tool_callback_state(
                        &mut wait.checkpoint_variable_pool,
                        VisibleInternalLlmToolCallbackStateInput {
                            main_node_id: &state.main_node_id,
                            tool_call: &state.tool_call,
                            tool_name: &state.tool_name,
                            target_node_id: &state.target_node_id,
                            main_visible_transcript: &state.main_visible_transcript,
                            branch_text: &branch_text,
                            route_events: &route_events,
                            completed_tool_results: &state.completed_tool_results,
                            remaining_tool_calls: &state.remaining_tool_calls,
                        },
                    );
                    return Ok(VisibleInternalLlmToolResume::Waiting(wait));
                }
                VisibleInternalLlmToolBranchExecution::Failed {
                    error_payload,
                    route_events,
                } => {
                    if let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                        &state.tool_call,
                        &state.tool_name,
                        &error_payload,
                    ) {
                        let mut completed_tool_results = state.completed_tool_results.clone();
                        completed_tool_results.push(tool_result);
                        let visible_transcript =
                            format!("{}{}", state.main_visible_transcript, state.branch_text);
                        match execute_remaining_visible_internal_llm_tool_calls(
                            VisibleInternalLlmToolRemainingContext {
                                plan,
                                variable_pool: &variable_pool,
                                runtime_context,
                                invoker,
                                main_node_id: &state.main_node_id,
                            },
                            completed_tool_results,
                            visible_transcript,
                            state.remaining_tool_calls.clone(),
                        )
                        .await?
                        {
                            VisibleInternalLlmToolRemainingExecution::Completed {
                                tool_results,
                                visible_transcript,
                                provider_events: _remaining_provider_events,
                                route_events: remaining_route_events,
                                variable_pool: remaining_variable_pool,
                            } => {
                                variable_pool = remaining_variable_pool;
                                let mut route_events = route_events;
                                route_events.extend(remaining_route_events);
                                append_llm_tool_result_messages(
                                    &mut variable_pool,
                                    &state.main_node_id,
                                    &json!({ "tool_results": tool_results }),
                                )?;
                                set_pending_llm_tool_callback_visible_internal_transcript(
                                    &mut variable_pool,
                                    &state.main_node_id,
                                    visible_transcript,
                                )?;
                                set_pending_llm_tool_callback_visible_internal_events(
                                    &mut variable_pool,
                                    &state.main_node_id,
                                    route_events,
                                )?;
                                variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_CALLBACK_STATE_KEY);
                                return Ok(VisibleInternalLlmToolResume::Ready(variable_pool));
                            }
                            VisibleInternalLlmToolRemainingExecution::Waiting(wait) => {
                                return Ok(VisibleInternalLlmToolResume::Waiting(wait));
                            }
                            VisibleInternalLlmToolRemainingExecution::Failed {
                                error_payload,
                                provider_events: remaining_provider_events,
                                route_events: remaining_route_events,
                            } => {
                                let mut route_events = route_events;
                                route_events.extend(remaining_route_events);
                                let main_node =
                                    plan.nodes.get(&state.main_node_id).ok_or_else(|| {
                                        anyhow!("visible internal llm tool main node not found")
                                    })?;
                                let execution = visible_internal_llm_tool_failure(
                                    main_node,
                                    remaining_provider_events,
                                    error_payload,
                                    route_events,
                                )?;
                                return Ok(VisibleInternalLlmToolResume::Failed {
                                    node_id: main_node.node_id.clone(),
                                    node_alias: main_node.alias.clone(),
                                    execution: Box::new(execution),
                                });
                            }
                        }
                    }

                    let main_node = plan
                        .nodes
                        .get(&state.main_node_id)
                        .ok_or_else(|| anyhow!("visible internal llm tool main node not found"))?;
                    let execution = visible_internal_llm_tool_failure(
                        main_node,
                        Vec::new(),
                        error_payload,
                        route_events,
                    )?;
                    return Ok(VisibleInternalLlmToolResume::Failed {
                        node_id: main_node.node_id.clone(),
                        node_alias: main_node.alias.clone(),
                        execution: Box::new(execution),
                    });
                }
            };

            let VisibleInternalLlmToolOutput {
                text: branch_text,
                provider_events: branch_provider_events,
                route_events: branch_route_events,
                variable_pool: branch_variable_pool,
            } = branch_output;
            variable_pool = branch_variable_pool;
            provider_events = branch_provider_events;
            route_events = branch_route_events;
            let mut completed_tool_results = state.completed_tool_results.clone();
            completed_tool_results.push(visible_internal_llm_tool_result(
                &state.tool_call,
                &state.tool_name,
                branch_text.clone(),
            ));
            let visible_transcript = format!("{}{}", state.main_visible_transcript, branch_text);
            match execute_remaining_visible_internal_llm_tool_calls(
                VisibleInternalLlmToolRemainingContext {
                    plan,
                    variable_pool: &variable_pool,
                    runtime_context,
                    invoker,
                    main_node_id: &state.main_node_id,
                },
                completed_tool_results,
                visible_transcript,
                state.remaining_tool_calls.clone(),
            )
            .await?
            {
                VisibleInternalLlmToolRemainingExecution::Completed {
                    tool_results,
                    visible_transcript,
                    provider_events: remaining_provider_events,
                    route_events: remaining_route_events,
                    variable_pool: remaining_variable_pool,
                } => {
                    variable_pool = remaining_variable_pool;
                    provider_events.extend(remaining_provider_events);
                    route_events.extend(remaining_route_events);
                    append_llm_tool_result_messages(
                        &mut variable_pool,
                        &state.main_node_id,
                        &json!({ "tool_results": tool_results }),
                    )?;
                    set_pending_llm_tool_callback_visible_internal_transcript(
                        &mut variable_pool,
                        &state.main_node_id,
                        visible_transcript,
                    )?;
                    set_pending_llm_tool_callback_visible_internal_events(
                        &mut variable_pool,
                        &state.main_node_id,
                        route_events,
                    )?;
                    variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_CALLBACK_STATE_KEY);
                    Ok(VisibleInternalLlmToolResume::Ready(variable_pool))
                }
                VisibleInternalLlmToolRemainingExecution::Waiting(wait) => {
                    Ok(VisibleInternalLlmToolResume::Waiting(wait))
                }
                VisibleInternalLlmToolRemainingExecution::Failed {
                    error_payload,
                    provider_events: remaining_provider_events,
                    route_events: remaining_route_events,
                } => {
                    provider_events.extend(remaining_provider_events);
                    route_events.extend(remaining_route_events);
                    let main_node = plan
                        .nodes
                        .get(&state.main_node_id)
                        .ok_or_else(|| anyhow!("visible internal llm tool main node not found"))?;
                    let execution = visible_internal_llm_tool_failure(
                        main_node,
                        provider_events,
                        error_payload,
                        route_events,
                    )?;
                    Ok(VisibleInternalLlmToolResume::Failed {
                        node_id: main_node.node_id.clone(),
                        node_alias: main_node.alias.clone(),
                        execution: Box::new(execution),
                    })
                }
            }
        }
        VisibleInternalLlmToolNodeExecution::Failed(error_payload) => {
            let main_node = plan
                .nodes
                .get(&state.main_node_id)
                .ok_or_else(|| anyhow!("visible internal llm tool main node not found"))?;
            let tool = VisibleInternalLlmTool {
                name: state.tool_name.clone(),
                description: None,
                target_node_id: state.target_node_id.clone(),
                target_node_ids: vec![state.target_node_id.clone()],
                input_schema: None,
                tool_mode: visible_internal_llm_tool_mode(&variable_pool),
                external_tool_policy: visible_internal_llm_tool_external_tool_policy(
                    &variable_pool,
                ),
                external_callback_policy: visible_internal_llm_tool_external_callback_policy(
                    &variable_pool,
                ),
                execution_mode: visible_internal_llm_tool_execution_mode(&variable_pool),
                preconditions: visible_internal_llm_tool_preconditions_from_variable_pool(
                    &variable_pool,
                ),
            };
            let error_payload = visible_internal_llm_tool_node_error(
                &state.tool_call,
                &tool,
                node,
                "visible internal LLM tool branch node failed",
                None,
                Some(error_payload),
            );
            route_events.push(visible_internal_llm_tool_route_event(
                "visible_internal_llm_tool_failed",
                &state.main_node_id,
                &state.tool_call,
                &tool,
                json!({
                    "route_node_id": node.node_id,
                    "node_id": node.node_id,
                    "node_alias": node.alias,
                    "node_type": node.node_type,
                    "error_payload": error_payload,
                }),
            ));
            if let Some(tool_result) = visible_internal_llm_tool_recoverable_result(
                &state.tool_call,
                &state.tool_name,
                &error_payload,
            ) {
                let mut completed_tool_results = state.completed_tool_results.clone();
                completed_tool_results.push(tool_result);
                let visible_transcript =
                    format!("{}{}", state.main_visible_transcript, state.branch_text);
                match execute_remaining_visible_internal_llm_tool_calls(
                    VisibleInternalLlmToolRemainingContext {
                        plan,
                        variable_pool: &variable_pool,
                        runtime_context,
                        invoker,
                        main_node_id: &state.main_node_id,
                    },
                    completed_tool_results,
                    visible_transcript,
                    state.remaining_tool_calls.clone(),
                )
                .await?
                {
                    VisibleInternalLlmToolRemainingExecution::Completed {
                        tool_results,
                        visible_transcript,
                        provider_events: remaining_provider_events,
                        route_events: remaining_route_events,
                        variable_pool: remaining_variable_pool,
                    } => {
                        variable_pool = remaining_variable_pool;
                        provider_events.extend(remaining_provider_events);
                        route_events.extend(remaining_route_events);
                        append_llm_tool_result_messages(
                            &mut variable_pool,
                            &state.main_node_id,
                            &json!({ "tool_results": tool_results }),
                        )?;
                        set_pending_llm_tool_callback_visible_internal_transcript(
                            &mut variable_pool,
                            &state.main_node_id,
                            visible_transcript,
                        )?;
                        set_pending_llm_tool_callback_visible_internal_events(
                            &mut variable_pool,
                            &state.main_node_id,
                            route_events,
                        )?;
                        variable_pool.remove(VISIBLE_INTERNAL_LLM_TOOL_CALLBACK_STATE_KEY);
                        return Ok(VisibleInternalLlmToolResume::Ready(variable_pool));
                    }
                    VisibleInternalLlmToolRemainingExecution::Waiting(wait) => {
                        return Ok(VisibleInternalLlmToolResume::Waiting(wait));
                    }
                    VisibleInternalLlmToolRemainingExecution::Failed {
                        error_payload,
                        provider_events: remaining_provider_events,
                        route_events: remaining_route_events,
                    } => {
                        provider_events.extend(remaining_provider_events);
                        route_events.extend(remaining_route_events);
                        let execution = visible_internal_llm_tool_failure(
                            main_node,
                            provider_events,
                            error_payload,
                            route_events,
                        )?;
                        return Ok(VisibleInternalLlmToolResume::Failed {
                            node_id: main_node.node_id.clone(),
                            node_alias: main_node.alias.clone(),
                            execution: Box::new(execution),
                        });
                    }
                }
            }
            let execution = visible_internal_llm_tool_failure(
                main_node,
                provider_events,
                error_payload,
                route_events,
            )?;
            Ok(VisibleInternalLlmToolResume::Failed {
                node_id: main_node.node_id.clone(),
                node_alias: main_node.alias.clone(),
                execution: Box::new(execution),
            })
        }
    }
}
