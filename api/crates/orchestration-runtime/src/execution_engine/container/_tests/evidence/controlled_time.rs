use super::*;
use model::{Policy, Spec};
use observations::Observer;
use tokio::sync::{mpsc, Semaphore};

struct GatedCode {
    calls: AtomicUsize,
    live: AtomicUsize,
    peak: AtomicUsize,
    entered: mpsc::UnboundedSender<u8>,
    gates: [Semaphore; 2],
    fail: Option<u8>,
}
impl GatedCode {
    fn new(entered: mpsc::UnboundedSender<u8>, fail: Option<u8>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            entered,
            gates: [Semaphore::new(0), Semaphore::new(0)],
            fail,
        }
    }
}
#[async_trait]
impl ProviderInvoker for GatedCode {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        bail!("unexpected provider")
    }
}
#[async_trait]
impl CapabilityInvoker for GatedCode {
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
impl CodeInvoker for GatedCode {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        input: Value,
    ) -> Result<CodeInvocationOutput> {
        let index = input.get("item").and_then(Value::as_u64).unwrap_or(0) as u8;
        let identity = scoped_execution_identity();
        self.calls.fetch_add(1, Ordering::SeqCst);
        let live = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(live, Ordering::SeqCst);
        let _guard = Live(&self.live);
        self.entered.send(index).unwrap();
        self.gates[index as usize].acquire().await.unwrap().forget();
        assert_eq!(scoped_execution_identity(), identity);
        if self.fail == Some(index) {
            bail!("controlled break failure");
        }
        Ok(CodeInvocationOutput {
            output_payload: json!({"value":index}),
            console_logs: Vec::new(),
        })
    }
}
async fn committed(
    receiver: &mut mpsc::UnboundedReceiver<(ContainerTransitionKind, String, Option<u64>)>,
    index: u64,
) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some((kind, node, round)) = receiver.recv().await {
            if kind == ContainerTransitionKind::RoundCommitted
                && node == "each"
                && round == Some(index)
            {
                return;
            }
        }
        panic!("transition channel closed");
    })
    .await
    .expect("expected round commit");
}
#[tokio::test]
async fn barriers_order_success_before_and_after_parallel_break_stop() {
    for parallelism in [1, 2] {
        for success_first in [true, false] {
            let mut each = container("each", "iteration", None, 1);
            each.config["parallelism"] = json!(parallelism);
            each.config["error_policy"] = json!("break");
            each.bindings
                .insert("output".into(), selector(&["leaf", "result", "value"]));
            let mut leaf = code("leaf", "each");
            leaf.bindings
                .insert("item".into(), selector(&["each", "item"]));
            let plan = plan(vec![each, leaf]);
            let observer = Arc::new(Observer::new(
                "each",
                Spec {
                    limit: 2,
                    parallelism,
                    policy: Policy::Break,
                },
            ));
            let (tx, mut transitions) = mpsc::unbounded_channel();
            *observer.transitions.lock().unwrap() = Some(tx);
            let (entered, mut arrivals) = mpsc::unbounded_channel();
            let probe = Arc::new(GatedCode::new(entered, Some(1)));
            let items = if parallelism == 1 && !success_first {
                json!([1, 0])
            } else {
                json!([0, 1])
            };
            let task = {
                let observer = observer.clone();
                let probe = probe.clone();
                tokio::spawn(async move {
                    start_flow_debug_run_with_runtime_context_and_lifecycle(
                        &plan,
                        &json!({"source":{"items":items}}),
                        ExecutionRuntimeContext::default(),
                        probe.as_ref(),
                        observer.as_ref(),
                    )
                    .await
                })
            };
            if parallelism == 2 {
                let mut indices = BTreeSet::new();
                indices.insert(arrivals.recv().await.unwrap());
                indices.insert(arrivals.recv().await.unwrap());
                assert_eq!(indices, BTreeSet::from([0, 1]));
            } else {
                assert_eq!(
                    arrivals.recv().await.unwrap(),
                    if success_first { 0 } else { 1 }
                );
            }
            if success_first {
                probe.gates[0].add_permits(1);
                committed(&mut transitions, 0).await;
                if parallelism == 1 {
                    assert_eq!(arrivals.recv().await.unwrap(), 1);
                }
                probe.gates[1].add_permits(1);
            } else {
                probe.gates[1].add_permits(1);
            }
            let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            observer.finish(&outcome);
            assert!(observer.verify().closed);
            assert_eq!(
                outcome.variable_pool["each"]["result"],
                if success_first { json!([0]) } else { json!([]) }
            );
            assert_eq!(probe.live.load(Ordering::SeqCst), 0);
            assert!(probe.peak.load(Ordering::SeqCst) <= parallelism as usize);
            println!("container_stop parallelism={parallelism} success_before_stop={success_first} calls={} peak={} active_after=0 verified_trace={:?}",probe.calls.load(Ordering::SeqCst),probe.peak.load(Ordering::SeqCst),observer.events());
        }
    }
}

