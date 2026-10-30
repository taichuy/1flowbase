use super::*;

#[tokio::test]
async fn unlimited_wait_survives_recovery_and_callback_without_inventing_deadline() {
    let mut outer = container("loop", "loop", None, 1);
    outer
        .config
        .as_object_mut()
        .unwrap()
        .remove("round_timeout_ms");
    outer
        .bindings
        .insert("output".into(), selector(&["human", "value"]));
    let mut human = node("human", "human_input", Some("loop"));
    human.outputs = vec![output("value", "string")];
    let plan = plan(vec![outer, human]);
    let ledger = Ledger::default();
    let probe = Probe::default();
    let first = run(&plan, json!({}), &probe, &ledger).await.unwrap();
    let checkpoint = first.checkpoint_snapshot.unwrap();
    let state = checkpoint.container_state.as_ref().unwrap();
    let round = &state.frames.values().next().unwrap().rounds[&0];
    assert_eq!(round.deadline_unix_ms, None);
    assert_eq!(round.deadline_owner_execution_id, None);
    let scope = Scope::new(
        Session::new(state.clone(), true).unwrap(),
        state.frames.values().next().unwrap().identity.execution_id,
        0,
        None,
    )
    .await
    .unwrap();
    assert!(!scope.expired());
    assert_eq!(scope.remaining_ms(), None);
    let recovered = recover_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    let recovered = recovered.checkpoint_snapshot.unwrap();
    assert_eq!(
        recovered.container_state.as_ref().unwrap().frames,
        state.frames
    );
    let id = waiting_execution_id(&recovered, "human").unwrap();
    let done = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &recovered,
        id,
        &json!({"value":"accepted"}),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(done.variable_pool["loop"]["result"], json!(["accepted"]));
}

#[tokio::test]
async fn optional_nested_deadlines_keep_the_nearest_finite_owner() {
    for (outer_timeout, inner_timeout, expected_owner) in [
        (json!(60000), Value::Null, Some("outer")),
        (Value::Null, json!(60000), Some("inner")),
        (Value::Null, Value::Null, None),
    ] {
        let mut outer = container("outer", "loop", None, 1);
        outer.config["round_timeout_ms"] = outer_timeout;
        let mut inner = container("inner", "loop", Some("outer"), 1);
        inner.config["round_timeout_ms"] = inner_timeout;
        let human = node("human", "human_input", Some("inner"));
        let plan = plan(vec![outer, inner, human]);
        let outcome = run(&plan, json!({}), &Probe::default(), &Ledger::default())
            .await
            .unwrap();
        let checkpoint = outcome
            .checkpoint_snapshot
            .unwrap()
            .container_state
            .unwrap();
        let inner_frame = checkpoint
            .frames
            .values()
            .find(|frame| frame.identity.node_id == "inner")
            .unwrap();
        let round = &inner_frame.rounds[&0];
        let expected = expected_owner.map(|name| {
            checkpoint
                .frames
                .values()
                .find(|frame| frame.identity.node_id == name)
                .unwrap()
                .identity
                .execution_id
        });
        assert_eq!(round.deadline_owner_execution_id, expected);
        assert_eq!(round.deadline_unix_ms.is_some(), expected.is_some());
        if let Some(owner) = expected {
            assert_eq!(
                round.deadline_unix_ms,
                checkpoint.frames[&owner].rounds[&0].deadline_unix_ms
            );
            // Historical numeric/UUID checkpoint fields still decode into Some.
            let encoded = serde_json::to_value(&checkpoint).unwrap();
            let decoded: ContainerCheckpoint = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                decoded.frames[&inner_frame.identity.execution_id].rounds[&0]
                    .deadline_owner_execution_id,
                Some(owner)
            );
        }
        Session::new(checkpoint, true).unwrap();
    }
}

#[tokio::test]
async fn unlimited_iteration_still_obeys_array_cardinality() {
    for items in [json!([]), json!([1, 2, 3])] {
        let mut each = container("each", "iteration", None, 1);
        each.config["round_timeout_ms"] = Value::Null;
        let mut leaf = code("leaf", "each");
        leaf.code_runtime
            .as_mut()
            .unwrap()
            .isolation_profile
            .timeout_ms = 12345;
        let plan = plan(vec![each, leaf]);
        let probe = Probe {
            expected_timeout_ms: Some(12345),
            ..Default::default()
        };
        let ledger = Ledger::default();
        let done = run(&plan, json!({"source":{"items":items}}), &probe, &ledger)
            .await
            .unwrap();
        assert_eq!(done.stop_reason, ExecutionStopReason::Completed);
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            items.as_array().unwrap().len()
        );
        let state = ledger.latest.lock().unwrap();
        let frame = state.as_ref().unwrap().frames.values().next().unwrap();
        assert_eq!(frame.started_rounds, items.as_array().unwrap().len() as u64);
        assert!(frame
            .rounds
            .values()
            .all(|round| round.deadline_unix_ms.is_none()));
    }
}
