use super::*;

/// Only routing metadata is retained; registry sessions/tombstones own its lifetime.
#[derive(Default)]
pub(super) struct TransportAffinity {
    pub(super) bases: BTreeMap<String, TransportSessionId>,
    pub(super) cursors: BTreeMap<(String, String), TransportSessionId>,
}

impl TransportAffinity {
    pub(super) fn record_slot(&mut self, base: &TransportSessionId, slot: &TransportSessionId) {
        self.bases.insert(slot.as_str().into(), base.clone());
    }

    pub(super) fn bind_cursor(&mut self, slot: &TransportSessionId, cursor: &str) {
        if let Some(base) = self.bases.get(slot.as_str()) {
            self.cursors
                .insert((base.as_str().into(), cursor.into()), slot.clone());
        }
    }

    fn retain(&mut self, live: &std::collections::BTreeSet<&str>) {
        self.bases.retain(|slot, _| live.contains(slot.as_str()));
        self.cursors.retain(|_, slot| live.contains(slot.as_str()));
    }
}

/// Native wire cursors and semantic cursors share an owner. Malformed or
/// contradictory cursor input is not proof of an independent request.
fn response_cursor(input: &ProviderInvocationInput) -> Result<Option<&str>, ()> {
    let native = input
        .native_transport
        .as_ref()
        .and_then(|native| native.wire_body.get("previous_response_id"))
        .filter(|value| !value.is_null());
    let native = native
        .map(|value| value.as_str().filter(|id| !id.is_empty()).ok_or(()))
        .transpose()?;
    let semantic = input.previous_response_id.as_deref();
    if semantic == Some("") || matches!((native, semantic), (Some(a), Some(b)) if a != b) {
        return Err(());
    }
    Ok(native.or(semantic))
}

fn independent_native_request(input: &ProviderInvocationInput) -> bool {
    input.operation == ProviderWireOperation::Generate
        && input.protocol == "openai_responses"
        && matches!(response_cursor(input), Ok(None))
        && input.native_transport.as_ref().is_some_and(|native| {
            native.protocol == "openai_responses"
                && native.wire_body.is_object()
                && native
                    .wire_body
                    .get("model")
                    .is_some_and(|model| model.is_string())
                && native
                    .wire_body
                    .get("input")
                    .is_some_and(|body| body.is_array() || body.is_string())
        })
}

impl<C: TransportClock + 'static> TransportSessionCoordinator<C> {
    pub(super) fn select_affinity_slot(
        &self,
        registry: &TransportSessionRegistry<C>,
        base: &TransportSessionId,
        scope: Option<&TransportConnectionScope>,
        input: &ProviderInvocationInput,
        target: &TransportRuntimeTargetId,
    ) -> anyhow::Result<TransportSessionId> {
        let cursor = response_cursor(input);
        match cursor {
            Ok(Some(cursor)) => {
                return Ok(self
                    .affinity
                    .lock()
                    .expect("transport affinity")
                    .cursors
                    .get(&(base.as_str().into(), cursor.into()))
                    .cloned()
                    .unwrap_or_else(|| base.clone()));
            }
            Err(()) => return Ok(base.clone()),
            Ok(None) => {}
        }
        let Some(scope) = scope else {
            return Ok(base.clone());
        };
        if let Some(slot) = scope
            .state
            .lock()
            .expect("transport connection bindings")
            .affinity
            .get(base.as_str())
            .cloned()
        {
            return Ok(slot);
        }
        // A delivery only forks a proven complete request, never an owned
        // continuation or a target change. Every resulting physical slot still
        // passes the registry's singleton invocation and generation fencing.
        if independent_native_request(input) {
            if let Some(fence) = registry.fence(base) {
                if registry.runtime_target_id(&fence)? == target
                    && registry.invocation_inflight(&fence)?
                    && matches!(
                        registry.state(&fence)?,
                        TransportSessionState::Active | TransportSessionState::Orphaned
                    )
                {
                    return Ok(TransportSessionId::new(hash_identity(&[
                        base.as_str(),
                        scope.id(),
                    ]))?);
                }
            }
        }
        Ok(base.clone())
    }

    pub(super) fn reclaim_affinity(&self, snapshot: &SafeRegistrySnapshot) {
        let live = snapshot
            .sessions
            .iter()
            .map(|session| session.fence.session_id.as_str())
            .chain(
                snapshot
                    .tombstones
                    .iter()
                    .map(|receipt| receipt.fence.session_id.as_str()),
            )
            .collect::<std::collections::BTreeSet<_>>();
        self.affinity
            .lock()
            .expect("transport affinity")
            .retain(&live);
        let mut scopes = self.connection_scopes.lock().expect("transport scopes");
        scopes.retain(|_, scope| {
            let Some(scope) = scope.upgrade() else {
                return false;
            };
            scope
                .state
                .lock()
                .expect("transport connection bindings")
                .affinity
                .retain(|_, slot| live.contains(slot.as_str()));
            true
        });
    }
}
