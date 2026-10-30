use super::*;

fn final_condition_error_plan(policy: &str) -> CompiledPlan {
    let mut outer = container("loop", "loop", None, 1);
    outer.config["error_policy"] = json!(policy);
    outer.config["loop_variables"] = json!([{"key":"gate","value_type":"object"}]);
    outer.bindings.insert(
        "variables".into(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "named_bindings".into(),
            raw_value: json!([{"name":"gate","value":{"kind":"constant","value":{"ready":1}}}]),
            selector_paths: Vec::new(),
        },
    );
    outer.bindings.insert("variable_updates".into(),CompiledBinding{i18n_text_ref:None,kind:"named_bindings".into(),raw_value:json!([{"name":"gate","value":{"kind":"selector","selector":["leaf","result","value"]}}]),selector_paths:vec![vec!["leaf".into(),"result".into(),"value".into()]]});
    outer.bindings.insert("break_condition".into(),CompiledBinding{i18n_text_ref:None,kind:"condition_group".into(),raw_value:json!({"operator":"and","conditions":[{"left":["loop","variables","gate","ready"],"comparator":"equals","right":{"kind":"constant","value":0}}]}),selector_paths:vec![vec!["loop".into(),"variables".into(),"gate".into(),"ready".into()]]});
    outer
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "loop");
    leaf.bindings
        .insert("item".into(), selector(&["source", "replacement"]));
    plan(vec![outer, leaf])
}

#[tokio::test]
async fn last_success_condition_error_preserves_commit_and_records_control_failure_for_all_policies(
) {
    for policy in ["fail", "break", "continue"] {
        let plan = final_condition_error_plan(policy);
        let probe = Probe::default();
        let ledger = Ledger::default();
        let outcome = run(&plan, json!({"source":{"replacement":{}}}), &probe, &ledger)
            .await
            .unwrap();
        assert_eq!(
            matches!(outcome.stop_reason, ExecutionStopReason::Failed(_)),
            policy == "fail"
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        let state = ledger.latest.lock().unwrap();
        let frame = state.as_ref().unwrap().frames.values().next().unwrap();
        assert_eq!(
            frame.started_rounds, 1,
            "condition failure cannot grant or consume another round"
        );
        assert_eq!(
            frame.variables,
            json!({"gate":{}}).as_object().unwrap().clone()
        );
        assert_eq!(frame.results[&0], json!({}));
        assert!(frame.final_condition_checked);
        let failures = ledger.control_failures.lock().unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].after_round_index, 0);
        assert_eq!(
            failures[0].error_payload["error_code"],
            json!("container_condition_failed")
        );
        if policy != "fail" {
            assert_eq!(outcome.variable_pool["loop"]["result"], json!([{}]));
        }
    }
}

#[tokio::test]
async fn final_condition_recovery_covers_both_sides_of_its_durable_control_fact() {
    for crash in [
        ContainerTransitionKind::RoundCommitted,
        ContainerTransitionKind::ControlFailed,
    ] {
        let plan = final_condition_error_plan("continue");
        let probe = Probe::default();
        let ledger = Ledger::default();
        *ledger.fail_after.lock().unwrap() = Some(crash);
        assert!(
            run(&plan, json!({"source":{"replacement":{}}}), &probe, &ledger)
                .await
                .is_err()
        );
        let outcome = recover_container_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot(&ledger),
            ExecutionRuntimeContext::default(),
            &probe,
            &ledger,
        )
        .await
        .unwrap();
        assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
        assert_eq!(
            outcome.variable_pool["loop"]["variables"],
            json!({"gate":{}})
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            ledger.control_failures.lock().unwrap().len(),
            1,
            "recovery cannot duplicate the control failure fact"
        );
    }
}
