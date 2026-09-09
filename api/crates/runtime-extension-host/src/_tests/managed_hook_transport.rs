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
            interface_protocol: None,
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
        input: input.into(),
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
    workers.finish_unmount(&handle);
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
        "attack.observed_patch",
        "attack.correlation",
        "attack.flood",
        "crash",
        "attack.observer_deny",
    ] {
        let input = if matches!(handler, "attack.observer_deny" | "attack.observed_patch") {
            ManagedCreateHookInput::Completion {
                terminal: ManagedHookTerminal::Succeeded,
            }
        } else {
            before("model")
        };
        let mut workers = ManagedWorkers::default();
        let handle = workers
            .mount(identity(), fixture.binding(&input, handler))
            .unwrap_or_else(|error| panic!("{handler}: fixture mount failed: {error}"));
        // A flooding process sleeps 30 seconds after writing; the byte limit must abort it first.
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            workers
                .admit_hook(request(handle.clone(), input))
                .unwrap_or_else(|error| panic!("{handler}: fixture admission failed: {error}")),
        )
        .await
        .unwrap_or_else(|_| panic!("{handler}: invalid worker output did not terminate promptly"))
        .expect_err(&format!(
            "{handler}: malicious worker output must be rejected"
        ));
        assert!(
            !error.to_string().contains("deadline"),
            "{handler}: {error}"
        );
        tokio::time::timeout(
            Duration::from_secs(1),
            workers
                .unmount(&handle)
                .unwrap_or_else(|error| panic!("{handler}: fixture unmount failed: {error}"))
                .dispose(),
        )
        .await
        .unwrap_or_else(|_| panic!("{handler}: fixture cleanup did not finish"))
        .unwrap_or_else(|error| panic!("{handler}: fixture cleanup failed: {error}"));
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

#[cfg(unix)]
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_worker_cancel_and_reap() {
    use std::sync::Arc;
    let fixture = WorkerFixture::new();
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(identity(), fixture.binding(&before("model"), "sleep"))
        .unwrap();
    let mut admitted = Vec::new();
    for _ in 0..32 {
        admitted.push(
            workers
                .admit_hook(request(handle.clone(), before("model")))
                .unwrap(),
        );
    }
    let full = workers
        .admit_hook(request(handle.clone(), before("model")))
        .err()
        .unwrap();
    assert!(matches!(
        full,
        runtime_core::runtime_backend::RuntimeBackendError::Execution { .. }
    ));
    drop(admitted.pop());
    admitted.push(
        workers
            .admit_hook(request(handle.clone(), before("model")))
            .unwrap(),
    );
    let operation = tokio::spawn(admitted.pop().unwrap());
    let pid = fixture.pid().await;
    workers.close_admission().unwrap();
    assert!(workers
        .admit_hook(request(handle.clone(), before("model")))
        .is_err());
    let scope = workers.unmount(&handle).unwrap();
    assert!(tokio::time::timeout(Duration::ZERO, scope.dispose())
        .await
        .is_err());
    assert_eq!(
        workers.loaded_count(),
        1,
        "unfinished scopes stay registered"
    );
    operation.abort();
    let _ = operation.await;
    drop(admitted);
    tokio::time::timeout(Duration::from_secs(3), scope.dispose())
        .await
        .unwrap()
        .unwrap();
    assert_process_exited(pid).await;
    workers.finish_unmount(&handle);
    assert_eq!(workers.loaded_count(), 0);

    // A global managed budget counts admitted work, including never-polled futures.
    let budget = Arc::new(tokio::sync::Semaphore::new(128));
    let scopes = (0..5)
        .map(|i| crate::plugin_scope::PluginScope::managed(i, budget.clone()))
        .collect::<Vec<_>>();
    let leases = scopes[..4]
        .iter()
        .flat_map(|scope| (0..32).map(move |_| scope.admit().unwrap()))
        .collect::<Vec<_>>();
    assert!(scopes[4].admit().is_err());
    drop(leases);
    assert!(scopes[4].admit().is_ok());

    let flood = WorkerFixture::new();
    let mut flooded = ManagedWorkers::default();
    let flooded_handle = flooded
        .mount(identity(), flood.binding(&before("model"), "attack.flood"))
        .unwrap();
    assert!(flooded
        .admit_hook(request(flooded_handle.clone(), before("model")))
        .unwrap()
        .await
        .is_err());
    tokio::time::timeout(
        Duration::from_secs(3),
        flooded.unmount(&flooded_handle).unwrap().dispose(),
    )
    .await
    .unwrap()
    .unwrap();
    flooded.finish_unmount(&flooded_handle);

    // Identity capacity is independent of active calls and never evicts an old handle.
    let binding = fixture.binding(&before("model"), "sleep");
    let mut identities = ManagedWorkers::default();
    let mut first = None;
    for i in 0..4096 {
        let id = ManagedExecutionIdentity::new(
            ManagedInstallationId::new(format!("installation-{i}")).unwrap(),
            ManagedWorkspaceId::new("workspace-1").unwrap(),
            ContributionId::new("hook").unwrap(),
            ManagedArtifactFingerprint::from_bytes(b"artifact"),
            ManagedBindingFingerprint::from_bytes(b"binding"),
        );
        let handle = identities.mount(id, binding.clone()).unwrap();
        if first.is_none() {
            first = Some(handle);
        }
    }
    assert!(identities.mount(identity(), binding).is_err());
    assert!(identities
        .admit_hook(request(first.unwrap(), before("model")))
        .is_ok());
    assert_eq!(identities.loaded_count(), 4096);
    identities.close_admission().unwrap();
    for scope in identities.scopes() {
        scope.dispose().await.unwrap();
    }
    identities.clear_disposed();
}

