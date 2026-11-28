//! Debug startup uses the real service and PG store but must not call an upstream.
use control_plane::capability_plugin_runtime::*;
use control_plane::ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort};
use extension_contracts::provider_contract::{ProviderInvocationInput, ProviderModelDescriptor};
use serde_json::Value;

#[derive(Clone)]
pub(super) struct NoProvider;
#[async_trait::async_trait]
impl ProviderRuntimePort for NoProvider {
    async fn reconcile_provider_worker_demand(
        &self,
        _: &domain::PluginInstallationRecord,
        _: u64,
        _: Option<bool>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn ensure_loaded(
        &self,
        _: &domain::LocalPluginInstallationRecord,
    ) -> anyhow::Result<Option<domain::LoadedRuntimeArtifactIdentity>> {
        anyhow::bail!("unexpected provider")
    }
    async fn validate_provider(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected provider")
    }
    async fn list_models(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        anyhow::bail!("unexpected provider")
    }
    async fn invoke_stream(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        anyhow::bail!("unexpected provider")
    }
}
#[async_trait::async_trait]
impl CapabilityPluginRuntimePort for NoProvider {
    async fn validate_config(&self, _: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_dynamic_options(
        &self,
        _: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_output_schema(
        &self,
        _: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn execute_node(
        &self,
        _: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        anyhow::bail!("unexpected capability")
    }
}
