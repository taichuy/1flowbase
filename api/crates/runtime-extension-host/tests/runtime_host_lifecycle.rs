use std::sync::Arc;

use async_trait::async_trait;
use runtime_core::runtime_backend::{
    RuntimeBackendError, RuntimeBackendLifecycle, RuntimeBackendSlot, RuntimeExecutionPort,
    RuntimeObservationPort, RuntimeRequestId,
};
use runtime_extension_host::RuntimeExtensionHost;
use time::OffsetDateTime;

#[tokio::test]
async fn d_001_one_host_owns_all_runtime_registries_and_the_backend_slot() {
    let host = Arc::new(RuntimeExtensionHost::new(OffsetDateTime::now_utc()).unwrap());
    let mut slot = RuntimeBackendSlot::default();
    slot.bind(host.clone()).unwrap();
    host.mark_ready().unwrap();

    let snapshot = RuntimeObservationPort::snapshot(host.as_ref())
        .await
        .unwrap();
    assert_eq!(snapshot.backend_kind, "in_process");
    assert_eq!(snapshot.lifecycle, RuntimeBackendLifecycle::Ready);
    assert_eq!(snapshot.registries.providers, 0);
    assert_eq!(snapshot.registries.data_sources, 0);
    assert_eq!(snapshot.registries.capabilities, 0);
    assert_eq!(snapshot.registries.network_egress_providers, 0);
    assert!(slot.backend().is_ok());
}

#[tokio::test]
async fn d_003_ready_drain_stop_is_monotonic_and_cancel_is_idempotent() {
    let host = RuntimeExtensionHost::new(OffsetDateTime::now_utc()).unwrap();
    host.mark_ready().unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Ready);

    let request_id = RuntimeRequestId::new("missing-request").unwrap();
    assert_eq!(
        RuntimeExecutionPort::cancel(&host, &request_id)
            .await
            .unwrap(),
        runtime_core::runtime_backend::RuntimeCancelOutcome::NotFound
    );

    host.drain().await.unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Draining);
    host.stop().await.unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Stopped);
    let error = host.mark_ready().unwrap_err();
    assert!(matches!(
        error,
        RuntimeBackendError::Unavailable(RuntimeBackendLifecycle::Stopped)
    ));
}

#[derive(Default)]
struct RecordingSink;

#[async_trait]
impl runtime_core::runtime_backend::RuntimeStreamEventSink for RecordingSink {
    async fn emit(
        &self,
        _event: extension_contracts::provider_contract::ProviderStreamEvent,
    ) -> Result<(), RuntimeBackendError> {
        Ok(())
    }
}

#[test]
fn d_010_sdk_facing_contract_does_not_require_host_internals() {
    let _: Arc<dyn runtime_core::runtime_backend::RuntimeStreamEventSink> = Arc::new(RecordingSink);
}
