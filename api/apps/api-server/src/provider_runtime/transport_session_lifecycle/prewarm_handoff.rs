//! Host-owned identity of an admitted, non-generating Responses prewarm.
//! It permits waiting for that lease, never a second concurrent invocation.
use super::*;

#[derive(Default)]
pub(super) struct PrewarmHandoff {
    leases: StdMutex<BTreeMap<String, InvocationLease>>,
    pub(super) changed: Notify,
}

impl PrewarmHandoff {
    pub(super) fn record(
        &self,
        input: &ProviderInvocationInput,
        transport: RecoveryTransport,
        lease: &InvocationLease,
    ) {
        let mut leases = self.leases.lock().expect("prewarm handoff leases");
        let key = lease.fence.session_id.as_str().to_owned();
        if transport == RecoveryTransport::AiNativeWebSocket
            && input.operation == ProviderWireOperation::Generate
            && input.native_transport.as_ref().is_some_and(|native| {
                native.protocol == "openai_responses"
                    && native
                        .wire_body
                        .get("generate")
                        .and_then(serde_json::Value::as_bool)
                        == Some(false)
            })
        {
            leases.insert(key, lease.clone());
        } else {
            leases.remove(&key);
        }
    }

    pub(super) fn contains(&self, fence: &TransportFence) -> bool {
        self.leases
            .lock()
            .expect("prewarm handoff leases")
            .get(fence.session_id.as_str())
            .is_some_and(|lease| lease.fence == *fence)
    }

    pub(super) fn retain_inflight(&self, snapshot: &SafeRegistrySnapshot) -> bool {
        let mut leases = self.leases.lock().expect("prewarm handoff leases");
        let previous_len = leases.len();
        leases.retain(|_, lease| {
            snapshot
                .sessions
                .iter()
                .any(|session| session.fence == lease.fence && session.inflight)
        });
        leases.len() != previous_len
    }
}
