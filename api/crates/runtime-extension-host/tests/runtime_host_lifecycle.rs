use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use extension_contracts::ProviderInvocationInput;
use runtime_core::runtime_backend::{
    ProviderRuntimePort, RuntimeArtifactReference, RuntimeBackendError, RuntimeBackendLifecycle,
    RuntimeBackendSlot, RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeObservationPort,
    RuntimePackageActivation, RuntimeRequestId, RuntimeStreamSinks, RuntimeTargetId,
};
use runtime_extension_host::{RuntimeArtifactResolver, RuntimeExtensionHost};
use time::OffsetDateTime;

const LIFECYCLE_PLUGIN_ID: &str = "lifecycle_provider@0.1.0";

struct LifecycleProviderPackage(PathBuf);

impl LifecycleProviderPackage {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "runtime-host-lifecycle-{}-{}",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        fs::create_dir_all(root.join("provider")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("i18n")).unwrap();
        fs::write(
            root.join("manifest.yaml"),
            r#"manifest_version: 1
plugin_id: lifecycle_provider
version: 0.1.0
publisher_namespace: 1flowbase
vendor: 1flowbase
display_name: Lifecycle Provider
description: Runtime host lifecycle fixture
source_kind: uploaded
trust_level: checksum_only
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/lifecycle_provider
  capabilities:
    - config.validate
  limits:
    timeout_ms: 30000
node_contributions: []
"#,
        )
        .unwrap();
        fs::write(
            root.join("provider/lifecycle_provider.yaml"),
            r#"provider_code: lifecycle_provider
display_name: Lifecycle Provider
protocol: openai_compatible
model_discovery: static
config_schema: []
"#,
        )
        .unwrap();
        fs::write(
            root.join("i18n/en_US.json"),
            r#"{ "plugin": { "label": "Lifecycle Provider" } }"#,
        )
        .unwrap();
        let executable = root.join("bin/lifecycle_provider");
        fs::write(
            &executable,
            include_str!("_fixtures/provider_stdio/lifecycle_worker.sh"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&executable).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&executable, permissions).unwrap();
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for LifecycleProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct FixtureArtifactResolver(PathBuf);

#[async_trait]
impl RuntimeArtifactResolver for FixtureArtifactResolver {
    async fn resolve(
        &self,
        _artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        Ok(self.0.clone())
    }
}

fn lifecycle_request(request_id: &str) -> RuntimeExecutionRequest {
    RuntimeExecutionRequest {
        request_id: RuntimeRequestId::new(request_id).unwrap(),
        target: RuntimeTargetId::new(LIFECYCLE_PLUGIN_ID).unwrap(),
        input: ProviderInvocationInput {
            provider_instance_id: "lifecycle-instance".to_string(),
            provider_code: "lifecycle_provider".to_string(),
            protocol: "openai_compatible".to_string(),
            model: "slow".to_string(),
            provider_config: serde_json::json!({ "mode": "slow" }),
            ..ProviderInvocationInput::default()
        },
        principal: None,
    }
}

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
    let package = LifecycleProviderPackage::new();
    let host = Arc::new(
        RuntimeExtensionHost::new_with_artifact_resolver(
            OffsetDateTime::now_utc(),
            Arc::new(FixtureArtifactResolver(package.path().to_path_buf())),
        )
        .unwrap(),
    );
    host.activate_provider(RuntimePackageActivation {
        plugin_id: LIFECYCLE_PLUGIN_ID.to_string(),
        artifact: RuntimeArtifactReference::new("lifecycle-artifact").unwrap(),
        source_identity: Some("lifecycle-fixture".to_string()),
        legacy_eligibility: None,
    })
    .await
    .expect("package activation is valid while the host is starting");

    let starting_error = host
        .execute_stream(
            lifecycle_request("starting-request"),
            RuntimeStreamSinks::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        starting_error,
        RuntimeBackendError::Unavailable(RuntimeBackendLifecycle::Starting)
    ));

    host.mark_ready().unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Ready);

    let request_id = RuntimeRequestId::new("active-request").unwrap();
    let execution_host = Arc::clone(&host);
    let execution = tokio::spawn(async move {
        execution_host
            .execute_stream(
                lifecycle_request("active-request"),
                RuntimeStreamSinks::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = RuntimeObservationPort::snapshot(host.as_ref())
                .await
                .unwrap();
            if snapshot.active_request_ids == ["active-request"] {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("active request must become observable before cancellation");

    assert_eq!(
        RuntimeExecutionPort::cancel(host.as_ref(), &request_id)
            .await
            .unwrap(),
        runtime_core::runtime_backend::RuntimeCancelOutcome::Cancelled
    );
    assert!(matches!(
        execution.await.unwrap().unwrap_err(),
        RuntimeBackendError::Cancelled(cancelled) if cancelled == request_id
    ));
    assert_eq!(
        RuntimeExecutionPort::cancel(host.as_ref(), &request_id)
            .await
            .unwrap(),
        runtime_core::runtime_backend::RuntimeCancelOutcome::NotFound
    );

    let draining_host = Arc::clone(&host);
    let draining_execution = tokio::spawn(async move {
        draining_host
            .execute_stream(
                lifecycle_request("drain-active-request"),
                RuntimeStreamSinks::default(),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = RuntimeObservationPort::snapshot(host.as_ref())
                .await
                .unwrap();
            if snapshot.active_request_ids == ["drain-active-request"] {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("drain fixture request must become active");

    host.drain().await.unwrap();
    assert_eq!(host.lifecycle(), RuntimeBackendLifecycle::Draining);
    assert!(matches!(
        draining_execution.await.unwrap().unwrap_err(),
        RuntimeBackendError::Cancelled(cancelled)
            if cancelled.as_str() == "drain-active-request"
    ));
    assert!(
        RuntimeObservationPort::snapshot(host.as_ref())
            .await
            .unwrap()
            .active_request_ids
            .is_empty(),
        "drain must leave no request admitted before its lifecycle transition"
    );
    let draining_error = host
        .execute_stream(
            lifecycle_request("draining-request"),
            RuntimeStreamSinks::default(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        draining_error,
        RuntimeBackendError::Unavailable(RuntimeBackendLifecycle::Draining)
    ));
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
