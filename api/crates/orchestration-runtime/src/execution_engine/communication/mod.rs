//! Optional durable input owner for one communication reply turn. No service/storage dependency.
use super::*;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

mod continuation;
pub(super) use continuation::allow_scheduled_node;
pub use continuation::continue_communication_reply;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicationExecutionInput {
    pub input_id: Uuid,
    pub input_sequence: i64,
    /// Host-prepared user messages with safe imported file references, never download secrets.
    pub messages: Vec<Value>,
}

pub struct CommunicationExecutionInputs {
    pub received_watermark: i64,
    pub consumed_watermark: i64,
    pub checkpoint: Option<Value>,
    pub inputs: Vec<CommunicationExecutionInput>,
    pub pending_admission: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationExecutionSeal {
    Reserved,
    Pending,
}

/// Drop of this host-owned lease acknowledges that the owned execution future exited.
pub trait CommunicationExecutionLease: Send {}
impl<T: Send> CommunicationExecutionLease for T {}

#[async_trait]
pub trait CommunicationExecutionActivity: Send + Sync {
    /// Atomically register activity, rejecting an already cancelled session epoch.
    async fn enter(
        &self,
        session_id: Uuid,
        session_epoch: i64,
    ) -> Result<Box<dyn CommunicationExecutionLease>>;
    async fn cancelled(&self, session_id: Uuid, session_epoch: i64);
}

#[async_trait]
pub trait CommunicationExecutionPort: Send + Sync {
    async fn inspect(&self) -> Result<CommunicationExecutionInputs>;
    /// Commits this versioned continuation and its input acknowledgement atomically.
    async fn checkpoint(&self, consumed_watermark: i64, continuation: Value) -> Result<()>;
    async fn check_active(&self) -> Result<()>;
    /// Fencing is an error. Reserved alone grants one provider send to the host owner.
    async fn seal(&self, final_reply: &str) -> Result<CommunicationExecutionSeal>;
    async fn enter_execution(&self) -> Result<Box<dyn CommunicationExecutionLease>> {
        Ok(Box::new(()))
    }
    async fn cancelled(&self) {
        loop {
            if self.check_active().await.is_err() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}

const VERSION: u32 = 1;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Continuation {
    version: u32,
    consumed_watermark: i64,
    main_node_id: Option<String>,
    accepted_inputs: Vec<CommunicationExecutionInput>,
    node_watermarks: BTreeMap<String, i64>,
    llm_states: BTreeMap<String, Value>,
    node_receipts: BTreeMap<String, Uuid>,
    retained_llms: BTreeMap<String, RetainedLlm>,
    /// Completed graph values only. Actor, env, protocol credentials and provider pins
    /// are reconstructed by their existing owners; they never enter this checkpoint.
    completed_nodes: Map<String, Value>,
    boundary: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct RetainedLlm {
    node_id: String,
    node_alias: String,
    output_payload: Value,
    metrics_payload: Value,
    tool_evidence: Value,
    provider_events: Vec<ProviderStreamEvent>,
}

impl RetainedLlm {
    fn trace(&self) -> NodeExecutionTrace {
        NodeExecutionTrace {
            execution_identity: None,
            node_id: self.node_id.clone(),
            node_type: "llm".into(),
            node_alias: self.node_alias.clone(),
            input_payload: json!({}),
            output_payload: self.output_payload.clone(),
            error_payload: None,
            metrics_payload: self.metrics_payload.clone(),
            debug_payload: self.tool_evidence.clone(),
            provider_events: self.provider_events.clone(),
        }
    }
}

struct State {
    initialized: bool,
    continuation: Continuation,
}

pub struct CommunicationExecution {
    port: Arc<dyn CommunicationExecutionPort>,
    initial_prompt_watermark: i64,
    state: Mutex<State>,
}

#[derive(Debug)]
pub(super) struct CommunicationCancelled(anyhow::Error);
impl std::fmt::Display for CommunicationCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
impl std::error::Error for CommunicationCancelled {}

impl CommunicationExecution {
    pub async fn enter_execution(&self) -> Result<Box<dyn CommunicationExecutionLease>> {
        self.port.enter_execution().await
    }

    pub async fn run_until_cancelled<T>(
        &self,
        execution: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T> {
        tokio::pin!(execution);
        tokio::select! {
            biased;
            _ = self.port.cancelled() => Err(anyhow!("communication_execution_cancelled")),
            result = &mut execution => result,
        }
    }
    /// The declared prefix must already occur exactly once in the host's initial prompt.
    /// Use zero when the port supplies the entire initial batch.
    pub fn new(
        port: Arc<dyn CommunicationExecutionPort>,
        initial_prompt_watermark: i64,
    ) -> Result<Self> {
        if initial_prompt_watermark < 0 {
            bail!("communication_initial_watermark_invalid");
        }
        Ok(Self {
            port,
            initial_prompt_watermark,
            state: Mutex::new(State {
                initialized: false,
                continuation: Continuation {
                    version: VERSION,
                    ..Continuation::default()
                },
            }),
        })
    }

    pub async fn check_active(&self) -> Result<()> {
        self.port
            .check_active()
            .await
            .map_err(|error| CommunicationCancelled(error).into())
    }

    async fn inspect_ready(&self) -> Result<CommunicationExecutionInputs> {
        loop {
            let inputs = self.port.inspect().await?;
            if !inputs.pending_admission {
                return Ok(inputs);
            }
            // Classification and control commands run in the host's independent lane.
            // Never seal or start a new request past an unresolved command receipt.
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }

    async fn initialize(
        &self,
        state: &mut State,
        inputs: &CommunicationExecutionInputs,
    ) -> Result<()> {
        if state.initialized {
            return Ok(());
        }
        let main = state.continuation.main_node_id.clone();
        if let Some(checkpoint) = &inputs.checkpoint {
            let restored: Continuation = serde_json::from_value(checkpoint.clone())
                .map_err(|_| anyhow!("communication_continuation_invalid"))?;
            if restored.version != VERSION
                || restored.consumed_watermark != inputs.consumed_watermark
            {
                bail!("communication_continuation_version_or_watermark_invalid");
            }
            state.continuation = restored;
        } else if inputs.consumed_watermark != 0 {
            bail!("communication_consumption_without_continuation");
        }
        if main.is_some() {
            state.continuation.main_node_id = main;
        }
        if self.initial_prompt_watermark > inputs.received_watermark {
            bail!("communication_initial_watermark_invalid");
        }
        state.initialized = true;
        Ok(())
    }

    pub(super) async fn activate_node(&self, node: &CompiledNode) -> Result<()> {
        self.check_active().await?;
        self.state.lock().await.continuation.main_node_id = Some(node.node_id.clone());
        Ok(())
    }

    pub(super) async fn record_node_receipt(&self, node: &CompiledNode, id: Option<Uuid>) {
        if let Some(id) = id {
            self.state
                .lock()
                .await
                .continuation
                .node_receipts
                .insert(node.node_id.clone(), id);
        }
    }

    pub(super) async fn retain_trace(&self, trace: &NodeExecutionTrace) -> Result<()> {
        let tool_evidence = [
            "runtime_internal_tool_events",
            "visible_internal_llm_tool_events",
        ]
        .into_iter()
        .filter_map(|key| {
            trace
                .debug_payload
                .get(key)
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect::<Map<String, Value>>();
        let mut state = self.state.lock().await;
        state.continuation.retained_llms.insert(
            trace.node_id.clone(),
            RetainedLlm {
                node_id: trace.node_id.clone(),
                node_alias: trace.node_alias.clone(),
                output_payload: trace.output_payload.clone(),
                metrics_payload: trace.metrics_payload.clone(),
                tool_evidence: Value::Object(tool_evidence),
                provider_events: trace.provider_events.clone(),
            },
        );
        state.continuation.boundary = "retained_llm".into();
        self.persist(&state).await
    }

    pub async fn retained_node_receipts(&self) -> Result<BTreeMap<String, Uuid>> {
        let inputs = self.inspect_ready().await?;
        let mut state = self.state.lock().await;
        self.initialize(&mut state, &inputs).await?;
        Ok(state.continuation.node_receipts.clone())
    }

    /// A fresh human/callback segment keeps completed graph actions in its existing
    /// checkpoint, while these unfinished LLM receipts remain owned until turn seal.
    pub async fn restore_retained_traces(
        &self,
        plan: &CompiledPlan,
        outcome: &mut FlowDebugExecutionOutcome,
    ) -> Result<()> {
        let inputs = self.inspect_ready().await?;
        let mut state = self.state.lock().await;
        self.initialize(&mut state, &inputs).await?;
        let mut retained = Vec::new();
        for node_id in &plan.topological_order {
            if outcome
                .node_traces
                .iter()
                .any(|trace| &trace.node_id == node_id)
            {
                continue;
            }
            if let Some(llm) = state.continuation.retained_llms.get(node_id) {
                outcome
                    .variable_pool
                    .entry(node_id.clone())
                    .or_insert_with(|| llm.output_payload.clone());
                retained.push(llm.trace());
            }
        }
        retained.append(&mut outcome.node_traces);
        outcome.node_traces = retained;
        Ok(())
    }

    pub(super) async fn is_main(&self, node: &CompiledNode) -> bool {
        node.container_id.is_none()
            && self.state.lock().await.continuation.main_node_id.as_deref()
                == Some(node.node_id.as_str())
    }

    async fn persist(&self, state: &State) -> Result<()> {
        self.port
            .checkpoint(
                state.continuation.consumed_watermark,
                serde_json::to_value(&state.continuation)?,
            )
            .await
    }

    pub(super) async fn prepare_round(
        &self,
        node: &CompiledNode,
        pool: &Map<String, Value>,
        transcript: &LlmToolPromptTranscript,
    ) -> Result<Map<String, Value>> {
        let inputs = self.inspect_ready().await?;
        let mut state = self.state.lock().await;
        self.initialize(&mut state, &inputs).await?;
        if inputs.consumed_watermark != state.continuation.consumed_watermark {
            bail!("communication_checkpoint_owner_changed");
        }
        for input in inputs.inputs {
            if input.input_sequence != state.continuation.consumed_watermark + 1
                || input.messages.is_empty()
                || input
                    .messages
                    .iter()
                    .any(|message| message.get("role").and_then(Value::as_str) != Some("user"))
            {
                bail!("communication_input_sequence_or_messages_invalid");
            }
            state.continuation.consumed_watermark = input.input_sequence;
            state.continuation.accepted_inputs.push(input);
        }
        if state.continuation.consumed_watermark != inputs.received_watermark {
            bail!("communication_input_batch_incomplete");
        }
        let mut prepared = if pending_llm_tool_callback_state(pool, &node.node_id).is_some() {
            pool.clone()
        } else {
            variable_pool_with_pending_llm_tool_callback(
                node,
                pool,
                &json!({}),
                serde_json::to_value(&transcript.system)?,
                transcript.messages.clone(),
            )
        };
        let applied = state
            .continuation
            .node_watermarks
            .get(&node.node_id)
            .copied()
            .unwrap_or(self.initial_prompt_watermark);
        let appended = state
            .continuation
            .accepted_inputs
            .iter()
            .filter(|input| input.input_sequence > applied)
            .flat_map(|input| input.messages.iter().cloned())
            .collect::<Vec<_>>();
        let callback = prepared
            .get_mut(&node.node_id)
            .and_then(|node| node.get_mut(LLM_TOOL_CALLBACK_STATE_KEY))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| anyhow!("communication_transcript_missing"))?;
        if !appended.is_empty() {
            callback
                .get_mut("history")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| anyhow!("communication_history_missing"))?
                .extend(appended);
            // A tool-only websocket delta cannot represent a new user message. Preserve
            // its provenance, but use the complete semantic transcript for this request.
            callback.remove("delta_messages");
        }
        let consumed = state.continuation.consumed_watermark;
        state
            .continuation
            .node_watermarks
            .insert(node.node_id.clone(), consumed);
        state
            .continuation
            .llm_states
            .insert(node.node_id.clone(), Value::Object(callback.clone()));
        state.continuation.boundary = "before_provider".into();
        self.persist(&state).await?;
        Ok(prepared)
    }

    pub(super) async fn retain_response(
        &self,
        node: &CompiledNode,
        pool: &Map<String, Value>,
        transcript: &LlmToolPromptTranscript,
        output: &Value,
    ) -> Result<()> {
        self.check_active().await?;
        let retained = variable_pool_with_pending_llm_tool_callback(
            node,
            pool,
            output,
            serde_json::to_value(&transcript.system)?,
            llm_callback_history_after_assistant_tool_call(transcript, output),
        );
        self.retain_pool(node, &retained, "after_provider").await
    }

    pub(super) async fn retain_pool(
        &self,
        node: &CompiledNode,
        pool: &Map<String, Value>,
        boundary: &str,
    ) -> Result<()> {
        if !self.is_main(node).await {
            return self.check_active().await;
        }
        let callback = pending_llm_tool_callback_state(pool, &node.node_id)
            .ok_or_else(|| anyhow!("communication_transcript_missing"))?;
        let mut state = self.state.lock().await;
        state
            .continuation
            .llm_states
            .insert(node.node_id.clone(), Value::Object(callback.clone()));
        state.continuation.boundary = boundary.into();
        self.persist(&state).await
    }

    pub(super) async fn retained_pool(
        &self,
        node: &CompiledNode,
        pool: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        let state = self.state.lock().await;
        let retained = state
            .continuation
            .llm_states
            .get(&node.node_id)
            .ok_or_else(|| anyhow!("communication_continuation_missing"))?;
        let mut pool = pool.clone();
        pool.insert(
            node.node_id.clone(),
            json!({LLM_TOOL_CALLBACK_STATE_KEY: retained}),
        );
        Ok(pool)
    }

    pub(super) async fn has_new_inputs(&self) -> Result<bool> {
        let inputs = self.inspect_ready().await?;
        let state = self.state.lock().await;
        if inputs.consumed_watermark != state.continuation.consumed_watermark {
            bail!("communication_checkpoint_owner_changed");
        }
        Ok(inputs.received_watermark > state.continuation.consumed_watermark)
    }

    pub async fn checkpoint_graph(&self, outcome: &FlowDebugExecutionOutcome) -> Result<()> {
        self.check_active().await?;
        let inputs = self.inspect_ready().await?;
        let mut state = self.state.lock().await;
        self.initialize(&mut state, &inputs).await?;
        if state.continuation.consumed_watermark != inputs.consumed_watermark {
            bail!("communication_checkpoint_owner_changed");
        }
        // A human/tool-only graph can consume the declared initial prompt prefix without
        // invoking a model. Later inputs remain pending until a response LLM can use them.
        if state.continuation.main_node_id.is_none() {
            for input in inputs
                .inputs
                .into_iter()
                .take_while(|input| input.input_sequence <= self.initial_prompt_watermark)
            {
                if input.input_sequence != state.continuation.consumed_watermark + 1 {
                    bail!("communication_initial_watermark_invalid");
                }
                state.continuation.consumed_watermark = input.input_sequence;
                state.continuation.accepted_inputs.push(input);
            }
        }
        for trace in &outcome.node_traces {
            if trace.error_payload.is_none() {
                if let Some(value) = outcome.variable_pool.get(&trace.node_id) {
                    state
                        .continuation
                        .completed_nodes
                        .insert(trace.node_id.clone(), value.clone());
                }
            }
        }
        state.continuation.boundary = "graph_candidate".into();
        self.persist(&state).await
    }

    pub async fn seal(&self, final_reply: &str) -> Result<CommunicationExecutionSeal> {
        self.port.seal(final_reply).await
    }
}