#[tokio::test(start_paused = true)]
async fn virtual_ancestor_deadline_cancels_child_despite_continue_policy() {
    let mut outer = container("outer", "loop", None, 1);
    outer.config["round_timeout_ms"] = json!(100);
    outer.config["error_policy"] = json!("continue");
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "loop", Some("outer"), 3);
    inner.config["round_timeout_ms"] = json!(1000);
    inner.config["error_policy"] = json!("continue");
    inner
        .bindings
        .insert("output".into(), selector(&["leaf", "result", "value"]));
    let plan = plan(vec![outer, inner, code("leaf", "inner")]);
    let (entered, mut arrivals) = mpsc::unbounded_channel();
    let probe = Arc::new(GatedCode::new(entered, None));
    let observer = Arc::new(Observer::new(
        "outer",
        Spec {
            limit: 1,
            parallelism: 1,
            policy: Policy::Continue,
        },
    ));
    let task = {
        let observer = observer.clone();
        let probe = probe.clone();
        tokio::spawn(async move {
            start_flow_debug_run_with_runtime_context_and_lifecycle(
                &plan,
                &json!({}),
                ExecutionRuntimeContext::default(),
                probe.as_ref(),
                observer.as_ref(),
            )
            .await
        })
    };
    arrivals.recv().await.unwrap();
    tokio::time::advance(std::time::Duration::from_millis(101)).await;
    let outcome = task.await.unwrap().unwrap();
    observer.finish(&outcome);
    assert!(observer.verify().closed);
    assert_eq!(outcome.variable_pool["outer"]["result"], json!([]));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        observer
            .metrics
            .lock()
            .unwrap()
            .round_admissions
            .get("inner"),
        Some(&1)
    );
    assert_eq!(probe.live.load(Ordering::SeqCst), 0);
    println!("container_clock active=tokio_virtual advanced_ms=101 ancestor_ms=100 child_ms=1000 child_continue_cannot_swallow=true calls=1");
}

