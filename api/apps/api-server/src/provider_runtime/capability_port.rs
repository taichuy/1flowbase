use super::*;

#[async_trait]
impl CapabilityPluginRuntimePort for ApiProviderRuntime {
    async fn validate_config(&self, input: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_validate(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn resolve_dynamic_options(
        &self,
        input: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_resolve_dynamic_options(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn resolve_output_schema(
        &self,
        input: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        self.ensure_capability_loaded(&input.installation).await?;
        self.services
            .runtime_backend
            .capability_resolve_output_schema(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
            )
            .await
            .map_err(map_runtime_backend_error)
    }

    async fn execute_node(
        &self,
        input: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        let activity = self.start_runtime_activity(ApplicationActivityKind::ToolCall);
        self.ensure_capability_loaded(&input.installation).await?;
        let result = self
            .services
            .runtime_backend
            .capability_execute(
                &input.installation.plugin_id,
                &input.contribution_code,
                input.config_payload,
                input.input_payload,
            )
            .await
            .map(|output| CapabilityExecutionOutput {
                output_payload: output.output_payload,
                granted_credit_permissions: output.granted_credit_permissions,
            })
            .map_err(map_runtime_backend_error);
        finish_runtime_activity(activity, &result);
        result
    }
}
