use super::*;
use crate::execution_engine::*;
use uuid::Uuid;

pub(crate) struct RootLifecycle<'a> {
    pub session: Session,
    pub owner: &'a dyn ExecutionLifecycle,
    pub current_identity: CurrentIdentity,
}

impl Session {
    pub async fn advance_root(
        &self,
        index: usize,
        pool: &Map<String, Value>,
        active: &BTreeSet<String>,
        lifecycle: &dyn ExecutionLifecycle,
    ) -> Result<()> {
        let mut state = self.state.lock().await;
        state.root_next_node_index = index;
        state.root_variable_pool.clone_from(pool);
        state.root_active_node_ids = active.iter().cloned().collect();
        // An advance after a completed node carries its real execution identity.
        // The very first admission is committed together with NodeStarted.
        let identity = state
            .root_nodes
            .values()
            .find(|node| node.terminal_trace.is_some())
            .map(|node| node.identity.clone())
            .or_else(|| {
                state
                    .frames
                    .values()
                    .find(|frame| frame.identity.parent_execution_id.is_none())
                    .map(|frame| frame.identity.clone())
            });
        if let Some(identity) = identity {
            emit(
                lifecycle,
                &state,
                ContainerTransitionKind::ScopeAdvanced,
                &identity,
                None,
                None,
                Some(&json!({"root":true,"next_node_index":index})),
            )
            .await?;
        }
        Ok(())
    }
    pub async fn cached_root_trace(&self, node_id: &str) -> Option<NodeExecutionTrace> {
        self.state
            .lock()
            .await
            .root_nodes
            .get(node_id)?
            .terminal_trace
            .clone()
    }
    pub async fn uncertain_root(&self, node_id: &str) -> bool {
        self.state
            .lock()
            .await
            .root_nodes
            .get(node_id)
            .is_some_and(|node| {
                node.terminal_trace.is_none()
                    && self.recovered_nodes.contains(&node.identity.execution_id)
            })
    }
}

impl RootLifecycle<'_> {
    async fn identity(&self, node_id: &str) -> Result<ScopedExecutionIdentity> {
        self.session
            .state
            .lock()
            .await
            .root_nodes
            .get(node_id)
            .map(|node| node.identity.clone())
            .ok_or_else(|| anyhow!("root execution missing"))
    }
    async fn commit_trace(
        &self,
        trace: &NodeExecutionTrace,
        pool: Option<&Map<String, Value>>,
    ) -> Result<()> {
        let mut state = self.session.state.lock().await;
        if let Some(pool) = pool {
            state.root_variable_pool.clone_from(pool);
        }
        let node = state
            .root_nodes
            .get_mut(&trace.node_id)
            .ok_or_else(|| anyhow!("root terminal without start"))?;
        if node.terminal_trace.is_some() {
            return Ok(());
        }
        let identity = node.identity.clone();
        let mut trace = trace.clone();
        trace.execution_identity = Some(identity.clone());
        node.terminal_trace = Some(trace.clone());
        emit(
            self.owner,
            &state,
            ContainerTransitionKind::NodeCompleted,
            &identity,
            None,
            Some(&trace),
            None,
        )
        .await?;
        leave_identity(&self.current_identity, &identity)?;
        Ok(())
    }
}

#[async_trait]
impl ExecutionLifecycle for RootLifecycle<'_> {
    async fn commit_container_transition(
        &self,
        transition: &ContainerTransition<'_>,
    ) -> Result<()> {
        self.owner.commit_container_transition(transition).await
    }
    async fn scoped_node_receipt(
        &self,
        identity: &ScopedExecutionIdentity,
    ) -> Result<Option<Uuid>> {
        self.owner.scoped_node_receipt(identity).await
    }
    async fn begin_node(&self, node: &CompiledNode, input: &Value) -> Result<()> {
        let mut state = self.session.state.lock().await;
        let existing = state.root_nodes.contains_key(&node.node_id);
        let identity = state
            .root_nodes
            .entry(node.node_id.clone())
            .or_insert_with(|| ScopedNodeState {
                identity: ScopedExecutionIdentity {
                    execution_id: Uuid::now_v7(),
                    node_id: node.node_id.clone(),
                    parent_execution_id: None,
                    round_path: Vec::new(),
                },
                input_payload: input.clone(),
                terminal_trace: None,
            })
            .identity
            .clone();
        enter_identity(&self.current_identity, &identity)?;
        emit(
            self.owner,
            &state,
            if existing {
                ContainerTransitionKind::Resumed
            } else {
                ContainerTransitionKind::NodeStarted
            },
            &identity,
            Some(input),
            None,
            Some(&json!({"node_type":node.node_type,"node_alias":node.alias,"root":true})),
        )
        .await
    }
    async fn complete_node(&self, trace: &NodeExecutionTrace) -> Result<()> {
        self.commit_trace(trace, None).await
    }
    async fn complete_node_with_state(
        &self,
        trace: &NodeExecutionTrace,
        pool: &Map<String, Value>,
    ) -> Result<()> {
        self.commit_trace(trace, Some(pool)).await
    }
    async fn communication_node_receipt(&self, node: &CompiledNode) -> Result<Option<Uuid>> {
        self.owner
            .scoped_node_receipt(&self.identity(&node.node_id).await?)
            .await
    }
    async fn activate_retained_communication_node(&self, node: &CompiledNode) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        enter_identity(&self.current_identity, &identity)?;
        let state = self.session.state.lock().await;
        emit(
            self.owner,
            &state,
            ContainerTransitionKind::Resumed,
            &identity,
            None,
            None,
            None,
        )
        .await
    }
    async fn runtime_internal_tool_started(
        &self,
        node: &CompiledNode,
        call: &Value,
        usage: &Value,
    ) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        let state = self.session.state.lock().await;
        emit(
            self.owner,
            &state,
            ContainerTransitionKind::InternalToolStarted,
            &identity,
            None,
            None,
            Some(&json!({"tool_call":call,"call_usage":usage})),
        )
        .await
    }
    async fn runtime_internal_tool_finished(
        &self,
        node: &CompiledNode,
        call: &Value,
        usage: &Value,
        result: &Value,
        duration_ms: u64,
    ) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        let state = self.session.state.lock().await;
        emit(self.owner,&state,ContainerTransitionKind::InternalToolFinished,&identity,None,None,Some(&json!({"tool_call":call,"call_usage":usage,"tool_result":result,"duration_ms":duration_ms}))).await
    }
}

