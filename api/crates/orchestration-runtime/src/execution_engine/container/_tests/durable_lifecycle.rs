use super::*;
use uuid::Uuid;

/// Model the actual repository's one-start/one-terminal constraint, not an
/// append-only event recorder that would accept duplicate durable receipts.
#[derive(Default)]
struct StrictLifecycle {
    ledger: Ledger,
    terminal: Mutex<BTreeMap<Uuid, bool>>,
}
#[async_trait]
impl ExecutionLifecycle for StrictLifecycle {
    async fn begin_node(&self, _: &CompiledNode, _: &Value) -> Result<()> {
        bail!("container fixture cannot bypass scoped lifecycle")
    }
    async fn commit_container_transition(&self, event: &ContainerTransition<'_>) -> Result<()> {
        {
            let mut terminal = self.terminal.lock().unwrap();
            match event.kind {
                ContainerTransitionKind::NodeStarted | ContainerTransitionKind::FrameEntered => {
                    if terminal.contains_key(&event.identity.execution_id) {
                        bail!("container_node_already_started");
                    }
                    if event.kind == ContainerTransitionKind::FrameEntered {
                        assert!(event
                            .checkpoint
                            .frames
                            .contains_key(&event.identity.execution_id));
                        if let Some(path) = event.identity.round_path.last() {
                            let parent = &event.checkpoint.frames[&path.container_execution_id]
                                .rounds[&path.round_index]
                                .nodes[&event.identity.node_id];
                            assert_eq!(
                                parent.identity, *event.identity,
                                "parent identity must be in the same start checkpoint"
                            );
                            assert!(parent.terminal_trace.is_none());
                        }
                    }
                    terminal.insert(event.identity.execution_id, false);
                }
                ContainerTransitionKind::NodeCompleted | ContainerTransitionKind::FrameClosed => {
                    let done = terminal
                        .get_mut(&event.identity.execution_id)
                        .ok_or_else(|| anyhow!("terminal without start"))?;
                    if *done {
                        bail!("container_node_terminal_or_missing");
                    }
                    if event.kind == ContainerTransitionKind::FrameClosed {
                        let trace = event
                            .trace
                            .ok_or_else(|| anyhow!("frame terminal trace missing"))?;
                        assert_eq!(
                            event.checkpoint.frames[&event.identity.execution_id]
                                .terminal_trace
                                .as_ref(),
                            Some(trace)
                        );
                        if let Some(path) = event.identity.round_path.last() {
                            let parent = &event.checkpoint.frames[&path.container_execution_id]
                                .rounds[&path.round_index];
                            assert_eq!(
                                parent.nodes[&event.identity.node_id]
                                    .terminal_trace
                                    .as_ref(),
                                Some(trace),
                                "parent terminal cache must share the terminal commit"
                            );
                            if trace.error_payload.is_none() {
                                assert_eq!(
                                    parent.variable_pool[&event.identity.node_id],
                                    trace.output_payload
                                );
                            }
                        }
                    }
                    *done = true;
                }
                ContainerTransitionKind::RoundCommitted => {
                    let index = event.detail.unwrap()["round_index"].as_u64().unwrap();
                    let round =
                        &event.checkpoint.frames[&event.identity.execution_id].rounds[&index];
                    if matches!(
                        round.status,
                        ContainerRoundStatus::Failed | ContainerRoundStatus::Cancelled
                    ) {
                        // The repository closes admitted unfinished children as
                        // part of the same failed/cancelled round transaction.
                        for node in round
                            .nodes
                            .values()
                            .filter(|node| node.terminal_trace.is_none())
                        {
                            if let Some(done) = terminal.get_mut(&node.identity.execution_id) {
                                *done = true;
                            }
                        }
                    }
                }
                ContainerTransitionKind::Resumed
                    if terminal.get(&event.identity.execution_id) != Some(&false) =>
                {
                    bail!("cannot resume an already terminal receipt");
                }
                _ => {}
            }
        }
        self.ledger.commit_container_transition(event).await
    }
}
fn nested_plan() -> CompiledPlan {
    let mut outer = container("outer", "loop", None, 2);
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "iteration", Some("outer"), 1);
    inner
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    plan(vec![outer, inner, code("leaf", "inner")])
}
async fn start(
    plan: &CompiledPlan,
    probe: &Probe,
    lifecycle: &StrictLifecycle,
) -> Result<FlowDebugExecutionOutcome> {
    start_flow_debug_run_with_runtime_context_and_lifecycle(
        plan,
        &json!({"source":{"items":[1,2,3]}}),
        ExecutionRuntimeContext::default(),
        probe,
        lifecycle,
    )
    .await
}
#[tokio::test]
async fn nested_containers_have_exactly_one_durable_start_and_terminal() {
    let plan = nested_plan();
    let probe = Probe::default();
    let lifecycle = StrictLifecycle::default();
    let result = start(&plan, &probe, &lifecycle).await.unwrap();
    assert_eq!(result.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        result.variable_pool["outer"]["result"],
        json!([[1, 1, 1], [1, 1, 1]])
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 6);
    let terminal = lifecycle.terminal.lock().unwrap();
    assert_eq!(terminal.len(), 9);
    assert!(terminal.values().all(|done| *done));
}
#[tokio::test]
async fn frame_start_close_and_parent_cache_recover_without_duplicate_receipts() {
    for (point, node) in [
        (ContainerTransitionKind::FrameEntered, "inner"),
        (ContainerTransitionKind::FrameClosed, "inner"),
        (ContainerTransitionKind::FrameClosed, "outer"),
    ] {
        let plan = nested_plan();
        let probe = Probe::default();
        let lifecycle = StrictLifecycle::default();
        *lifecycle.ledger.fail_after.lock().unwrap() = Some(point);
        *lifecycle.ledger.fail_node.lock().unwrap() = Some(node.to_owned());
        assert!(start(&plan, &probe, &lifecycle).await.is_err());
        let checkpoint = snapshot(&lifecycle.ledger);
        let result = recover_container_with_runtime_context_and_lifecycle(
            &plan,
            &checkpoint,
            ExecutionRuntimeContext::default(),
            &probe,
            &lifecycle,
        )
        .await
        .unwrap();
        assert_eq!(
            result.stop_reason,
            ExecutionStopReason::Completed,
            "{point:?}:{node}"
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 6, "{point:?}:{node}");
        assert_eq!(
            result.variable_pool["outer"]["result"],
            json!([[1, 1, 1], [1, 1, 1]])
        );
        let terminal = lifecycle.terminal.lock().unwrap();
        assert_eq!(terminal.len(), 9);
        assert!(terminal.values().all(|done| *done));
    }
}

