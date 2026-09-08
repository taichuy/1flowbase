//! Root #2007 AC-003/004 (AUTH-03/04/06/09 execution boundaries).
//! Requires the actual SDK example executable via MANAGED_HOOK_WORKER_FIXTURE; never skips.
use crate::managed_worker::{LoadedManagedBinding, ManagedWorkers};
use extension_contracts::*;
use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};
use runtime_core::runtime_backend::{
    RuntimeExecutionPrincipal, RuntimeManagedCapabilityRequest, RuntimeManagedHookRequest,
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct WorkerFixture {
    root: PathBuf,
    executable: PathBuf,
}
impl WorkerFixture {
    fn new() -> Self {
        let source = std::env::var_os("MANAGED_HOOK_WORKER_FIXTURE").expect("build runtime-extension-sdk --example managed_hook_worker and set MANAGED_HOOK_WORKER_FIXTURE to that executable");
        let root = std::env::temp_dir().join(format!(
            "managed-hook-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let executable = root.join("worker");
        std::fs::copy(source, &executable).unwrap();
        Self { root, executable }
    }
    fn binding(&self, input: &ManagedCreateHookInput, handler: &str) -> LoadedManagedBinding {
        LoadedManagedBinding {
            plugin_id: "publisher/plugin/1".into(),
            executable_fingerprint: ManagedArtifactFingerprint::from_bytes(
                &std::fs::read(&self.executable).unwrap(),
            ),
            runtime_executable: self.executable.clone(),
            execution_mode: PluginExecutionMode::ProcessPerCall,
            limits: PluginRuntimeLimits::default(),
            handler: handler.into(),
            contribution: ContributionDescriptor {
                contribution_id: ContributionId::new("hook").unwrap(),
                contributor_module_id: ModuleId::new("publisher.plugin").unwrap(),
                point_id: ExtensionPointId::new(input.point_id()).unwrap(),
                contract_version: ContractVersion::new("1").unwrap(),
                required_permissions: Default::default(),
                mode: ContributionMode::Append,
                ordering: Default::default(),
            },
        }
    }
    async fn pid(&self) -> i32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(raw) = std::fs::read_to_string(self.executable.with_extension("pid")) {
                    break raw.parse().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("worker must start and report PID")
    }
}
impl Drop for WorkerFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn identity() -> ManagedExecutionIdentity {
    ManagedExecutionIdentity::new(
        ManagedInstallationId::new("installation-1").unwrap(),
        ManagedWorkspaceId::new("workspace-1").unwrap(),
        ContributionId::new("hook").unwrap(),
        ManagedArtifactFingerprint::from_bytes(b"artifact-1"),
        ManagedBindingFingerprint::from_bytes(b"binding-1"),
    )
}
fn before(code: &str) -> ManagedCreateHookInput {
    ManagedCreateHookInput::Before {
        create: ManagedCreateView {
            code: code.into(),
            template_provider: "core".into(),
            template_code: "general".into(),
            template_version: "1".into(),
        },
    }
}
fn request(
    handle: ManagedExecutionHandle,
    input: ManagedCreateHookInput,
) -> RuntimeManagedHookRequest {
    RuntimeManagedHookRequest {
        handle,
        input,
        principal: RuntimeExecutionPrincipal {
            workspace_id: "workspace-1".into(),
            actor_id: Some("actor-1".into()),
            deadline_unix_ms: now_ms() + 10_000,
        },
        invocation: ManagedHookInvocation {
            invocation_id: "invocation-1".into(),
            registry_fingerprint: "registry-g1".into(),
            graph_fingerprint: "graph-g1".into(),
            authority_revision: 7,
        },
    }
}
fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}

#[tokio::test]
async fn root_2007_ac_003_004_hook_transport_real_sdk_roundtrip_and_identity() {
    let fixture = WorkerFixture::new();
    let create = ManagedCreateView {
        code: "model".into(),
        template_provider: "core".into(),
        template_code: "general".into(),
        template_version: "1".into(),
    };
    for input in [
        ManagedCreateHookInput::Authorization {
            create: create.clone(),
        },
        ManagedCreateHookInput::Admission { create },
        before("model"),
        ManagedCreateHookInput::After {
            model_id: "model-1".into(),
        },
        ManagedCreateHookInput::Failure {
            classification: "failed".into(),
        },
        ManagedCreateHookInput::Completion {
            terminal: ManagedHookTerminal::Cancelled,
        },
    ] {
        let mut workers = ManagedWorkers::default();
        let handle = workers
            .mount(identity(), fixture.binding(&input, "verify_context"))
            .unwrap();
        let expected = if input.is_observer() {
            ManagedHookOutcome::Observed
        } else {
            ManagedHookOutcome::Continue
        };
        assert_eq!(
            workers
                .admit_hook(request(handle.clone(), input))
                .unwrap()
                .await
                .unwrap(),
            expected
        );
        workers.unmount(&handle).unwrap().dispose().await.unwrap();
    }
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(
            identity(),
            fixture.binding(&before("denied"), "verify_context"),
        )
        .unwrap();
    assert_eq!(
        workers
            .admit_hook(request(handle, before("denied")))
            .unwrap()
            .await
            .unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.denied".into()
        }
    );
}

#[tokio::test]
async fn root_2007_ac_003_004_hook_transport_scope_generation_phase_and_deadline() {
    let fixture = WorkerFixture::new();
    let mut workers = ManagedWorkers::default();
    let binding = fixture.binding(&before("model"), "verify_context");
    let handle = workers.mount(identity(), binding.clone()).unwrap();
    let mut wrong = request(handle.clone(), before("model"));
    wrong.principal.workspace_id = "workspace-2".into();
    assert!(workers.admit_hook(wrong).is_err());
    let forged = ManagedExecutionHandle::new(identity(), handle.generation());
    assert!(workers
        .admit_hook(request(forged, before("model")))
        .is_err());
    assert!(workers
        .admit_hook(request(
            handle.clone(),
            ManagedCreateHookInput::Completion {
                terminal: ManagedHookTerminal::Succeeded
            }
        ))
        .is_err());
    let mut expired = request(handle.clone(), before("model"));
    expired.principal.deadline_unix_ms = now_ms() - 1;
    assert!(workers.admit_hook(expired).is_err());
    assert!(workers
        .execute(RuntimeManagedCapabilityRequest {
            handle: handle.clone(),
            principal: request(handle.clone(), before("model")).principal,
            config_payload: serde_json::json!({}),
            input_payload: serde_json::json!({})
        })
        .is_err());
    workers.unmount(&handle).unwrap().dispose().await.unwrap();
    let replacement = workers.mount(identity(), binding).unwrap();
    assert_ne!(handle.generation(), replacement.generation());
    assert!(workers
        .admit_hook(request(handle, before("model")))
        .is_err());
    assert_eq!(
        workers
            .admit_hook(request(replacement, before("model")))
            .unwrap()
            .await
            .unwrap(),
        ManagedHookOutcome::Continue
    );
}

#[tokio::test]
async fn root_2007_ac_003_004_hook_transport_untrusted_output_is_bounded_and_rejected() {
    let fixture = WorkerFixture::new();
    for handler in [
        "attack.identity",
        "attack.patch",
        "attack.correlation",
        "attack.flood",
        "crash",
        "attack.observer_deny",
    ] {
        let input = if handler == "attack.observer_deny" {
            ManagedCreateHookInput::Completion {
                terminal: ManagedHookTerminal::Succeeded,
            }
        } else {
            before("model")
        };
        let mut workers = ManagedWorkers::default();
        let handle = workers
            .mount(identity(), fixture.binding(&input, handler))
            .unwrap();
        // A flooding process sleeps 30 seconds after writing; the byte limit must abort it first.
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            workers.admit_hook(request(handle.clone(), input)).unwrap(),
        )
        .await
        .expect("invalid worker output must terminate promptly")
        .unwrap_err();
        assert!(
            !error.to_string().contains("deadline"),
            "{handler}: {error}"
        );
        tokio::time::timeout(
            Duration::from_secs(1),
            workers.unmount(&handle).unwrap().dispose(),
        )
        .await
        .unwrap()
        .unwrap();
    }
}

