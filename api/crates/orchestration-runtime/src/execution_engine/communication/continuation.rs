use super::*;

pub(super) const FINAL_PROJECTION_KEY: &str = "__communication_final_projection";

fn pure_projection(node_type: &str) -> bool {
    matches!(
        node_type,
        "answer" | "template_transform" | "variable_aggregator"
    )
}

/// Resume only the retained top-level LLM and already selected pure answer projections.
/// Completed graph actions remain in the variable pool and are never scheduled again.
pub async fn continue_communication_reply<I>(
    plan: &CompiledPlan,
    mut outcome: FlowDebugExecutionOutcome,
    context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let communication = context
        .communication_execution()
        .ok_or_else(|| anyhow!("communication_execution_missing"))?;
    if !communication.has_new_inputs().await? {
        return Ok(outcome);
    }
    let node_id = communication
        .state
        .lock()
        .await
        .continuation
        .main_node_id
        .clone()
        .ok_or_else(|| anyhow!("communication_continuation_has_no_llm"))?;
    let node = plan
        .nodes
        .get(&node_id)
        .ok_or_else(|| anyhow!("communication_continuation_node_missing"))?;
    let index = plan
        .topological_order
        .iter()
        .position(|id| id == &node_id)
        .ok_or_else(|| anyhow!("communication_continuation_node_not_scheduled"))?;
    let trace_index = outcome
        .node_traces
        .iter()
        .position(|trace| trace.node_id == node_id)
        .ok_or_else(|| anyhow!("communication_retained_llm_receipt_missing"))?;
    ensure_refreshable_answer(plan, &outcome, &node_id)?;
    let projections = outcome
        .node_traces
        .iter()
        .filter(|trace| pure_projection(&trace.node_type))
        .map(|trace| trace.node_id.clone())
        .collect::<Vec<_>>();
    let mut pool = communication
        .retained_pool(node, &outcome.variable_pool)
        .await?;
    pool.insert(
        FINAL_PROJECTION_KEY.into(),
        json!({"node_id":node_id,"projection_ids":projections}),
    );
    let resolved = resolve_node_inputs(node, &pool)?;
    let rendered = render_templated_bindings(node, &resolved);
    lifecycle.activate_retained_communication_node(node).await?;
    let mut execution = Box::pin(execute_llm_node(
        plan, node, &resolved, &rendered, &mut pool, context, invoker, lifecycle,
    ))
    .await?;
    let previous = &outcome.node_traces[trace_index];
    let prior = LlmNodeExecution {
        output_payload: previous.output_payload.clone(),
        error_payload: previous.error_payload.clone(),
        metrics_payload: previous.metrics_payload.clone(),
        debug_payload: previous.debug_payload.clone(),
        provider_events: Vec::new(),
        pending_callback: None,
        failure_projection: LlmFailureProjection::NoNodeOutput,
        recoverable_error_message: None,
    };
    let mut metrics = LlmRoundMetricsAccumulator::default();
    metrics.absorb(&prior);
    metrics.absorb(&execution);
    metrics.apply(&mut execution);
    for key in [
        "runtime_internal_tool_events",
        "visible_internal_llm_tool_events",
    ] {
        let mut events = previous
            .debug_payload
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(next) = execution.debug_payload.get(key).and_then(Value::as_array) {
            events.extend(next.iter().cloned());
        }
        if !events.is_empty() {
            if key == "runtime_internal_tool_events" {
                execution.metrics_payload["internal_tool_call_count"] = json!(events.len());
            }
            execution.debug_payload[key] = Value::Array(events);
        }
    }
    outcome.node_traces[trace_index] = NodeExecutionTrace {
        node_id: node.node_id.clone(),
        node_type: node.node_type.clone(),
        node_alias: node.alias.clone(),
        input_payload: Value::Object(resolved),
        output_payload: execution.output_payload.clone(),
        error_payload: execution.error_payload.clone(),
        metrics_payload: execution.metrics_payload.clone(),
        debug_payload: execution.debug_payload.clone(),
        provider_events: execution.provider_events.clone(),
    };
    if execution.error_payload.is_none() {
        communication
            .retain_trace(&outcome.node_traces[trace_index])
            .await?;
    }
    if let Some(error_payload) = execution.error_payload {
        outcome.stop_reason = ExecutionStopReason::Failed(NodeExecutionFailure {
            node_id: node.node_id.clone(),
            node_alias: node.alias.clone(),
            error_payload,
        });
        outcome.variable_pool = pool;
        return Ok(outcome);
    }
    if let Some(wait) = execution.pending_callback {
        let mut active_node_ids = projections;
        active_node_ids.push(node.node_id.clone());
        outcome.stop_reason = ExecutionStopReason::WaitingCallback(PendingCallbackTask {
            node_id: wait.node_id,
            node_alias: wait.node_alias,
            callback_kind: LLM_TOOL_CALLBACK_KIND.into(),
            request_payload: wait.request_payload,
        });
        outcome.checkpoint_snapshot = Some(CheckpointSnapshot {
            container_state: None,
            next_node_index: index,
            variable_pool: wait.checkpoint_variable_pool,
            active_node_ids,
        });
        outcome.variable_pool = pool;
        return Ok(outcome);
    }
    pool.insert(node.node_id.clone(), execution.output_payload);
    for trace in &mut outcome.node_traces {
        if !pure_projection(&trace.node_type) {
            continue;
        }
        let projection = plan
            .nodes
            .get(&trace.node_id)
            .ok_or_else(|| anyhow!("communication_projection_node_missing"))?;
        let (inputs, output) = if trace.node_type == "variable_aggregator" {
            let execution = execute_variable_aggregator_node(projection, &pool)?;
            if execution.error_payload.is_some() {
                bail!("communication_projection_binding_changed");
            }
            (execution.input_payload, execution.output_payload)
        } else {
            let inputs = resolve_node_inputs(projection, &pool)?;
            let rendered = render_templated_bindings(projection, &inputs);
            let value = rendered
                .values()
                .next()
                .cloned()
                .or_else(|| inputs.values().next().cloned())
                .unwrap_or(Value::Null);
            let output =
                template_output_payload(projection, first_output_key(projection), value, &pool);
            (inputs, output)
        };
        pool.insert(
            projection.node_id.clone(),
            project_node_variable_payload(projection, &output)?,
        );
        trace.input_payload = Value::Object(inputs);
        trace.output_payload = output;
    }
    outcome.variable_pool = pool;
    outcome.stop_reason = successful_flow_stop_reason(&outcome.node_traces);
    Ok(outcome)
}

