use super::*;
use crate::execution_engine::communication::{
    continue_communication_reply, CommunicationExecution, CommunicationExecutionInput,
    CommunicationExecutionInputs, CommunicationExecutionPort, CommunicationExecutionSeal,
};
use crate::execution_state::FlowDebugExecutionOutcome;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::Notify;

#[derive(Default)]
struct TurnState {
    inputs: Vec<CommunicationExecutionInput>,
    consumed: i64,
    checkpoint: Option<Value>,
    inject_before_seal: Option<String>,
    replies: Vec<String>,
}

struct Turn {
    active: AtomicBool,
    state: Mutex<TurnState>,
}

impl Turn {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicBool::new(true),
            state: Mutex::new(TurnState::default()),
        })
    }
    fn append(&self, text: &str) {
        let mut state = self.state.lock().unwrap();
        let sequence = state.inputs.len() as i64 + 1;
        state.inputs.push(CommunicationExecutionInput {
            input_id: Uuid::now_v7(),
            input_sequence: sequence,
            messages: vec![json!({"role":"user","content":text})],
        });
    }
}

#[async_trait]
impl CommunicationExecutionPort for Turn {
    async fn inspect(&self) -> Result<CommunicationExecutionInputs> {
        self.check_active().await?;
        let state = self.state.lock().unwrap();
        Ok(CommunicationExecutionInputs {
            received_watermark: state.inputs.len() as i64,
            consumed_watermark: state.consumed,
            checkpoint: state.checkpoint.clone(),
            inputs: state
                .inputs
                .iter()
                .filter(|input| input.input_sequence > state.consumed)
                .cloned()
                .collect(),
            pending_admission: false,
        })
    }
    async fn checkpoint(&self, consumed: i64, continuation: Value) -> Result<()> {
        self.check_active().await?;
        let mut state = self.state.lock().unwrap();
        assert!(consumed >= state.consumed && consumed <= state.inputs.len() as i64);
        assert_eq!(continuation["consumed_watermark"], consumed);
        assert_eq!(continuation["version"], 1);
        state.checkpoint = Some(continuation);
        state.consumed = consumed;
        Ok(())
    }
    async fn check_active(&self) -> Result<()> {
        if !self.active.load(Ordering::SeqCst) {
            anyhow::bail!("communication_turn_fenced");
        }
        Ok(())
    }
    async fn seal(&self, reply: &str) -> Result<CommunicationExecutionSeal> {
        self.check_active().await?;
        let mut state = self.state.lock().unwrap();
        if let Some(text) = state.inject_before_seal.take() {
            let sequence = state.inputs.len() as i64 + 1;
            state.inputs.push(CommunicationExecutionInput {
                input_id: Uuid::now_v7(),
                input_sequence: sequence,
                messages: vec![json!({"role":"user","content":text})],
            });
        }
        if state.consumed != state.inputs.len() as i64 {
            return Ok(CommunicationExecutionSeal::Pending);
        }
        assert!(
            state.replies.is_empty(),
            "one turn must reserve only one final reply"
        );
        state.replies.push(reply.into());
        Ok(CommunicationExecutionSeal::Reserved)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Text,
    RegisteredTool,
    VisibleTool,
}

struct CommunicationProvider {
    mode: Mode,
    turn: Arc<Turn>,
    inputs: Mutex<Vec<ProviderInvocationInput>>,
    main_calls: AtomicUsize,
    mounted_calls: AtomicUsize,
    effects: AtomicUsize,
    block_first: bool,
    entered: Notify,
    release: Notify,
}

impl CommunicationProvider {
    fn new(turn: Arc<Turn>, mode: Mode, block_first: bool) -> Self {
        Self {
            mode,
            turn,
            inputs: Mutex::new(Vec::new()),
            main_calls: AtomicUsize::new(0),
            mounted_calls: AtomicUsize::new(0),
            effects: AtomicUsize::new(0),
            block_first,
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}

#[async_trait]
impl ProviderInvoker for CommunicationProvider {
    async fn resolve_llm_route(&self, _: &CompiledLlmRuntime) -> Result<ResolvedProviderRoute> {
        Ok(ResolvedProviderRoute::new(
            BTreeSet::from(["native_continuation_supported".into()]),
            (),
        ))
    }
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.inputs.lock().unwrap().push(input.clone());
        if input.trace_context.get("node_id").map(String::as_str) == Some("node-mounted-llm") {
            self.mounted_calls.fetch_add(1, Ordering::SeqCst);
            self.turn.append("supplement after mounted tool");
            return Ok(provider_output(final_llm_response("mounted result")));
        }
        let call = self.main_calls.fetch_add(1, Ordering::SeqCst);
        if self.block_first && call == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        if call == 0 && self.mode != Mode::Text {
            return Ok(provider_output(tool_call_response(vec![
                ProviderToolCall {
                    id: "one-effect".into(),
                    name: if self.mode == Mode::RegisteredTool {
                        "catalog_mcp_call"
                    } else {
                        "inspect_visible_context"
                    }
                    .into(),
                    arguments: json!({"query":"inspect","tool_id":"lookup","arguments":{"id":1}}),
                    provider_metadata: json!({}),
                },
            ])));
        }
        let text = input
            .messages
            .iter()
            .filter(|message| {
                matches!(
                    message.role,
                    ProviderMessageRole::User | ProviderMessageRole::Tool
                )
            })
            .map(|message| message.content.clone())
            .collect::<Vec<_>>()
            .join(" | ");
        let mut response = final_llm_response(&text);
        response.response_id = Some(format!("response-{call}"));
        response.provider_metadata = json!({"transport":"http","provenance":"fixture-response"});
        Ok(provider_output(response))
    }
}

#[async_trait]
impl CapabilityInvoker for CommunicationProvider {
    async fn invoke_capability_node(
        &self,
        _: &CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> Result<CapabilityInvocationOutput> {
        anyhow::bail!("unexpected capability");
    }
    async fn invoke_notification_node(
        &self,
        _: &CompiledNode,
        _: &serde_json::Map<String, Value>,
    ) -> Result<crate::execution_engine::NativeNotificationInvocationOutput> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        Ok(
            crate::execution_engine::NativeNotificationInvocationOutput {
                output_payload: json!({"delivered":true}),
                error_payload: None,
            },
        )
    }
}
#[async_trait]
impl CodeInvoker for CommunicationProvider {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        _: Value,
    ) -> Result<CodeInvocationOutput> {
        anyhow::bail!("unexpected code");
    }
}

struct AppendingTool {
    inner: StubRuntimeInternalToolInvoker,
    turn: Arc<Turn>,
}
#[async_trait]
impl RuntimeInternalToolInvoker for AppendingTool {
    fn registrations_for_node(&self, node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        self.inner.registrations_for_node(node)
    }
    async fn invoke_runtime_internal_tool(
        &self,
        node: &CompiledNode,
        registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput> {
        let result = self
            .inner
            .invoke_runtime_internal_tool(node, registration, arguments)
            .await?;
        self.turn.append("supplement after catalog tool");
        Ok(result)
    }
}

fn context(
    plan: &CompiledPlan,
    input: &Value,
    turn: Arc<Turn>,
    seeded: i64,
) -> ExecutionRuntimeContext {
    ExecutionRuntimeContext::from_plan_input(plan, input.as_object().unwrap())
        .unwrap()
        .with_communication_execution(Arc::new(CommunicationExecution::new(turn, seeded).unwrap()))
}

async fn finish(
    plan: &CompiledPlan,
    mut outcome: FlowDebugExecutionOutcome,
    context: &ExecutionRuntimeContext,
    provider: &CommunicationProvider,
    lifecycle: &dyn ExecutionLifecycle,
) -> FlowDebugExecutionOutcome {
    let turn = context.communication_execution().unwrap();
    loop {
        turn.restore_retained_traces(plan, &mut outcome)
            .await
            .unwrap();
        turn.checkpoint_graph(&outcome).await.unwrap();
        let reply = outcome.variable_pool["node-answer"]["answer"]
            .as_str()
            .unwrap();
        match turn.seal(reply).await.unwrap() {
            CommunicationExecutionSeal::Reserved => return outcome,
            CommunicationExecutionSeal::Pending => {
                outcome = continue_communication_reply(plan, outcome, context, provider, lifecycle)
                    .await
                    .unwrap()
            }
        }
    }
}

#[tokio::test]
async fn communication_initial_batch_is_consumed_once_and_preserves_system_session_provenance() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    turn.append("initial supplement");
    let input = json!({"node-start":{"query":"original"},"env":{"private":"must-not-be-checkpointed"},"__native_model_prompt_context":{"system":[{"type":"text","text":"stable application policy"}],"messages":[{"role":"system","content":"opaque history conversation UUID","__communication_session":true}]}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context.clone(),
        &provider,
        &lifecycle,
    )
    .await
    .unwrap();
    let outcome = finish(&plan, outcome, &context, &provider, &lifecycle).await;
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    let captured = provider.inputs.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0]
            .messages
            .iter()
            .filter(|message| message.content == "original")
            .count(),
        1
    );
    assert!(captured[0]
        .messages
        .iter()
        .any(|message| message.content == "initial supplement"));
    assert!(captured[0]
        .system
        .iter()
        .any(|block| block.text_content() == "stable application policy"));
    let state = turn.state.lock().unwrap();
    assert_eq!(state.consumed, 2);
    assert!(
        state.replies[0].contains("original") && state.replies[0].contains("initial supplement")
    );
    let checkpoint = state.checkpoint.as_ref().unwrap();
    assert!(!checkpoint.to_string().contains("must-not-be-checkpointed"));
    assert!(checkpoint["llm_states"]["node-llm"]["history"]
        .as_array()
        .unwrap()
        .iter()
        .any(|message| message["__communication_session"] == true));
    assert_eq!(
        checkpoint["llm_states"]["node-llm"]["response_id"],
        "response-0"
    );
}

