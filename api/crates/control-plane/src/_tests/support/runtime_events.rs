use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::ports::{
    RuntimeEventCloseReason, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventStream,
    RuntimeEventStreamPolicy, RuntimeEventSubscription, RuntimeEventTrimPolicy,
};

#[derive(Default)]
pub struct RecordingRuntimeEventStream {
    events: Mutex<Vec<RuntimeEventEnvelope>>,
    close_calls: Mutex<Vec<(Uuid, RuntimeEventCloseReason)>>,
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
}

#[async_trait]
impl RuntimeEventStream for RecordingRuntimeEventStream {
    async fn open_run(&self, _run_id: Uuid, _policy: RuntimeEventStreamPolicy) -> Result<()> {
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<RuntimeEventEnvelope> {
        let mut events = self
            .events
            .lock()
            .expect("runtime event stream lock should be available");
        let envelope = RuntimeEventEnvelope::new(run_id, events.len() as i64 + 1, event);
        events.push(envelope.clone());
        Ok(envelope)
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
            live_events: receiver,
            closure,
        })
    }

    async fn replay(
        &self,
        _run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        Ok(self
            .events()
            .into_iter()
            .filter(|event| from_sequence.is_none_or(|sequence| event.sequence > sequence))
            .take(limit)
            .collect())
    }

    async fn close_run(&self, run_id: Uuid, reason: RuntimeEventCloseReason) -> Result<()> {
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