#[cfg(unix)]
async fn assert_process_exited(pid: i32) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while unsafe { libc::kill(pid, 0) == 0 } {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cancelled/timed-out worker must exit");
}

#[cfg(unix)]
#[tokio::test]
async fn root_2007_ac_003_004_hook_transport_cancel_timeout_and_scope_cleanup() {
    for cancel in [true, false] {
        let fixture = WorkerFixture::new();
        let mut workers = ManagedWorkers::default();
        let handle = workers
            .mount(identity(), fixture.binding(&before("model"), "sleep"))
            .unwrap();
        let mut invocation = request(handle.clone(), before("model"));
        invocation.principal.deadline_unix_ms = now_ms() + if cancel { 10_000 } else { 2_000 };
        let operation = tokio::spawn(workers.admit_hook(invocation).unwrap());
        let pid = fixture.pid().await;
        if cancel {
            operation.abort();
            assert!(operation.await.unwrap_err().is_cancelled());
        } else {
            assert!(operation
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("deadline"));
        }
        tokio::time::timeout(
            Duration::from_secs(1),
            workers.unmount(&handle).unwrap().dispose(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_process_exited(pid).await;
    }
    // An admitted but never polled execution cannot retain a scope after its owner drops it.
    let fixture = WorkerFixture::new();
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(identity(), fixture.binding(&before("model"), "sleep"))
        .unwrap();
    let pending = workers
        .admit_hook(request(handle.clone(), before("model")))
        .unwrap();
    let scope = workers.unmount(&handle).unwrap();
    drop(pending);
    tokio::time::timeout(Duration::from_secs(1), scope.dispose())
        .await
        .unwrap()
        .unwrap();
}

/// AC-008: actual per-call SDK workers can coexist without substituting a new artifact/handler
/// into an already admitted generation. The filesystem marker fixes the interleaving.
#[tokio::test]
async fn root_2007_ac_008_snapshot_restart_exact_worker_versions() {
    let first = WorkerFixture::new();
    let second = WorkerFixture::new();
    let mut workers = ManagedWorkers::default();
    let g1 = workers
        .mount(identity(), first.binding(&before("model"), "first"))
        .unwrap();
    let running = tokio::spawn(
        workers
            .admit_hook(request(g1.clone(), before("fixture_barrier")))
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        while !first.executable.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let old_identity = identity();
    let g2_identity = ManagedExecutionIdentity::new(
        old_identity.installation_id().clone(),
        old_identity.workspace_id().clone(),
        old_identity.contribution_id().clone(),
        ManagedArtifactFingerprint::from_bytes(b"artifact-2"),
        ManagedBindingFingerprint::from_bytes(b"binding-2"),
    );
    let g2 = workers
        .mount(g2_identity, second.binding(&before("model"), "second"))
        .unwrap();
    let mut new_request = request(g2.clone(), before("identify"));
    new_request.invocation.graph_fingerprint = "graph-g2".into();
    assert_eq!(
        workers.admit_hook(new_request).unwrap().await.unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.second".into()
        }
    );
    assert!(!running.is_finished());
    std::fs::write(first.executable.with_extension("release"), "release").unwrap();
    assert_eq!(
        running.await.unwrap().unwrap(),
        ManagedHookOutcome::Continue
    );
    assert_eq!(
        workers
            .admit_hook(request(g1.clone(), before("identify")))
            .unwrap()
            .await
            .unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.first".into()
        }
    );
    assert_ne!(g1.generation(), g2.generation());
    assert_eq!(workers.loaded_count(), 2);
    let frozen = workers.executable_for_handle(&g1).unwrap();
    std::fs::remove_file(&first.executable).unwrap();
    assert!(
        frozen.verify_executable().is_err(),
        "old artifact absence must not resolve G2"
    );
    workers.unmount(&g1).unwrap().dispose().await.unwrap();
    assert!(workers.admit_hook(request(g1, before("identify"))).is_err());
    assert_eq!(
        workers
            .admit_hook(request(g2.clone(), before("identify")))
            .unwrap()
            .await
            .unwrap(),
        ManagedHookOutcome::Deny {
            classification: "fixture.second".into()
        }
    );
    workers.unmount(&g2).unwrap().dispose().await.unwrap();
}
