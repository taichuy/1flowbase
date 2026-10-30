use super::*;
use crate::{
    container_contract::{ContainerConfig, ContainerErrorPolicy},
    execution_engine::*,
};
use uuid::Uuid;

/// Stage only the changed fields. A rejected admission must never leave its
/// proposed result/variables in the next closure checkpoint.
pub(super) async fn commit_round_terminal(
    state: &mut ContainerCheckpoint,
    frame_id: Uuid,
    index: u64,
    node: &CompiledNode,
    config: &ContainerConfig,
    output: Value,
    variables: Map<String, Value>,
    error_payload: Option<Value>,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    let success = error_payload.is_none();
    let frame = state.frames.get_mut(&frame_id).expect("active frame");
    let identity = frame.identity.clone();
    let was_closed = frame.closed;
    let previous_failure = frame.global_failure.clone();
    let round = frame.rounds.get_mut(&index).expect("active round");
    let prior_status = round.status;
    let prior_wait = round.waiting.take();
    let prior_error = std::mem::replace(&mut round.error_payload, error_payload.clone());
    round.status = if success {
        ContainerRoundStatus::Succeeded
    } else {
        ContainerRoundStatus::Failed
    };
    let mut prior_result = None;
    let mut prior_variables = None;
    let mut prior_context_variables = None;
    if !frame.closed {
        if let Some(error) = error_payload {
            match config.error_policy {
                ContainerErrorPolicy::Fail => {
                    frame.closed = true;
                    frame.global_failure = Some(error);
                }
                ContainerErrorPolicy::Break => frame.closed = true,
                ContainerErrorPolicy::Continue => {}
            }
        } else {
            prior_result = frame.results.insert(index, output);
            if node.node_type == "loop" {
                prior_variables = Some(std::mem::replace(&mut frame.variables, variables));
                if let Some(context) = round
                    .variable_pool
                    .get_mut(&node.node_id)
                    .and_then(Value::as_object_mut)
                {
                    prior_context_variables = context.insert(
                        "variables".to_owned(),
                        Value::Object(frame.variables.clone()),
                    );
                }
            }
        }
    }
    let committed = emit(
        lifecycle,
        state,
        ContainerTransitionKind::RoundCommitted,
        &identity,
        None,
        None,
        Some(&json!({"round_index":index})),
    )
    .await;
    if let Err(error) = committed {
        let frame = state.frames.get_mut(&frame_id).expect("active frame");
        frame.closed = was_closed;
        frame.global_failure = previous_failure;
        let round = frame.rounds.get_mut(&index).expect("active round");
        round.status = prior_status;
        round.waiting = prior_wait;
        round.error_payload = prior_error;
        if success && !was_closed {
            if let Some(value) = prior_result {
                frame.results.insert(index, value);
            } else {
                frame.results.remove(&index);
            }
            if let Some(variables) = prior_variables {
                frame.variables = variables;
                if let Some(context) = round
                    .variable_pool
                    .get_mut(&node.node_id)
                    .and_then(Value::as_object_mut)
                {
                    if let Some(value) = prior_context_variables {
                        context.insert("variables".to_owned(), value);
                    } else {
                        context.remove("variables");
                    }
                }
            }
        }
        if success
            && error
                .downcast_ref::<DeadlineExpired>()
                .is_some_and(|expired| expired.0 == frame_id)
        {
            // The failed closure is not admission and remains legal after expiry.
            return Box::pin(commit_round_terminal(state,frame_id,index,node,config,Value::Null,Map::new(),Some(json!({"error_code":"container_round_timeout","message":"Round deadline expired while awaiting durable result commit"})),lifecycle)).await;
        }
        return Err(error);
    }
    Ok(())
}
