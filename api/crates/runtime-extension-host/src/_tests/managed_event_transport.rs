//! Root #2007 AC-005. A real SDK binary exercises typed worker output and host admission.
use crate::managed_worker::{LoadedManagedBinding, ManagedWorkers};
use extension_contracts::*;
use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};
use runtime_core::runtime_backend::RuntimeManagedEventRequest;

fn fixture(
    handler: &str,
    publish: bool,
) -> (ManagedWorkers, ManagedExecutionHandle, std::path::PathBuf) {
    let source = std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE").expect("build runtime-extension-sdk --example managed_event_worker and set MANAGED_EVENT_WORKER_FIXTURE");
    let root = std::env::temp_dir().join(format!(
        "event-worker-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let executable = root.join("worker");
    std::fs::copy(source, &executable).unwrap();
    let identity = ManagedExecutionIdentity::new(
        ManagedInstallationId::new("installation").unwrap(),
        ManagedWorkspaceId::new("workspace").unwrap(),
        ContributionId::new("acme.composition-a.events").unwrap(),
        ManagedArtifactFingerprint::from_bytes(b"a"),
        ManagedBindingFingerprint::from_bytes(b"b"),
    );
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(
            identity.clone(),
            LoadedManagedBinding {
                plugin_id: "acme.composition-a".into(),
                executable_fingerprint: ManagedArtifactFingerprint::from_bytes(
                    &std::fs::read(&executable).unwrap(),
                ),
                runtime_executable: executable,
                execution_mode: PluginExecutionMode::ProcessPerCall,
                limits: PluginRuntimeLimits::default(),
                handler: handler.into(),
                contribution: ContributionDescriptor {
                    contribution_id: identity.contribution_id().clone(),
                    contributor_module_id: ModuleId::new("acme.composition-a").unwrap(),
                    point_id: ExtensionPointId::new(MANAGED_CREATE_EVENT_POINT).unwrap(),
                    contract_version: ContractVersion::new("1").unwrap(),
                    required_permissions: if publish {
                        vec!["event.subscribe", "event.publish"]
                    } else {
                        vec!["event.subscribe"]
                    }
                    .into_iter()
                    .map(|p| PermissionCode::new(p).unwrap())
                    .collect(),
                    mode: ContributionMode::Append,
                    ordering: Default::default(),
                },
            },
        )
        .unwrap();
    (workers, handle, root)
}
fn request(handle: ManagedExecutionHandle) -> RuntimeManagedEventRequest {
    RuntimeManagedEventRequest {
        handle,
        graph_fingerprint: "graph".into(),
        authority_revision: 3,
        deadline_unix_ms: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
            as i64
            + 10000,
        delivery: ManagedEventDelivery {
            event_id: "event".into(),
            contract_id: MANAGED_CREATE_EVENT_ID.into(),
            contract_version: "v1".into(),
            workspace_id: "workspace".into(),
            causation_id: "event".into(),
            correlation_id: "event".into(),
            payload: ManagedEventPayload {
                model_id: "model-1".into(),
                status: ManagedEventStatus::Committed,
                result_reference: None,
            },
        },
    }
}
#[tokio::test]
async fn root_2007_ac_005_event_authority_real_worker() {
    for (handler, publish) in [("ack", false), ("publish", true)] {
        let (mut workers, handle, root) = fixture(handler, publish);
        let result = workers
            .admit_event(request(handle.clone()))
            .unwrap()
            .await
            .unwrap();
        assert_eq!(
            matches!(result, ManagedEventOutcome::Publish { .. }),
            publish
        );
        let frame: ManagedEventHostFrame = serde_json::from_str(
            std::fs::read_to_string(root.join("worker.trace"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(frame.execution_identity, *handle.identity());
        assert_eq!(frame.generation, handle.generation());
        assert_eq!(frame.delivery.correlation_id, "event");
        assert!(serde_json::to_value(frame)
            .unwrap()
            .get("actor_id")
            .is_none());
        let mut wrong = request(handle.clone());
        wrong.delivery.workspace_id = "foreign".into();
        assert!(workers.admit_event(wrong).is_err());
        let mut wrong = request(handle.clone());
        wrong.delivery.contract_version = "1".into();
        assert!(workers.admit_event(wrong).is_err());
        let mut wrong = request(handle.clone());
        wrong.deadline_unix_ms = 1;
        assert!(workers.admit_event(wrong).is_err());
        let forged = ManagedExecutionHandle::new(handle.identity().clone(), handle.generation());
        assert!(workers.admit_event(request(forged)).is_err());
        workers.unmount(&handle).unwrap().dispose().await.unwrap();
        assert!(workers.admit_event(request(handle)).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[tokio::test]
async fn root_2007_ac_005_event_authority_untrusted_output() {
    for handler in [
        "attack.identity",
        "attack.workspace",
        "attack.correlation",
        "attack.version",
        "attack.flood",
        "publish",
    ] {
        let (mut workers, handle, root) = fixture(handler, false);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            workers.admit_event(request(handle.clone())).unwrap(),
        )
        .await
        .expect("host must reject malformed worker promptly");
        assert!(result.is_err(), "{handler}");
        workers.unmount(&handle).unwrap().dispose().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
