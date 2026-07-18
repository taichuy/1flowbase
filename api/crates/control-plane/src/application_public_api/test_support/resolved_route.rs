use super::*;

#[async_trait]
impl run_service::PublishedProviderManifestCapabilityRepository
    for ApplicationPublicApiTestRepository
{
    async fn supports_published_generate(
        &self,
        _workspace_id: Uuid,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        profile: run_service::GenerateExecutionProfile,
    ) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.published_generate_capability_checks += 1;
        inner.published_generate_capability_profiles.push(profile);
        Ok(inner
            .published_generate_capability_supported
            .unwrap_or(true))
    }

    async fn supports_published_count_tokens(
        &self,
        _workspace_id: Uuid,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
    ) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.published_count_tokens_capability_checks += 1;
        Ok(inner
            .published_count_tokens_capability_supported
            .unwrap_or(true))
    }
}
