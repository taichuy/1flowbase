use super::super::ManagedServiceRegistration;
use super::{
    broker, cache,
    host::{self, ManagedSource},
};
use crate::{app_state::ApiState, error_response::ApiError};
use control_plane::errors::ControlPlaneError as Error;
use interface_runtime::UserPrincipal;
use plugin_framework::ManagedServiceOperation;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::OwnedMutexGuard;

pub(in crate::managed_services) enum SnapshotLookup {
    Ready(Value),
    Fallback(Box<SnapshotFallback>),
}
pub(in crate::managed_services) struct SnapshotFallback {
    // Serial permit survives the entire raw unary execution and cache write.
    _guard: OwnedMutexGuard<()>,
    _inspection: OwnedMutexGuard<()>,
    source: ManagedSource,
    revision: String,
    operation: String,
    interval: u64,
}
impl SnapshotFallback {
    pub(in crate::managed_services) async fn store(&self, value: &Value) -> Result<(), ApiError> {
        cache::put_projection(
            self.source
                .state
                .infrastructure
                .ephemeral_kv_store()
                .as_ref(),
            &self.source.key,
            &self.revision,
            &self.operation,
            self.interval,
            value.clone(),
        )
        .await
    }
}
async fn cached(
    source: &ManagedSource,
    revision: &str,
    operation: &ManagedServiceOperation,
    principal: &UserPrincipal,
    payload: &Value,
) -> Result<Option<Value>, ApiError> {
    let specification = operation
        .shared_snapshot
        .as_ref()
        .ok_or(Error::Conflict("managed_service_shared_snapshot"))?;
    let validator = jsonschema::validator_for(&operation.output_schema)?;
    let store = source.state.infrastructure.ephemeral_kv_store();
    let from_event = cache::get(store.as_ref(), &source.key, revision)
        .await?
        .and_then(|event| {
            (event.pointer(&specification.ready_pointer) == Some(&specification.ready_value))
                .then(|| event.pointer(&specification.value_pointer).cloned())
                .flatten()
        });
    let value = match from_event {
        Some(value) if validator.is_valid(&value) => Some(value),
        _ => cache::get_projection(
            store.as_ref(),
            &source.key,
            revision,
            &operation.interface_id,
        )
        .await?
        .filter(|value| validator.is_valid(value)),
    };
    if value.is_some() {
        super::authority::authorize(source, principal, payload).await?;
    }
    Ok(value)
}
pub(in crate::managed_services) async fn snapshot(
    state: Arc<ApiState>,
    registration: ManagedServiceRegistration,
    principal: UserPrincipal,
    operation: &ManagedServiceOperation,
    payload: &Value,
) -> Result<SnapshotLookup, ApiError> {
    let specification = operation
        .shared_snapshot
        .as_ref()
        .ok_or(Error::Conflict("managed_service_shared_snapshot"))?;
    let source_operation = host::operation(&registration, &specification.source_interface_id)?;
    let interval = host::interval_at(operation, payload, &specification.interval_pointer)?;
    let record = payload
        .pointer(host::record_pointer(operation)?)
        .and_then(Value::as_str)
        .ok_or(Error::InvalidInput("record_id"))?;
    let mut source_payload = host::record_payload(&source_operation, record, interval)?;
    let source = ManagedSource::new(
        state,
        registration,
        source_operation,
        &principal,
        &source_payload,
    )?;
    host::set_pointer(
        &mut source_payload,
        host::record_pointer(&source.operation)?,
        serde_json::json!(source.key.record),
    )?;
    let inspection = broker::global().inspection(&source.key).lock_owned().await;
    // Every queued caller still performs its own current Kernel/row admission
    // and revision check. A shared permit never conveys another caller's grant.
    let principal = source.admit(&principal, &source_payload).await?;
    let revision = source.resolve(&principal, &source_payload).await?;
    if let Some(value) = cached(&source, &revision, operation, &principal, &source_payload).await? {
        return Ok(SnapshotLookup::Ready(value));
    }
    loop {
        if let Some(entry) = broker::global().active(&source.key) {
            let mut notices = entry.notices.subscribe();
            entry.refresh_revision(&revision);
            // The producer owns this wait. Do not queue new members or cadence
            // changes behind a cache reader waiting for its next ready sample.
            drop(inspection);
            loop {
                let notice = notices.borrow_and_update().clone();
                if let Some(value) =
                    cached(&source, &revision, operation, &principal, &source_payload).await?
                {
                    return Ok(SnapshotLookup::Ready(value));
                }
                if notice.revision == revision
                    && notice
                        .error
                        .is_some_and(|code| code != "managed_service_source_retrying")
                {
                    return Err(Error::UpstreamUnavailable("managed_service_source_stopped").into());
                }
                notices
                    .changed()
                    .await
                    .map_err(|_| Error::UpstreamUnavailable("managed_service_source_stopped"))?;
            }
        }
        let guard = broker::global().serial(&source.key).lock_owned().await;
        if broker::global().active(&source.key).is_some() {
            drop(guard);
            continue;
        }
        // A preceding unary miss may have populated this cache while we waited.
        if let Some(value) =
            cached(&source, &revision, operation, &principal, &source_payload).await?
        {
            return Ok(SnapshotLookup::Ready(value));
        }
        return Ok(SnapshotLookup::Fallback(Box::new(SnapshotFallback {
            _guard: guard,
            _inspection: inspection,
            source,
            revision,
            operation: operation.interface_id.clone(),
            interval,
        })));
    }
}
