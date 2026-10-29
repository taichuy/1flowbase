use super::*;
use crate::ports::{CommitContainerTransitionInput, ContainerNodeFact, ContainerRoundFact};
use orchestration_runtime::execution_engine::container::{
    ContainerRoundStatus, ContainerTransition, ContainerTransitionKind, ContainerWait,
};

impl<R, H> PersistedNodeLifecycle<'_, R, H>
where
    R: OrchestrationRuntimeRepository + Send + Sync,
    H: Send + Sync,
{
    pub(super) async fn persist_container_transition(
        &self,
        transition: &ContainerTransition<'_>,
    ) -> Result<()> {
        let selected_round = transition
            .detail
            .and_then(|detail| detail.get("round_index"))
            .and_then(Value::as_u64)
            .and_then(|index| {
                transition
                    .checkpoint
                    .frames
                    .get(&transition.identity.execution_id)?
                    .rounds
                    .get(&index)
            });
        let suspended_node = if transition.kind == ContainerTransitionKind::ScopeSuspended {
            selected_round.and_then(|round| {
                let waiting = round.waiting.as_ref()?;
                let node_id = match waiting {
                    ContainerWait::Human { node_id, .. }
                    | ContainerWait::Callback { node_id, .. } => node_id,
                };
                round
                    .nodes
                    .get(node_id)
                    .map(|node| (&node.identity, waiting))
            })
        } else {
            None
        };
        let logical_identity = suspended_node
            .map(|(identity, _)| identity)
            .unwrap_or(transition.identity);
        let identity: domain::NodeExecutionIdentity =
            serde_json::from_value(serde_json::to_value(logical_identity)?)?;
        let waiting = suspended_node.map(|(_, wait)| wait);
        let node = match transition.kind {
            ContainerTransitionKind::NodeStarted | ContainerTransitionKind::FrameEntered => {
                let detail = transition
                    .detail
                    .ok_or_else(|| anyhow::anyhow!("container node start is missing metadata"))?;
                Some(ContainerNodeFact::Started {
                    node_type: detail["node_type"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("container node_type missing"))?
                        .to_owned(),
                    node_alias: detail["node_alias"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("container node_alias missing"))?
                        .to_owned(),
                    input_payload: transition
                        .input_payload
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                })
            }
            ContainerTransitionKind::Resumed => Some(ContainerNodeFact::Updated {
                status: domain::NodeRunStatus::Running,
                output_payload: json!({}),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
                terminal: false,
            }),
            ContainerTransitionKind::FrameClosed if transition.trace.is_none() => {
                Some(ContainerNodeFact::Updated {
                    status: domain::NodeRunStatus::Cancelled,
                    output_payload: json!({}),
                    error_payload: Some(json!({"code":"container_cancelled"})),
                    metrics_payload: json!({}),
                    debug_payload: json!({}),
                    terminal: true,
                })
            }
            _ if (transition.kind != ContainerTransitionKind::ScopeSuspended
                && transition.trace.is_some())
                || waiting.is_some() =>
            {
                let trace = transition.trace;
                let status = match waiting {
                    Some(ContainerWait::Human { .. }) => domain::NodeRunStatus::WaitingHuman,
                    Some(ContainerWait::Callback { .. }) => domain::NodeRunStatus::WaitingCallback,
                    None if trace.is_some_and(|trace| trace.error_payload.is_some()) => {
                        domain::NodeRunStatus::Failed
                    }
                    None => domain::NodeRunStatus::Succeeded,
                };
                Some(ContainerNodeFact::Updated {
                    status,
                    output_payload: trace
                        .map(|trace| {
                            super::super::persisted_node_output_payload(
                                &trace.output_payload,
                                &trace.metrics_payload,
                                trace.error_payload.as_ref(),
                                &trace.debug_payload,
                            )
                        })
                        .unwrap_or_else(|| json!({})),
                    error_payload: trace.and_then(|trace| trace.error_payload.clone()),
                    metrics_payload: trace
                        .map(|trace| trace.metrics_payload.clone())
                        .unwrap_or_else(|| json!({})),
                    debug_payload: trace
                        .map(|trace| trace.debug_payload.clone())
                        .unwrap_or_else(|| json!({})),
                    terminal: waiting.is_none(),
                })
            }
            _ => None,
        };
        // Round facts contain scalar coordinates and terminal outcomes, never the
        // shared runtime variable pool. The current checkpoint owns that one copy.
        let mut coordinates = transition
            .identity
            .round_path
            .iter()
            .map(|path| (path.container_execution_id, path.round_index))
            .collect::<Vec<_>>();
        if let Some(round) = selected_round {
            coordinates.push((transition.identity.execution_id, round.round_index));
        }
        coordinates.sort_unstable();
        coordinates.dedup();
        let rounds = coordinates
            .into_iter()
            .filter_map(|(frame_id, index)| {
                let frame = transition.checkpoint.frames.get(&frame_id)?;
                let round = frame.rounds.get(&index)?;
                Some(ContainerRoundFact {
                    execution_id: round.execution_id,
                    container_execution_id: frame.identity.execution_id,
                    round_index: round.round_index,
                    deadline_unix_ms: round.deadline_unix_ms,
                    status: match round.status {
                        ContainerRoundStatus::Running => "running",
                        ContainerRoundStatus::Waiting => "waiting",
                        ContainerRoundStatus::Succeeded => "succeeded",
                        ContainerRoundStatus::Failed => "failed",
                        ContainerRoundStatus::Cancelled => "cancelled",
                    }
                    .into(),
                    error_payload: round.error_payload.clone(),
                })
            })
            .collect();
        let record = self
            .service
            .repository
            .commit_container_transition(&CommitContainerTransitionInput {
                flow_run_id: self.flow_run_id,
                resume_claim_owner: self.resume_claim_owner.clone(),
                execution_identity: identity,
                checkpoint: serde_json::to_value(transition.checkpoint)?,
                node,
                rounds,
            })
            .await?;
        if let Some(record) = record {
            match transition.kind {
                ContainerTransitionKind::NodeStarted | ContainerTransitionKind::FrameEntered => {
                    *self
                        .flow_execution_context
                        .active_node
                        .lock()
                        .map_err(|_| anyhow::anyhow!("active runtime node lock is poisoned"))? =
                        Some(RuntimeActiveNode {
                            node_id: record.node_id.clone(),
                            node_run_id: record.id,
                        });
                    append_runtime_event(
                        self.service,
                        self.flow_run_id,
                        debug_stream_events::node_started(&record),
                    )
                    .await;
                }
                ContainerTransitionKind::NodeCompleted | ContainerTransitionKind::FrameClosed
                    if record.finished_at.is_some() =>
                {
                    append_runtime_event(
                        self.service,
                        self.flow_run_id,
                        debug_stream_events::node_finished(&record),
                    )
                    .await;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(in crate::orchestration_runtime) async fn waiting_execution_id<
    R: OrchestrationRuntimeRepository,
>(
    repository: &R,
    flow_run_id: uuid::Uuid,
    snapshot: &orchestration_runtime::execution_state::CheckpointSnapshot,
    node_id: &str,
    node_run_id: Option<uuid::Uuid>,
) -> Result<Option<uuid::Uuid>> {
    let Some(state) = &snapshot.container_state else {
        return Ok(None);
    };
    let node_run_id =
        node_run_id.ok_or_else(|| anyhow::anyhow!("container waiting node receipt missing"))?;
    for round in state
        .frames
        .values()
        .flat_map(|frame| frame.rounds.values())
    {
        let Some(node) = round.nodes.get(node_id) else {
            continue;
        };
        if let Some(record) = repository
            .get_container_node_run(flow_run_id, node.identity.execution_id)
            .await?
        {
            if record.id == node_run_id {
                return Ok(Some(node.identity.execution_id));
            }
        }
    }
    Err(anyhow::anyhow!("container waiting execution mismatch"))
}

pub(in crate::orchestration_runtime) fn waiting_variable_pool<'a>(
    snapshot: &'a orchestration_runtime::execution_state::CheckpointSnapshot,
    execution_id: Option<uuid::Uuid>,
) -> Result<&'a serde_json::Map<String, Value>> {
    if let Some(execution_id) = execution_id {
        return snapshot
            .container_state
            .as_ref()
            .into_iter()
            .flat_map(|state| state.frames.values())
            .flat_map(|frame| frame.rounds.values())
            .find(|round| {
                round
                    .nodes
                    .values()
                    .any(|node| node.identity.execution_id == execution_id)
            })
            .map(|round| &round.variable_pool)
            .ok_or_else(|| anyhow::anyhow!("container waiting scope missing"));
    }
    Ok(&snapshot.variable_pool)
}
