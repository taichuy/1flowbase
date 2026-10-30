use super::*;
use crate::{
    binding_runtime::resolve_binding,
    container_contract::{is_container_type, ContainerConfig, ContainerErrorPolicy},
    execution_engine::*,
};
use futures_util::{stream::FuturesUnordered, StreamExt};
use uuid::Uuid;

pub(crate) struct ContainerExecution {
    pub output: Value,
    pub traces: Vec<NodeExecutionTrace>,
    pub waiting: Option<Box<FlowDebugExecutionOutcome>>,
}

pub(crate) async fn execute_container<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    pool: &Map<String, Value>,
    index: usize,
    active: &BTreeSet<String>,
    parent: Option<&Scope>,
    restored_session: Option<&Session>,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<ContainerExecution>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let config = ContainerConfig::from_node(node)?;
    let session = match parent.map(|scope| &scope.session).or(restored_session) {
        Some(session) => session.clone(),
        None => {
            let budget = plan
                .nodes
                .values()
                .filter(|n| is_container_type(&n.node_type))
                .map(ContainerConfig::from_node)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .map(|c| c.parallelism)
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
    if parent.is_none() {
        let mut state = session.state.lock().await;
        let previous_roots: BTreeSet<_> = state
            .frames
            .values()
            .filter(|frame| {
                frame.identity.parent_execution_id.is_none()
                    && frame.identity.node_id != node.node_id
                    && frame.closed
            })
            .map(|frame| frame.identity.execution_id)
            .collect();
        state.frames.retain(|id, frame| {
            !previous_roots.contains(id)
                && !frame
                    .identity
                    .round_path
                    .iter()
                    .any(|path| previous_roots.contains(&path.container_execution_id))
        });
        state.root_next_node_index = index;
        state.root_variable_pool.clone_from(pool);
        state.root_active_node_ids = active.iter().cloned().collect();
    }
    let parent_id = parent.map(|scope| scope.frame_id);
    let parent_round = parent.map(|scope| scope.round_index);
    let existing = {
        let state = session.state.lock().await;
        state
            .frames
            .values()
            .find(|frame| {
                frame.identity.node_id == node.node_id
                    && frame.identity.parent_execution_id == parent_id
                    && frame.identity.round_path.last().map(|p| p.round_index) == parent_round
            })
            .map(|frame| frame.identity.execution_id)
    };
    let frame_id = if let Some(frame_id) = existing {
        let state = session.state.lock().await;
        let frame = &state.frames[&frame_id];
        if !frame.closed {
            emit(
                lifecycle,
                &state,
                ContainerTransitionKind::Resumed,
                &frame.identity,
                None,
                None,
                None,
            )
            .await?;
        }
        frame_id
    } else {
        let items = if node.node_type == "iteration" {
            let value = resolve_binding(
                node.bindings
                    .get("items")
                    .ok_or_else(|| anyhow!("iteration requires items"))?,
                pool,
            )?;
            Some(
                value
                    .as_array()
                    .ok_or_else(|| anyhow!("iteration items must be an array"))?
                    .clone(),
            )
        } else {
            None
        };
        let variables = resolve_variables(node, "variables", pool)?;
        validate_variables(&config, &variables)?;
        let mut path = Vec::new();
        let identity_id = Uuid::now_v7();
        if let Some(scope) = parent {
            let state = session.state.lock().await;
            let parent_frame = &state.frames[&scope.frame_id];
            path = parent_frame.identity.round_path.clone();
            path.push(ContainerRoundPath {
                container_execution_id: scope.frame_id,
                container_node_id: parent_frame.identity.node_id.clone(),
                round_index: scope.round_index,
            });
        }
        let identity = ScopedExecutionIdentity {
            execution_id: identity_id,
            node_id: node.node_id.clone(),
            parent_execution_id: parent_id,
            round_path: path,
        };
        let limit = items
            .as_ref()
            .map(|items| items.len() as u64)
            .or(config.max_rounds)
            .ok_or_else(|| anyhow!("container limit missing"))?;
        let mut state = session.state.lock().await;
        let input_payload = json!({"variables": variables});
        if let Some(scope) = parent {
            let round = state
                .frames
                .get_mut(&scope.frame_id)
                .expect("parent frame")
                .rounds
                .get_mut(&scope.round_index)
                .expect("parent round");
            if round.nodes.contains_key(&node.node_id) {
                bail!("container parent execution exists without its frame");
            }
            round.nodes.insert(
                node.node_id.clone(),
                ScopedNodeState {
                    identity: identity.clone(),
                    input_payload: input_payload.clone(),
                    terminal_trace: None,
                },
            );
        }
        state.frames.insert(
            identity_id,
            ContainerFrame {
                identity: identity.clone(),
                frozen_limit: limit,
                started_rounds: 0,
                frozen_items: items,
                variables,
                rounds: BTreeMap::new(),
                results: BTreeMap::new(),
                closed: false,
                terminal_trace: None,
                global_failure: None,
                final_condition_checked: false,
                control_failures: Vec::new(),
            },
        );
        let entered = emit(
            lifecycle,
            &state,
            ContainerTransitionKind::FrameEntered,
            &identity,
            Some(&input_payload),
            None,
            Some(&json!({"node_type": node.node_type, "node_alias": node.alias})),
        )
        .await;
        if let Err(error) = entered {
            state.frames.remove(&identity_id);
            if let Some(scope) = parent {
                state
                    .frames
                    .get_mut(&scope.frame_id)
                    .expect("parent frame")
                    .rounds
                    .get_mut(&scope.round_index)
                    .expect("parent round")
                    .nodes
                    .remove(&node.node_id);
            }
            return Err(error);
        }
        identity_id
    };
    if let Some(scope) = parent {
        let state = session.state.lock().await;
        enter_identity(&scope.current_identity, &state.frames[&frame_id].identity)?;
    }
    let (closed, failure) = {
        let mut state = session.state.lock().await;
        check_final_condition(&mut state, frame_id, node, &config, lifecycle).await?;
        (
            state.frames[&frame_id].closed,
            state.frames[&frame_id].global_failure.clone(),
        )
    };
    // A committed break/fail may precede host cancellation or FrameClosed.
    // Recovery finishes that closure; it cannot restart the old in-flight rounds.
    if closed {
        cancel_active_rounds(&session, frame_id, lifecycle).await?;
    }
    if let Some(error) = failure {
        let mut state = session.state.lock().await;
        emit_failed_frame(&mut state, frame_id, node, &error, lifecycle).await?;
        return Err(GlobalFailure(error).into());
    }
    let inherited_deadline = match parent {
        Some(scope) => scope.deadline().await,
        None => None,
    };
    let mut futures = FuturesUnordered::new();
    let mut traces = Vec::new();
    let mut resumed = {
        let state = session.state.lock().await;
        let frame = &state.frames[&frame_id];
        if let Some(error) = &frame.global_failure {
            return Err(GlobalFailure(error.clone()).into());
        }
        frame
            .rounds
            .iter()
            .filter(|(_, round)| {
                matches!(
                    round.status,
                    ContainerRoundStatus::Running | ContainerRoundStatus::Waiting
                )
            })
            .map(|(index, _)| *index)
            .collect::<std::collections::VecDeque<_>>()
    };
    let mut suspended = false;
    loop {
        while futures.len() < config.parallelism && !suspended {
            let round_index = if let Some(round_index) = resumed.pop_front() {
                Some(round_index)
            } else {
                start_round(
                    plan,
                    node,
                    &config,
                    pool,
                    &session,
                    frame_id,
                    inherited_deadline,
                    lifecycle,
                )
                .await?
            };
            let Some(round_index) = round_index else {
                break;
            };
            let round_scope = Scope::new(
                session.clone(),
                frame_id,
                round_index,
                parent.and_then(|scope| scope.expires_at),
            )
            .await?;
            futures.push(Box::pin(execute_round(
                plan,
                node,
                &config,
                round_scope,
                runtime,
                invoker,
                lifecycle,
            )));
        }
        let Some(result) = futures.next().await else {
            break;
        };
        let (scope, outcome) = match result {
            Ok(result) => result,
            Err(error) if error.is::<GlobalFailure>() => {
                drop(futures);
                let failure = error
                    .downcast_ref::<GlobalFailure>()
                    .expect("global failure")
                    .0
                    .clone();
                propagate_global_failure(&session, frame_id, node, &failure, lifecycle).await?;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        match outcome {
            RoundOutcome::Waiting(mut round_traces) => {
                traces.append(&mut round_traces);
                suspended = true;
            }
            RoundOutcome::Terminal {
                already_committed,
                output,
                variables,
                mut error,
                mut node_traces,
            } => {
                if error.is_none() {
                    if scope.expired() {
                        let (_, owner) = scope
                            .deadline()
                            .await
                            .ok_or_else(|| anyhow!("finite deadline missing owner"))?;
                        if owner != frame_id {
                            return Err(DeadlineExpired(owner).into());
                        }
                        error = Some(
                            json!({"error_code":"container_round_timeout","message":"Round deadline expired before its result commit"}),
                        );
                    }
                }
                traces.append(&mut node_traces);
                let mut state = session.state.lock().await;
                if error.is_some() {
                    cancel_descendants(&mut state, frame_id, scope.round_index, lifecycle).await?;
                }
                if !already_committed {
                    commit_round_terminal(
                        &mut state,
                        frame_id,
                        scope.round_index,
                        node,
                        &config,
                        output,
                        variables,
                        error,
                        lifecycle,
                    )
                    .await?;
                }
                check_final_condition(&mut state, frame_id, node, &config, lifecycle).await?;
                let closed = state.frames[&frame_id].closed;
                let global_failure = state.frames[&frame_id].global_failure.clone();
                // Terminal facts have now been committed. Retain only values required
                // by the result contract; historical node payloads live in storage.
                let frame = state.frames.get_mut(&frame_id).expect("active frame");
                if global_failure.is_none() {
                    frame.rounds.remove(&scope.round_index);
                    prune_descendants(&mut state, frame_id, scope.round_index);
                }
                drop(state);
                if closed {
                    // Futures are owned here, never detached; dropping cancels all
                    // in-flight host awaits before a break/fail returns to its parent.
                    drop(futures);
                    cancel_active_rounds(&session, frame_id, lifecycle).await?;
                    suspended = false;
                    if let Some(error) = global_failure {
                        let mut state = session.state.lock().await;
                        emit_failed_frame(&mut state, frame_id, node, &error, lifecycle).await?;
                        return Err(GlobalFailure(error).into());
                    }
                    break;
                }
            }
        }
    }
    let mut state = session.state.lock().await;
    if suspended {
        let waiting: Vec<_> = state
            .frames
            .values()
            .flat_map(|frame| {
                frame.rounds.values().filter_map(|round| {
                    let wait = round.waiting.as_ref()?;
                    let waiting_node = round.nodes.get(wait.node_id())?;
                    Some((
                        frame.identity.clone(),
                        round.round_index,
                        waiting_node.identity.execution_id,
                        wait.clone(),
                    ))
                })
            })
            .collect();
        let selected = waiting
            .iter()
            .find(|(_, _, id, _)| Some(*id) == state.waiting_execution_id)
            .or_else(|| waiting.first())
            .ok_or_else(|| anyhow!("suspended container missing leaf wait"))?;
        let (waiting_frame, waiting_round, waiting_id, wait) = selected.clone();
        if state.waiting_execution_id != Some(waiting_id) {
            state.waiting_execution_id = Some(waiting_id);
            emit(
                lifecycle,
                &state,
                ContainerTransitionKind::ScopeSuspended,
                &waiting_frame,
                None,
                None,
                Some(&json!({"round_index": waiting_round})),
            )
            .await?;
        }
        let snapshot = state.clone();
        return Ok(ContainerExecution {
            output: Value::Null,
            traces,
            waiting: Some(Box::new(FlowDebugExecutionOutcome {
                stop_reason: wait.stop_reason(),
                variable_pool: pool.clone(),
                operation_terminal: None,
                node_traces: Vec::new(),
                checkpoint_snapshot: Some(CheckpointSnapshot {
                    next_node_index: index,
                    variable_pool: pool.clone(),
                    active_node_ids: active.iter().cloned().collect(),
                    container_state: Some(snapshot),
                }),
            })),
        });
    }
    let frame = state.frames.get_mut(&frame_id).expect("active frame");
    frame.closed = true;
    let mut output = container_output(frame, config.flatten_output)?;
    if node.node_type == "iteration" {
        output
            .as_object_mut()
            .expect("container output object")
            .remove("variables");
    }
    let identity = frame.identity.clone();
    let trace = NodeExecutionTrace {
        execution_identity: Some(identity.clone()),
        node_id: node.node_id.clone(),
        node_type: node.node_type.clone(),
        node_alias: node.alias.clone(),
        input_payload: json!({}),
        output_payload: output.clone(),
        error_payload: None,
        metrics_payload: json!({"started_rounds": frame.started_rounds}),
        debug_payload: json!({}),
        provider_events: Vec::new(),
    };
    let trace = commit_frame_terminal(&mut state, frame_id, trace, lifecycle).await?;
    drop(state);
    if let Some(scope) = parent {
        leave_identity(&scope.current_identity, &identity)?;
    }
    traces.push(trace);
    Ok(ContainerExecution {
        output,
        traces,
        waiting: None,
    })
}

async fn start_round(
    plan: &CompiledPlan,
    node: &CompiledNode,
    config: &ContainerConfig,
    parent_pool: &Map<String, Value>,
    session: &Session,
    frame_id: Uuid,
    inherited: Option<(i64, Uuid)>,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<Option<u64>> {
    let mut state = session.state.lock().await;
    let frame = state.frames.get_mut(&frame_id).expect("active frame");
    if frame.closed || frame.started_rounds >= frame.frozen_limit {
        return Ok(None);
    }
    let index = frame.started_rounds;
    let mut pool = projected_parent_pool(plan, node, parent_pool);
    let item = frame
        .frozen_items
        .as_ref()
        .and_then(|items| items.get(index as usize))
        .cloned()
        .unwrap_or(Value::Null);
    pool.insert(
        node.node_id.clone(),
        json!({"index": index, "item": item, "variables": frame.variables}),
    );
    let mut condition_error = None;
    if node.node_type == "loop" {
        if let Some(condition) = node.bindings.get("break_condition") {
            match strict_condition(&condition.raw_value, &pool) {
                Ok(true) => {
                    frame.closed = true;
                    return Ok(None);
                }
                Ok(false) => {}
                Err(error) => {
                    condition_error = Some(
                        json!({"error_code": "container_condition_failed", "message": error.to_string()}),
                    )
                }
            }
        }
    }
    let own_deadline = config
        .round_timeout_ms
        .map(|timeout| {
            unix_ms()
                .checked_add(
                    i64::try_from(timeout).map_err(|_| anyhow!("round deadline overflow"))?,
                )
                .ok_or_else(|| anyhow!("round deadline overflow"))
        })
        .transpose()?
        .map(|deadline| (deadline, frame_id));
    let effective = inherited
        .into_iter()
        .chain(own_deadline)
        .min_by_key(|(deadline, _)| *deadline);
    let deadline = effective.map(|(deadline, _)| deadline);
    let deadline_owner = effective.map(|(_, owner)| owner);
    // The count, identity and deadline become visible in one durable transition.
    frame.started_rounds = frame
        .started_rounds
        .checked_add(1)
        .ok_or_else(|| anyhow!("round counter overflow"))?;
    frame.rounds.insert(
        index,
        ContainerRound {
            execution_id: Uuid::now_v7(),
            round_index: index,
            deadline_unix_ms: deadline,
            deadline_owner_execution_id: deadline_owner,
            status: ContainerRoundStatus::Running,
            next_node_index: 0,
            variable_pool: pool,
            active_node_ids: initial_active_node_ids(plan)
                .into_iter()
                .filter(|id| plan.nodes[id].container_id.as_deref() == Some(node.node_id.as_str()))
                .collect(),
            nodes: BTreeMap::new(),
            waiting: None,
            error_payload: condition_error,
        },
    );
    let identity = frame.identity.clone();
    let admission = emit(
        lifecycle,
        &state,
        ContainerTransitionKind::RoundStarted,
        &identity,
        None,
        None,
        Some(&json!({"round_index":index})),
    )
    .await;
    if let Err(error) = admission {
        let frame = state.frames.get_mut(&frame_id).expect("active frame");
        let mut rejected = frame.rounds.remove(&index).expect("staged round");
        frame.started_rounds = index;
        if error
            .downcast_ref::<DeadlineExpired>()
            .is_some_and(|expired| expired.0 == frame_id)
        {
            // No body was admitted. Commit one failed attempt atomically so a
            // continue policy cannot repeatedly renew the deadline at zero cost.
            rejected.error_payload = Some(
                json!({"error_code":"container_round_admission_timeout","message":"Round deadline expired while awaiting durable admission"}),
            );
            let failure = rejected.error_payload.clone();
            frame.started_rounds = index + 1;
            frame.rounds.insert(index, rejected);
            commit_round_terminal(
                &mut state,
                frame_id,
                index,
                node,
                config,
                Value::Null,
                Map::new(),
                failure,
                lifecycle,
            )
            .await?;
            return Ok(Some(index));
        }
        return Err(error);
    }
    Ok(Some(index))
}

enum RoundOutcome {
    Waiting(Vec<NodeExecutionTrace>),
    Terminal {
        already_committed: bool,
        output: Value,
        variables: Map<String, Value>,
        error: Option<Value>,
        node_traces: Vec<NodeExecutionTrace>,
    },
}

async fn execute_round<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    config: &ContainerConfig,
    scope: Scope,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<(Scope, RoundOutcome)>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let (index, pool, active, waiting, condition_error, already_committed) = {
        let state = scope.session.state.lock().await;
        let round = &state.frames[&scope.frame_id].rounds[&scope.round_index];
        (
            round.next_node_index,
            round.variable_pool.clone(),
            round.active_node_ids.iter().cloned().collect(),
            round.waiting.clone(),
            round.error_payload.clone(),
            round.status == ContainerRoundStatus::Failed,
        )
    };
    if let Some(error) = condition_error {
        return Ok((
            scope,
            RoundOutcome::Terminal {
                already_committed,
                output: Value::Null,
                variables: Map::new(),
                error: Some(error),
                node_traces: Vec::new(),
            },
        ));
    }
    let deadline = scope.deadline().await;
    let result = if scope.expired() {
        let (_, owner) = deadline.ok_or_else(|| anyhow!("finite deadline missing owner"))?;
        Err(anyhow::Error::new(DeadlineExpired(owner)))
    } else if waiting.is_some() {
        return Ok((scope, RoundOutcome::Waiting(Vec::new())));
    } else {
        let scoped_lifecycle = ScopedLifecycle {
            scope: scope.clone(),
            owner: lifecycle,
        };
        let execution = CURRENT_EXECUTION.scope(
            scope.current_identity.clone(),
            Box::pin(super::super::scheduler::execute_from_scoped(
                plan,
                index,
                pool,
                Some(active),
                runtime,
                invoker,
                &scoped_lifecycle,
                Some(&scope),
                None,
            )),
        );
        if let Some(expires_at) = scope.expires_at {
            let (_, owner) = deadline.ok_or_else(|| anyhow!("finite deadline missing owner"))?;
            tokio::time::timeout_at(expires_at, execution)
                .await
                .unwrap_or_else(|_| Err(DeadlineExpired(owner).into()))
        } else {
            execution.await
        }
    };
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            if error.is::<communication::CommunicationCancelled>() {
                return Err(GlobalFailure(json!({"error_code":"container_execution_cancelled","message":error.to_string()})).into());
            }
            if error.is::<CommitFailure>()
                || error.is::<GlobalFailure>()
                || error
                    .downcast_ref::<DeadlineExpired>()
                    .is_some_and(|deadline| deadline.0 != scope.frame_id)
            {
                return Err(error);
            }
            return Ok((
                scope,
                RoundOutcome::Terminal {
                    already_committed: false,
                    output: Value::Null,
                    variables: Map::new(),
                    error: Some(
                        json!({"error_code": if error.is::<DeadlineExpired>() {"container_round_timeout"} else {"container_round_failed"}, "message": error.to_string()}),
                    ),
                    node_traces: Vec::new(),
                },
            ));
        }
    };
    let mut traces = outcome.node_traces;
    {
        let state = scope.session.state.lock().await;
        let nodes = &state.frames[&scope.frame_id].rounds[&scope.round_index].nodes;
        for trace in &mut traces {
            if trace.execution_identity.is_none() {
                trace.execution_identity =
                    nodes.get(&trace.node_id).map(|node| node.identity.clone());
            }
        }
    }
    match outcome.stop_reason {
        ExecutionStopReason::WaitingHuman(_) | ExecutionStopReason::WaitingCallback(_) => {
            let wait =
                ContainerWait::from_stop_reason(outcome.stop_reason).expect("waiting stop reason");
            let checkpoint = outcome
                .checkpoint_snapshot
                .ok_or_else(|| anyhow!("waiting round missing checkpoint"))?;
            let mut state = scope.session.state.lock().await;
            let frame = state.frames.get_mut(&scope.frame_id).expect("active frame");
            let identity = frame.identity.clone();
            let waiting_trace = traces.iter().find(|trace| trace.node_id == wait.node_id());
            let round = frame
                .rounds
                .get_mut(&scope.round_index)
                .expect("active round");
            round.status = ContainerRoundStatus::Waiting;
            round.waiting = Some(wait);
            round.next_node_index = checkpoint.next_node_index;
            round.variable_pool = checkpoint.variable_pool;
            round.active_node_ids = checkpoint.active_node_ids;
            if let Some(waiting_id) = round
                .waiting
                .as_ref()
                .and_then(|wait| round.nodes.get(wait.node_id()))
                .map(|node| node.identity.execution_id)
            {
                state.waiting_execution_id = Some(waiting_id);
            }
            emit(
                lifecycle,
                &state,
                ContainerTransitionKind::ScopeSuspended,
                &identity,
                None,
                waiting_trace,
                Some(&json!({"round_index": scope.round_index})),
            )
            .await?;
            drop(state);
            Ok((scope, RoundOutcome::Waiting(traces)))
        }
        ExecutionStopReason::Failed(failure) => Ok((
            scope,
            RoundOutcome::Terminal {
                already_committed: false,
                output: Value::Null,
                variables: Map::new(),
                error: Some(failure.error_payload),
                node_traces: traces,
            },
        )),
        ExecutionStopReason::Completed | ExecutionStopReason::Incomplete(_) => {
            let values = (|| -> Result<_> {
                let output = node
                    .bindings
                    .get("output")
                    .map(|binding| resolve_binding(binding, &outcome.variable_pool))
                    .transpose()?
                    .unwrap_or(Value::Null);
                if config.flatten_output && !output.is_array() {
                    bail!("flatten_output requires array round outputs");
                }
                let variables = if node.bindings.contains_key("variable_updates") {
                    resolve_variables(node, "variable_updates", &outcome.variable_pool)?
                } else {
                    outcome
                        .variable_pool
                        .get(&node.node_id)
                        .and_then(|value| value.get("variables"))
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default()
                };
                validate_variables(config, &variables)?;
                Ok((output, variables))
            })();
            let (output, variables, error) = match values {
                Ok((output, variables)) => (output, variables, None),
                Err(error) => (
                    Value::Null,
                    Map::new(),
                    Some(
                        json!({"error_code": "container_output_failed", "message": error.to_string()}),
                    ),
                ),
            };
            Ok((
                scope,
                RoundOutcome::Terminal {
                    already_committed: false,
                    output,
                    variables,
                    error,
                    node_traces: traces,
                },
            ))
        }
    }
}

fn projected_parent_pool(
    plan: &CompiledPlan,
    container: &CompiledNode,
    parent: &Map<String, Value>,
) -> Map<String, Value> {
    let mut paths = BTreeSet::new();
    // Borrow the parent scope and copy only explicitly consumed selector leaves.
    // Entry bindings (frozen items/initial variables) are not imported every round.
    // A reference to source.small never copies unrelated source.large values.
    for node in plan.nodes.values() {
        let mut owner = node.container_id.as_deref();
        let mut descendant = node.node_id == container.node_id;
        let mut visited = BTreeSet::new();
        while let Some(id) = owner {
            if !visited.insert(id) {
                break;
            }
            if id == container.node_id {
                descendant = true;
                break;
            }
            owner = plan
                .nodes
                .get(id)
                .and_then(|node| node.container_id.as_deref());
        }
        if descendant {
            for (key, binding) in &node.bindings {
                if node.node_id == container.node_id
                    && matches!(key.as_str(), "items" | "variables")
                {
                    continue;
                }
                paths.extend(binding.selector_paths.iter().cloned());
            }
        }
    }
    // These are runtime inputs, not implicit writable imports. The rest of sys
    // is available only when selected explicitly by an authored binding.
    for key in ["model_parameters", "native_inference_recovery"] {
        paths.insert(vec!["sys".to_owned(), key.to_owned()]);
    }
    let mut local = Map::new();
    for path in paths {
        if let Ok(value) = crate::binding_runtime::lookup_selector_value(parent, &path) {
            insert_import(&mut local, &path, value);
        }
    }
    local
}

fn insert_import(pool: &mut Map<String, Value>, path: &[String], value: Value) {
    let Some((head, tail)) = path.split_first() else {
        return;
    };
    if tail.is_empty() {
        pool.insert(head.clone(), value);
        return;
    }
    let current = pool
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(object) = current.as_object_mut() {
        insert_import(object, tail, value);
    }
}

fn resolve_variables(
    node: &CompiledNode,
    key: &str,
    pool: &Map<String, Value>,
) -> Result<Map<String, Value>> {
    node.bindings
        .get(key)
        .map(|binding| {
            resolve_binding(binding, pool).and_then(|value| {
                value
                    .as_object()
                    .cloned()
                    .ok_or_else(|| anyhow!("{key} must produce an object"))
            })
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn validate_variables(config: &ContainerConfig, variables: &Map<String, Value>) -> Result<()> {
    if variables.len() != config.loop_variables.len() {
        bail!("loop variables do not match declarations");
    }
    for definition in &config.loop_variables {
        let value = variables
            .get(&definition.key)
            .ok_or_else(|| anyhow!("loop variable missing: {}", definition.key))?;
        let valid = match definition.value_type.as_str() {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            _ => false,
        };
        if !valid {
            bail!("loop variable type mismatch: {}", definition.key);
        }
    }
    Ok(())
}

fn container_output(frame: &ContainerFrame, flatten: bool) -> Result<Value> {
    let mut results = Vec::new();
    for value in frame.results.values() {
        if flatten {
            results.extend(
                value
                    .as_array()
                    .ok_or_else(|| anyhow!("flatten_output requires array round outputs"))?
                    .iter()
                    .cloned(),
            );
        } else {
            results.push(value.clone());
        }
    }
    Ok(json!({"result": results, "variables": frame.variables}))
}

fn prune_descendants(state: &mut ContainerCheckpoint, frame_id: Uuid, round_index: u64) {
    state.frames.retain(|_, frame| {
        !frame
            .identity
            .round_path
            .iter()
            .any(|path| path.container_execution_id == frame_id && path.round_index == round_index)
    });
}

async fn cancel_active_rounds(
    session: &Session,
    frame_id: Uuid,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    let mut state = session.state.lock().await;
    let frames: Vec<_> = state
        .frames
        .values()
        .filter(|frame| {
            frame.identity.execution_id == frame_id
                || frame
                    .identity
                    .round_path
                    .iter()
                    .any(|path| path.container_execution_id == frame_id)
        })
        .map(|frame| frame.identity.execution_id)
        .collect();
    for id in frames {
        let indices: Vec<_> = state.frames[&id]
            .rounds
            .iter()
            .filter(|(_, round)| {
                matches!(
                    round.status,
                    ContainerRoundStatus::Running | ContainerRoundStatus::Waiting
                )
            })
            .map(|(index, _)| *index)
            .collect();
        for index in indices {
            let frame = state.frames.get_mut(&id).expect("active frame");
            let identity = frame.identity.clone();
            let round = frame.rounds.get_mut(&index).expect("active round");
            round.status = ContainerRoundStatus::Cancelled;
            round.waiting = None;
            emit(
                lifecycle,
                &state,
                ContainerTransitionKind::RoundCommitted,
                &identity,
                None,
                None,
                Some(&json!({"round_index": index})),
            )
            .await?;
            state
                .frames
                .get_mut(&id)
                .expect("active frame")
                .rounds
                .remove(&index);
        }
    }
    Ok(())
}

async fn cancel_descendants(
    state: &mut ContainerCheckpoint,
    owner: Uuid,
    round_index: u64,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    let frames: Vec<_> =
        state
            .frames
            .values()
            .filter(|frame| {
                frame.identity.round_path.iter().any(|path| {
                    path.container_execution_id == owner && path.round_index == round_index
                })
            })
            .map(|frame| frame.identity.execution_id)
            .collect();
    for frame_id in frames {
        let indices: Vec<_> = state.frames[&frame_id]
            .rounds
            .iter()
            .filter(|(_, round)| {
                matches!(
                    round.status,
                    ContainerRoundStatus::Running | ContainerRoundStatus::Waiting
                )
            })
            .map(|(index, _)| *index)
            .collect();
        for index in indices {
            let frame = state.frames.get_mut(&frame_id).expect("descendant frame");
            let identity = frame.identity.clone();
            let round = frame.rounds.get_mut(&index).expect("descendant round");
            round.status = ContainerRoundStatus::Cancelled;
            round.waiting = None;
            emit(
                lifecycle,
                state,
                ContainerTransitionKind::RoundCommitted,
                &identity,
                None,
                None,
                Some(&json!({"round_index": index})),
            )
            .await?;
        }
    }
    Ok(())
}

async fn propagate_global_failure(
    session: &Session,
    frame_id: Uuid,
    node: &CompiledNode,
    error: &Value,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    let mut state = session.state.lock().await;
    let failing_rounds: BTreeSet<_> = state
        .frames
        .values()
        .filter(|frame| frame.global_failure.is_some())
        .flat_map(|frame| frame.identity.round_path.iter())
        .filter(|path| path.container_execution_id == frame_id)
        .map(|path| path.round_index)
        .collect();
    let frame = state.frames.get_mut(&frame_id).expect("parent frame");
    frame.closed = true;
    frame.global_failure = Some(error.clone());
    let identity = frame.identity.clone();
    let indices: Vec<_> = frame
        .rounds
        .iter()
        .filter(|(_, round)| {
            matches!(
                round.status,
                ContainerRoundStatus::Running | ContainerRoundStatus::Waiting
            )
        })
        .map(|(index, _)| *index)
        .collect();
    for index in indices {
        let round = state
            .frames
            .get_mut(&frame_id)
            .expect("parent frame")
            .rounds
            .get_mut(&index)
            .expect("parent round");
        // Preserve the failed continuation for explicit native recovery; siblings
        // are cancelled and can never be silently replayed by a parent policy.
        round.status = if failing_rounds.contains(&index) {
            ContainerRoundStatus::Failed
        } else {
            ContainerRoundStatus::Cancelled
        };
        round.error_payload = Some(error.clone());
        round.waiting = None;
        emit(
            lifecycle,
            &state,
            ContainerTransitionKind::RoundCommitted,
            &identity,
            None,
            None,
            Some(&json!({"round_index": index})),
        )
        .await?;
    }
    let trace = NodeExecutionTrace {
        execution_identity: Some(identity.clone()),
        node_id: node.node_id.clone(),
        node_type: node.node_type.clone(),
        node_alias: node.alias.clone(),
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: Some(error.clone()),
        metrics_payload: json!({}),
        debug_payload: json!({}),
        provider_events: Vec::new(),
    };
    commit_frame_terminal(&mut state, frame_id, trace, lifecycle)
        .await
        .map(|_| ())
}

async fn check_final_condition(
    state: &mut ContainerCheckpoint,
    frame_id: Uuid,
    node: &CompiledNode,
    config: &ContainerConfig,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    if node.node_type != "loop" {
        return Ok(());
    }
    let Some(condition) = node.bindings.get("break_condition") else {
        return Ok(());
    };
    let frame = &state.frames[&frame_id];
    if frame.final_condition_checked
        || frame.started_rounds != frame.frozen_limit
        || frame.frozen_limit == 0
    {
        return Ok(());
    }
    let final_index = frame.frozen_limit - 1;
    let Some(round) = frame
        .rounds
        .get(&final_index)
        .filter(|round| round.status == ContainerRoundStatus::Succeeded)
    else {
        return Ok(());
    };
    let failure = strict_condition(&condition.raw_value, &round.variable_pool)
        .err()
        .map(
            |error| json!({"error_code":"container_condition_failed","message":error.to_string()}),
        );
    let frame = state
        .frames
        .get_mut(&frame_id)
        .expect("final condition frame");
    frame.final_condition_checked = true;
    frame.closed = true;
    let identity = frame.identity.clone();
    let (kind, detail) = if let Some(error_payload) = failure {
        // Body output and variables were committed by RoundCommitted already.
        // This is a distinct control failure and consumes no additional round.
        frame.control_failures.push(ContainerControlFailure {
            after_round_index: final_index,
            error_payload: error_payload.clone(),
        });
        if config.error_policy == ContainerErrorPolicy::Fail {
            frame.global_failure = Some(error_payload.clone());
        }
        (
            ContainerTransitionKind::ControlFailed,
            json!({"after_round_index":final_index,"error_payload":error_payload}),
        )
    } else {
        (
            ContainerTransitionKind::ScopeAdvanced,
            json!({"round_index":final_index,"final_condition_checked":true}),
        )
    };
    emit(lifecycle, state, kind, &identity, None, None, Some(&detail)).await
}

async fn emit_failed_frame(
    state: &mut ContainerCheckpoint,
    frame_id: Uuid,
    node: &CompiledNode,
    error: &Value,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<()> {
    let identity = state.frames[&frame_id].identity.clone();
    let trace = NodeExecutionTrace {
        execution_identity: Some(identity.clone()),
        node_id: node.node_id.clone(),
        node_type: node.node_type.clone(),
        node_alias: node.alias.clone(),
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: Some(error.clone()),
        metrics_payload: json!({}),
        debug_payload: json!({}),
        provider_events: Vec::new(),
    };
    commit_frame_terminal(state, frame_id, trace, lifecycle)
        .await
        .map(|_| ())
}
