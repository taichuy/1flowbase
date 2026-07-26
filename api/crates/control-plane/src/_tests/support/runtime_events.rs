use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventCloseReason, RuntimeEventEnvelope,
    RuntimeEventPayload, RuntimeEventStream, RuntimeEventStreamPolicy, RuntimeEventSubscription,
    RuntimeEventTrimPolicy,
};

#[derive(Default)]
pub struct RecordingRuntimeEventStream {
    events: Mutex<Vec<RuntimeEventEnvelope>>,
    close_calls: Mutex<Vec<(Uuid, RuntimeEventCloseReason)>>,
    closed_runs: Mutex<HashSet<Uuid>>,
    fail_next_append: Mutex<bool>,
    fail_next_close: Mutex<bool>,
    append_barrier: Mutex<Option<Arc<tokio::sync::Barrier>>>,
    terminal_claim: Mutex<()>,
}

impl RecordingRuntimeEventStream {
    pub fn events(&self) -> Vec<RuntimeEventEnvelope> {
        self.events
            .lock()
            .expect("runtime event stream lock should be available")
            .clone()
    }

    pub fn close_calls(&self) -> Vec<(Uuid, RuntimeEventCloseReason)> {
        self.close_calls
            .lock()
            .expect("runtime event stream close lock should be available")
            .clone()
    }

    pub fn is_closed(&self, run_id: Uuid) -> bool {
        self.closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .contains(&run_id)
    }

    pub fn fail_next_append(&self) {
        *self
            .fail_next_append
            .lock()
            .expect("runtime event stream append-failure lock should be available") = true;
    }

    pub fn fail_next_close(&self) {
        *self
            .fail_next_close
            .lock()
            .expect("runtime event stream close-failure lock should be available") = true;
    }

    pub fn synchronize_next_appends(&self, participants: usize) {
        *self
            .append_barrier
            .lock()
            .expect("runtime event stream append-barrier lock should be available") =
            Some(Arc::new(tokio::sync::Barrier::new(participants)));
    }
}

#[async_trait]
impl RuntimeEventStream for RecordingRuntimeEventStream {
    async fn open_run(&self, run_id: Uuid, _policy: RuntimeEventStreamPolicy) -> Result<()> {
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("runtime event stream terminal-claim lock should be available");
        self.closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .remove(&run_id);
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<RuntimeEventEnvelope> {
        if std::mem::take(
            &mut *self
                .fail_next_append
                .lock()
                .expect("runtime event stream append-failure lock should be available"),
        ) {
            anyhow::bail!("simulated runtime event stream append failure");
        }
        if self
            .closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .contains(&run_id)
        {
            anyhow::bail!("runtime event stream is closed");
        }
        let append_barrier = self
            .append_barrier
            .lock()
            .expect("runtime event stream append-barrier lock should be available")
            .clone();
        if let Some(append_barrier) = append_barrier {
            append_barrier.wait().await;
        }
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("runtime event stream terminal-claim lock should be available");
        if self
            .closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .contains(&run_id)
        {
            anyhow::bail!("runtime event stream is closed");
        }
        let mut events = self
            .events
            .lock()
            .expect("runtime event stream lock should be available");
        let envelope = RuntimeEventEnvelope::new(run_id, events.len() as i64 + 1, event);
        events.push(envelope.clone());
        Ok(envelope)
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        let incoming_reason = RuntimeEventCloseReason::from_terminal_event_type(&event.event_type)
            .ok_or_else(|| {
                anyhow::anyhow!("runtime event stream terminal append requires a terminal event")
            })?;
        // This test double deliberately does not wait on `append_barrier`: the barrier exists to
        // make the retired replay-then-append implementation race. The production operation
        // instead claims terminal ownership as one critical section.
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("runtime event stream terminal-claim lock should be available");
        let existing_terminal_reason = self
            .events
            .lock()
            .expect("runtime event stream lock should be available")
            .iter()
            .find_map(|existing| {
                (existing.run_id == run_id)
                    .then(|| {
                        RuntimeEventCloseReason::from_terminal_event_type(&existing.event_type)
                    })
                    .flatten()
            });
        let is_closed = self
            .closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .contains(&run_id);
        if is_closed {
            if existing_terminal_reason.is_some() {
                return Ok(AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal);
            }
            anyhow::bail!("runtime event stream is closed without a terminal event");
        }

        let (outcome, close_reason) = if let Some(existing_reason) = existing_terminal_reason {
            (
                AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal,
                existing_reason,
            )
        } else {
            if std::mem::take(
                &mut *self
                    .fail_next_append
                    .lock()
                    .expect("runtime event stream append-failure lock should be available"),
            ) {
                anyhow::bail!("simulated runtime event stream append failure");
            }
            let mut events = self
                .events
                .lock()
                .expect("runtime event stream lock should be available");
            let envelope = RuntimeEventEnvelope::new(run_id, events.len() as i64 + 1, event);
            events.push(envelope);
            (
                AppendTerminalIfMissingAndCloseOutcome::Appended,
                incoming_reason,
            )
        };

        if std::mem::take(
            &mut *self
                .fail_next_close
                .lock()
                .expect("runtime event stream close-failure lock should be available"),
        ) {
            anyhow::bail!("simulated runtime event stream close failure");
        }
        self.closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .insert(run_id);
        self.close_calls
            .lock()
            .expect("runtime event stream close lock should be available")
            .push((run_id, close_reason));
        Ok(outcome)
    }

    async fn subscribe(
        &self,
        _run_id: Uuid,
        _from_sequence: Option<i64>,
    ) -> Result<RuntimeEventSubscription> {
        let (_sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let (_closure_sender, closure) = tokio::sync::watch::channel(None);
        Ok(RuntimeEventSubscription {
            replay: self.events(),
            live_events: crate::ports::RuntimeEventReceiver::from_unbounded(receiver),
            closure,
        })
    }

    async fn replay(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        Ok(self
            .events()
            .into_iter()
            .filter(|event| event.run_id == run_id)
            .filter(|event| from_sequence.is_none_or(|sequence| event.sequence > sequence))
            .take(limit)
            .collect())
    }

    async fn close_run(&self, run_id: Uuid, reason: RuntimeEventCloseReason) -> Result<()> {
        let _terminal_claim = self
            .terminal_claim
            .lock()
            .expect("runtime event stream terminal-claim lock should be available");
        if std::mem::take(
            &mut *self
                .fail_next_close
                .lock()
                .expect("runtime event stream close-failure lock should be available"),
        ) {
            anyhow::bail!("simulated runtime event stream close failure");
        }
        if !self
            .closed_runs
            .lock()
            .expect("runtime event stream closed-runs lock should be available")
            .insert(run_id)
        {
            anyhow::bail!("runtime event stream is closed");
        }
        self.close_calls
            .lock()
            .expect("runtime event stream close lock should be available")
            .push((run_id, reason));
        Ok(())
    }

    async fn trim(&self, _run_id: Uuid, _policy: RuntimeEventTrimPolicy) -> Result<()> {
        Ok(())
    }
}
