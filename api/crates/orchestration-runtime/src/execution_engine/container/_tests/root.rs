use super::*;

fn plan_with_root_tail() -> CompiledPlan {
    let mut plan = simple_loop(1);
    for id in ["after_one", "after_two"] {
        let mut node = code(id, "unused");
        node.container_id = None;
        plan.topological_order.push(id.to_owned());
        plan.nodes.insert(id.to_owned(), node);
    }
    plan
}

#[tokio::test]
async fn completed_root_successor_is_not_replayed_after_a_crash() {
    let plan = plan_with_root_tail();
    let probe = Probe::default();
    let ledger = Ledger::default();
    *ledger.fail_after.lock().unwrap() = Some(ContainerTransitionKind::NodeCompleted);
    *ledger.fail_node.lock().unwrap() = Some("after_one".to_owned());
    assert!(run(&plan, json!({}), &probe, &ledger).await.is_err());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    let checkpoint = snapshot(&ledger);
    assert!(
        checkpoint.container_state.as_ref().unwrap().root_nodes["after_one"]
            .terminal_trace
            .is_some()
    );
    let outcome = recover_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 3);
    assert!(outcome
        .node_traces
        .iter()
        .filter(|trace| trace.node_id.starts_with("after_"))
        .all(|trace| trace
            .execution_identity
            .as_ref()
            .is_some_and(|identity| identity.round_path.is_empty())));
}

#[tokio::test]
async fn unconfirmed_root_successor_is_failed_without_replay() {
    let plan = plan_with_root_tail();
    let probe = Probe::default();
    let ledger = Ledger::default();
    *ledger.fail_after.lock().unwrap() = Some(ContainerTransitionKind::NodeStarted);
    *ledger.fail_node.lock().unwrap() = Some("after_one".to_owned());
    assert!(run(&plan, json!({}), &probe, &ledger).await.is_err());
    let outcome = recover_container_with_runtime_context_and_lifecycle(
        &plan,
        &snapshot(&ledger),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn root_wait_resumes_with_the_same_identity_after_container_completion() {
    let mut plan = simple_loop(1);
    let mut wait = node("root_human", "human_input", None);
    wait.outputs = vec![output("value", "string")];
    plan.topological_order.push(wait.node_id.clone());
    plan.nodes.insert(wait.node_id.clone(), wait);
    let probe = Probe::default();
    let ledger = Ledger::default();
    let outcome = run(&plan, json!({}), &probe, &ledger).await.unwrap();
    let checkpoint = outcome.checkpoint_snapshot.unwrap();
    let state = checkpoint.container_state.as_ref().unwrap();
    assert!(state.root_waiting.is_some());
    let execution_id = state.waiting_execution_id.unwrap();
    assert_eq!(
        state.root_nodes["root_human"].identity.execution_id,
        execution_id
    );
    let resumed = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        execution_id,
        &json!({"value":"confirmed"}),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(
        resumed.variable_pool["root_human"]["value"],
        json!("confirmed")
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn nested_new_ordinary_flow_does_not_inherit_parent_execution_identity() {
    let fake = ScopedExecutionIdentity {
        execution_id: uuid::Uuid::now_v7(),
        node_id: "parent".into(),
        parent_execution_id: None,
        round_path: Vec::new(),
    };
    let parent_slot = Arc::new(Mutex::new(Some(fake.clone())));
    let mut leaf = code("ordinary", "unused");
    leaf.container_id = None;
    let plan = plan(vec![leaf]);
    let probe = Probe::default();
    CURRENT_EXECUTION
        .scope(parent_slot, async {
            assert_eq!(scoped_execution_identity(), Some(fake.clone()));
            let outcome = run(&plan, json!({}), &probe, &Ledger::default())
                .await
                .unwrap();
            assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
            assert_eq!(scoped_execution_identity(), Some(fake));
        })
        .await;
    assert!(probe.identities.lock().unwrap().is_empty());
}

#[tokio::test]
async fn completed_variable_write_and_terminal_fact_restore_atomically() {
    let mut plan = simple_loop(1);
    let mut assign = node("assign", "variable_assigner", None);
    assign.bindings.insert("operations".into(),CompiledBinding{i18n_text_ref:None,kind:"state_write".into(),raw_value:json!([{"path":["conversation","value"],"operator":"set","value":{"kind":"constant","value":42}}]),selector_paths:vec![]});
    plan.topological_order.push(assign.node_id.clone());
    plan.nodes.insert(assign.node_id.clone(), assign);
    let probe = Probe::default();
    let ledger = Ledger::default();
    *ledger.fail_after.lock().unwrap() = Some(ContainerTransitionKind::NodeCompleted);
    *ledger.fail_node.lock().unwrap() = Some("assign".into());
    assert!(run(&plan, json!({}), &probe, &ledger).await.is_err());
    let checkpoint = snapshot(&ledger);
    assert_eq!(
        checkpoint
            .container_state
            .as_ref()
            .unwrap()
            .root_variable_pool["conversation"]["value"],
        json!(42)
    );
    let outcome = recover_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(outcome.variable_pool["conversation"]["value"], json!(42));
}