#[tokio::test]
async fn communication_input_during_blocked_text_generation_continues_retained_transcript() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, true);
    let lifecycle = RecordingExecutionLifecycle::default();
    let execution = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context.clone(),
        &provider,
        &lifecycle,
    );
    let supplement = async {
        provider.entered.notified().await;
        turn.append("during model");
        provider.release.notify_one();
    };
    let (outcome, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(execution, supplement)
    })
    .await
    .unwrap();
    let outcome = finish(&plan, outcome.unwrap(), &context, &provider, &lifecycle).await;
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 2);
    assert!(outcome.variable_pool["node-answer"]["answer"]
        .as_str()
        .unwrap()
        .contains("during model"));
    let inputs = provider.inputs.lock().unwrap();
    assert!(
        inputs[1]
            .messages
            .iter()
            .any(|message| message.role == ProviderMessageRole::Assistant
                && message.content.contains("original")),
        "pure-text candidate was retained as prior assistant context"
    );
    assert_eq!(
        inputs[1]
            .messages
            .iter()
            .filter(|message| message.content == "during model")
            .count(),
        1
    );
    let state = turn.state.lock().unwrap();
    assert_eq!(state.replies.len(), 1);
    assert!(state.replies[0].contains("original") && state.replies[0].contains("during model"));
}

