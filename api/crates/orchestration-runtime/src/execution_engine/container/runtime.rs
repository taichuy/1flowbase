use super::*;
use crate::execution_engine::*;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Session {
    pub state: Arc<Mutex<ContainerCheckpoint>>,
    pub permits: Arc<Semaphore>,
    pub recovered_nodes: Arc<BTreeSet<Uuid>>,
}

#[derive(Clone)]
pub(crate) struct Scope {
    pub session: Session,
    pub frame_id: Uuid,
    pub round_index: u64,
    pub expires_at: Option<tokio::time::Instant>,
    pub current_identity: CurrentIdentity,
}

#[derive(Debug)]
pub(crate) struct CommitFailure(pub String);
impl std::fmt::Display for CommitFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "container state commit failed: {}", self.0)
    }
}
impl std::error::Error for CommitFailure {}

#[derive(Debug)]
pub(crate) struct GlobalFailure(pub Value);
impl std::fmt::Display for GlobalFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "container global failure: {}", self.0)
    }
}
impl std::error::Error for GlobalFailure {}

#[derive(Debug)]
pub(crate) struct DeadlineExpired(pub Uuid);
impl std::fmt::Display for DeadlineExpired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "container deadline expired: {}", self.0)
    }
}
impl std::error::Error for DeadlineExpired {}

pub(crate) async fn emit(
    lifecycle: &dyn ExecutionLifecycle,
    state: &ContainerCheckpoint,
    kind: ContainerTransitionKind,
    identity: &ScopedExecutionIdentity,
    input_payload: Option<&Value>,
    trace: Option<&NodeExecutionTrace>,
    detail: Option<&Value>,
) -> Result<()> {
    lifecycle
        .commit_container_transition(&ContainerTransition {
            kind,
            identity,
            checkpoint: state,
            input_payload,
            trace,
            detail,
        })
        .await
        .map_err(|error| {
            if error.is::<DeadlineExpired>() || error.is::<GlobalFailure>() {
                error
            } else {
                anyhow::Error::new(CommitFailure(error.to_string()))
            }
        })
}

impl Session {
    pub fn new(state: ContainerCheckpoint, recovering: bool) -> Result<Self> {
        if state.version != 1
            || state.execution_budget == 0
            || state.execution_budget > Semaphore::MAX_PERMITS
        {
            bail!("invalid container checkpoint version or execution budget");
        }
        let mut recovered_nodes = BTreeSet::new();
        if recovering {
            recovered_nodes.extend(
                state
                    .root_nodes
                    .values()
                    .map(|node| node.identity.execution_id),
            );
        }
        for frame in state.frames.values() {
            if frame.started_rounds > frame.frozen_limit
                || frame.rounds.len() as u64 > frame.started_rounds
                || frame
                    .rounds
                    .keys()
                    .any(|index| *index >= frame.started_rounds)
            {
                bail!("invalid container checkpoint round budget");
            }
            for round in frame.rounds.values() {
                if round.deadline_unix_ms.is_some() != round.deadline_owner_execution_id.is_some() {
                    bail!("container checkpoint deadline and owner must both be present or absent");
                }
            }
            if recovering {
                for round in frame.rounds.values() {
                    recovered_nodes
                        .extend(round.nodes.values().map(|node| node.identity.execution_id));
                }
            }
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(state.execution_budget)),
            state: Arc::new(Mutex::new(state)),
            recovered_nodes: Arc::new(recovered_nodes),
        })
    }
}

