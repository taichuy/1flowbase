use super::*;
use extension_contracts::{
    PluginEphemeralBinding, PluginEphemeralPort, PluginEphemeralRequest, PluginEphemeralResponse,
};
use std::sync::Mutex;
pub(super) struct Ephemeral(pub(super) Mutex<Vec<PluginEphemeralBinding>>);
impl PluginEphemeralPort for Ephemeral {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginEphemeralBinding,
        _: &'a PluginEphemeralRequest,
    ) -> extension_contracts::PluginEphemeralFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(binding.clone());
            Ok(PluginEphemeralResponse {
                value: Some(serde_json::json!("fixture-secret")),
            })
        })
    }
}
struct NoData;
impl PluginDataPort for NoData {
    fn execute<'a>(
        &'a self,
        _: &'a PluginDataBinding,
        _: &'a PluginDataRequest,
    ) -> extension_contracts::PluginDataFuture<'a> {
        Box::pin(async { panic!("ephemeral callback reached data port") })
    }
}
pub(super) fn context(port: Arc<Ephemeral>) -> ProviderHostCallContext {
    let binding = PluginEphemeralBinding {
        contribution_id: "ssh.settings".into(),
        installation_id: "installation".into(),
        publisher_namespace: "trusted".into(),
        plugin_code: "ssh".into(),
        plugin_version: "1.0.0".into(),
        scope_id: "scope".into(),
        deadline_unix_ms: now_unix_ms() + 10_000,
    };
    ProviderHostCallContext {
        binding: PluginDataBinding {
            record_permissions: None,
            managed_subject: None,
            publisher_namespace: "trusted".into(),
            plugin_code: "ssh".into(),
            plugin_version: "1.0.0".into(),
            storage_binding: "main".into(),
            workspace_id: "scope".into(),
            actor_id: None,
            provider_instance_id: "instance".into(),
            permissions: Default::default(),
            deadline_unix_ms: binding.deadline_unix_ms,
        },
        plugin_data: Arc::new(NoData),
        file_read: None,
        application_logs: None,
        http_egress: None,
        file_import: None,
        execution_gate: None,
        workspace_governance: None,
        plugin_ephemeral: Some((binding, port)),
        plugin_credentials: None,
    }
}
#[tokio::test]
async fn ephemeral_use_only_host_identity_and_require_granted_live_context() {
    let port = Arc::new(Ephemeral(Mutex::new(vec![])));
    let request = serde_json::json!({"operation":"get","key":"host-1"});
    let response = execute_callback(
        MultiplexHostService::PluginEphemeralV1,
        request.clone(),
        Some(context(port.clone())),
    )
    .await;
    assert_eq!(response["result"]["value"], "fixture-secret");
    assert_eq!(port.0.lock().unwrap()[0].publisher_namespace, "trusted");
    let denied = execute_callback(
        MultiplexHostService::PluginEphemeralV1,
        request.clone(),
        None,
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_ephemeral_not_granted");
    let mut forged = request.clone();
    forged["plugin_code"] = serde_json::json!("other");
    let denied = execute_callback(
        MultiplexHostService::PluginEphemeralV1,
        forged,
        Some(context(port.clone())),
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_ephemeral_request_invalid");
    let mut expired = context(port.clone());
    expired
        .plugin_ephemeral
        .as_mut()
        .unwrap()
        .0
        .deadline_unix_ms = 0;
    let denied = execute_callback(
        MultiplexHostService::PluginEphemeralV1,
        request,
        Some(expired),
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_ephemeral_deadline");
    assert_eq!(port.0.lock().unwrap().len(), 1);
}