#[tokio::test]
async fn communication_append_after_registered_tool_preserves_result_without_replaying_effect() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let tool = Arc::new(AppendingTool {
        inner: StubRuntimeInternalToolInvoker::default(),
        turn: turn.clone(),
    });
    let context =
        context(&plan, &input, turn.clone(), 1).with_runtime_internal_tool_invoker(tool.clone());
    let provider = CommunicationProvider::new(turn.clone(), Mode::RegisteredTool, false);
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context.clone(),
        &provider,
        &lifecycle,
    )
    .await
    .unwrap();
    finish(&plan, outcome, &context, &provider, &lifecycle).await;
    assert_eq!(tool.inner.calls.lock().unwrap().len(), 1);
    let state = turn.state.lock().unwrap();
    assert!(
        state.replies[0].contains("original")
            && state.replies[0].contains("supplement after catalog tool")
            && state.replies[0].contains("catalog result")
    );
    let history = state.checkpoint.as_ref().unwrap()["llm_states"]["node-llm"]["history"]
        .as_array()
        .unwrap();
    assert_eq!(
        history
            .iter()
            .filter(|message| message["role"] == "tool" && message["tool_call_id"] == "one-effect")
            .count(),
        1
    );
    assert_eq!(state.consumed, 2);
}

#[tokio::test]
async fn communication_visible_internal_tool_finishes_before_consuming_supplement() {
    let plan = visible_internal_llm_tool_fixtures::visible_internal_llm_tool_plan_with_result();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = CommunicationProvider::new(turn.clone(), Mode::VisibleTool, false);
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context.clone(),
        &provider,
        &lifecycle,
    )
    .await
    .unwrap();
    finish(&plan, outcome, &context, &provider, &lifecycle).await;
    assert_eq!(provider.mounted_calls.load(Ordering::SeqCst), 1);
    let state = turn.state.lock().unwrap();
    assert!(state.replies[0].contains("supplement after mounted tool"));
    let history = state.checkpoint.as_ref().unwrap()["llm_states"]["node-llm"]["history"]
        .as_array()
        .unwrap();
    let tool = history
        .iter()
        .position(|message| message["role"] == "tool")
        .unwrap();
    let supplement = history
        .iter()
        .position(|message| message["content"] == "supplement after mounted tool")
        .unwrap();
    assert!(tool < supplement);
    assert!(history[tool]["content"]
        .as_str()
        .unwrap()
        .contains("mounted result"));
}

