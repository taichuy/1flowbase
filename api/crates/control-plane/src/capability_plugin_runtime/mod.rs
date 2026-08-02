use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ValidateCapabilityConfigInput {
    pub installation: domain::LocalPluginInstallationRecord,
    pub contribution_code: String,
    pub config_payload: Value,
}

#[derive(Debug, Clone)]
pub struct ResolveCapabilityOptionsInput {
    pub installation: domain::LocalPluginInstallationRecord,
    pub contribution_code: String,
    pub config_payload: Value,
}

#[derive(Debug, Clone)]
pub struct ResolveCapabilityOutputSchemaInput {
    pub installation: domain::LocalPluginInstallationRecord,
    pub contribution_code: String,
    pub config_payload: Value,
}

#[derive(Debug, Clone)]
pub struct ExecuteCapabilityNodeInput {
    pub installation: domain::LocalPluginInstallationRecord,
    pub contribution_code: String,
    pub config_payload: Value,
    pub input_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityExecutionOutput {
    pub output_payload: Value,
}

#[async_trait]
pub trait CapabilityPluginRuntimePort: Send + Sync {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> anyhow::Result<Value>;
    async fn resolve_dynamic_options(
        &self,
        input: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value>;
    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value>;
    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput>;
}