struct DeadlineObserver {
    observer: Observer,
    cross_during_callback: bool,
    callback_entered: tokio::sync::Notify,
    release: Semaphore,
    rejected: AtomicUsize,
}
#[async_trait]
impl ExecutionLifecycle for DeadlineObserver {
    async fn begin_node(&self, _: &CompiledNode, _: &Value) -> Result<()> {
        Ok(())
    }
    async fn commit_container_transition(&self, event: &ContainerTransition<'_>) -> Result<()> {
        let callback = event.kind == ContainerTransitionKind::NodeCompleted
            && event.identity.node_id == "tool";
        if callback && self.cross_during_callback {
            self.callback_entered.notify_one();
            self.release.acquire().await.unwrap().forget();
        }
        let admits = matches!(
            event.kind,
            ContainerTransitionKind::NodeStarted
                | ContainerTransitionKind::FrameEntered
                | ContainerTransitionKind::Resumed
        ) || (event.kind == ContainerTransitionKind::NodeCompleted
            && event
                .trace
                .is_some_and(|trace| trace.error_payload.is_none()));
        if admits {
            let deadline = event
                .identity
                .round_path
                .iter()
                .filter_map(|path| {
                    event
                        .checkpoint
                        .frames
                        .get(&path.container_execution_id)?
                        .rounds
                        .get(&path.round_index)
                })
                .min_by_key(|round| round.deadline_unix_ms);
            if let Some(round) = deadline.filter(|round| round.deadline_unix_ms <= unix_ms()) {
                self.rejected.fetch_add(1, Ordering::SeqCst);
                return Err(deadline_expired(round.deadline_owner_execution_id));
            }
        }
        self.observer.commit_container_transition(event).await
    }
}
async fn after_absolute_deadline(deadline: i64) {
    loop {
        let left = deadline.saturating_sub(unix_ms());
        if left <= 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(left as u64)).await;
    }
}
fn callback_plan(policy: &str, timeout: u64) -> CompiledPlan {
    let mut outer = container("loop", "loop", None, 1);
    outer.config["round_timeout_ms"] = json!(timeout);
    outer.config["error_policy"] = json!(policy);
    outer
        .bindings
        .insert("output".into(), selector(&["tool", "value"]));
    let mut tool = node("tool", "tool", Some("loop"));
    tool.outputs = vec![output("value", "number")];
    plan(vec![outer, tool])
}
#[tokio::test]
async fn callback_and_absolute_timeout_both_orders_and_commit_lock_crossing_keep_one_result() {
    for policy in ["fail", "break", "continue"] {
        for order in ["callback_first", "timeout_first", "commit_crossing"] {
            let timeout = if order == "callback_first" {
                10000
            } else {
                250
            };
            let plan = callback_plan(policy, timeout);
            let model_policy = match policy {
                "fail" => Policy::Fail,
                "break" => Policy::Break,
                _ => Policy::Continue,
            };
            let owner = Arc::new(DeadlineObserver {
                observer: Observer::new(
                    "loop",
                    Spec {
                        limit: 1,
                        parallelism: 1,
                        policy: model_policy,
                    },
                ),
                cross_during_callback: order == "commit_crossing",
                callback_entered: tokio::sync::Notify::new(),
                release: Semaphore::new(0),
                rejected: AtomicUsize::new(0),
            });
            let probe = Probe::default();
            let first = start_flow_debug_run_with_runtime_context_and_lifecycle(
                &plan,
                &json!({}),
                ExecutionRuntimeContext::default(),
                &probe,
                owner.as_ref(),
            )
            .await
            .unwrap();
            let checkpoint = first.checkpoint_snapshot.unwrap();
            let before = checkpoint.container_state.as_ref().unwrap().clone();
            let frame = before.frames.values().next().unwrap();
            let deadline = frame.rounds[&0].deadline_unix_ms;
            let execution_id = before.waiting_execution_id.unwrap();
            if order == "timeout_first" {
                after_absolute_deadline(deadline).await;
            }
            let mut outcome = if order == "commit_crossing" {
                let task = {
                    let plan = plan.clone();
                    let checkpoint = checkpoint.clone();
                    let owner = owner.clone();
                    tokio::spawn(async move {
                        resume_scoped_container_with_runtime_context_and_lifecycle(
                            &plan,
                            &checkpoint,
                            execution_id,
                            &json!({"value":7}),
                            ExecutionRuntimeContext::default(),
                            &Probe::default(),
                            owner.as_ref(),
                        )
                        .await
                    })
                };
                owner.callback_entered.notified().await;
                after_absolute_deadline(deadline).await;
                owner.release.add_permits(1);
                task.await.unwrap().unwrap()
            } else {
                resume_scoped_container_with_runtime_context_and_lifecycle(
                    &plan,
                    &checkpoint,
                    execution_id,
                    &json!({"value":7}),
                    ExecutionRuntimeContext::default(),
                    &probe,
                    owner.as_ref(),
                )
                .await
                .unwrap()
            };
            if order == "callback_first" {
                // A deadline arriving after the durable success can only observe
                // that terminal; recovery of current truth cannot overwrite it.
                let committed = snapshot(&owner.observer.ledger);
                after_absolute_deadline(deadline).await;
                outcome = recover_container_with_runtime_context_and_lifecycle(
                    &plan,
                    &committed,
                    ExecutionRuntimeContext::default(),
                    &probe,
                    owner.as_ref(),
                )
                .await
                .unwrap();
                assert_eq!(
                    owner
                        .observer
                        .ledger
                        .completed
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|identity| identity.node_id == "tool")
                        .count(),
                    1
                );
            }
            owner.observer.finish(&outcome);
            assert!(owner.observer.verify().closed);
            let success = order == "callback_first";
            assert_eq!(
                matches!(outcome.stop_reason, ExecutionStopReason::Failed(_)),
                !success && policy == "fail"
            );
            if success {
                assert_eq!(outcome.variable_pool["loop"]["result"], json!([7]));
            } else if policy != "fail" {
                assert_eq!(outcome.variable_pool["loop"]["result"], json!([]));
            }
            let state = owner.observer.ledger.latest.lock().unwrap();
            let final_frame = &state.as_ref().unwrap().frames[&frame.identity.execution_id];
            assert_eq!(final_frame.started_rounds, 1);
            assert_eq!(final_frame.identity, frame.identity);
            if !success {
                assert!(final_frame.results.is_empty());
            }
            if order == "commit_crossing" {
                assert_eq!(owner.rejected.load(Ordering::SeqCst), 1);
            }
            assert_eq!(
                checkpoint.container_state.as_ref().unwrap(),
                &before,
                "original checkpoint is immutable"
            );
            println!("container_callback order={order} policy={policy} clock=real_absolute typed_rejections={} accepted_output={} rounds=1",owner.rejected.load(Ordering::SeqCst),success);
        }
    }
}