#[tokio::test]
async fn communication_final_candidate_race_refreshes_answer_without_replaying_graph_action() {
    let mut plan = llm_answer_plan();
    let mut effect = plan.nodes["node-answer"].clone();
    effect.node_id = "notify-once".into();
    effect.node_type = "notification".into();
    effect.bindings.clear();
    effect.outputs.clear();
    effect.downstream_node_ids = vec!["node-answer".into()];
    effect.dependency_node_ids = vec!["node-llm".into()];
    plan.nodes.get_mut("node-llm").unwrap().downstream_node_ids = vec!["notify-once".into()];
    plan.nodes
        .get_mut("node-answer")
        .unwrap()
        .dependency_node_ids = vec!["notify-once".into()];
    plan.nodes.insert(effect.node_id.clone(), effect);
    plan.topological_order.insert(2, "notify-once".into());
    let turn = Turn::new();
    turn.append("original");
    turn.state.lock().unwrap().inject_before_seal = Some("late before seal".into());
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan,
        &input,
        context.clone(),
        &provider,
        &lifecycle,
    )
    .await
    .unwrap();
    assert_eq!(provider.effects.load(Ordering::SeqCst), 1);
    let outcome = finish(&plan, outcome, &context, &provider, &lifecycle).await;
    assert_eq!(provider.effects.load(Ordering::SeqCst), 1);
    assert_eq!(
        outcome
            .node_traces
            .iter()
            .filter(|trace| trace.node_id == "notify-once")
            .count(),
        1
    );
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 2);
    let state = turn.state.lock().unwrap();
    assert_eq!(state.replies.len(), 1);
    assert!(state.replies[0].contains("late before seal") && state.replies[0].contains("original"));
}

#[tokio::test]
async fn communication_epoch_fence_after_model_request_stops_continuation_and_send() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, true);
    let lifecycle = RecordingExecutionLifecycle::default();
    let execution = start_flow_debug_run_with_runtime_context_and_lifecycle(
        &plan, &input, context, &provider, &lifecycle,
    );
    let rotation = async {
        provider.entered.notified().await;
        turn.active.store(false, Ordering::SeqCst);
        provider.release.notify_one();
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(execution, rotation)
    })
    .await
    .unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("communication_turn_fenced"));
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
    assert!(turn.state.lock().unwrap().replies.is_empty());
}

#[tokio::test]
async fn communication_task_abort_drops_blocked_provider_without_seal() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let provider = Arc::new(CommunicationProvider::new(turn.clone(), Mode::Text, true));
    let executing_provider = provider.clone();
    let task = tokio::spawn(async move {
        start_flow_debug_run_with_runtime_context(
            &plan,
            &input,
            context,
            executing_provider.as_ref(),
        )
        .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        provider.entered.notified(),
    )
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
    let state = turn.state.lock().unwrap();
    assert_eq!(
        state.consumed, 1,
        "acknowledged input remains in the durable continuation"
    );
    assert!(
        state.checkpoint.as_ref().unwrap()["llm_states"]["node-llm"]["history"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"] == "original")
    );
    assert!(state.replies.is_empty());
}

#[tokio::test]
async fn communication_cancellation_race_drops_provider_without_waiting_for_its_result() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let context = context(&plan, &input, turn.clone(), 1);
    let owner = context.communication_execution().unwrap().clone();
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, true);
    let lease = owner.enter_execution().await.unwrap();
    let execution = owner.run_until_cancelled(start_flow_debug_run_with_runtime_context(
        &plan, &input, context, &provider,
    ));
    let cancel = async {
        provider.entered.notified().await;
        turn.active.store(false, Ordering::SeqCst);
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        tokio::join!(execution, cancel)
    })
    .await
    .unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("communication_execution_cancelled"));
    drop(lease);
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
    assert!(turn.state.lock().unwrap().replies.is_empty());
}

