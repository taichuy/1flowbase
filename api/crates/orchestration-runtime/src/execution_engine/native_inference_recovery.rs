//! Re-enter one proven failed inference; never traverse the workflow prefix again.
use super::*;

/// Validates the complete possible tail, including error edges, before any invocation.
pub fn validate_native_inference_recovery_scope(plan: &CompiledPlan, node_id: &str) -> Result<()> {
    recovery_scope(plan, node_id).map(|_| ())
}

fn recovery_scope(plan: &CompiledPlan, node_id: &str) -> Result<BTreeSet<String>> {
    let selected = plan
        .nodes
        .get(node_id)
        .ok_or_else(|| anyhow!("native_recovery_node_missing"))?;
    if selected.node_type != "llm"
        || selected.llm_runtime.is_none()
        || selected.container_id.is_some()
        || !visible_internal_llm_tool_target_node_ids(plan).is_empty()
    {
        return Err(anyhow!("native_recovery_scope_unsupported"));
    }
    let positions: std::collections::BTreeMap<_, _> = plan
        .topological_order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    if positions.len() != plan.topological_order.len() || !positions.contains_key(node_id) {
        return Err(anyhow!("native_recovery_scope_invalid"));
    }
    let mut visited = BTreeSet::new();
    let mut pending = vec![node_id.to_string()];
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let node = plan
            .nodes
            .get(&id)
            .ok_or_else(|| anyhow!("native_recovery_node_missing"))?;
        if id != node_id
            && (!matches!(node.node_type.as_str(), "answer" | "variable_aggregator")
                || node.plugin_runtime.is_some()
                || node.code_runtime.is_some()
                || node.container_id.is_some())
        {
            return Err(anyhow!("native_recovery_scope_unsupported"));
        }
        let targets: Vec<&str> = if plan.edges.is_empty() {
            node.downstream_node_ids
                .iter()
                .map(String::as_str)
                .collect()
        } else {
            plan.edges
                .iter()
                .filter(|edge| edge.source == id)
                .map(|edge| edge.target.as_str())
                .collect()
        };
        for target in targets {
            if positions
                .get(target)
                .zip(positions.get(id.as_str()))
                .is_none_or(|(target_index, index)| target_index <= index)
            {
                return Err(anyhow!("native_recovery_scope_invalid"));
            }
            pending.push(target.to_string());
        }
    }
    Ok(visited)
}

pub async fn recover_native_inference_with_runtime_context_and_lifecycle<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    node_id: &str,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let scope = recovery_scope(plan, node_id)?;
    if plan
        .topological_order
        .get(checkpoint.next_node_index)
        .map(String::as_str)
        != Some(node_id)
        || !checkpoint.active_node_ids.iter().any(|id| id == node_id)
        || has_visible_internal_llm_tool_callback_state(&checkpoint.variable_pool)
    {
        return Err(anyhow!("native_recovery_checkpoint_mismatch"));
    }
    for active in &checkpoint.active_node_ids {
        if plan
            .topological_order
            .iter()
            .position(|id| id == active)
            .is_none_or(|index| index >= checkpoint.next_node_index && !scope.contains(active))
        {
            return Err(anyhow!("native_recovery_parallel_scope_unsupported"));
        }
    }
    // The full, admitted native request already carries the consumed outputs. Dropping the
    // pending state prevents cursor reuse, callback re-consumption and tool delivery replay.
    let mut variables = checkpoint.variable_pool.clone();
    variables.remove(node_id);
    execute_from(
        plan,
        checkpoint.next_node_index,
        variables,
        Some(BTreeSet::from([node_id.to_owned()])),
        &runtime_context,
        invoker,
        lifecycle,
    )
    .await
}

#[cfg(test)]
#[path = "_tests/native_inference_recovery.rs"]
mod tests;