impl Scope {
    pub async fn new(
        session: Session,
        frame_id: Uuid,
        round_index: u64,
        ancestor_deadline: Option<tokio::time::Instant>,
    ) -> Result<Self> {
        let deadline = {
            let state = session.state.lock().await;
            state.frames[&frame_id].rounds[&round_index].deadline_unix_ms
        };
        let own = deadline
            .map(|deadline| {
                let remaining = deadline.saturating_sub(unix_ms()).max(0) as u64;
                tokio::time::Instant::now()
                    .checked_add(std::time::Duration::from_millis(remaining))
                    .ok_or_else(|| anyhow!("container monotonic deadline overflow"))
            })
            .transpose()?;
        Ok(Self {
            session,
            frame_id,
            round_index,
            expires_at: own.into_iter().chain(ancestor_deadline).min(),
            current_identity: Arc::default(),
        })
    }
    pub fn expired(&self) -> bool {
        self.expires_at
            .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
    }
    pub fn remaining_ms(&self) -> Option<u64> {
        self.expires_at.map(|deadline| {
            deadline
                .saturating_duration_since(tokio::time::Instant::now())
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX)
        })
    }

    pub async fn advance(
        &self,
        index: usize,
        pool: &Map<String, Value>,
        active: &BTreeSet<String>,
        lifecycle: &dyn ExecutionLifecycle,
    ) -> Result<()> {
        let mut state = self.session.state.lock().await;
        let frame = state
            .frames
            .get_mut(&self.frame_id)
            .ok_or_else(|| anyhow!("container frame missing"))?;
        let identity = frame.identity.clone();
        let round = frame
            .rounds
            .get_mut(&self.round_index)
            .ok_or_else(|| anyhow!("container round missing"))?;
        round.next_node_index = index;
        round.variable_pool.clone_from(pool);
        round.active_node_ids = active.iter().cloned().collect();
        emit(
            lifecycle,
            &state,
            ContainerTransitionKind::ScopeAdvanced,
            &identity,
            None,
            None,
            Some(&json!({"round_index": self.round_index})),
        )
        .await
    }

    pub async fn cached_trace(&self, node_id: &str) -> Option<NodeExecutionTrace> {
        let state = self.session.state.lock().await;
        let node = state
            .frames
            .get(&self.frame_id)?
            .rounds
            .get(&self.round_index)?
            .nodes
            .get(node_id)?;
        // A global container failure must re-enter its frame boundary; ordinary
        // cached-node error policies cannot turn it into a parent-local failure.
        if state
            .frames
            .get(&node.identity.execution_id)
            .is_some_and(|frame| frame.global_failure.is_some())
        {
            return None;
        }
        node.terminal_trace.clone()
    }

    pub async fn uncertain(&self, node_id: &str) -> bool {
        let state = self.session.state.lock().await;
        state
            .frames
            .get(&self.frame_id)
            .and_then(|f| f.rounds.get(&self.round_index))
            .and_then(|r| r.nodes.get(node_id))
            .is_some_and(|n| {
                n.terminal_trace.is_none()
                    && self
                        .session
                        .recovered_nodes
                        .contains(&n.identity.execution_id)
            })
    }

    pub async fn deadline(&self) -> Option<(i64, Uuid)> {
        let state = self.session.state.lock().await;
        let round = &state.frames[&self.frame_id].rounds[&self.round_index];
        round
            .deadline_unix_ms
            .zip(round.deadline_owner_execution_id)
    }
}

pub(crate) struct ScopedLifecycle<'a> {
    pub scope: Scope,
    pub owner: &'a dyn ExecutionLifecycle,
}

impl ScopedLifecycle<'_> {
    async fn identity(&self, node_id: &str) -> Result<ScopedExecutionIdentity> {
        let state = self.scope.session.state.lock().await;
        Ok(
            state.frames[&self.scope.frame_id].rounds[&self.scope.round_index]
                .nodes
                .get(node_id)
                .ok_or_else(|| anyhow!("scoped node was not started"))?
                .identity
                .clone(),
        )
    }
}