#[tokio::test]
async fn communication_hook_absent_keeps_ordinary_inference_unchanged() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("unrelated queued communication input");
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let outcome = start_flow_debug_run(
        &plan,
        &json!({"node-start":{"query":"ordinary request"}}),
        &provider,
    )
    .await
    .unwrap();
    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
    assert!(!outcome.variable_pool["node-answer"]["answer"]
        .as_str()
        .unwrap()
        .contains("unrelated"));
    assert!(turn.state.lock().unwrap().checkpoint.is_none());
}

#[tokio::test]
async fn communication_fresh_callback_context_keeps_completed_result_and_admits_new_input() {
    let plan = llm_answer_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let provider = CommunicationProvider::new(turn.clone(), Mode::RegisteredTool, false);
    // No inline registration: this call is owned by the existing external callback lane.
    let waiting = start_flow_debug_run_with_runtime_context(
        &plan,
        &input,
        context(&plan, &input, turn.clone(), 1),
        &provider,
    )
    .await
    .unwrap();
    assert!(matches!(
        waiting.stop_reason,
        ExecutionStopReason::WaitingCallback(_)
    ));
    turn.append("during callback wait");
    let snapshot = waiting.checkpoint_snapshot.unwrap();
    let restored = context(
        &plan,
        &Value::Object(snapshot.variable_pool.clone()),
        turn.clone(),
        1,
    );
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome =
        crate::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot,
            "node-llm",
            &json!({"tool_results":[{"tool_call_id":"one-effect","content":"external result"}]}),
            restored.clone(),
            &provider,
            &lifecycle,
        )
        .await
        .unwrap();
    finish(&plan, outcome, &restored, &provider, &lifecycle).await;
    let state = turn.state.lock().unwrap();
    assert_eq!(state.replies.len(), 1);
    assert!(
        state.replies[0].contains("external result")
            && state.replies[0].contains("during callback wait")
    );
    assert_eq!(state.consumed, 2);
}

#[tokio::test]
async fn communication_fresh_human_resume_without_late_input_can_seal_existing_answer() {
    let plan = base_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let waiting = start_flow_debug_run_with_runtime_context(
        &plan,
        &input,
        context(&plan, &input, turn.clone(), 1),
        &provider,
    )
    .await
    .unwrap();
    assert!(matches!(
        waiting.stop_reason,
        ExecutionStopReason::WaitingHuman(_)
    ));
    let snapshot = waiting.checkpoint_snapshot.unwrap();
    let restored = context(
        &plan,
        &Value::Object(snapshot.variable_pool.clone()),
        turn.clone(),
        1,
    );
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome =
        crate::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot,
            "node-human",
            &json!({"input":"human approved"}),
            restored.clone(),
            &provider,
            &lifecycle,
        )
        .await
        .unwrap();
    finish(&plan, outcome, &restored, &provider, &lifecycle).await;
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
    assert_eq!(turn.state.lock().unwrap().replies, vec!["human approved"]);
}

