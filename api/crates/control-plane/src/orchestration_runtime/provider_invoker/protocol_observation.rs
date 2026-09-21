use super::*;

/// Invocation-local capture identity and completeness; independent of business outcome.
pub(super) struct Capture {
    node: Option<(String, Uuid)>,
    invocation_id: Uuid,
    provider_attempt_index: u64,
    observed_count: u64,
    pub(super) persist_failed_count: u64,
    ended: bool,
}

impl Capture {
    pub(super) fn new(node: Option<(String, Uuid)>, input: &ProviderInvocationInput) -> Self {
        Self {
            node,
            invocation_id: input
                .trace_context
                .get("provider_invocation_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .unwrap_or_else(Uuid::now_v7),
            provider_attempt_index: input
                .trace_context
                .get("provider_attempt_index")
                .and_then(|index| index.parse().ok())
                .unwrap_or(0),
            observed_count: 0,
            persist_failed_count: 0,
            ended: false,
        }
    }

    fn payload(
        &self,
        flow_run_id: Uuid,
        event_type: &str,
        mut payload: Value,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        let (node_id, node_run_id) = self.node.as_ref()?;
        let object = payload.as_object_mut()?;
        object.insert("type".into(), json!(event_type));
        object.insert("flow_run_id".into(), json!(flow_run_id));
        object.insert("node_id".into(), json!(node_id));
        object.insert("node_run_id".into(), json!(node_run_id));
        object.insert("invocation_id".into(), json!(self.invocation_id));
        object.insert(
            "provider_attempt_index".into(),
            json!(self.provider_attempt_index),
        );
        Some(crate::ports::RuntimeEventPayload {
            event_type: event_type.into(),
            source: crate::ports::RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload,
        })
    }

    pub(super) fn observe(
        &mut self,
        flow_run_id: Option<Uuid>,
        event: &ProviderStreamEvent,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        let ProviderStreamEvent::ProtocolObservation { kind, .. } = event else {
            return None;
        };
        self.observed_count += 1;
        // A new transport request after a completed exchange requires fresh end evidence.
        if kind == "request" {
            self.ended = false;
        }
        if kind == "stream_end" {
            self.ended = true;
        }
        let mut payload = serde_json::to_value(event).ok()?;
        payload["sequence"] = json!(self.observed_count);
        self.payload(flow_run_id?, "provider_protocol_observation", payload)
    }

    pub(super) fn integrity(
        &self,
        flow_run_id: Uuid,
        invocation_ok: bool,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        self.payload(flow_run_id, "provider_protocol_integrity", json!({
            "observed_count": self.observed_count,
            "persist_failed_count": self.persist_failed_count,
            "status": if self.observed_count == 0 { "unavailable" }
                else if self.ended && invocation_ok && self.persist_failed_count == 0 { "complete" }
                else { "incomplete" },
        }))
    }
}

#[cfg(test)]
#[path = "_tests/protocol_observation.rs"]
mod tests;
