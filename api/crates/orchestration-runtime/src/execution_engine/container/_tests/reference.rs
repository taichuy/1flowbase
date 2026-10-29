use super::*;

// This reference is deliberately just a finite sequence fold. It has no access
// to production frame, scope, scheduler, binding, or checkpoint helpers.
fn reference(items: &[bool], policy: &str) -> (usize, Vec<usize>, bool) {
    let mut started = 0;
    let mut outputs = Vec::new();
    for (index, fails) in items.iter().copied().enumerate() {
        started += 1;
        if fails {
            match policy {
                "fail" => return (started, outputs, true),
                "break" => break,
                "continue" => continue,
                _ => unreachable!(),
            }
        }
        outputs.push(index);
    }
    (started, outputs, false)
}

#[tokio::test]
async fn finite_error_sequences_match_independent_reference_and_reject_reset_budget() {
    let mut explored = 0;
    for size in 0..=3usize {
        for mask in 0..(1usize << size) {
            for policy in ["fail", "break", "continue"] {
                let failures: Vec<_> = (0..size).map(|index| mask & (1 << index) != 0).collect();
                let expected = reference(&failures, policy);
                let mut each = container("each", "iteration", None, 1);
                each.config["error_policy"] = json!(policy);
                each.bindings
                    .insert("output".into(), selector(&["leaf", "result", "value"]));
                let mut leaf = code("leaf", "each");
                leaf.bindings
                    .insert("item".into(), selector(&["each", "item"]));
                let plan = plan(vec![each, leaf]);
                let probe = Probe::default();
                let items: Vec<_> = failures
                    .iter()
                    .enumerate()
                    .map(|(index, fails)| json!({"value":index,"fail":fails}))
                    .collect();
                let outcome = run(
                    &plan,
                    json!({"source":{"items":items}}),
                    &probe,
                    &Ledger::default(),
                )
                .await
                .unwrap();
                assert_eq!(
                    probe.calls.load(Ordering::SeqCst),
                    expected.0,
                    "size={size} mask={mask} policy={policy}"
                );
                assert_eq!(
                    matches!(outcome.stop_reason, ExecutionStopReason::Failed(_)),
                    expected.2
                );
                if !expected.2 {
                    assert_eq!(outcome.variable_pool["each"]["result"], json!(expected.1));
                }
                // Controlled wrong implementation: continue refunds every failed round.
                // A nonempty all-failure input must disagree with that zero-cost model.
                if policy == "continue" && failures.iter().any(|fails| *fails) {
                    let wrong_consumed = failures.iter().filter(|fails| !**fails).count();
                    assert_ne!(expected.0, wrong_consumed);
                }
                explored += 1;
            }
        }
    }
    assert_eq!(explored, 45, "the declared finite domain must be exhausted");
}

#[tokio::test]
async fn parallel_break_closes_output_before_late_success_and_stops_admission() {
    let mut each = container("each", "iteration", None, 1);
    each.config["parallelism"] = json!(2);
    each.config["error_policy"] = json!("break");
    each.bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "each");
    leaf.bindings
        .insert("item".into(), selector(&["each", "item"]));
    let plan = plan(vec![each, leaf]);
    let probe = Probe::default();
    let outcome = run(
        &plan,
        json!({"source":{"items":[{"delay":100,"value":1},{"fail":true},{"value":3},{"value":4}]}}),
        &probe,
        &Ledger::default(),
    )
    .await
    .unwrap();
    assert_eq!(outcome.variable_pool["each"]["result"], json!([]));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        probe.live.load(Ordering::SeqCst),
        0,
        "cancelled futures must not be detached"
    );
}

#[tokio::test]
async fn serial_loop_variables_commit_only_after_successful_rounds() {
    let mut outer = container("loop", "loop", None, 3);
    outer.config["error_policy"] = json!("continue");
    outer.config["loop_variables"] = json!([{"key":"value","value_type":"number"}]);
    outer.bindings.insert(
        "variables".into(),
        CompiledBinding {
            kind: "named_bindings".into(),
            raw_value: json!([{"name":"value","value":{"kind":"constant","value":0}}]),
            selector_paths: vec![],
            i18n_text_ref: None,
        },
    );
    outer.bindings.insert("variable_updates".into(), CompiledBinding { kind:"named_bindings".into(), raw_value:json!([{"name":"value","value":{"kind":"selector","selector":["leaf","result","value"]}}]),selector_paths:vec![vec!["leaf".into(),"result".into(),"value".into()]],i18n_text_ref:None });
    outer
        .bindings
        .insert("output".into(), selector(&["loop", "variables", "value"]));
    let mut leaf = code("leaf", "loop");
    leaf.config = json!({"fail_on":1});
    leaf.bindings
        .insert("item".into(), selector(&["loop", "index"]));
    let plan = plan(vec![outer, leaf]);
    let probe = Probe::default();
    let outcome = run(&plan, json!({}), &probe, &Ledger::default())
        .await
        .unwrap();
    assert_eq!(outcome.variable_pool["loop"]["result"], json!([0, 0]));
    assert_eq!(
        outcome.variable_pool["loop"]["variables"],
        json!({"value":2})
    );
}

