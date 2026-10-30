use super::*;
use crate::execution_engine::communication::{
    CommunicationExecution, CommunicationExecutionInputs, CommunicationExecutionPort,
    CommunicationExecutionSeal,
};
use std::sync::atomic::AtomicBool;

struct Turn {
    active: AtomicBool,
    checkpoint: Mutex<Option<Value>>,
}
impl Default for Turn {
    fn default() -> Self {
        Self {
            active: AtomicBool::new(true),
            checkpoint: Mutex::new(None),
        }
    }
}
#[async_trait]
impl CommunicationExecutionPort for Turn {
    async fn inspect(&self) -> Result<CommunicationExecutionInputs> {
        self.check_active().await?;
        Ok(CommunicationExecutionInputs {
            received_watermark: 0,
            consumed_watermark: 0,
            checkpoint: self.checkpoint.lock().unwrap().clone(),
            inputs: Vec::new(),
            pending_admission: false,
        })
    }
    async fn checkpoint(&self, _: i64, value: Value) -> Result<()> {
        *self.checkpoint.lock().unwrap() = Some(value);
        Ok(())
    }
    async fn check_active(&self) -> Result<()> {
        if self.active.load(Ordering::SeqCst) {
            Ok(())
        } else {
            bail!("controlled communication cancellation")
        }
    }
    async fn seal(&self, _: &str) -> Result<CommunicationExecutionSeal> {
        Ok(CommunicationExecutionSeal::Reserved)
    }
}
struct LlmProbe {
    calls: Mutex<Vec<ScopedExecutionIdentity>>,
    turn: Arc<Turn>,
    cancel_after_first: bool,
}
#[async_trait]
impl ProviderInvoker for LlmProbe {
    async fn resolve_llm_route(&self, _: &CompiledLlmRuntime) -> Result<ResolvedProviderRoute> {
        Ok(ResolvedProviderRoute::new(
            BTreeSet::from(["native_continuation_supported".into()]),
            (),
        ))
    }
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        let identity =
            scoped_execution_identity().expect("LLM invocation requires execution identity");
        let text = if identity.round_path.is_empty() {
            "final".to_owned()
        } else {
            format!("round-{}", identity.round_path.last().unwrap().round_index)
        };
        self.calls.lock().unwrap().push(identity);
        if self.cancel_after_first {
            self.turn.active.store(false, Ordering::SeqCst);
        }
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: text.clone(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some(text),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}
#[async_trait]
impl CapabilityInvoker for LlmProbe {
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
impl CodeInvoker for LlmProbe {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        _: Value,
    ) -> Result<CodeInvocationOutput> {
        bail!("unexpected code")
    }
}
fn llm(id: &str, parent: Option<&str>) -> CompiledNode {
    let mut node = node(id, "llm", parent);
    node.outputs = vec![output("text", "string")];
    node.llm_runtime = Some(CompiledLlmRuntime {
        provider_instance_id: "fixture".into(),
        provider_instance_display_name: "fixture".into(),
        provider_code: "openai".into(),
        protocol: "responses".into(),
        model: "fixture".into(),
        routing: None,
    });
    node.bindings.insert(
        "prompt_messages".into(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "prompt_messages".into(),
            raw_value: json!([{"role":"user","content":{"value":"hello"}}]),
            selector_paths: Vec::new(),
        },
    );
    node
}
fn conversation_plan() -> CompiledPlan {
    let mut outer = container("loop", "loop", None, 2);
    outer.config["error_policy"] = json!("continue");
    outer
        .bindings
        .insert("output".into(), selector(&["body_llm", "text"]));
    plan(vec![
        outer,
        llm("body_llm", Some("loop")),
        llm("final_llm", None),
    ])
}
#[tokio::test]
async fn communication_body_llms_complete_per_round_and_only_root_is_retained() {
    let turn = Arc::new(Turn::default());
    let communication = Arc::new(CommunicationExecution::new(turn.clone(), 0).unwrap());
    let runtime =
        ExecutionRuntimeContext::default().with_communication_execution(communication.clone());
    let probe = LlmProbe {
        calls: Mutex::new(Vec::new()),
        turn: turn.clone(),
        cancel_after_first: false,
    };
    let ledger = Ledger::default();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &conversation_plan(),
        &json!({}),
        runtime,
        &probe,
        &ledger,
    )
    .await
    .unwrap();
    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["loop"]["result"],
        json!(["round-0", "round-1"])
    );
    let completed = ledger.completed.lock().unwrap();
    assert_eq!(
        completed
            .iter()
            .filter(|id| id.node_id == "body_llm")
            .count(),
        2
    );
    assert!(
        !completed.iter().any(|id| id.node_id == "final_llm"),
        "root final candidate keeps its existing terminal owner"
    );
    let checkpoint = turn.checkpoint.lock().unwrap().clone().unwrap();
    assert_eq!(checkpoint["main_node_id"], json!("final_llm"));
    assert_eq!(
        checkpoint["retained_llms"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["final_llm"]
    );
    let receipts = communication.retained_node_receipts().await.unwrap();
    assert_eq!(
        receipts.keys().cloned().collect::<Vec<_>>(),
        vec!["final_llm"]
    );
    assert_eq!(probe.calls.lock().unwrap().len(), 3);
}
#[tokio::test]
async fn communication_cancellation_is_not_swallowed_by_body_continue() {
    let turn = Arc::new(Turn::default());
    let communication = Arc::new(CommunicationExecution::new(turn.clone(), 0).unwrap());
    let probe = LlmProbe {
        calls: Mutex::new(Vec::new()),
        turn,
        cancel_after_first: true,
    };
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &conversation_plan(),
        &json!({}),
        ExecutionRuntimeContext::default().with_communication_execution(communication),
        &probe,
        &Ledger::default(),
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Failed(_)
    ));
    assert_eq!(probe.calls.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn internal_node_completion_restores_calling_provider_identity() {
    let state = ContainerCheckpoint {
        version: 1,
        waiting_execution_id: None,
        root_next_node_index: 0,
        root_variable_pool: Map::new(),
        root_active_node_ids: Vec::new(),
        root_nodes: BTreeMap::new(),
        root_waiting: None,
        frames: BTreeMap::new(),
        execution_budget: 1,
    };
    let ledger = Ledger::default();
    let lifecycle = RootLifecycle {
        session: Session::new(state, false).unwrap(),
        owner: &ledger,
        current_identity: Arc::default(),
    };
    CURRENT_EXECUTION
        .scope(lifecycle.current_identity.clone(), async {
            let parent = node("llm", "llm", None);
            let child = node("internal_tool", "tool_result", None);
            lifecycle.begin_node(&parent, &json!({})).await.unwrap();
            let parent_identity = scoped_execution_identity().unwrap();
            lifecycle.begin_node(&child, &json!({})).await.unwrap();
            tokio::task::yield_now().await;
            assert_eq!(
                scoped_execution_identity().unwrap().node_id,
                "internal_tool"
            );
            lifecycle
                .complete_node(&NodeExecutionTrace {
                    execution_identity: None,
                    node_id: child.node_id.clone(),
                    node_type: child.node_type.clone(),
                    node_alias: child.alias.clone(),
                    input_payload: json!({}),
                    output_payload: json!({}),
                    error_payload: None,
                    metrics_payload: json!({}),
                    debug_payload: json!({}),
                    provider_events: Vec::new(),
                })
                .await
                .unwrap();
            assert_eq!(scoped_execution_identity(), Some(parent_identity));
        })
        .await;
    assert!(scoped_execution_identity().is_none());
}