fn ensure_refreshable_answer(
    plan: &CompiledPlan,
    outcome: &FlowDebugExecutionOutcome,
    response_node_id: &str,
) -> Result<()> {
    let Some(answer) = outcome
        .node_traces
        .iter()
        .rev()
        .find(|trace| trace.node_type == "answer")
    else {
        if outcome
            .node_traces
            .last()
            .is_some_and(|trace| trace.node_id == response_node_id)
        {
            return Ok(());
        }
        bail!("communication_answer_not_refreshable_from_retained_llm");
    };
    let mut refreshed = BTreeSet::from([response_node_id.to_owned()]);
    for id in &plan.topological_order {
        let Some(node) = plan.nodes.get(id) else {
            continue;
        };
        if pure_projection(&node.node_type)
            && outcome.node_traces.iter().any(|trace| &trace.node_id == id)
            && node
                .bindings
                .values()
                .flat_map(|binding| &binding.selector_paths)
                .any(|path| {
                    path.first()
                        .is_some_and(|source| refreshed.contains(source))
                })
        {
            refreshed.insert(id.clone());
        }
    }
    if !refreshed.contains(&answer.node_id) {
        bail!("communication_answer_not_refreshable_from_retained_llm");
    }
    Ok(())
}

pub(in crate::execution_engine) fn allow_scheduled_node(
    pool: &Map<String, Value>,
    node: &CompiledNode,
) -> bool {
    let Some(continuation) = pool.get(FINAL_PROJECTION_KEY) else {
        return true;
    };
    continuation.get("node_id").and_then(Value::as_str) == Some(node.node_id.as_str())
        || continuation
            .get("projection_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| {
                ids.iter()
                    .any(|id| id.as_str() == Some(node.node_id.as_str()))
            })
}