#[cfg(unix)]
#[tokio::test]
async fn root_2007_ac_010_lane_budgets_capability_output_and_reap() {
    use std::os::unix::fs::PermissionsExt;
    for flood in [false, true] {
        let fixture = WorkerFixture::new();
        // This finite capability wire has no SDK managed Hook envelope. Exercise its actual
        // process path using an exec-only hostile peer, without changing the ordinary host path.
        std::fs::write(
            &fixture.executable,
            if flood {
                "#!/bin/sh\nprintf '%s' \"$$\" > \"$0.pid\"\nexec cat /dev/zero\n"
            } else {
                "#!/bin/sh\nprintf '%s' \"$$\" > \"$0.pid\"\nexec sleep 30\n"
            },
        )
        .unwrap();
        std::fs::set_permissions(&fixture.executable, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let mut binding = fixture.binding(&before("model"), "execute");
        binding.contribution.point_id = ExtensionPointId::new("acme.capability.execute").unwrap();
        let mut workers = ManagedWorkers::default();
        let handle = workers.mount(identity(), binding).unwrap();
        let operation = workers
            .execute(RuntimeManagedCapabilityRequest {
                handle: handle.clone(),
                principal: request(handle.clone(), before("model")).principal,
                config_payload: serde_json::json!({}),
                input_payload: serde_json::json!({}),
            })
            .unwrap();
        let operation = tokio::spawn(operation);
        let pid = fixture.pid().await;
        if flood {
            assert!(tokio::time::timeout(Duration::from_secs(3), operation)
                .await
                .unwrap()
                .unwrap()
                .is_err());
        } else {
            workers.close_admission().unwrap();
            operation.abort();
            assert!(operation.await.unwrap_err().is_cancelled());
        }
        tokio::time::timeout(
            Duration::from_secs(3),
            workers.unmount(&handle).unwrap().dispose(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_process_exited(pid).await;
        workers.finish_unmount(&handle);
        assert_eq!(workers.loaded_count(), 0);
    }
}

/// Root #2014 AC-004: actual generic worker wire, including malicious peer responses.
#[tokio::test]
async fn root_2014_interface_transport_real_sdk_and_peer_rejection() {
    use runtime_core::runtime_backend::RuntimeManagedHookInput;
    let fixture = WorkerFixture::new();
    let legacy = ManagedCreateHookInput::Completion {
        terminal: ManagedHookTerminal::Succeeded,
    };
    for handler in [
        "completion",
        "attack.identity",
        "attack.patch",
        "attack.correlation",
        "attack.observer_deny",
        "attack.phase",
    ] {
        let mut binding = fixture.binding(&legacy, handler);
        binding.contribution.point_id = ExtensionPointId::new(managed_interface_hook_point_id(
            "host_infrastructure.providers.view",
            HookPhase::Completion,
        ))
        .unwrap();
        let mut workers = ManagedWorkers::default();
        let handle = workers.mount(identity(), binding).unwrap();
        let mut call = request(handle.clone(), legacy.clone());
        call.input = RuntimeManagedHookInput::Interface {
            interface_id: "host_infrastructure.providers.view".into(),
            interface_version: "1".into(),
            input: ManagedInterfaceInput::Completion {
                terminal: ManagedHookTerminal::Succeeded,
            },
        };
        let result = workers.admit_hook(call).unwrap().await;
        if handler == "completion" {
            assert_eq!(result.unwrap(), ManagedHookOutcome::Observed);
        } else {
            assert!(result.is_err(), "malicious peer accepted: {handler}");
        }
        workers.unmount(&handle).unwrap().dispose().await.unwrap();
    }
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(identity(), fixture.binding(&legacy, "completion"))
        .unwrap();
    let mut call = request(handle.clone(), legacy);
    call.input = RuntimeManagedHookInput::Interface {
        interface_id: "different.interface".into(),
        interface_version: "1".into(),
        input: ManagedInterfaceInput::Completion {
            terminal: ManagedHookTerminal::Succeeded,
        },
    };
    assert!(
        workers.admit_hook(call).is_err(),
        "different bound interface must fail before spawn"
    );
    workers.unmount(&handle).unwrap().dispose().await.unwrap();
}

/// Root #2014 AC-015/016/018: exact installed binding and a fresh real SDK process per call.
#[tokio::test]
async fn root_2014_r3_probe_reference_worker_roundtrip() {
    use runtime_core::runtime_backend::{
        RuntimeArtifactReference, RuntimeManagedActivation, RuntimeManagedHookInput,
    };
    let fixture = WorkerFixture::new();
    let raw =
        include_str!("../../../../plugins/fixtures/northwind.lifecycle-auditor/manifest.yaml");
    let package_header = raw.split_once("\nmanaged:\n").unwrap().0;
    // Author documents are serialized; the parsed package model remains validation-only.
    let mut managed_document = serde_json::json!({
        "module": {
            "bus_version": "v1",
            "module_id": "northwind.lifecycle-auditor",
            "module_version": "1.0.0",
            "module_kind": "runtime",
            "contributions": [{
                "contribution_id": "northwind.lifecycle-auditor.i0.authorization",
                "contributor_module_id": "northwind.lifecycle-auditor",
                "point_id": managed_interface_hook_point_id("probe.values", HookPhase::Before),
                "contract_version": "1",
                "required_permissions": ["hook.interface.authorization"],
                "mode": "append"
            }]
        },
        "execution_bindings": [{
            "contribution_id": "northwind.lifecycle-auditor.i0.authorization",
            "execution_mode": "process_per_call",
            "interface_protocol": "reference-v2",
            "runtime": {"protocol": "stdio_json", "entry": "worker"},
            "handler": "trace.before"
        }]
    });
    let contract = ManagedProjectionContract {
        contract_id: "probe.values".into(),
        contract_version: "1".into(),
        schema: serde_json::json!({"type":"array","items":{"type":"string","maxLength":16384},"maxItems":256}),
    };
    let compiled = contract.compile_for_registration().unwrap();
    let value = serde_json::json!([
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16368),
        ""
    ]);
    let view = ManagedInterfaceReferenceView {
        contract: compiled.reference(),
        value,
    };
    compiled.validate_reference(&view).unwrap();
    for handler in [
        "trace.before",
        "attack.identity",
        "attack.patch",
        "attack.correlation",
        "attack.observer_deny",
        "attack.phase",
        "attack.protocol",
        "attack.fingerprint",
        "attack.flood",
    ] {
        managed_document["execution_bindings"][0]["handler"] = handler.into();
        let raw = format!("{package_header}\nmanaged: {managed_document}\n").into_bytes();
        let manifest =
            extension_package_runtime::parse_plugin_manifest(std::str::from_utf8(&raw).unwrap())
                .unwrap();
        std::fs::write(fixture.root.join("manifest.yaml"), &raw).unwrap();
        let managed = manifest.managed.as_ref().unwrap();
        let contribution = &managed.module.contributions[0].contribution_id;
        let bound = ManagedExecutionIdentity::new(
            ManagedInstallationId::new("installation-1").unwrap(),
            ManagedWorkspaceId::new("workspace-1").unwrap(),
            contribution.clone(),
            ManagedArtifactFingerprint::from_bytes(&raw),
            managed.execution_binding_fingerprint(contribution).unwrap(),
        );
        let activation = RuntimeManagedActivation {
            plugin_id: manifest.versioned_plugin_id().unwrap(),
            artifact: RuntimeArtifactReference::new("installation-1").unwrap(),
            identity: bound.clone(),
        };
        let loaded =
            crate::package_loader::PackageLoader::load_managed(&fixture.root, &activation).unwrap();
        assert_eq!(
            loaded.interface_protocol,
            Some(ManagedInterfaceProtocol::ReferenceV2)
        );
        let mut wrong = activation.clone();
        wrong.identity = ManagedExecutionIdentity::new(
            bound.installation_id().clone(),
            bound.workspace_id().clone(),
            contribution.clone(),
            bound.artifact_fingerprint().clone(),
            ManagedBindingFingerprint::from_bytes(b"forged-binding"),
        );
        assert!(crate::package_loader::PackageLoader::load_managed(&fixture.root, &wrong).is_err());
        let mut workers = ManagedWorkers::default();
        let handle = workers.mount(bound, loaded).unwrap();
        let mut call = request(handle.clone(), before("unused"));
        call.input = RuntimeManagedHookInput::InterfaceReference {
            interface_id: "probe.values".into(),
            interface_version: "1".into(),
            input: ManagedInterfaceReferenceInput::Before {
                input: view.clone(),
            },
        };
        let result = workers.admit_hook(call.clone()).unwrap().await;
        if handler == "trace.before" {
            assert_eq!(result.unwrap(), ManagedHookOutcome::Observed);
            assert_eq!(
                workers.admit_hook(call.clone()).unwrap().await.unwrap(),
                ManagedHookOutcome::Observed
            );
            let frames: Vec<ManagedInterfaceReferenceHostFrame> =
                std::fs::read_to_string(fixture.executable.with_extension("trace"))
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
            assert_eq!(frames.len(), 2);
            for frame in frames {
                let size = serde_json::to_vec(&frame).unwrap().len();
                assert!(
                    size > MANAGED_INTERFACE_MAX_FRAME_BYTES
                        && size <= MANAGED_INTERFACE_MAX_REQUEST_BYTES
                );
                let ManagedInterfaceReferenceInput::Before { input } = frame.input else {
                    panic!("wrong phase")
                };
                assert_eq!(input, view);
            }
            let pids: std::collections::BTreeSet<String> =
                std::fs::read_to_string(fixture.executable.with_extension("pids"))
                    .unwrap()
                    .lines()
                    .map(str::to_owned)
                    .collect();
            assert_eq!(
                pids.len(),
                2,
                "fresh worker per call, no resident schema cache"
            );
            let mut malformed = call.clone();
            if let RuntimeManagedHookInput::InterfaceReference {
                input: ManagedInterfaceReferenceInput::Before { input },
                ..
            } = &mut malformed.input
            {
                input.contract.schema_fingerprint = "malformed".into();
            }
            assert!(
                workers.admit_hook(malformed).is_err(),
                "malformed reference rejected before spawn"
            );
            call.input = RuntimeManagedHookInput::Interface {
                interface_id: "probe.values".into(),
                interface_version: "1".into(),
                input: ManagedInterfaceInput::Before {
                    input: ManagedInterfaceView {
                        contract: contract.clone(),
                        value: serde_json::json!(["old"]),
                    },
                },
            };
            assert!(
                workers.admit_hook(call).is_err(),
                "installed v2 rejects old input before spawn"
            );
        } else {
            assert!(
                result.is_err(),
                "malicious reference peer accepted: {handler}"
            );
        }
        workers.unmount(&handle).unwrap().dispose().await.unwrap();
    }
}