#[async_trait]
impl ExecutionLifecycle for ScopedLifecycle<'_> {
    async fn commit_container_transition(
        &self,
        transition: &ContainerTransition<'_>,
    ) -> Result<()> {
        self.owner.commit_container_transition(transition).await
    }

    async fn begin_node(&self, node: &CompiledNode, input_payload: &Value) -> Result<()> {
        let mut state = self.scope.session.state.lock().await;
        let frame = state
            .frames
            .get_mut(&self.scope.frame_id)
            .ok_or_else(|| anyhow!("container frame missing"))?;
        let mut path = frame.identity.round_path.clone();
        path.push(ContainerRoundPath {
            container_execution_id: frame.identity.execution_id,
            container_node_id: frame.identity.node_id.clone(),
            round_index: self.scope.round_index,
        });
        let round = frame
            .rounds
            .get_mut(&self.scope.round_index)
            .ok_or_else(|| anyhow!("container round missing"))?;
        if self.scope.expired() {
            return Err(DeadlineExpired(
                round
                    .deadline_owner_execution_id
                    .ok_or_else(|| anyhow!("finite deadline missing owner"))?,
            )
            .into());
        }
        let existing = round.nodes.contains_key(&node.node_id);
        let identity = round
            .nodes
            .entry(node.node_id.clone())
            .or_insert_with(|| ScopedNodeState {
                identity: ScopedExecutionIdentity {
                    execution_id: Uuid::now_v7(),
                    node_id: node.node_id.clone(),
                    parent_execution_id: Some(self.scope.frame_id),
                    round_path: path,
                },
                input_payload: input_payload.clone(),
                terminal_trace: None,
            })
            .identity
            .clone();
        enter_identity(&self.scope.current_identity, &identity)?;
        let admitted = emit(
            self.owner,
            &state,
            if existing {
                ContainerTransitionKind::Resumed
            } else {
                ContainerTransitionKind::NodeStarted
            },
            &identity,
            Some(input_payload),
            None,
            Some(&json!({"node_type": node.node_type, "node_alias": node.alias})),
        )
        .await;
        if admitted
            .as_ref()
            .is_err_and(|error| error.is::<DeadlineExpired>())
            && !existing
        {
            state
                .frames
                .get_mut(&self.scope.frame_id)
                .expect("scope frame")
                .rounds
                .get_mut(&self.scope.round_index)
                .expect("scope round")
                .nodes
                .remove(&node.node_id);
            leave_identity(&self.scope.current_identity, &identity)?;
        }
        admitted
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
    async fn scoped_node_receipt(
        &self,
        identity: &ScopedExecutionIdentity,
    ) -> Result<Option<Uuid>> {
        self.owner.scoped_node_receipt(identity).await
    }

    async fn communication_node_receipt(&self, node: &CompiledNode) -> Result<Option<Uuid>> {
        self.owner
            .scoped_node_receipt(&self.identity(&node.node_id).await?)
            .await
    }

    async fn activate_retained_communication_node(&self, node: &CompiledNode) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        enter_identity(&self.scope.current_identity, &identity)?;
        let state = self.scope.session.state.lock().await;
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
        tool_call: &Value,
        call_usage: &Value,
    ) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        let detail = json!({"tool_call": tool_call, "call_usage": call_usage});
        let state = self.scope.session.state.lock().await;
        emit(
            self.owner,
            &state,
            ContainerTransitionKind::InternalToolStarted,
            &identity,
            None,
            None,
            Some(&detail),
        )
        .await
    }

    async fn runtime_internal_tool_finished(
        &self,
        node: &CompiledNode,
        tool_call: &Value,
        call_usage: &Value,
        tool_result: &Value,
        duration_ms: u64,
    ) -> Result<()> {
        let identity = self.identity(&node.node_id).await?;
        let detail = json!({"tool_call": tool_call, "call_usage": call_usage, "tool_result": tool_result, "duration_ms": duration_ms});
        let state = self.scope.session.state.lock().await;
        emit(
            self.owner,
            &state,
            ContainerTransitionKind::InternalToolFinished,
            &identity,
            None,
            None,
            Some(&detail),
        )
        .await
    }
}

