use super::*;
use crate::execution_engine::*;
use uuid::Uuid;

/// The frame owns the only durable container terminal. Its receipt, closed frame,
/// parent cursor cache and public output become visible in the same transition.
pub(super) async fn commit_frame_terminal(
    state: &mut ContainerCheckpoint,
    frame_id: Uuid,
    trace: NodeExecutionTrace,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<NodeExecutionTrace> {
    let frame = state
        .frames
        .get_mut(&frame_id)
        .ok_or_else(|| anyhow!("terminal frame missing"))?;
    if let Some(committed) = &frame.terminal_trace {
        return Ok(committed.clone());
    }
    let identity = frame.identity.clone();
    let parent_path = identity.round_path.last().cloned();
    let was_closed = frame.closed;
    frame.closed = true;
    frame.terminal_trace = Some(trace.clone());
    let mut previous_parent_terminal = None;
    let mut previous_parent_output = None;
    if let Some(path) = &parent_path {
        let parent = state
            .frames
            .get_mut(&path.container_execution_id)
            .and_then(|frame| frame.rounds.get_mut(&path.round_index))
            .ok_or_else(|| anyhow!("terminal parent round missing"))?;
        let cached = parent
            .nodes
            .get_mut(&identity.node_id)
            .ok_or_else(|| anyhow!("terminal parent node missing"))?;
        previous_parent_terminal = cached.terminal_trace.replace(trace.clone());
        if trace.error_payload.is_none() {
            previous_parent_output = parent
                .variable_pool
                .insert(identity.node_id.clone(), trace.output_payload.clone());
        }
    }
    let committed = emit(
        lifecycle,
        state,
        ContainerTransitionKind::FrameClosed,
        &identity,
        None,
        Some(&trace),
        None,
    )
    .await;
    if let Err(error) = committed {
        let frame = state.frames.get_mut(&frame_id).expect("terminal frame");
        frame.closed = was_closed;
        frame.terminal_trace = None;
        if let Some(path) = &parent_path {
            let parent = state
                .frames
                .get_mut(&path.container_execution_id)
                .expect("parent frame")
                .rounds
                .get_mut(&path.round_index)
                .expect("parent round");
            parent
                .nodes
                .get_mut(&identity.node_id)
                .expect("parent node")
                .terminal_trace = previous_parent_terminal;
            if trace.error_payload.is_none() {
                if let Some(output) = previous_parent_output {
                    parent
                        .variable_pool
                        .insert(identity.node_id.clone(), output);
                } else {
                    parent.variable_pool.remove(&identity.node_id);
                }
            }
        }
        return Err(error);
    }
    Ok(trace)
}
