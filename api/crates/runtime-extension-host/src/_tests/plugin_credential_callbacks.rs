use super::*;
use extension_contracts::{
    PluginCredentialBinding, PluginCredentialPort, PluginCredentialRequest,
    PluginCredentialResponse,
};
use std::sync::Mutex;
struct Credentials(Mutex<Vec<PluginCredentialBinding>>);
impl PluginCredentialPort for Credentials {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginCredentialBinding,
        _: &'a PluginCredentialRequest,
    ) -> extension_contracts::PluginCredentialFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(binding.clone());
            Ok(PluginCredentialResponse {
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
        Box::pin(async { panic!("credential callback reached data port") })
    }
}
fn context(port: Arc<Credentials>) -> ProviderHostCallContext {
    let binding = PluginCredentialBinding {
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
        plugin_credentials: Some((binding, port)),
    }
}
#[tokio::test]
async fn credentials_use_only_host_identity_and_require_granted_live_context() {
    let port = Arc::new(Credentials(Mutex::new(vec![])));
    let request = serde_json::json!({"operation":"get","credential_id":"host-1"});
    let response = execute_callback(
        MultiplexHostService::PluginCredentialV1,
        request.clone(),
        Some(context(port.clone())),
    )
    .await;
    assert_eq!(response["result"]["value"], "fixture-secret");
    assert_eq!(port.0.lock().unwrap()[0].publisher_namespace, "trusted");
    let denied = execute_callback(
        MultiplexHostService::PluginCredentialV1,
        request.clone(),
        None,
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_credential_not_granted");
    let mut forged = request.clone();
    forged["plugin_code"] = serde_json::json!("other");
    let denied = execute_callback(
        MultiplexHostService::PluginCredentialV1,
        forged,
        Some(context(port.clone())),
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_credential_request_invalid");
    let mut expired = context(port.clone());
    expired
        .plugin_credentials
        .as_mut()
        .unwrap()
        .0
        .deadline_unix_ms = 0;
    let denied = execute_callback(
        MultiplexHostService::PluginCredentialV1,
        request,
        Some(expired),
    )
    .await;
    assert_eq!(denied["error"]["code"], "plugin_credential_deadline");
    assert_eq!(port.0.lock().unwrap().len(), 1);
}
