use super::*;
use plugin_framework::provider_contract::{ProviderCompactProfile, ProviderInvocationCapability};

#[async_trait]
impl run_service::PublishedProviderManifestCapabilityRepository
    for ApplicationPublicApiTestRepository
{
    async fn supports_published_generate(
        &self,
        _workspace_id: Uuid,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        profile: run_service::GenerateExecutionProfile,
        required_semantic_capabilities: &BTreeSet<ProviderInvocationCapability>,
    ) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.published_generate_capability_checks += 1;
        inner.published_generate_capability_profiles.push(profile);
        inner
            .published_generate_capability_requirements
            .push(required_semantic_capabilities.clone());
        Ok(inner
            .published_generate_manifest_capabilities
            .as_ref()
            .map_or(true, |declared_capabilities| {
                required_semantic_capabilities.is_subset(declared_capabilities)
            }))
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

    async fn supports_published_compact(
        &self,
        _workspace_id: Uuid,
        _runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        profile: ProviderCompactProfile,
    ) -> Result<bool> {
        let mut inner = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned");
        inner.published_compact_capability_checks += 1;
        inner.published_compact_capability_profiles.push(profile);
        Ok(inner.published_compact_capability_supported.unwrap_or(true))
    }
}
