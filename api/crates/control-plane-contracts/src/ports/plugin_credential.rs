use async_trait::async_trait;
use extension_contracts::{
    PluginCredentialBinding, PluginCredentialError, PluginCredentialRequest,
    PluginCredentialResponse,
};

/// Storage only: the control-plane owner holds current authority through this call.
#[async_trait]
pub trait PluginCredentialRepository: Send + Sync {
    async fn execute_plugin_credential(
        &self,
        binding: &PluginCredentialBinding,
        request: &PluginCredentialRequest,
    ) -> Result<PluginCredentialResponse, PluginCredentialError>;
}