pub(crate) fn unix_ms() -> i64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .try_into()
        .unwrap_or(i64::MAX)
}

impl ScopedLifecycle<'_> {
    async fn commit_trace(
        &self,
        trace: &NodeExecutionTrace,
        variable_pool: Option<&Map<String, Value>>,
    ) -> Result<()> {
        let mut state = self.scope.session.state.lock().await;
        let round = &state.frames[&self.scope.frame_id].rounds[&self.scope.round_index];
        if self.scope.expired() {
            return Err(DeadlineExpired(
                round
                    .deadline_owner_execution_id
                    .ok_or_else(|| anyhow!("finite deadline missing owner"))?,
            )
            .into());
        }
        if let Some(pool) = variable_pool {
            state
                .frames
                .get_mut(&self.scope.frame_id)
                .expect("scope frame")
                .rounds
                .get_mut(&self.scope.round_index)
                .expect("scope round")
                .variable_pool
                .clone_from(pool);
        }
        let node = state
            .frames
            .get_mut(&self.scope.frame_id)
            .and_then(|frame| frame.rounds.get_mut(&self.scope.round_index))
            .and_then(|round| round.nodes.get_mut(&trace.node_id))
            .ok_or_else(|| anyhow!("scoped terminal without start"))?;
        if node.terminal_trace.is_some() {
            return Ok(());
        }
        let identity = node.identity.clone();
        let mut trace = trace.clone();
        trace.execution_identity = Some(identity.clone());
        node.terminal_trace = Some(trace.clone());
        let committed = emit(
            self.owner,
            &state,
            ContainerTransitionKind::NodeCompleted,
            &identity,
            None,
            Some(&trace),
            None,
        )
        .await;
        if let Err(error) = committed {
            state
                .frames
                .get_mut(&self.scope.frame_id)
                .expect("scope frame")
                .rounds
                .get_mut(&self.scope.round_index)
                .expect("scope round")
                .nodes
                .get_mut(&trace.node_id)
                .expect("scope node")
                .terminal_trace = None;
            return Err(error);
        }
        leave_identity(&self.scope.current_identity, &identity)?;
        Ok(())
    }
}

pub(crate) type CurrentIdentity = Arc<std::sync::Mutex<Vec<ScopedExecutionIdentity>>>;
tokio::task_local! { pub(crate) static CURRENT_EXECUTION: CurrentIdentity; }

/// Invokers use this logical identity to resolve the durable node receipt.
/// Each independently polled round has its own slot; no run-global active node.
pub fn scoped_execution_identity() -> Option<ScopedExecutionIdentity> {
    CURRENT_EXECUTION
        .try_with(|slot| {
            slot.lock()
                .ok()
                .and_then(|identities| identities.last().cloned())
        })
        .ok()
        .flatten()
}

pub(crate) fn enter_identity(
    slot: &CurrentIdentity,
    identity: &ScopedExecutionIdentity,
) -> Result<()> {
    let mut stack = slot
        .lock()
        .map_err(|_| anyhow!("execution identity lock poisoned"))?;
    if stack.last().map(|current| current.execution_id) != Some(identity.execution_id) {
        stack.push(identity.clone());
    }
    Ok(())
}
pub(crate) fn leave_identity(
    slot: &CurrentIdentity,
    identity: &ScopedExecutionIdentity,
) -> Result<()> {
    let mut stack = slot
        .lock()
        .map_err(|_| anyhow!("execution identity lock poisoned"))?;
    if stack.last().map(|current| current.execution_id) == Some(identity.execution_id) {
        stack.pop();
    }
    Ok(())
}

/// Preserve a deadline owner's identity across the storage adapter boundary.
/// Durable lifecycle adapters use this when their transaction rejects late work.
pub fn deadline_expired(owner_execution_id: Uuid) -> anyhow::Error {
    DeadlineExpired(owner_execution_id).into()
}
