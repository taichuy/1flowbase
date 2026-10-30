use super::*;

struct RejectAdmission {
    ledger: Ledger,
    kind: ContainerTransitionKind,
    target: &'static str,
    index: u64,
    rejected: Mutex<bool>,
    failed_snapshots: Mutex<Vec<ContainerCheckpoint>>,
}
#[async_trait]
impl ExecutionLifecycle for RejectAdmission {
    async fn begin_node(&self, _: &CompiledNode, _: &Value) -> Result<()> {
        Ok(())
    }
    async fn commit_container_transition(&self, event: &ContainerTransition<'_>) -> Result<()> {
        let index = event
            .detail
            .and_then(|detail| detail.get("round_index"))
            .and_then(Value::as_u64);
        if event.identity.node_id == self.target
            && event.kind == self.kind
            && index == Some(self.index)
        {
            let round = &event.checkpoint.frames[&event.identity.execution_id].rounds[&self.index];
            if event.kind == ContainerTransitionKind::RoundStarted
                || round.status == ContainerRoundStatus::Succeeded
            {
                let mut rejected = self.rejected.lock().unwrap();
                if !*rejected {
                    *rejected = true;
                    return Err(deadline_expired(round.deadline_owner_execution_id.unwrap()));
                }
            }
        }
        if event.kind == ContainerTransitionKind::RoundCommitted {
            let index = index.expect("round transition index");
            if event.checkpoint.frames[&event.identity.execution_id].rounds[&index].status
                == ContainerRoundStatus::Failed
            {
                self.failed_snapshots
                    .lock()
                    .unwrap()
                    .push(event.checkpoint.clone());
            }
        }
        self.ledger.commit_container_transition(event).await
    }
}
fn reject(kind: ContainerTransitionKind, target: &'static str, index: u64) -> RejectAdmission {
    RejectAdmission {
        ledger: Ledger::default(),
        kind,
        target,
        index,
        rejected: Mutex::new(false),
        failed_snapshots: Mutex::new(Vec::new()),
    }
}
async fn run_rejected(
    plan: &CompiledPlan,
    probe: &Probe,
    owner: &RejectAdmission,
) -> FlowDebugExecutionOutcome {
    start_flow_debug_run_with_runtime_context_and_lifecycle(
        plan,
        &json!({}),
        ExecutionRuntimeContext::default(),
        probe,
        owner,
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn rejected_own_round_admission_uses_one_budget_without_dispatching_body() {
    for policy in ["continue", "break", "fail"] {
        let mut plan = simple_loop(3);
        plan.nodes.get_mut("loop").unwrap().config["error_policy"] = json!(policy);
        let probe = Probe::default();
        let owner = reject(ContainerTransitionKind::RoundStarted, "loop", 0);
        let outcome = run_rejected(&plan, &probe, &owner).await;
        assert!(*owner.rejected.lock().unwrap());
        assert_eq!(
            matches!(outcome.stop_reason, ExecutionStopReason::Failed(_)),
            policy == "fail"
        );
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            if policy == "continue" { 2 } else { 0 }
        );
        let checkpoint = owner.ledger.latest.lock().unwrap();
        let frame = checkpoint.as_ref().unwrap().frames.values().next().unwrap();
        assert_eq!(
            frame.started_rounds,
            if policy == "continue" { 3 } else { 1 }
        );
        if policy != "fail" {
            assert_eq!(
                outcome.variable_pool["loop"]["result"],
                if policy == "continue" {
                    json!([1, 1])
                } else {
                    json!([])
                }
            );
        }
        let failures = owner.failed_snapshots.lock().unwrap();
        let failed = &failures[0].frames[&frame.identity.execution_id].rounds[&0];
        assert_eq!(failed.status, ContainerRoundStatus::Failed);
        assert!(failed.nodes.is_empty());
    }
}
#[tokio::test]
async fn rejected_success_commit_rolls_back_result_and_loop_variables_before_failure_closure() {
    let mut outer = container("loop", "loop", None, 3);
    outer.config["error_policy"] = json!("continue");
    outer.config["loop_variables"] = json!([{"key":"value","value_type":"number"}]);
    outer.bindings.insert(
        "variables".into(),
        CompiledBinding {
            kind: "named_bindings".into(),
            raw_value: json!([{"name":"value","value":{"kind":"constant","value":0}}]),
            selector_paths: Vec::new(),
            i18n_text_ref: None,
        },
    );
    outer.bindings.insert("variable_updates".into(),CompiledBinding{kind:"named_bindings".into(),raw_value:json!([{"name":"value","value":{"kind":"selector","selector":["leaf","result","value"]}}]),selector_paths:vec![vec!["leaf".into(),"result".into(),"value".into()]],i18n_text_ref:None});
    outer
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "loop");
    leaf.bindings
        .insert("item".into(), selector(&["loop", "index"]));
    let plan = plan(vec![outer, leaf]);
    let probe = Probe::default();
    let owner = reject(ContainerTransitionKind::RoundCommitted, "loop", 1);
    let outcome = run_rejected(&plan, &probe, &owner).await;
    assert_eq!(outcome.variable_pool["loop"]["result"], json!([0, 2]));
    assert_eq!(
        outcome.variable_pool["loop"]["variables"],
        json!({"value":2})
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 3);
    let failures = owner.failed_snapshots.lock().unwrap();
    let frame = failures[0].frames.values().next().unwrap();
    assert!(!frame.results.contains_key(&1));
    assert_eq!(frame.variables["value"], json!(0));
    assert_eq!(frame.rounds[&1].status, ContainerRoundStatus::Failed);
}
#[tokio::test]
async fn ancestor_deadline_rejection_rolls_back_child_proposal_and_reaches_ancestor_policy() {
    for kind in [
        ContainerTransitionKind::RoundStarted,
        ContainerTransitionKind::RoundCommitted,
    ] {
        let mut outer = container("outer", "loop", None, 1);
        outer.config["round_timeout_ms"] = json!(1000);
        outer.config["error_policy"] = json!("continue");
        outer
            .bindings
            .insert("output".into(), selector(&["inner", "result"]));
        let mut inner = container("inner", "loop", Some("outer"), 2);
        inner.config["round_timeout_ms"] = json!(60000);
        inner.config["error_policy"] = json!("continue");
        inner
            .bindings
            .insert("output".into(), selector(&["leaf", "result", "value"]));
        let plan = plan(vec![outer, inner, code("leaf", "inner")]);
        let probe = Probe::default();
        let owner = reject(kind, "inner", 0);
        let outcome = run_rejected(&plan, &probe, &owner).await;
        assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
        assert_eq!(outcome.variable_pool["outer"]["result"], json!([]));
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            if kind == ContainerTransitionKind::RoundStarted {
                0
            } else {
                1
            }
        );
        let failures = owner.failed_snapshots.lock().unwrap();
        let state = failures.last().unwrap();
        let child = state
            .frames
            .values()
            .find(|frame| frame.identity.node_id == "inner")
            .unwrap();
        assert!(child.results.is_empty());
        if kind == ContainerTransitionKind::RoundStarted {
            assert_eq!(child.started_rounds, 0);
            assert!(child.rounds.is_empty());
        }
    }
}
