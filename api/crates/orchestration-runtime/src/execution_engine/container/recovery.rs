use super::*;
use crate::{
    container_contract::{is_container_type, ContainerConfig},
    execution_engine::*,
};
use uuid::Uuid;

impl ContainerWait {
    pub(crate) fn from_stop_reason(reason: ExecutionStopReason) -> Option<Self> {
        match reason {
            ExecutionStopReason::WaitingHuman(wait) => Some(Self::Human {
                node_id: wait.node_id,
                node_alias: wait.node_alias,
                prompt: wait.prompt,
            }),
            ExecutionStopReason::WaitingCallback(wait) => Some(Self::Callback {
                node_id: wait.node_id,
                node_alias: wait.node_alias,
                callback_kind: wait.callback_kind,
                request_payload: wait.request_payload,
            }),
            _ => None,
        }
    }
    pub(crate) fn node_id(&self) -> &str {
        match self {
            Self::Human { node_id, .. } | Self::Callback { node_id, .. } => node_id,
        }
    }
    pub(crate) fn stop_reason(self) -> ExecutionStopReason {
        match self {
            Self::Human {
                node_id,
                node_alias,
                prompt,
            } => ExecutionStopReason::WaitingHuman(PendingHumanInput {
                node_id,
                node_alias,
                prompt,
            }),
            Self::Callback {
                node_id,
                node_alias,
                callback_kind,
                request_payload,
            } => ExecutionStopReason::WaitingCallback(PendingCallbackTask {
                node_id,
                node_alias,
                callback_kind,
                request_payload,
            }),
        }
    }
}

