use super::*;
use crate::{
    compiled_plan::{CompiledBinding, CompiledCodeRuntime, CompiledOutput},
    execution_engine::*,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Default)]
struct Probe {
    expected_timeout_ms: Option<u64>,
    calls: AtomicUsize,
    live: AtomicUsize,
    peak: AtomicUsize,
    identities: Mutex<Vec<ScopedExecutionIdentity>>,
}
struct Live<'a>(&'a AtomicUsize);
impl Drop for Live<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
#[async_trait]
impl ProviderInvoker for Probe {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        bail!("unexpected provider")
    }
}
#[async_trait]
impl CapabilityInvoker for Probe {
    async fn invoke_capability_node(
        &self,
        _: &CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> Result<CapabilityInvocationOutput> {
        bail!("unexpected plugin")
    }
}
#[async_trait]
impl CodeInvoker for Probe {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config: Value,
        input: Value,
    ) -> Result<CodeInvocationOutput> {
        if let Some(expected) = self.expected_timeout_ms {
            assert_eq!(runtime.isolation_profile.timeout_ms, expected);
        }
        let identity_before = scoped_execution_identity();
        if let Some(identity) = &identity_before {
            self.identities.lock().unwrap().push(identity.clone());
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let _guard = Live(&self.live);
        let item = input.get("item").cloned().unwrap_or(json!(1));
        let delay = item.get("delay").and_then(Value::as_u64).unwrap_or(0);
        for _ in 0..delay {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            scoped_execution_identity(),
            identity_before,
            "concurrent future poll changed execution identity"
        );
        if (config.get("fail_on").and_then(Value::as_u64).is_some()
            && config.get("fail_on").and_then(Value::as_u64) == item.as_u64())
            || config.get("fail").and_then(Value::as_bool) == Some(true)
            || item.get("fail").and_then(Value::as_bool) == Some(true)
        {
            bail!("controlled failure");
        }
        let value = item.get("value").cloned().unwrap_or(item);
        Ok(CodeInvocationOutput {
            output_payload: json!({"value": value}),
            console_logs: Vec::new(),
        })
    }
}
#[derive(Default)]
struct Ledger {
    latest: Mutex<Option<ContainerCheckpoint>>,
    started: Mutex<Vec<ScopedExecutionIdentity>>,
    completed: Mutex<Vec<ScopedExecutionIdentity>>,
    control_failures: Mutex<Vec<ContainerControlFailure>>,
    fail_after: Mutex<Option<ContainerTransitionKind>>,
    fail_node: Mutex<Option<String>>,
}
#[async_trait]
impl ExecutionLifecycle for Ledger {
    async fn scoped_node_receipt(
        &self,
        identity: &ScopedExecutionIdentity,
    ) -> Result<Option<uuid::Uuid>> {
        Ok(Some(identity.execution_id))
    }
    async fn begin_node(&self, _: &CompiledNode, _: &Value) -> Result<()> {
        Ok(())
    }
    async fn commit_container_transition(
        &self,
        transition: &ContainerTransition<'_>,
    ) -> Result<()> {
        *self.latest.lock().unwrap() = Some(transition.checkpoint.clone());
        if transition.kind == ContainerTransitionKind::NodeStarted {
            self.started
                .lock()
                .unwrap()
                .push(transition.identity.clone());
        }
        if transition.kind == ContainerTransitionKind::NodeCompleted {
            self.completed
                .lock()
                .unwrap()
                .push(transition.identity.clone());
        }
        if transition.kind == ContainerTransitionKind::ControlFailed {
            self.control_failures
                .lock()
                .unwrap()
                .push(serde_json::from_value(transition.detail.unwrap().clone()).unwrap());
        }
        let mut failure = self.fail_after.lock().unwrap();
        if failure.as_ref() == Some(&transition.kind)
            && self
                .fail_node
                .lock()
                .unwrap()
                .as_ref()
                .is_none_or(|node_id| node_id == &transition.identity.node_id)
        {
            *failure = None;
            bail!("simulated crash after durable commit");
        }
        Ok(())
    }
}
fn node(id: &str, kind: &str, parent: Option<&str>) -> CompiledNode {
    CompiledNode {
        node_id: id.into(),
        node_type: kind.into(),
        alias: id.into(),
        container_id: parent.map(str::to_owned),
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}
fn selector(path: &[&str]) -> CompiledBinding {
    CompiledBinding {
        i18n_text_ref: None,
        kind: "selector".into(),
        raw_value: json!(path),
        selector_paths: vec![path.iter().map(|segment| (*segment).into()).collect()],
    }
}
fn output(key: &str, kind: &str) -> CompiledOutput {
    CompiledOutput {
        key: key.into(),
        title: key.into(),
        value_type: kind.into(),
        selector: Vec::new(),
        json_schema: None,
    }
}
fn container(id: &str, kind: &str, parent: Option<&str>, rounds: u64) -> CompiledNode {
    let mut node = node(id, kind, parent);
    node.config = json!({"max_rounds": rounds, "round_timeout_ms": 60000, "error_policy": "fail", "parallelism": 1});
    node.outputs = vec![output("result", "array"), output("variables", "object")];
    if kind == "iteration" {
        node.bindings
            .insert("items".into(), selector(&["source", "items"]));
    }
    node
}
fn code(id: &str, parent: &str) -> CompiledNode {
    let mut node = node(id, "code", Some(parent));
    node.outputs = vec![output("value", "unknown")];
    node.code_runtime = Some(CompiledCodeRuntime {
        language: "javascript".into(),
        source: Some("function main(x){return x}".into()),
        source_ref: None,
        entrypoint: "main".into(),
        imports: Vec::new(),
        dependencies: Vec::new(),
        isolation_profile: Default::default(),
    });
    node
}
fn plan(nodes: Vec<CompiledNode>) -> CompiledPlan {
    CompiledPlan {
        flow_id: uuid::Uuid::nil(),
        source_draft_id: "container-fixture".into(),
        schema_version: "1".into(),
        topological_order: nodes.iter().map(|node| node.node_id.clone()).collect(),
        nodes: nodes
            .into_iter()
            .map(|node| (node.node_id.clone(), node))
            .collect(),
        edges: Vec::new(),
        compile_issues: Vec::new(),
    }
}
fn simple_loop(rounds: u64) -> CompiledPlan {
    let mut outer = container("loop", "loop", None, rounds);
    outer
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    plan(vec![outer, code("leaf", "loop")])
}
async fn run(
    plan: &CompiledPlan,
    input: Value,
    probe: &Probe,
    ledger: &Ledger,
) -> Result<FlowDebugExecutionOutcome> {
    start_flow_debug_run_with_runtime_context_and_lifecycle(
        plan,
        &input,
        ExecutionRuntimeContext::default(),
        probe,
        ledger,
    )
    .await
}
fn snapshot(ledger: &Ledger) -> CheckpointSnapshot {
    let state = ledger.latest.lock().unwrap().clone().unwrap();
    CheckpointSnapshot {
        next_node_index: state.root_next_node_index,
        variable_pool: state.root_variable_pool.clone(),
        active_node_ids: state.root_active_node_ids.clone(),
        container_state: Some(state),
    }
}

#[tokio::test]
async fn nested_round_counts_match_independent_tree_reference_for_small_domain() {
    // Independent reference: multiply authored finite tree cardinalities. No
    // production state machine, scope or budget helper participates in the oracle.
    for outer_count in 1..=3 {
        for middle_count in 0..=3 {
            for inner_count in 1..=3 {
                let mut outer = container("outer", "loop", None, outer_count);
                outer
                    .bindings
                    .insert("output".into(), selector(&["middle", "result"]));
                let mut middle = container("middle", "iteration", Some("outer"), 1);
                middle
                    .bindings
                    .insert("output".into(), selector(&["inner", "result"]));
                let mut inner = container("inner", "loop", Some("middle"), inner_count);
                inner
                    .bindings
                    .insert("output".into(), selector(&["leaf", "result", "value"]));
                let plan = plan(vec![outer, middle, inner, code("leaf", "inner")]);
                let probe = Probe::default();
                let ledger = Ledger::default();
                let result = run(
                    &plan,
                    json!({"source":{"items":vec![1;middle_count]}}),
                    &probe,
                    &ledger,
                )
                .await
                .unwrap();
                assert_eq!(result.stop_reason, ExecutionStopReason::Completed);
                let expected = outer_count as usize * middle_count * inner_count as usize;
                assert_eq!(probe.calls.load(Ordering::SeqCst), expected);
                let leaf_ids: BTreeSet<_> = ledger
                    .started
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|id| id.node_id == "leaf")
                    .map(|id| id.execution_id)
                    .collect();
                assert_eq!(
                    leaf_ids.len(),
                    expected,
                    "different rounds cannot overwrite one node identity"
                );
            }
        }
    }
}

#[tokio::test]
async fn serial_and_parallel_outputs_follow_input_indices_including_null() {
    for parallelism in [1, 2, 3] {
        let mut iteration = container("each", "iteration", None, 1);
        iteration.config["parallelism"] = json!(parallelism);
        iteration.config["error_policy"] = json!("continue");
        iteration
            .bindings
            .insert("output".into(), selector(&["leaf", "result", "value"]));
        let mut leaf = code("leaf", "each");
        leaf.bindings
            .insert("item".into(), selector(&["each", "item"]));
        let plan = plan(vec![iteration, leaf]);
        let probe = Probe::default();
        let result = run(&plan, json!({"source":{"items":[{"delay":20,"value":null},{"delay":1,"fail":true},{"delay":0,"value":3}]}}), &probe, &Ledger::default()).await.unwrap();
        assert_eq!(result.variable_pool["each"]["result"], json!([null, 3]));
        assert!(probe.peak.load(Ordering::SeqCst) <= parallelism);
        assert_eq!(probe.live.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn inner_break_finishes_only_inner_and_global_fail_bypasses_parent_continue() {
    for policy in ["break", "fail"] {
        let mut outer = container("outer", "loop", None, 3);
        outer.config["error_policy"] = json!("continue");
        outer
            .bindings
            .insert("output".into(), selector(&["inner", "result"]));
        let mut inner = container("inner", "loop", Some("outer"), 4);
        inner.config["error_policy"] = json!(policy);
        inner
            .bindings
            .insert("output".into(), selector(&["leaf", "result", "value"]));
        let mut leaf = code("leaf", "inner");
        leaf.config = json!({"fail":true});
        let plan = plan(vec![outer, inner, leaf]);
        let probe = Probe::default();
        let outcome = run(&plan, json!({}), &probe, &Ledger::default())
            .await
            .unwrap();
        if policy == "break" {
            assert_eq!(probe.calls.load(Ordering::SeqCst), 3);
            assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
        } else {
            assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
            assert!(matches!(
                outcome.stop_reason,
                ExecutionStopReason::Failed(_)
            ));
        }
    }
}

#[tokio::test]
async fn recovery_reuses_committed_work_and_never_resets_round_budget() {
    for point in [
        ContainerTransitionKind::RoundStarted,
        ContainerTransitionKind::NodeCompleted,
        ContainerTransitionKind::RoundCommitted,
        ContainerTransitionKind::FrameClosed,
    ] {
        let plan = simple_loop(3);
        let probe = Probe::default();
        let ledger = Ledger::default();
        *ledger.fail_after.lock().unwrap() = Some(point);
        assert!(run(&plan, json!({}), &probe, &ledger).await.is_err());
        let recovered = recover_container_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot(&ledger),
            ExecutionRuntimeContext::default(),
            &probe,
            &ledger,
        )
        .await
        .unwrap();
        assert_eq!(
            recovered.stop_reason,
            ExecutionStopReason::Completed,
            "{point:?}"
        );
        assert_eq!(probe.calls.load(Ordering::SeqCst), 3, "{point:?}");
        assert_eq!(recovered.variable_pool["loop"]["result"], json!([1, 1, 1]));
    }
}

#[tokio::test]
async fn started_without_terminal_is_uncertain_and_not_replayed() {
    let plan = simple_loop(2);
    let probe = Probe::default();
    let ledger = Ledger::default();
    *ledger.fail_after.lock().unwrap() = Some(ContainerTransitionKind::NodeStarted);
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
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn strict_condition_failure_consumes_each_round_and_initial_true_runs_zero() {
    for condition in [
        json!({"operator":"and","conditions":[{"left":["loop","variables","missing"],"comparator":"exists"}]}),
        json!({"operator":"and","conditions":[{"left":["loop","index"],"comparator":"equals","right":{"kind":"constant","value":0}}]}),
    ] {
        let mut plan = simple_loop(3);
        let node = plan.nodes.get_mut("loop").unwrap();
        node.config["error_policy"] = json!("continue");
        node.bindings.insert(
            "break_condition".into(),
            CompiledBinding {
                kind: "condition_group".into(),
                raw_value: condition.clone(),
                selector_paths: vec![],
                i18n_text_ref: None,
            },
        );
        let probe = Probe::default();
        let ledger = Ledger::default();
        let outcome = run(&plan, json!({}), &probe, &ledger).await.unwrap();
        assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        let state = ledger.latest.lock().unwrap();
        let started = state
            .as_ref()
            .unwrap()
            .frames
            .values()
            .next()
            .unwrap()
            .started_rounds;
        assert_eq!(
            started,
            if condition["conditions"][0]["comparator"] == "exists" {
                3
            } else {
                0
            }
        );
    }
}

#[tokio::test]
async fn waiting_resume_keeps_identity_deadline_and_exact_round() {
    let mut outer = container("loop", "loop", None, 2);
    outer
        .bindings
        .insert("output".into(), selector(&["human", "value"]));
    let mut human = node("human", "human_input", Some("loop"));
    human.outputs = vec![output("value", "string")];
    let plan = plan(vec![outer, human]);
    let ledger = Ledger::default();
    let probe = Probe::default();
    let first = run(&plan, json!({}), &probe, &ledger).await.unwrap();
    let first_checkpoint = first.checkpoint_snapshot.unwrap();
    let id = waiting_execution_id(&first_checkpoint, "human").unwrap();
    let second = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &first_checkpoint,
        id,
        &json!({"value":"first"}),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    let second_checkpoint = second.checkpoint_snapshot.unwrap();
    let second_id = waiting_execution_id(&second_checkpoint, "human").unwrap();
    assert_ne!(id, second_id);
    let done = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &second_checkpoint,
        second_id,
        &json!({"value":"second"}),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(
        done.variable_pool["loop"]["result"],
        json!(["first", "second"])
    );
}

#[tokio::test]
async fn expired_wait_is_failed_before_callback_payload_is_admitted() {
    let mut outer = container("loop", "loop", None, 1);
    outer.config["error_policy"] = json!("continue");
    outer
        .bindings
        .insert("output".into(), selector(&["human", "value"]));
    let mut human = node("human", "human_input", Some("loop"));
    human.outputs = vec![output("value", "string")];
    let plan = plan(vec![outer, human]);
    let ledger = Ledger::default();
    let probe = Probe::default();
    let outcome = run(&plan, json!({}), &probe, &ledger).await.unwrap();
    let mut checkpoint = outcome.checkpoint_snapshot.unwrap();
    let id = waiting_execution_id(&checkpoint, "human").unwrap();
    for frame in checkpoint
        .container_state
        .as_mut()
        .unwrap()
        .frames
        .values_mut()
    {
        for round in frame.rounds.values_mut() {
            round.deadline_unix_ms = Some(0);
        }
    }
    let done = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        id,
        &json!({"invalid":"must not admit"}),
        ExecutionRuntimeContext::default(),
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(done.variable_pool["loop"]["result"], json!([]));
    assert_eq!(done.stop_reason, ExecutionStopReason::Completed);
}
mod reference;

mod root;

mod communication;

mod control;

mod deadline_rejection;
mod durable_lifecycle;

mod evidence;

mod optional_deadline;
