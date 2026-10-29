use super::*;
use model::{Policy, Spec};
use observations::Observer;
use std::sync::atomic::{AtomicBool, AtomicU64};

struct RealCode {
    timeout: AtomicU64,
    finished: Arc<AtomicBool>,
    results: Arc<Mutex<Vec<String>>>,
    workers: tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
#[async_trait]
impl ProviderInvoker for RealCode {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        bail!("unexpected provider")
    }
}
#[async_trait]
impl CapabilityInvoker for RealCode {
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
impl CodeInvoker for RealCode {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config: Value,
        input: Value,
    ) -> Result<CodeInvocationOutput> {
        self.timeout
            .store(runtime.isolation_profile.timeout_ms, Ordering::SeqCst);
        let runtime = runtime.clone();
        let finished = self.finished.clone();
        let results = self.results.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        // The test owns and joins this observer task even if the container drops
        // its receiver. Completion proves the real runner returned, not just that
        // the workflow ceased awaiting the spawn_blocking worker.
        let worker = tokio::spawn(async move {
            let result = QuickJsCodeInvoker::default()
                .invoke_code_node(&runtime, config, input)
                .await;
            results.lock().unwrap().push(
                result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unexpected success".into()),
            );
            finished.store(true, Ordering::SeqCst);
            let _ = tx.send(result);
        });
        self.workers.lock().await.push(worker);
        rx.await.map_err(|_| anyhow!("runner observer dropped"))?
    }
}
#[tokio::test]
async fn real_quickjs_container_deadline_narrows_budget_and_worker_actually_finishes() {
    let mut plan = simple_loop(1);
    plan.nodes.get_mut("loop").unwrap().config["round_timeout_ms"] = json!(80);
    plan.nodes.get_mut("loop").unwrap().config["error_policy"] = json!("continue");
    let runtime = plan
        .nodes
        .get_mut("leaf")
        .unwrap()
        .code_runtime
        .as_mut()
        .unwrap();
    runtime.source = Some("function main(inputs) { while (true) {} }".into());
    runtime.isolation_profile.timeout_ms = 1000;
    let probe = RealCode {
        timeout: AtomicU64::new(0),
        finished: Arc::new(AtomicBool::new(false)),
        results: Arc::new(Mutex::new(Vec::new())),
        workers: tokio::sync::Mutex::new(Vec::new()),
    };
    let started = std::time::Instant::now();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &json!({}),
        ExecutionRuntimeContext::default(),
        &probe,
        &Ledger::default(),
    )
    .await;
    let finished_at_container_return = probe.finished.load(Ordering::SeqCst);
    let workers = std::mem::take(&mut *probe.workers.lock().await);
    assert_eq!(workers.len(), 1);
    for worker in workers {
        tokio::time::timeout(std::time::Duration::from_secs(5), worker)
            .await
            .expect("actual QuickJS worker must terminate")
            .unwrap();
    }
    let outcome = outcome.unwrap();
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(outcome.variable_pool["loop"]["result"], json!([]));
    let budget = probe.timeout.load(Ordering::SeqCst);
    assert!((1..=80).contains(&budget));
    assert!(probe.finished.load(Ordering::SeqCst));
    assert_eq!(
        probe.results.lock().unwrap().as_slice(),
        ["timeout: code execution timed out"]
    );
    assert_eq!(
        plan.nodes["leaf"]
            .code_runtime
            .as_ref()
            .unwrap()
            .isolation_profile
            .timeout_ms,
        1000,
        "authored budget must remain unchanged"
    );
    println!("container_quickjs authored_ms=1000 effective_ms={budget} round_ms=80 real_while_true=true worker_joined=true elapsed_ms={} finished_at_container_return={finished_at_container_return} remote_stop_claim=false",started.elapsed().as_millis());
}

#[tokio::test]
async fn nested_parallel_activity_and_retained_values_are_measured_without_global_pool_copy() {
    let mut sizes = Vec::new();
    for unrelated_bytes in [0, 65536usize] {
        let mut nodes = Vec::new();
        for level in 0..3 {
            let id = format!("level{level}");
            let parent = (level > 0).then(|| format!("level{}", level - 1));
            let mut current = container(&id, "iteration", parent.as_deref(), 1);
            current.config["parallelism"] = json!(2);
            current.bindings.insert(
                "items".into(),
                selector(&["source", &format!("items{level}")]),
            );
            current.bindings.insert(
                "output".into(),
                if level == 2 {
                    selector(&["leaf", "result", "value"])
                } else {
                    selector(&[&format!("level{}", level + 1), "result"])
                },
            );
            nodes.push(current);
        }
        let mut leaf = code("leaf", "level2");
        leaf.bindings
            .insert("item".into(), selector(&["level2", "item"]));
        nodes.push(leaf);
        let plan = plan(nodes);
        let probe = Probe::default();
        let observer = Observer::new(
            "level0",
            Spec {
                limit: 2,
                parallelism: 2,
                policy: Policy::Fail,
            },
        );
        let items = json!([{"value":1,"delay":4},{"value":1,"delay":4}]);
        let outcome=start_flow_debug_run_with_runtime_context_and_lifecycle(&plan,&json!({"source":{"items0":items,"items1":items,"items2":items,"unrelated":"x".repeat(unrelated_bytes)}}),ExecutionRuntimeContext::default(),&probe,&observer).await.unwrap();
        observer.finish(&outcome);
        assert!(observer.verify().closed);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 8);
        assert_eq!(probe.live.load(Ordering::SeqCst), 0);
        assert_eq!(probe.peak.load(Ordering::SeqCst), 2);
        let metrics = observer.metrics.lock().unwrap();
        assert_eq!(metrics.execution_ids.len(), 15);
        assert_eq!(metrics.round_commits, 14);
        assert!((3..=7).contains(&metrics.peak_frames));
        sizes.push(metrics.peak_single_local_bytes);
        println!("container_cost depth=3 items_per_layer=2 parallelism=2 leaf_calls=8 node_records={} peak_leaf_tasks={} peak_frames={} peak_retained_local_bytes={} peak_single_local_bytes={} unrelated_root_bytes={unrelated_bytes}",metrics.execution_ids.len(),probe.peak.load(Ordering::SeqCst),metrics.peak_frames,metrics.peak_local_bytes,metrics.peak_single_local_bytes);
    }
    assert_eq!(
        sizes[0], sizes[1],
        "an unrelated root payload must not enlarge round-local scopes"
    );
}
