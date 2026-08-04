use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRuntimeInvocationOutput {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
}

#[derive(Debug, Clone)]
pub struct ProviderLiveEventSenders {
    pub required: tokio::sync::mpsc::Sender<ProviderStreamEvent>,
    pub diagnostic: tokio::sync::mpsc::Sender<ProviderStreamEvent>,
}

#[async_trait]
pub trait ProviderRuntimePort: Send + Sync {
    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()>;
    async fn validate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
    async fn list_models(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: serde_json::Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>>;
    async fn get_balance(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        provider_config: serde_json::Value,
    ) -> anyhow::Result<ProviderBalanceResult> {
        let _ = installation;
        let _ = provider_config;
        anyhow::bail!("provider balance is not implemented by this runtime")
    }
    async fn count_tokens(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderCountTokensInput,
    ) -> anyhow::Result<ProviderCountTokensResult> {
        let _ = installation;
        let _ = input;
        anyhow::bail!("provider CountTokens is not implemented by this runtime")
    }
    async fn compact(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderCompactResult> {
        let _ = installation;
        let _ = input;
        anyhow::bail!("provider Compact is not implemented by this runtime")
    }
    async fn invoke_stream(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput>;
    async fn invoke_stream_with_live_events(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
        live_events: Option<ProviderLiveEventSenders>,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let _ = live_events;
        self.invoke_stream(installation, input).await
    }
}
