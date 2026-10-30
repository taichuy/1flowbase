use super::*;
use model::{Event, Phase, Spec};
use std::sync::atomic::AtomicU8;

#[derive(Default)]
pub struct Metrics {
    pub peak_frames: usize,
    pub peak_local_bytes: usize,
    pub peak_single_local_bytes: usize,
    pub execution_ids: BTreeSet<uuid::Uuid>,
    pub round_commits: usize,
    pub round_admissions: BTreeMap<String, usize>,
    frozen_deadlines: BTreeMap<uuid::Uuid, (Option<i64>, Option<uuid::Uuid>)>,
}
#[derive(Default)]
struct Observed {
    events: Vec<Event>,
    phases: BTreeMap<u8, Phase>,
    started: u8,
    outputs: Vec<u8>,
    propagated: bool,
}
pub struct Observer {
    pub ledger: Ledger,
    pub metrics: Mutex<Metrics>,
    state: Mutex<Observed>,
    pub generation: AtomicU8,
    pub target: &'static str,
    pub spec: Spec,
    pub transitions: Mutex<
        Option<tokio::sync::mpsc::UnboundedSender<(ContainerTransitionKind, String, Option<u64>)>>,
    >,
}
impl Observer {
    pub fn new(target: &'static str, spec: Spec) -> Self {
        Self {
            ledger: Ledger::default(),
            metrics: Mutex::default(),
            state: Mutex::default(),
            generation: AtomicU8::new(0),
            target,
            spec,
            transitions: Mutex::new(None),
        }
    }
    pub fn events(&self) -> Vec<Event> {
        self.state.lock().unwrap().events.clone()
    }
    pub fn crash(&self) {
        self.state.lock().unwrap().events.push(Event::Crash);
    }
    pub fn transfer(&self) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut state = self.state.lock().unwrap();
        let started = state.started;
        state.events.push(Event::Recover {
            generation,
            reported_started: started,
        });
    }
    pub fn finish(&self, outcome: &FlowDebugExecutionOutcome) {
        let mut state = self.state.lock().unwrap();
        let outputs = state.outputs.clone();
        let propagated = state.propagated;
        state.events.push(Event::Finish {
            global_failure: matches!(outcome.stop_reason, ExecutionStopReason::Failed(_)),
            propagated,
            outputs,
        });
    }
    pub fn verify(&self) -> model::State {
        let events = self.events();
        model::verify(self.spec, &events).unwrap_or_else(|failure| {
            panic!("production trace rejected: {:?}; trace={events:?}", failure)
        })
    }
}
#[async_trait]
impl ExecutionLifecycle for Observer {
    async fn begin_node(&self, _: &CompiledNode, _: &Value) -> Result<()> {
        Ok(())
    }
    async fn commit_container_transition(&self, event: &ContainerTransition<'_>) -> Result<()> {
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.peak_frames = metrics.peak_frames.max(
                event
                    .checkpoint
                    .frames
                    .values()
                    .filter(|frame| !frame.closed)
                    .count(),
            );
            let sizes: Vec<_> = event
                .checkpoint
                .frames
                .values()
                .flat_map(|frame| frame.rounds.values())
                .map(|round| serde_json::to_vec(&round.variable_pool).unwrap().len())
                .collect();
            metrics.peak_local_bytes = metrics.peak_local_bytes.max(sizes.iter().sum());
            metrics.peak_single_local_bytes = metrics
                .peak_single_local_bytes
                .max(sizes.iter().copied().max().unwrap_or(0));
            if matches!(
                event.kind,
                ContainerTransitionKind::NodeStarted | ContainerTransitionKind::FrameEntered
            ) {
                assert!(
                    metrics.execution_ids.insert(event.identity.execution_id),
                    "duplicate logical execution start"
                );
            }
            if event.kind == ContainerTransitionKind::RoundStarted {
                *metrics
                    .round_admissions
                    .entry(event.identity.node_id.clone())
                    .or_default() += 1;
            }
            for round in event
                .checkpoint
                .frames
                .values()
                .flat_map(|frame| frame.rounds.values())
            {
                let deadline = (round.deadline_unix_ms, round.deadline_owner_execution_id);
                if let Some(previous) = metrics
                    .frozen_deadlines
                    .insert(round.execution_id, deadline)
                {
                    assert_eq!(
                        previous, deadline,
                        "recovery cannot renew an absolute deadline or change its owner"
                    );
                }
            }
            if event.kind == ContainerTransitionKind::RoundCommitted {
                metrics.round_commits += 1;
            }
        }
        if event.identity.node_id == self.target {
            let frame = &event.checkpoint.frames[&event.identity.execution_id];
            let mut state = self.state.lock().unwrap();
            let index = event
                .detail
                .and_then(|detail| detail.get("round_index"))
                .and_then(Value::as_u64)
                .map(|index| u8::try_from(index).unwrap());
            match event.kind {
                ContainerTransitionKind::RoundStarted => {
                    let index = index.unwrap();
                    state.events.push(Event::Start {
                        index,
                        reported_started: u8::try_from(frame.started_rounds).unwrap(),
                    });
                    state.started += 1;
                    state.phases.insert(index, Phase::Running);
                }
                ContainerTransitionKind::ScopeSuspended => {
                    if let Some(index) = index {
                        if state.phases.get(&index) == Some(&Phase::Running) {
                            state.events.push(Event::Wait(index));
                            state.phases.insert(index, Phase::Waiting);
                        }
                    }
                }
                ContainerTransitionKind::RoundCommitted => {
                    let index = index.unwrap();
                    let round = &frame.rounds[&(index as u64)];
                    if state.started <= index {
                        state.events.push(Event::Start {
                            index,
                            reported_started: index + 1,
                        });
                        state.started = index + 1;
                        state.phases.insert(index, Phase::Running);
                    }
                    let phase = match round.status {
                        ContainerRoundStatus::Succeeded => Phase::Succeeded,
                        ContainerRoundStatus::Failed => Phase::Failed,
                        ContainerRoundStatus::Cancelled => Phase::Cancelled,
                        _ => panic!("nonterminal round commit"),
                    };
                    if state.phases.get(&index) != Some(&phase) {
                        let observed = match phase {
                            Phase::Succeeded => Event::Complete {
                                index,
                                generation: self.generation.load(Ordering::SeqCst),
                                accepted: true,
                            },
                            Phase::Cancelled => Event::Cancel(index),
                            _ => {
                                let timeout = round
                                    .error_payload
                                    .as_ref()
                                    .and_then(|error| error.get("error_code"))
                                    .and_then(Value::as_str)
                                    .is_some_and(|code| code.contains("timeout"));
                                if timeout {
                                    state.events.push(Event::Expire(index));
                                    let ancestor = round.deadline_owner_execution_id
                                        != Some(frame.identity.execution_id);
                                    state.propagated = ancestor;
                                    Event::Timeout { index, ancestor }
                                } else {
                                    Event::Fail {
                                        index,
                                        global: frame.global_failure.is_some(),
                                    }
                                }
                            }
                        };
                        state.events.push(observed);
                        state.phases.insert(index, phase);
                    }
                }
                _ => {}
            }
            state.outputs = frame
                .results
                .keys()
                .map(|index| u8::try_from(*index).unwrap())
                .collect();
        }
        self.ledger.commit_container_transition(event).await?;
        if let Some(sender) = self.transitions.lock().unwrap().as_ref() {
            let _ = sender.send((
                event.kind,
                event.identity.node_id.clone(),
                event
                    .detail
                    .and_then(|detail| detail.get("round_index"))
                    .and_then(Value::as_u64),
            ));
        }
        Ok(())
    }
}
