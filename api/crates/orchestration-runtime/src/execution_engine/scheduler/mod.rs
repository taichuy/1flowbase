use super::*;

pub(super) async fn execute_from_scoped<I>(
    plan: &CompiledPlan,
    next_node_index: usize,
    mut variable_pool: Map<String, Value>,
    active_node_ids: Option<BTreeSet<String>>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
    scope: Option<&container::Scope>,
    session: Option<&container::Session>,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    if scope.is_none() {
        normalize_plan_variable_pool(plan, &mut variable_pool);
        synchronize_runtime_global_variables(plan, &mut variable_pool, runtime_context);
    }
    let mut runtime_context = runtime_context.clone();
    runtime_context.refresh_frozen_run_tools(plan, &variable_pool);
    let edge_index = CompiledEdgeIndex::new(&plan.edges);
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

        let scope_container_id = if let Some(scope) = scope {
            Some(
                scope.session.state.lock().await.frames[&scope.frame_id]
                    .identity
                    .node_id
                    .clone(),
            )
        } else {
            None
        };
        if node.container_id != scope_container_id {
            continue;
        }

        if !active_node_ids.contains(node_id) {
            continue;
        }
        if mounted_llm_target_node_ids.contains(node_id) {
            continue;
        }
        if runtime_context.communication.is_some()
            && !communication::allow_scheduled_node(&variable_pool, node)
        {
            continue;
        }
        if let Some(communication) = &runtime_context.communication {
            if let Err(error) = communication.check_active().await {
                if scope.is_some() {
                    return Err(container::GlobalFailure(json!({"error_code": "container_execution_cancelled", "message": error.to_string()})).into());
                }
                return Err(error);
            }
            if node.node_type == "llm" && scope.is_none() {
                communication.activate_node(node).await?;
            }
        }

        if scope.is_some() || session.is_some() {
            let cached = if let Some(scope) = scope {
                scope
                    .advance(index, &variable_pool, &active_node_ids, lifecycle)
                    .await?;
                scope.cached_trace(node_id).await
            } else if let Some(session) = session {
                session
                    .advance_root(index, &variable_pool, &active_node_ids, lifecycle)
                    .await?;
                session.cached_root_trace(node_id).await
            } else {
                None
            };
            if let Some(trace) = cached {
                let selected = trace
                    .debug_payload
                    .get("selected_source_handle")
                    .and_then(Value::as_str);
                if let Some(error_payload) = trace.error_payload.clone() {
                    let failure = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
                        failed_node_index: index,
                        active_node_ids: &mut active_node_ids,
                        variable_pool: &mut variable_pool,
                        pending_failure: &mut pending_failure,
                        node,
                        output_payload: &trace.output_payload,
                        error_payload,
                        failure_projection: LlmFailureProjection::NoNodeOutput,
                    })?;
                    node_traces.push(trace);
                    if let Some(failure) = failure {
                        return Ok(FlowDebugExecutionOutcome {
                            stop_reason: ExecutionStopReason::Failed(failure),
                            variable_pool,
                            checkpoint_snapshot: None,
                            operation_terminal: None,
                            node_traces,
                        });
                    }
                } else {
                    if !matches!(
                        node.node_type.as_str(),
                        "start" | "workflow_start" | "container_start" | "if_else"
                    ) {
                        variable_pool.insert(
                            node_id.clone(),
                            project_node_variable_payload(node, &trace.output_payload)?,
                        );
                    }
                    activate_indexed_downstream_nodes(
                        &edge_index,
                        &mut active_node_ids,
                        node,
                        selected,
                    );
                    node_traces.push(trace);
                }
                continue;
            }
            let uncertain = if let Some(scope) = scope {
                scope.uncertain(node_id).await
            } else if let Some(session) = session {
                session.uncertain_root(node_id).await
            } else {
                false
            };
            if uncertain && !crate::container_contract::is_container_type(&node.node_type) {
                let error_payload = json!({"error_code": "container_execution_outcome_uncertain", "message": "The previous attempt started without a durable result; automatic replay is forbidden"});
                let trace = NodeExecutionTrace {
                    execution_identity: None,
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: json!({}),
                    output_payload: json!({}),
                    error_payload: Some(error_payload.clone()),
                    metrics_payload: json!({}),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                };
                lifecycle.complete_node(&trace).await?;
                let failure = apply_node_error_policy(NodeErrorPolicyApplication {
                    plan,
                    edge_index: &edge_index,
                    failed_node_index: index,
                    active_node_ids: &mut active_node_ids,
                    variable_pool: &mut variable_pool,
                    pending_failure: &mut pending_failure,
                    node,
                    output_payload: &trace.output_payload,
                    error_payload,
                    failure_projection: LlmFailureProjection::NoNodeOutput,
                })?;
                node_traces.push(trace);
                if let Some(failure) = failure {
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
        }
        if crate::container_contract::is_container_type(&node.node_type) {
            let execution = Box::pin(container::execute_container(
                plan,
                node,
                &variable_pool,
                index,
                &active_node_ids,
                scope,
                session,
                &runtime_context,
                invoker,
                lifecycle,
            ))
            .await?;
            node_traces.extend(execution.traces);
            if let Some(waiting) = execution.waiting {
                let mut waiting = *waiting;
                waiting.node_traces = node_traces;
                return Ok(waiting);
            }
            variable_pool.insert(node_id.clone(), execution.output);
            activate_indexed_downstream_nodes(&edge_index, &mut active_node_ids, node, None);
            continue;
        }
        // Waiting parent frames never hold a leaf permit. Nested containers share
        // the same run budget, so waiting for children cannot exhaust all workers.
        let _leaf_permit = if let Some(scope) = scope {
            Some(
                scope
                    .session
                    .permits
                    .acquire()
                    .await
                    .map_err(|_| anyhow!("container execution budget closed"))?,
            )
        } else {
            None
        };

        let (resolved_inputs, answer_binding_error_payload) =
            match if node.node_type == "variable_aggregator" {
                variable_aggregator_input_payload(node)
            } else {
                resolve_node_inputs(node, &variable_pool)
            } {
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
                        execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
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
        let llm_input_payload = (node.node_type == "llm").then(|| {
            llm_node_input_payload(
                plan,
                node,
                &resolved_inputs,
                &rendered_templates,
                &variable_pool,
                &runtime_context,
            )
        });
        let node_input_payload = if matches!(node.node_type.as_str(), "start" | "workflow_start") {
            start_node_execution_input(&variable_pool, &node.node_id)
        } else if let Some(input_payload) = &llm_input_payload {
            Value::Object(input_payload.clone())
        } else {
            Value::Object(resolved_inputs.clone())
        };
        lifecycle.begin_node(node, &node_input_payload).await?;
        if node.node_type == "llm" && scope.is_none() {
            if let Some(communication) = &runtime_context.communication {
                communication
                    .record_node_receipt(node, lifecycle.communication_node_receipt(node).await?)
                    .await;
            }
        }
        let mut selected_source_handle: Option<String> = None;

        match node.node_type.as_str() {
            "start" | "workflow_start" | "container_start" => {
                node_traces.push(NodeExecutionTrace {
                    execution_identity: None,
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
                    execution_identity: None,
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
                    execution_identity: None,
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
                let input_payload = llm_input_payload
                    .expect("LLM nodes must materialize their effective input before execution");
                // Keep the nested LLM operation state out of the workflow future so
                // its provider/tool execution fits a normal worker stack. Await it
                // in place to retain this workflow's cancellation and lifecycle owner.
                let execution = Box::pin(execute_llm_node(
                    plan,
                    node,
                    &resolved_inputs,
                    &rendered_templates,
                    &mut variable_pool,
                    &runtime_context,
                    invoker,
                    lifecycle,
                ))
                .await?;
                let trace = NodeExecutionTrace {
                    execution_identity: None,
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(input_payload),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: execution.metrics_payload.clone(),
                    debug_payload: execution.debug_payload.clone(),
                    provider_events: execution.provider_events.clone(),
                };
                node_traces.push(trace);

                if let Some(error_payload) = execution.error_payload {
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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

                if let Some(communication) = runtime_context
                    .communication
                    .as_ref()
                    .filter(|_| scope.is_none())
                {
                    communication
                        .retain_trace(node_traces.last().expect("LLM trace just appended"))
                        .await?;
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
                            container_state: None,
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
                    execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
            "notification" => {
                let execution = invoker
                    .invoke_notification_node(node, &resolved_inputs)
                    .await?;
                node_traces.push(NodeExecutionTrace {
                    execution_identity: None,
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: node_input_payload,
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: json!({}),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                });
                if let Some(error_payload) = execution.error_payload {
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                    execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                    execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                    activate_indexed_downstream_nodes(
                        &edge_index,
                        &mut active_node_ids,
                        node,
                        None,
                    );
                    return Ok(FlowDebugExecutionOutcome {
                        stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                            node_id: node.node_id.clone(),
                            node_alias: node.alias.clone(),
                            callback_kind: callback.callback_kind,
                            request_payload: callback.request_payload,
                        }),
                        variable_pool: variable_pool.clone(),
                        checkpoint_snapshot: Some(CheckpointSnapshot {
                            container_state: None,
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
                    execution_identity: None,
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
            "variable_aggregator" => {
                let execution = execute_variable_aggregator_node(node, &variable_pool)?;
                node_traces.push(NodeExecutionTrace {
                    execution_identity: None,
                    node_id: node.node_id.clone(),
                    node_type: node.node_type.clone(),
                    node_alias: node.alias.clone(),
                    input_payload: Value::Object(execution.input_payload),
                    output_payload: execution.output_payload.clone(),
                    error_payload: execution.error_payload.clone(),
                    metrics_payload: json!({ "preview_mode": true }),
                    debug_payload: execution.debug_payload,
                    provider_events: Vec::new(),
                });

                if let Some(error_payload) = execution.error_payload {
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                    execution_identity: None,
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
                    execution_identity: None,
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
                activate_indexed_downstream_nodes(&edge_index, &mut active_node_ids, node, None);
                return Ok(FlowDebugExecutionOutcome {
                    stop_reason: ExecutionStopReason::WaitingHuman(PendingHumanInput {
                        node_id: node.node_id.clone(),
                        node_alias: node.alias.clone(),
                        prompt,
                    }),
                    variable_pool: variable_pool.clone(),
                    checkpoint_snapshot: Some(CheckpointSnapshot {
                        container_state: None,
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
                    execution_identity: None,
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
                activate_indexed_downstream_nodes(&edge_index, &mut active_node_ids, node, None);
                return Ok(FlowDebugExecutionOutcome {
                    stop_reason: ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                        node_id: node.node_id.clone(),
                        node_alias: node.alias.clone(),
                        callback_kind: node.node_type.clone(),
                        request_payload: Value::Object(resolved_inputs),
                    }),
                    variable_pool: variable_pool.clone(),
                    checkpoint_snapshot: Some(CheckpointSnapshot {
                        container_state: None,
                        next_node_index: index + 1,
                        variable_pool,
                        active_node_ids: checkpoint_active_node_ids(&active_node_ids),
                    }),
                    operation_terminal: None,
                    node_traces,
                });
            }
            "http_request" => {
                let execution = execute_http_request_node_with_provider_invoker(
                    node,
                    &resolved_inputs,
                    &variable_pool,
                    runtime_context.http_response_file_persister.as_deref(),
                    Some(invoker),
                )
                .await?;
                node_traces.push(NodeExecutionTrace {
                    execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                let mut bounded_node;
                let execution_node = if let Some((scope, remaining)) =
                    scope.and_then(|scope| scope.remaining_ms().map(|remaining| (scope, remaining)))
                {
                    let (_, owner) = scope
                        .deadline()
                        .await
                        .ok_or_else(|| anyhow!("finite deadline missing owner"))?;
                    if remaining == 0 {
                        return Err(container::DeadlineExpired(owner).into());
                    }
                    bounded_node = node.clone();
                    if let Some(runtime) = bounded_node.code_runtime.as_mut() {
                        runtime.isolation_profile.timeout_ms =
                            runtime.isolation_profile.timeout_ms.min(remaining);
                    }
                    &bounded_node
                } else {
                    node
                };
                let execution =
                    execute_code_node(plan, execution_node, &resolved_inputs, invoker).await?;
                node_traces.push(NodeExecutionTrace {
                    execution_identity: None,
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
                    complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
                    if let Some(failure) = apply_node_error_policy(NodeErrorPolicyApplication {
                        plan,
                        edge_index: &edge_index,
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
                    execution_identity: None,
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
                complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
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
        // A communication LLM remains running across final-candidate continuation.
        // The persistence owner commits its one terminal node fact after send seal.
        if scope.is_some() || runtime_context.communication.is_none() || node.node_type != "llm" {
            complete_latest_node_trace(lifecycle, &node_traces, &variable_pool).await?;
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
            activate_indexed_downstream_nodes(
                &edge_index,
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

pub(super) async fn execute_from<I>(
    plan: &CompiledPlan,
    next_node_index: usize,
    variable_pool: Map<String, Value>,
    active_node_ids: Option<BTreeSet<String>>,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let has_containers = plan
        .nodes
        .values()
        .any(|node| crate::container_contract::is_container_type(&node.node_type));
    if !has_containers {
        return container::CURRENT_EXECUTION
            .scope(
                Arc::default(),
                execute_from_scoped(
                    plan,
                    next_node_index,
                    variable_pool,
                    active_node_ids,
                    runtime_context,
                    invoker,
                    lifecycle,
                    None,
                    None,
                ),
            )
            .await;
    }
    container::execute_root(
        plan,
        next_node_index,
        variable_pool,
        active_node_ids,
        runtime_context,
        invoker,
        lifecycle,
        None,
    )
    .await
}