#[tokio::test]
async fn parallel_wait_payload_and_checkpoint_select_the_same_execution() {
    let mut each = container("each", "iteration", None, 1);
    each.config["parallelism"] = json!(2);
    each.bindings
        .insert("output".into(), selector(&["tool", "value"]));
    let mut tool = node("tool", "tool", Some("each"));
    tool.bindings
        .insert("item".into(), selector(&["each", "item"]));
    tool.outputs = vec![output("value", "string")];
    let plan = plan(vec![each, tool]);
    let ledger = Ledger::default();
    let probe = Probe::default();
    let mut outcome = run(
        &plan,
        json!({"source":{"items":["input-0","input-1"]}}),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    let mut visited = BTreeSet::new();
    for _ in 0..2 {
        let checkpoint = outcome.checkpoint_snapshot.as_ref().unwrap();
        let state = checkpoint.container_state.as_ref().unwrap();
        let execution_id = state.waiting_execution_id.expect("selected wait");
        let index = state
            .frames
            .values()
            .flat_map(|frame| frame.rounds.values())
            .find(|round| {
                round
                    .nodes
                    .values()
                    .any(|node| node.identity.execution_id == execution_id)
            })
            .unwrap()
            .round_index;
        assert!(visited.insert(index));
        let ExecutionStopReason::WaitingCallback(wait) = &outcome.stop_reason else {
            panic!("expected callback")
        };
        assert_eq!(
            wait.request_payload["item"],
            json!(format!("input-{index}"))
        );
        outcome = resume_scoped_container_with_runtime_context_and_lifecycle(
            &plan,
            checkpoint,
            execution_id,
            &json!({"value":format!("result-{index}")}),
            ExecutionRuntimeContext::default(),
            &probe,
            &ledger,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        outcome.variable_pool["each"]["result"],
        json!(["result-0", "result-1"])
    );
}

#[tokio::test]
async fn checkpoint_round_scope_omits_unreferenced_parent_values_and_frozen_items() {
    let mut each = container("each", "iteration", None, 1);
    each.bindings
        .insert("output".into(), selector(&["human", "value"]));
    let mut human = node("human", "human_input", Some("each"));
    human.outputs = vec![output("value", "string")];
    human
        .bindings
        .insert("prompt".into(), selector(&["source", "small"]));
    let plan = plan(vec![each, human]);
    let ledger = Ledger::default();
    let outcome = run(&plan,json!({"source":{"items":[1,2,3],"small":"prompt","large":vec![0;10000]},"unrelated":vec![0;10000]}),&Probe::default(),&ledger).await.unwrap();
    let state = outcome
        .checkpoint_snapshot
        .unwrap()
        .container_state
        .unwrap();
    let round = state
        .frames
        .values()
        .next()
        .unwrap()
        .rounds
        .values()
        .next()
        .unwrap();
    assert_eq!(round.variable_pool["source"], json!({"small":"prompt"}));
    assert!(!round.variable_pool.contains_key("unrelated"));
}

#[tokio::test]
async fn concurrent_round_invocations_keep_their_own_identity_across_yields() {
    let mut each = container("each", "iteration", None, 1);
    each.config["parallelism"] = json!(3);
    each.bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "each");
    leaf.bindings
        .insert("item".into(), selector(&["each", "item"]));
    let plan = plan(vec![each, leaf]);
    let probe = Probe::default();
    let outcome = run(&plan,json!({"source":{"items":[{"delay":3,"value":0},{"delay":2,"value":1},{"delay":1,"value":2}]}}),&probe,&Ledger::default()).await.unwrap();
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    let identities = probe.identities.lock().unwrap();
    assert_eq!(identities.len(), 3);
    assert_eq!(
        identities
            .iter()
            .map(|identity| identity.execution_id)
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        identities
            .iter()
            .map(|identity| identity.round_path.last().unwrap().round_index)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2])
    );
    assert!(
        scoped_execution_identity().is_none(),
        "identity must not leak out of the scoped future"
    );
}