pub(crate) async fn resume_root_wait<I>(
    plan: &CompiledPlan,
    mut state: ContainerCheckpoint,
    execution_id: Uuid,
    payload: &Value,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    let wait = state
        .root_waiting
        .take()
        .ok_or_else(|| anyhow!("root waiting execution missing"))?;
    let node = plan
        .nodes
        .get(wait.node_id())
        .ok_or_else(|| anyhow!("root waiting node missing"))?;
    let identity = state.root_nodes[&node.node_id].identity.clone();
    if identity.execution_id != execution_id {
        bail!("root waiting execution mismatch");
    }
    let terminal =
        if pending_llm_tool_callback_state(&state.root_variable_pool, &node.node_id).is_some() {
            append_llm_tool_result_messages(&mut state.root_variable_pool, &node.node_id, payload)?;
            None
        } else {
            let patch = payload
                .as_object()
                .ok_or_else(|| anyhow!("resume payload must be an object"))?;
            for key in patch.keys() {
                if !node.outputs.iter().any(|output| &output.key == key) {
                    bail!(
                        "resume payload key {key} is not a public output for {}",
                        node.node_id
                    );
                }
            }
            state
                .root_variable_pool
                .insert(node.node_id.clone(), payload.clone());
            Some(NodeExecutionTrace {
                execution_identity: Some(identity.clone()),
                node_id: node.node_id.clone(),
                node_type: node.node_type.clone(),
                node_alias: node.alias.clone(),
                input_payload: state.root_nodes[&node.node_id].input_payload.clone(),
                output_payload: payload.clone(),
                error_payload: None,
                metrics_payload: json!({"resumed":true}),
                debug_payload: json!({}),
                provider_events: Vec::new(),
            })
        };
    state
        .root_nodes
        .get_mut(&node.node_id)
        .expect("root waiting node")
        .terminal_trace = terminal.clone();
    state.waiting_execution_id = None;
    emit(
        lifecycle,
        &state,
        if terminal.is_some() {
            ContainerTransitionKind::NodeCompleted
        } else {
            ContainerTransitionKind::Resumed
        },
        &identity,
        Some(payload),
        terminal.as_ref(),
        Some(&json!({"root":true})),
    )
    .await?;
    let (index, pool, active) = (
        state.root_next_node_index,
        state.root_variable_pool.clone(),
        state.root_active_node_ids.iter().cloned().collect(),
    );
    let mut session = Session::new(state, true)?;
    Arc::make_mut(&mut session.recovered_nodes).remove(&execution_id);
    Box::pin(execute_root(
        plan,
        index,
        pool,
        Some(active),
        runtime,
        invoker,
        lifecycle,
        Some(session),
    ))
    .await
}

pub(crate) async fn recover_root_native<I>(
    plan: &CompiledPlan,
    checkpoint: &CheckpointSnapshot,
    mut state: ContainerCheckpoint,
    node_id: &str,
    runtime: &ExecutionRuntimeContext,
    invoker: &I,
    lifecycle: &dyn ExecutionLifecycle,
) -> Result<FlowDebugExecutionOutcome>
where
    I: ProviderInvoker + CapabilityInvoker + CodeInvoker + ?Sized,
{
    super::super::native_inference_recovery::validate_native_inference_recovery_scope(
        plan, node_id,
    )?;
    let identity = state
        .root_nodes
        .get(node_id)
        .ok_or_else(|| anyhow!("root recovery execution missing"))?
        .identity
        .clone();
    state
        .root_nodes
        .get_mut(node_id)
        .expect("root recovery node")
        .terminal_trace = None;
    state.root_variable_pool.remove(node_id);
    if let Some(grant) = checkpoint
        .variable_pool
        .get("sys")
        .and_then(|sys| sys.get("native_inference_recovery"))
    {
        state
            .root_variable_pool
            .entry("sys".to_owned())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("invalid root system scope"))?
            .insert("native_inference_recovery".to_owned(), grant.clone());
    }
    state.root_next_node_index = plan
        .topological_order
        .iter()
        .position(|id| id == node_id)
        .ok_or_else(|| anyhow!("root recovery node missing"))?;
    state.root_waiting = None;
    state.waiting_execution_id = None;
    emit(
        lifecycle,
        &state,
        ContainerTransitionKind::Resumed,
        &identity,
        None,
        None,
        Some(&json!({"root":true})),
    )
    .await?;
    let (index, pool, active) = (
        state.root_next_node_index,
        state.root_variable_pool.clone(),
        state.root_active_node_ids.iter().cloned().collect(),
    );
    let mut session = Session::new(state, true)?;
    Arc::make_mut(&mut session.recovered_nodes).remove(&identity.execution_id);
    Box::pin(execute_root(
        plan,
        index,
        pool,
        Some(active),
        runtime,
        invoker,
        lifecycle,
        Some(session),
    ))
    .await
}