#[tokio::test]
async fn communication_human_only_workflow_acknowledges_its_initial_prompt_prefix() {
    let mut plan = base_plan();
    plan.nodes.remove("node-llm");
    plan.topological_order.retain(|id| id != "node-llm");
    plan.nodes
        .get_mut("node-start")
        .unwrap()
        .downstream_node_ids = vec!["node-human".into()];
    let human = plan.nodes.get_mut("node-human").unwrap();
    human.dependency_node_ids = vec!["node-start".into()];
    human.bindings.get_mut("prompt").unwrap().raw_value = json!("Approve {{ node-start.query }}");
    human.bindings.get_mut("prompt").unwrap().selector_paths =
        vec![vec!["node-start".into(), "query".into()]];
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let waiting = start_flow_debug_run_with_runtime_context(
        &plan,
        &input,
        context(&plan, &input, turn.clone(), 1),
        &provider,
    )
    .await
    .unwrap();
    let snapshot = waiting.checkpoint_snapshot.unwrap();
    let restored = context(
        &plan,
        &Value::Object(snapshot.variable_pool.clone()),
        turn.clone(),
        1,
    );
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome =
        crate::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot,
            "node-human",
            &json!({"input":"human answer"}),
            restored.clone(),
            &provider,
            &lifecycle,
        )
        .await
        .unwrap();
    finish(&plan, outcome, &restored, &provider, &lifecycle).await;
    let state = turn.state.lock().unwrap();
    assert_eq!(state.consumed, 1);
    assert_eq!(state.replies, vec!["human answer"]);
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn communication_response_llm_continues_across_fresh_human_resume_without_graph_replay() {
    let mut plan = base_plan();
    // Human approval gates delivery; the Answer still presents the response LLM.
    plan.nodes.get_mut("node-answer").unwrap().bindings =
        llm_answer_plan().nodes["node-answer"].bindings.clone();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let waiting = start_flow_debug_run_with_runtime_context(
        &plan,
        &input,
        context(&plan, &input, turn.clone(), 1),
        &provider,
    )
    .await
    .unwrap();
    assert!(matches!(
        waiting.stop_reason,
        ExecutionStopReason::WaitingHuman(_)
    ));
    let snapshot = waiting.checkpoint_snapshot.unwrap();
    turn.append("supplement during approval");
    let restored = context(
        &plan,
        &Value::Object(snapshot.variable_pool.clone()),
        turn.clone(),
        1,
    );
    let lifecycle = RecordingExecutionLifecycle::default();
    let outcome =
        crate::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot,
            "node-human",
            &json!({"input":"approved"}),
            restored.clone(),
            &provider,
            &lifecycle,
        )
        .await
        .unwrap();
    let final_outcome = finish(&plan, outcome, &restored, &provider, &lifecycle).await;
    assert!(matches!(
        final_outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 2);
    assert!(!lifecycle.inputs.lock().unwrap().contains_key("node-human"));
    let state = turn.state.lock().unwrap();
    assert_eq!(state.replies.len(), 1);
    assert!(
        state.replies[0].contains("original")
            && state.replies[0].contains("supplement during approval")
    );
}

#[tokio::test]
async fn communication_non_refreshable_answer_fails_without_consuming_late_input() {
    let plan = base_plan();
    let turn = Turn::new();
    turn.append("original");
    let input = json!({"node-start":{"query":"original"}});
    let provider = CommunicationProvider::new(turn.clone(), Mode::Text, false);
    let waiting = start_flow_debug_run_with_runtime_context(
        &plan,
        &input,
        context(&plan, &input, turn.clone(), 1),
        &provider,
    )
    .await
    .unwrap();
    let snapshot = waiting.checkpoint_snapshot.unwrap();
    turn.append("late after completed human-separated LLM");
    let restored = context(
        &plan,
        &Value::Object(snapshot.variable_pool.clone()),
        turn.clone(),
        1,
    );
    let lifecycle = RecordingExecutionLifecycle::default();
    let mut outcome =
        crate::execution_engine::resume_flow_debug_run_with_runtime_context_and_lifecycle(
            &plan,
            &snapshot,
            "node-human",
            &json!({"input":"human approved"}),
            restored.clone(),
            &provider,
            &lifecycle,
        )
        .await
        .unwrap();
    restored
        .communication_execution()
        .unwrap()
        .restore_retained_traces(&plan, &mut outcome)
        .await
        .unwrap();
    restored
        .communication_execution()
        .unwrap()
        .checkpoint_graph(&outcome)
        .await
        .unwrap();
    assert_eq!(
        restored
            .communication_execution()
            .unwrap()
            .seal("human approved")
            .await
            .unwrap(),
        CommunicationExecutionSeal::Pending
    );
    let error = continue_communication_reply(&plan, outcome, &restored, &provider, &lifecycle)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("communication_answer_not_refreshable_from_retained_llm"));
    let state = turn.state.lock().unwrap();
    assert_eq!(state.consumed, 1);
    assert_eq!(state.inputs.len(), 2);
    assert!(state.replies.is_empty());
    assert_eq!(provider.main_calls.load(Ordering::SeqCst), 1);
}