pub(crate) async fn execute_root<I>(
    plan: &CompiledPlan,
    index: usize,
    pool: Map<String, Value>,
    active: Option<BTreeSet<String>>,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
    restored: Option<Session>,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let active = active.unwrap_or_else(|| initial_active_node_ids(plan));
    let session = match restored {
        Some(session) => session,
        None => {
            let budget = plan
                .nodes
                .values()
                .filter(|node| is_container_type(&node.node_type))
                .map(ContainerConfig::from_node)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(|config| config.parallelism)
                .max()
                .unwrap_or(1);
            Session::new(
                ContainerCheckpoint {
                    version: 1,
                    waiting_execution_id: None,
                    root_next_node_index: index,
                    root_variable_pool: pool.clone(),
                    root_active_node_ids: active.iter().cloned().collect(),
                    root_nodes: BTreeMap::new(),
                    root_waiting: None,
                    frames: BTreeMap::new(),
                    execution_budget: budget,
                },
                false,
            )?
        }
    };
    {
        let state = session.state.lock().await;
        if let Some(wait) = &state.root_waiting {
            return Ok(FlowDebugExecutionOutcome {
                stop_reason: wait.clone().stop_reason(),
                variable_pool: state.root_variable_pool.clone(),
                checkpoint_snapshot: Some(snapshot_from_state(&state)),
                operation_terminal: None,
                node_traces: Vec::new(),
            });
        }
    }
    let root_lifecycle = RootLifecycle {
        session: session.clone(),
        owner: lifecycle,
        current_identity: Arc::default(),
    };
    let outcome = CURRENT_EXECUTION
        .scope(
            root_lifecycle.current_identity.clone(),
            Box::pin(super::super::scheduler::execute_from_scoped(
                plan,
                index,
                pool,
                Some(active),
                runtime,
                invoker,
                &root_lifecycle,
                None,
                Some(&session),
            )),
        )
        .await;
    match outcome {
        Ok(mut outcome) => {
            let mut state = session.state.lock().await;
            for trace in &mut outcome.node_traces {
                if trace.execution_identity.is_none() {
                    trace.execution_identity = state
                        .root_nodes
                        .get(&trace.node_id)
                        .map(|node| node.identity.clone());
                }
            }
            if let Some(wait) = ContainerWait::from_stop_reason(outcome.stop_reason.clone()) {
                if let Some(identity) = state
                    .root_nodes
                    .get(wait.node_id())
                    .map(|node| node.identity.clone())
                {
                    let checkpoint = outcome
                        .checkpoint_snapshot
                        .as_ref()
                        .ok_or_else(|| anyhow!("root wait checkpoint missing"))?;
                    state.root_next_node_index = checkpoint.next_node_index;
                    state
                        .root_variable_pool
                        .clone_from(&checkpoint.variable_pool);
                    state
                        .root_active_node_ids
                        .clone_from(&checkpoint.active_node_ids);
                    state.root_waiting = Some(wait);
                    state.waiting_execution_id = Some(identity.execution_id);
                    let trace = outcome
                        .node_traces
                        .iter()
                        .find(|trace| trace.node_id == identity.node_id);
                    emit(
                        lifecycle,
                        &state,
                        ContainerTransitionKind::ScopeSuspended,
                        &identity,
                        None,
                        trace,
                        Some(&json!({"root":true})),
                    )
                    .await?;
                    outcome.checkpoint_snapshot = Some(snapshot_from_state(&state));
                }
            } else if matches!(outcome.stop_reason, ExecutionStopReason::Failed(_))
                && outcome.checkpoint_snapshot.is_none()
            {
                outcome.checkpoint_snapshot = Some(snapshot_from_state(&state));
            }
            Ok(outcome)
        }
        Err(error) if error.is::<GlobalFailure>() => {
            let state = session.state.lock().await;
            let failure = error
                .downcast_ref::<GlobalFailure>()
                .expect("global failure");
            let node_id = plan
                .topological_order
                .get(state.root_next_node_index)
                .cloned()
                .ok_or_else(|| anyhow!("container recovery root missing"))?;
            Ok(FlowDebugExecutionOutcome {
                stop_reason: ExecutionStopReason::Failed(NodeExecutionFailure {
                    node_alias: plan.nodes[&node_id].alias.clone(),
                    node_id,
                    error_payload: failure.0.clone(),
                }),
                variable_pool: state.root_variable_pool.clone(),
                checkpoint_snapshot: Some(CheckpointSnapshot {
                    container_state: Some(state.clone()),
                    next_node_index: state.root_next_node_index,
                    variable_pool: state.root_variable_pool.clone(),
                    active_node_ids: state.root_active_node_ids.clone(),
                }),
                operation_terminal: None,
                node_traces: Vec::new(),
            })
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn waiting_execution_id(
    checkpoint: &CheckpointSnapshot,
    waiting_node_id: &str,
) -> Result<Uuid> {
    let state = checkpoint
        .container_state
        .as_ref()
        .ok_or_else(|| anyhow!("container checkpoint missing"))?;
    if state
        .root_waiting
        .as_ref()
        .is_some_and(|wait| wait.node_id() == waiting_node_id)
    {
        return state
            .root_nodes
            .get(waiting_node_id)
            .map(|node| node.identity.execution_id)
            .ok_or_else(|| anyhow!("root waiting execution missing"));
    }
    let ids: Vec<_> = state
        .frames
        .values()
        .flat_map(|frame| frame.rounds.values())
        .filter(|round| {
            round
                .waiting
                .as_ref()
                .is_some_and(|wait| wait.node_id() == waiting_node_id)
        })
        .filter_map(|round| {
            round
                .nodes
                .get(waiting_node_id)
                .map(|node| node.identity.execution_id)
        })
        .collect();
    if let Some(selected) = state
        .waiting_execution_id
        .filter(|selected| ids.contains(selected))
    {
        return Ok(selected);
    }
    match ids.as_slice() {
        [id] => Ok(*id),
        [] => bail!("container waiting execution missing"),
        _ => bail!("container waiting node is ambiguous; select execution_id"),
    }
}

/// Resume one logical wait. Parallel rounds sharing a static node definition
/// must select its execution UUID, never whichever round happens to be first.
pub async fn resume_scoped_container_with_runtime_context_and_lifecycle<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    execution_id: Uuid,
    resume_payload: &Value,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut state = checkpoint
        .container_state
        .clone()
        .ok_or_else(|| anyhow!("container checkpoint missing"))?;
    if state
        .root_waiting
        .as_ref()
        .and_then(|wait| state.root_nodes.get(wait.node_id()))
        .is_some_and(|node| node.identity.execution_id == execution_id)
    {
        return resume_root_wait(
            plan,
            state,
            execution_id,
            resume_payload,
            &runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    }
    let selected = state
        .frames
        .iter()
        .find_map(|(frame_id, frame)| {
            frame.rounds.iter().find_map(|(round_index, round)| {
                let wait = round.waiting.as_ref()?;
                let node = round.nodes.get(wait.node_id())?;
                (node.identity.execution_id == execution_id)
                    .then(|| (*frame_id, *round_index, node.identity.clone()))
            })
        })
        .ok_or_else(|| anyhow!("container waiting execution missing"))?;
    let (frame_id, round_index, identity) = selected;
    let frame = state.frames.get_mut(&frame_id).expect("selected frame");
    if frame.closed {
        bail!("container is already closed");
    }
    let round = frame.rounds.get_mut(&round_index).expect("selected round");
    let expired = round.deadline_unix_ms <= unix_ms();
    if expired {
        return recover_callback_deadline(
            plan,
            checkpoint,
            &identity,
            round.deadline_owner_execution_id,
            &runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    }
    let node = plan
        .nodes
        .get(&identity.node_id)
        .ok_or_else(|| anyhow!("waiting node missing"))?;
    let mut terminal = None;
    if !expired {
        if pending_llm_tool_callback_state(&round.variable_pool, &node.node_id).is_some() {
            append_llm_tool_result_messages(
                &mut round.variable_pool,
                &node.node_id,
                resume_payload,
            )?;
            // The provider callback is an admitted continuation of the same
            // logical node, not an uncertain automatic replay.
            round
                .nodes
                .get_mut(&node.node_id)
                .expect("waiting node")
                .terminal_trace = None;
        } else {
            let patch = resume_payload
                .as_object()
                .ok_or_else(|| anyhow!("resume payload must be an object"))?;
            for key in patch.keys() {
                if !node.outputs.iter().any(|output| &output.key == key) {
                    bail!(
                        "resume payload key {key} is not a public output for {}",
                        node.node_id
                    );
                }
            }
            round
                .variable_pool
                .insert(node.node_id.clone(), Value::Object(patch.clone()));
            let trace = NodeExecutionTrace {
                execution_identity: Some(identity.clone()),
                node_id: node.node_id.clone(),
                node_type: node.node_type.clone(),
                node_alias: node.alias.clone(),
                input_payload: round.nodes[&node.node_id].input_payload.clone(),
                output_payload: Value::Object(patch.clone()),
                error_payload: None,
                metrics_payload: json!({"resumed": true}),
                debug_payload: json!({}),
                provider_events: Vec::new(),
            };
            round
                .nodes
                .get_mut(&node.node_id)
                .expect("selected node")
                .terminal_trace = Some(trace.clone());
            terminal = Some(trace);
        }
    }
    round.waiting = None;
    round.status = ContainerRoundStatus::Running;
    // Ancestor frames retain their exact container cursor. Clear the propagated
    // wait so they re-enter the same child frame, without consuming a new round.
    for path in &identity.round_path {
        if let Some(ancestor_round) = state
            .frames
            .get_mut(&path.container_execution_id)
            .and_then(|frame| frame.rounds.get_mut(&path.round_index))
        {
            ancestor_round.waiting = None;
            ancestor_round.status = ContainerRoundStatus::Running;
        }
    }
    state.waiting_execution_id = None;
    let admitted = emit(
        lifecycle,
        &state,
        if terminal.is_some() {
            ContainerTransitionKind::NodeCompleted
        } else {
            ContainerTransitionKind::Resumed
        },
        &identity,
        Some(resume_payload),
        terminal.as_ref(),
        Some(&json!({"expired": expired})),
    )
    .await;
    if let Err(error) = admitted {
        if let Some(expired) = error.downcast_ref::<DeadlineExpired>() {
            return recover_callback_deadline(
                plan,
                checkpoint,
                &identity,
                expired.0,
                &runtime_context,
                invoker,
                lifecycle,
            )
            .await;
        }
        return Err(error);
    }
    let index = state.root_next_node_index;
    let pool = state.root_variable_pool.clone();
    let active = state.root_active_node_ids.iter().cloned().collect();
    let mut session = Session::new(state, true)?;
    Arc::make_mut(&mut session.recovered_nodes).remove(&execution_id);
    execute_root(
        plan,
        index,
        pool,
        Some(active),
        &runtime_context,
        invoker,
        lifecycle,
        Some(session),
    )
    .await
}

pub(crate) async fn recover_native_container<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    node_id: &str,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut state = checkpoint
        .container_state
        .clone()
        .ok_or_else(|| anyhow!("container checkpoint missing"))?;
    if state.root_nodes.get(node_id).is_some_and(|node| {
        node.terminal_trace
            .as_ref()
            .is_some_and(|trace| trace.error_payload.is_some())
    }) {
        return recover_root_native(
            plan,
            checkpoint,
            state,
            node_id,
            runtime_context,
            invoker,
            lifecycle,
        )
        .await;
    }
    let candidates: Vec<_> = state
        .frames
        .iter()
        .flat_map(|(frame_id, frame)| {
            frame.rounds.iter().filter_map(move |(index, round)| {
                round
                    .nodes
                    .get(node_id)
                    .filter(|node| {
                        node.terminal_trace
                            .as_ref()
                            .is_some_and(|trace| trace.error_payload.is_some())
                    })
                    .map(|node| (*frame_id, *index, node.identity.clone()))
            })
        })
        .collect();
    let [(frame_id, round_index, identity)] = candidates.as_slice() else {
        bail!("native_recovery_container_scope_ambiguous_or_missing");
    };
    let node = plan
        .nodes
        .get(node_id)
        .ok_or_else(|| anyhow!("native_recovery_node_missing"))?;
    if node.node_type != "llm" || node.llm_runtime.is_none() {
        bail!("native_recovery_scope_unsupported");
    }
    let frame = state.frames.get_mut(frame_id).expect("recovery frame");
    frame.closed = false;
    frame.terminal_trace = None;
    frame.global_failure = None;
    let round = frame.rounds.get_mut(round_index).expect("recovery round");
    round.status = ContainerRoundStatus::Running;
    round.error_payload = None;
    round.waiting = None;
    if let Some(grant) = checkpoint
        .variable_pool
        .get("sys")
        .and_then(|sys| sys.get("native_inference_recovery"))
    {
        let sys = round
            .variable_pool
            .entry("sys".to_owned())
            .or_insert_with(|| json!({}));
        sys.as_object_mut()
            .ok_or_else(|| anyhow!("invalid recovery system scope"))?
            .insert("native_inference_recovery".to_owned(), grant.clone());
        if let Some(deadline) = grant
            .get("absolute_deadline_unix_ms")
            .and_then(Value::as_i64)
        {
            round.deadline_unix_ms = round.deadline_unix_ms.min(deadline);
        }
    }
    round.variable_pool.remove(node_id);
    round
        .nodes
        .get_mut(node_id)
        .expect("recovery node")
        .terminal_trace = None;
    round.next_node_index = plan
        .topological_order
        .iter()
        .position(|id| id == node_id)
        .ok_or_else(|| anyhow!("native_recovery_node_missing"))?;
    for path in &identity.round_path {
        if let Some(frame) = state.frames.get_mut(&path.container_execution_id) {
            frame.closed = false;
            frame.terminal_trace = None;
            frame.global_failure = None;
            if let Some(round) = frame.rounds.get_mut(&path.round_index) {
                round.status = ContainerRoundStatus::Running;
                round.waiting = None;
                round.error_payload = None;
            }
        }
    }
    // Explicit native recovery reopens only the failed ancestor continuation.
    // Its cached container terminal cannot short-circuit the selected retry.
    let reopened: Vec<_> = identity
        .round_path
        .iter()
        .filter_map(|path| {
            state
                .frames
                .get(&path.container_execution_id)
                .map(|frame| frame.identity.clone())
        })
        .collect();
    for container in reopened {
        if let Some(path) = container.round_path.last() {
            if let Some(parent) = state
                .frames
                .get_mut(&path.container_execution_id)
                .and_then(|frame| frame.rounds.get_mut(&path.round_index))
            {
                if let Some(node) = parent.nodes.get_mut(&container.node_id) {
                    node.terminal_trace = None;
                }
                parent.variable_pool.remove(&container.node_id);
            }
        }
    }
    emit(
        lifecycle,
        &state,
        ContainerTransitionKind::Resumed,
        identity,
        None,
        None,
        None,
    )
    .await?;
    let (index, pool, active) = (
        state.root_next_node_index,
        state.root_variable_pool.clone(),
        state.root_active_node_ids.iter().cloned().collect(),
    );
    let mut session = Session::new(state, true)?;
    Arc::make_mut(&mut session.recovered_nodes).remove(&identity.execution_id);
    execute_root(
        plan,
        index,
        pool,
        Some(active),
        runtime_context,
        invoker,
        lifecycle,
        Some(session),
    )
    .await
}

/// Recover a durable active checkpoint without inventing a callback. Completed
/// nodes are reused; an unacknowledged external execution is an uncertain failure.
pub async fn recover_container_with_runtime_context_and_lifecycle<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    runtime_context: ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let state = checkpoint
        .container_state
        .clone()
        .ok_or_else(|| anyhow!("container checkpoint missing"))?;
    let (index, pool, active) = (
        state.root_next_node_index,
        state.root_variable_pool.clone(),
        state.root_active_node_ids.iter().cloned().collect(),
    );
    let session = Session::new(state, true)?;
    execute_root(
        plan,
        index,
        pool,
        Some(active),
        &runtime_context,
        invoker,
        lifecycle,
        Some(session),
    )
    .await
}

pub(crate) fn snapshot_from_state(state: &ContainerCheckpoint) -> CheckpointSnapshot {
    CheckpointSnapshot {
        container_state: Some(state.clone()),
        next_node_index: state.root_next_node_index,
        variable_pool: state.root_variable_pool.clone(),
        active_node_ids: state.root_active_node_ids.clone(),
    }
}

/// A callback rejected at admission has not changed durable truth. Resume from
/// the original checkpoint and route a pending timeout to its actual owner;
/// do not renew any deadline, consume another round, or admit the rejected data.
async fn recover_callback_deadline<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    waiting_identity: &ScopedExecutionIdentity,
    owner: uuid::Uuid,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let mut state = checkpoint
        .container_state
        .clone()
        .ok_or_else(|| anyhow!("container checkpoint missing"))?;
    let owner_position = waiting_identity
        .round_path
        .iter()
        .position(|path| path.container_execution_id == owner)
        .ok_or_else(|| anyhow!("callback deadline owner is outside its scope"))?;
    for path in waiting_identity.round_path.iter().take(owner_position + 1) {
        let round = state
            .frames
            .get_mut(&path.container_execution_id)
            .and_then(|frame| frame.rounds.get_mut(&path.round_index))
            .ok_or_else(|| anyhow!("callback deadline round missing"))?;
        round.status = ContainerRoundStatus::Running;
        round.waiting = None;
        if path.container_execution_id == owner {
            round.error_payload = Some(
                json!({"error_code":"container_round_timeout","deadline_owner_execution_id":owner,"message":"Callback was not admitted before the frozen round deadline"}),
            );
        }
    }
    state.waiting_execution_id = None;
    let (index, pool, active) = (
        state.root_next_node_index,
        state.root_variable_pool.clone(),
        state.root_active_node_ids.iter().cloned().collect(),
    );
    let session = Session::new(state, true)?;
    Box::pin(execute_root(
        plan,
        index,
        pool,
        Some(active),
        runtime,
        invoker,
        lifecycle,
        Some(session),
    ))
    .await
}
