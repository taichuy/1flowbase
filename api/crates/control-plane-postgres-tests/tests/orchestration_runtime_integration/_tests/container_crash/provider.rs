use super::*;
use control_plane::capability_plugin_runtime::*;
use control_plane::ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult,
    ProviderModelDescriptor,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone)]
pub(super) struct ReceiptProvider {
    pub address: String,
    pub run_id: Uuid,
    pub fallback: callback_resume::semantic_runtime::PausedProvider,
}
#[async_trait::async_trait]
impl ProviderRuntimePort for ReceiptProvider {
    async fn reconcile_provider_worker_demand(
        &self,
        installation: &domain::PluginInstallationRecord,
        revision: u64,
        selectable: Option<bool>,
    ) -> anyhow::Result<()> {
        self.fallback
            .reconcile_provider_worker_demand(installation, revision, selectable)
            .await
    }
    async fn ensure_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<()> {
        self.fallback.ensure_loaded(installation).await
    }
    async fn validate_provider(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config: Value,
    ) -> anyhow::Result<Value> {
        self.fallback.validate_provider(installation, config).await
    }
    async fn list_models(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
        config: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        self.fallback.list_models(installation, config).await
    }
    async fn invoke_stream(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        let identity =
            orchestration_runtime::execution_engine::container::scoped_execution_identity()
                .ok_or_else(|| {
                    anyhow::anyhow!("receipt provider must execute in a canonical container scope")
                })?;
        let mut connection = tokio::net::TcpStream::connect(&self.address).await?;
        let mut request =
            serde_json::to_vec(&json!({"flow_run_id":self.run_id,"execution_identity":identity}))?;
        request.push(b'\n');
        connection.write_all(&request).await?;
        let mut response = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            BufReader::new(connection).read_line(&mut response),
        )
        .await??;
        anyhow::ensure!(
            response.trim() == "recorded",
            "external receipt was not acknowledged"
        );
        Ok(ProviderRuntimeInvocationOutput {
            events: vec![],
            result: ProviderInvocationResult {
                final_content: Some("external effect confirmed".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
        })
    }
}
#[async_trait::async_trait]
impl CapabilityPluginRuntimePort for ReceiptProvider {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        self.fallback.validate_config(input).await
    }
    async fn resolve_dynamic_options(
        &self,
        input: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        self.fallback.resolve_dynamic_options(input).await
    }
    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        self.fallback.resolve_output_schema(input).await
    }
    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        self.fallback.execute_node(input).await
    }
}
