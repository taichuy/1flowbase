use crate::managed_worker::{LoadedManagedBinding, ManagedWorkers};
use extension_contracts::*;
use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};
use runtime_core::runtime_backend::{RuntimeExecutionPrincipal, RuntimeManagedCapabilityRequest};
use std::{path::PathBuf, sync::Arc};

#[derive(Default)]
struct DataPort(tokio::sync::Mutex<Vec<PluginDataBinding>>);
impl PluginDataPort for DataPort {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        _: &'a PluginDataRequest,
    ) -> PluginDataFuture<'a> {
        Box::pin(async move {
            self.0.lock().await.push(binding.clone());
            Ok(PluginDataResponse {
                results: vec![PluginDataOperationResult::Count { count: 3 }],
                replayed: false,
            })
        })
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
#[cfg(unix)]
async fn managed_capability_multiplex_binds_data_and_rejects_stale_or_foreign_scope() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!(
        "managed-capability-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let _cleanup = Fixture(root.clone());
    let executable = root.join("worker.py");
    std::fs::write(&executable, r#"#!/usr/bin/env python3
import json, sys
call=json.loads(sys.stdin.readline())
assert call['request']['method']=='execute'
assert call['request']['input']['handler']=='list'
print(json.dumps({'protocol':'stdio_json_multiplex_v1','kind':'callback','call_id':call['call_id'],'callback_id':'1','service':'plugin_data_v1','request':{'operations':[{'operation':'count','target':{'kind':'owned_collection','collection_code':'hosts'},'filters':[]}]}}),flush=True)
reply=json.loads(sys.stdin.readline())
print(json.dumps({'protocol':'stdio_json_multiplex_v1','kind':'response','call_id':call['call_id'],'response':{'ok':True,'result':reply['response']}}),flush=True)
sys.stdin.read()
"#).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let identity = ManagedExecutionIdentity::new(
        ManagedInstallationId::new("installation").unwrap(),
        ManagedWorkspaceId::new("system-scope").unwrap(),
        ContributionId::new("service.list").unwrap(),
        ManagedArtifactFingerprint::from_bytes(b"manifest"),
        ManagedBindingFingerprint::from_bytes(b"binding"),
    );
    let mut workers = ManagedWorkers::default();
    let handle = workers
        .mount(
            identity.clone(),
            LoadedManagedBinding {
                plugin_id: "fixture.service@1.0.0".into(),
                publisher_namespace: "fixture".into(),
                plugin_code: "fixture.service".into(),
                plugin_version: "1.0.0".into(),
                protocol: STDIO_JSON_MULTIPLEX_V1.into(),
                runtime_executable: executable.clone(),
                executable_fingerprint: ManagedArtifactFingerprint::from_bytes(
                    &std::fs::read(&executable).unwrap(),
                ),
                execution_mode: PluginExecutionMode::ProcessPerCall,
                limits: PluginRuntimeLimits {
                    timeout_ms: Some(5000),
                    ..Default::default()
                },
                handler: "list".into(),
                interface_protocol: None,
                contribution: ContributionDescriptor {
                    contribution_id: identity.contribution_id().clone(),
                    contributor_module_id: ModuleId::new("fixture.service").unwrap(),
                    point_id: ExtensionPointId::new("1flowbase.managed-service.operation").unwrap(),
                    contract_version: ContractVersion::new("1").unwrap(),
                    required_permissions: [PermissionCode::new("plugin_data.owned.read").unwrap()]
                        .into(),
                    mode: ContributionMode::Append,
                    ordering: Default::default(),
                },
            },
        )
        .unwrap();
    let request = |scope: &str| RuntimeManagedCapabilityRequest {
        handle: handle.clone(),
        principal: RuntimeExecutionPrincipal {
            workspace_id: scope.into(),
            actor_id: Some("actor".into()),
            deadline_unix_ms: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
                as i64
                + 5000,
        },
        config_payload: serde_json::json!({}),
        input_payload: serde_json::json!({}),
    };
    let port = Arc::new(DataPort::default());
    assert!(workers
        .execute_with_services(request("foreign"), port.clone(), None)
        .is_err());
    let output = workers
        .execute_with_services(request("system-scope"), port.clone(), None)
        .unwrap()
        .await
        .unwrap();
    let calls = port.0.lock().await;
    assert_eq!(calls.len(), 1, "callback must reach the real bound service");
    assert_eq!(calls[0].workspace_id, "system-scope");
    assert_eq!(calls[0].publisher_namespace, "fixture");
    assert_eq!(calls[0].provider_instance_id, "installation");
    assert!(output.to_string().contains('3'));
    drop(calls);
    workers.unmount(&handle).unwrap();
    assert!(workers
        .execute_with_services(request("system-scope"), port, None)
        .is_err());
}
