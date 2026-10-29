use super::*;
use model::{Event, Policy, Spec, Variant};
use observations::Observer;

#[test]
fn finite_container_state_exploration_exhausts_declared_domain_and_rejects_mutants() {
    let mut total_states = 0;
    let mut total_transitions = 0;
    let mut cases = 0;
    for limit in 0..=3 {
        for parallelism in 1..=2 {
            for policy in [Policy::Fail, Policy::Break, Policy::Continue] {
                let spec = Spec {
                    limit,
                    parallelism,
                    policy,
                };
                let report = model::explore(spec, Variant::Valid);
                assert!(report.violation.is_none(), "{spec:?}: {report:?}");
                assert!(
                    report.cycle.is_none(),
                    "nonterminal cycle: {spec:?}: {report:?}"
                );
                assert_eq!(report.deadlocks, 0, "{spec:?}");
                assert!(report.terminals > 0);
                println!("container_model case={spec:?} visited_states={} transitions={} terminals={} nonterminal_cycles=0 deadlocks=0 exhaustive=true",report.states,report.transitions,report.terminals);
                total_states += report.states;
                total_transitions += report.transitions;
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 24);
    let spec = Spec {
        limit: 2,
        parallelism: 2,
        policy: Policy::Continue,
    };
    for (variant, reason) in [
        (Variant::ResetBudget, "recovery reset"),
        (Variant::SwallowGlobal, "global failure"),
        (Variant::AcceptStale, "stale generation"),
    ] {
        let report = model::explore(spec, variant);
        let violation = report
            .violation
            .expect("same verifier must reject bad producer");
        assert!(
            violation.reason.contains(reason),
            "{variant:?}: {violation:?}"
        );
        assert!(model::verify(spec, &violation.shortest_prefix[..violation.offset]).is_ok());
        assert!(model::verify(spec, &violation.shortest_prefix).is_err());
        println!("container_model mutant={variant:?} rejected=true shortest_steps={} counterexample={:?}",violation.shortest_prefix.len(),violation.shortest_prefix);
    }
    let spin = model::explore(
        Spec {
            limit: 1,
            parallelism: 1,
            policy: Policy::Continue,
        },
        Variant::NonterminalSpin,
    );
    assert!(spin.violation.is_none());
    assert!(
        spin.cycle.is_some(),
        "cycle detector must reject a spinning producer"
    );
    println!("container_model summary cases={cases} visited_states={total_states} transitions={total_transitions} caps=none assumptions=one_crash,finite_callbacks_and_deadlines,no_infinite_idle_or_owner_churn cycle_mutant={:?}",spin.cycle);
}

fn chain(counts: &[usize]) -> CompiledPlan {
    let mut nodes = Vec::new();
    for (level, count) in counts.iter().copied().enumerate() {
        let id = format!("level{level}");
        let parent = (level > 0).then(|| format!("level{}", level - 1));
        let kind = if level % 2 == 0 { "loop" } else { "iteration" };
        let mut current = container(&id, kind, parent.as_deref(), count.max(1) as u64);
        if kind == "iteration" {
            current.bindings.insert(
                "items".into(),
                selector(&["source", &format!("items{level}")]),
            );
        }
        if kind == "loop" && count == 0 {
            current.bindings.insert("break_condition".into(),CompiledBinding{kind:"condition_group".into(),raw_value:json!({"operator":"and","conditions":[{"left":[id,"index"],"comparator":"equals","right":{"kind":"constant","value":0}}]}),selector_paths:vec![vec![id.clone(),"index".into()]],i18n_text_ref:None});
        }
        if level + 1 == counts.len() {
            current
                .bindings
                .insert("output".into(), selector(&["leaf", "result", "value"]));
        } else {
            current.bindings.insert(
                "output".into(),
                selector(&[&format!("level{}", level + 1), "result"]),
            );
        }
        nodes.push(current);
    }
    let mut leaf = code("leaf", &format!("level{}", counts.len().saturating_sub(1)));
    if counts.is_empty() {
        leaf.container_id = None;
    }
    nodes.push(leaf);
    plan(nodes)
}
fn reference_tree(counts: &[usize]) -> Value {
    if let Some((count, tail)) = counts.split_first() {
        Value::Array((0..*count).map(|_| reference_tree(tail)).collect())
    } else {
        json!(1)
    }
}
#[tokio::test]
async fn real_engine_matches_independent_depth_zero_to_three_tree_domain() {
    let mut cases = 0;
    for depth in 0..=3usize {
        for encoded in 0..4usize.pow(depth as u32) {
            let mut digits = encoded;
            let counts: Vec<_> = (0..depth)
                .map(|_| {
                    let count = digits % 4;
                    digits /= 4;
                    count
                })
                .collect();
            let plan = chain(&counts);
            let probe = Probe::default();
            let ledger = Ledger::default();
            let source: Map<_, _> = counts
                .iter()
                .enumerate()
                .map(|(level, count)| (format!("items{level}"), json!(vec![1; *count])))
                .collect();
            let outcome = run(&plan, json!({"source":source}), &probe, &ledger)
                .await
                .unwrap();
            assert_eq!(
                outcome.stop_reason,
                ExecutionStopReason::Completed,
                "counts={counts:?}"
            );
            assert_eq!(
                probe.calls.load(Ordering::SeqCst),
                counts.iter().product::<usize>(),
                "counts={counts:?}"
            );
            let observed = if depth == 0 {
                outcome.variable_pool["leaf"]["result"]["value"].clone()
            } else {
                outcome.variable_pool["level0"]["result"].clone()
            };
            assert_eq!(observed, reference_tree(&counts), "counts={counts:?}");
            cases += 1;
        }
    }
    assert_eq!(cases, 85);
    println!("container_tree domain=depth0..3_count0..3 cases={cases} exhaustive=true zero_loop=initial_condition normal_only=true");
}

#[tokio::test]
async fn production_recovery_trace_and_all_three_bad_variants_use_the_same_verifier() {
    let spec = Spec {
        limit: 2,
        parallelism: 1,
        policy: Policy::Continue,
    };
    let observer = Observer::new("each", spec);
    let mut each = container("each", "iteration", None, 1);
    each.config["error_policy"] = json!("continue");
    each.bindings
        .insert("output".into(), selector(&["tool", "value"]));
    let mut tool = node("tool", "tool", Some("each"));
    tool.outputs = vec![output("value", "number")];
    let plan = plan(vec![each, tool]);
    let probe = Probe::default();
    let first = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &json!({"source":{"items":[0,1]}}),
        ExecutionRuntimeContext::default(),
        &probe,
        &observer,
    )
    .await
    .unwrap();
    observer.crash();
    observer.transfer();
    let mut outcome = first;
    for value in 0..2 {
        let checkpoint = outcome.checkpoint_snapshot.as_ref().unwrap();
        let execution_id = checkpoint
            .container_state
            .as_ref()
            .unwrap()
            .waiting_execution_id
            .unwrap();
        outcome = resume_scoped_container_with_runtime_context_and_lifecycle(
            &plan,
            checkpoint,
            execution_id,
            &json!({"value":value}),
            ExecutionRuntimeContext::default(),
            &probe,
            &observer,
        )
        .await
        .unwrap();
    }
    observer.finish(&outcome);
    assert!(observer.verify().closed);
    assert_eq!(outcome.variable_pool["each"]["result"], json!([0, 1]));
    let valid = observer.events();
    let mut reset = valid.clone();
    for event in &mut reset {
        if let Event::Recover {
            reported_started, ..
        } = event
        {
            *reported_started = 0;
            break;
        }
    }
    assert!(model::verify(spec, &reset)
        .unwrap_err()
        .reason
        .contains("recovery reset"));
    let mut stale = valid;
    for event in &mut stale {
        if let Event::Complete {
            generation,
            accepted: true,
            ..
        } = event
        {
            *generation = 0;
            break;
        }
    }
    assert!(model::verify(spec, &stale)
        .unwrap_err()
        .reason
        .contains("stale generation"));
    let mut outer = container("outer", "loop", None, 2);
    outer.config["error_policy"] = json!("continue");
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "loop", Some("outer"), 1);
    inner
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let mut leaf = code("leaf", "inner");
    leaf.config = json!({"fail":true});
    let plan = super::super::plan(vec![outer, inner, leaf]);
    let observer = Observer::new("outer", spec);
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &json!({}),
        ExecutionRuntimeContext::default(),
        &probe,
        &observer,
    )
    .await
    .unwrap();
    observer.finish(&outcome);
    assert!(observer.verify().closed);
    let mut swallowed = observer.events();
    if let Some(Event::Finish { global_failure, .. }) = swallowed.last_mut() {
        *global_failure = false;
    } else {
        panic!("missing observed finish")
    }
    assert!(model::verify(spec, &swallowed)
        .unwrap_err()
        .reason
        .contains("global failure"));
    println!("container_trace verifier=shared_oracle production_traces=2 controlled_variants=reset_budget,swallow_global,accept_stale all_rejected=true generation_scope=model_and_test_owner durable_generation_requires_pg=true");
}