#[tokio::test]
async fn expired_nested_callback_closes_ancestor_without_inner_continue_or_payload_admission() {
    let mut outer = container("outer", "loop", None, 1);
    outer.config["round_timeout_ms"] = json!(250);
    outer.config["error_policy"] = json!("break");
    outer
        .bindings
        .insert("output".into(), selector(&["inner", "result"]));
    let mut inner = container("inner", "loop", Some("outer"), 2);
    inner.config["round_timeout_ms"] = json!(1000);
    inner.config["error_policy"] = json!("continue");
    inner
        .bindings
        .insert("output".into(), selector(&["tool", "value"]));
    let mut tool = node("tool", "tool", Some("inner"));
    tool.outputs = vec![output("value", "number")];
    let plan = plan(vec![outer, inner, tool]);
    let owner = DeadlineObserver {
        observer: Observer::new(
            "outer",
            Spec {
                limit: 1,
                parallelism: 1,
                policy: Policy::Break,
            },
        ),
        cross_during_callback: false,
        callback_entered: tokio::sync::Notify::new(),
        release: Semaphore::new(0),
        rejected: AtomicUsize::new(0),
    };
    let first = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &json!({}),
        ExecutionRuntimeContext::default(),
        &Probe::default(),
        &owner,
    )
    .await
    .unwrap();
    let checkpoint = first.checkpoint_snapshot.unwrap();
    let state = checkpoint.container_state.as_ref().unwrap();
    let parent = state
        .frames
        .values()
        .find(|frame| frame.identity.node_id == "outer")
        .unwrap();
    let child = state
        .frames
        .values()
        .find(|frame| frame.identity.node_id == "inner")
        .unwrap();
    assert_eq!(
        child.rounds[&0].deadline_owner_execution_id,
        parent.identity.execution_id
    );
    after_absolute_deadline(parent.rounds[&0].deadline_unix_ms).await;
    let outcome = resume_scoped_container_with_runtime_context_and_lifecycle(
        &plan,
        &checkpoint,
        state.waiting_execution_id.unwrap(),
        &json!({"value":999}),
        ExecutionRuntimeContext::default(),
        &Probe::default(),
        &owner,
    )
    .await
    .unwrap();
    owner.observer.finish(&outcome);
    assert!(owner.observer.verify().closed);
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(outcome.variable_pool["outer"]["result"], json!([]));
    assert!(owner
        .observer
        .ledger
        .completed
        .lock()
        .unwrap()
        .iter()
        .all(|identity| identity.node_id != "tool"));
    assert_eq!(
        owner.observer.metrics.lock().unwrap().execution_ids.len(),
        3
    );
    println!("container_callback ancestor_owner=true inner_continue=false payload_admitted=false rounds_preserved=true clock=real_absolute");
}