#[tokio::test]
async fn committed_parallel_break_recovers_by_closing_pending_rounds_without_starting_them() {
    let mut outer = container("outer", "loop", None, 1);
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "iteration", Some("outer"), 1);
    inner.config["parallelism"] = json!(2);
    inner.config["error_policy"] = json!("break");
    inner
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "inner");
    leaf.bindings
        .insert("item".into(), selector(&["inner", "item"]));
    let plan = plan(vec![outer, inner, leaf]);
    let probe = Probe::default();
    let lifecycle = StrictLifecycle::default();
    *lifecycle.ledger.fail_after.lock().unwrap() = Some(ContainerTransitionKind::RoundCommitted);
    *lifecycle.ledger.fail_node.lock().unwrap() = Some("inner".into());
    assert!(start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &json!({"source":{"items":[{"fail":true},2,3]}}),
        ExecutionRuntimeContext::default(),
        &probe,
        &lifecycle
    )
    .await
    .is_err());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    let outcome = recover_container_with_runtime_context_and_lifecycle(
        &plan,
        &snapshot(&lifecycle.ledger),
        ExecutionRuntimeContext::default(),
        &probe,
        &lifecycle,
    )
    .await
    .unwrap();
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(outcome.variable_pool["outer"]["result"], json!([[]]));
    assert_eq!(
        probe.calls.load(Ordering::SeqCst),
        1,
        "committed break forbids replay of previously queued rounds"
    );
    assert!(lifecycle
        .terminal
        .lock()
        .unwrap()
        .values()
        .all(|done| *done));
}
