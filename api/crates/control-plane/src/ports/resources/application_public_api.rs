use super::*;

pub use control_plane_contracts::ports::application_public_api::*;

#[async_trait]
pub trait ApplicationCompiledPlanRepository: Send + Sync {
    async fn upsert_application_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> anyhow::Result<domain::CompiledPlanRecord>;

    async fn get_application_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> anyhow::Result<Option<domain::CompiledPlanRecord>>;
}

#[async_trait]
pub trait ApplicationCompileContextRepository: Send + Sync {
    async fn build_application_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<orchestration_runtime::compiler::FlowCompileContext>;

    async fn build_application_compile_context_with_cache(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        _cache_store: Option<&dyn CacheStore>,
    ) -> anyhow::Result<orchestration_runtime::compiler::FlowCompileContext> {
        self.build_application_compile_context(workspace_id, application_id)
            .await
    }
}

#[async_trait]
impl<T> ApplicationCompileContextRepository for T
where
    T: ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + ApplicationJsDependencySelectionRepository
        + ApplicationRepository
        + Send
        + Sync,
{
    async fn build_application_compile_context(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<orchestration_runtime::compiler::FlowCompileContext> {
        crate::orchestration_runtime::compile_context::build_application_compile_context(
            self,
            workspace_id,
            application_id,
        )
        .await
    }

    async fn build_application_compile_context_with_cache(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        cache_store: Option<&dyn CacheStore>,
    ) -> anyhow::Result<orchestration_runtime::compiler::FlowCompileContext> {
        crate::orchestration_runtime::compile_context::build_application_compile_context_with_cache(
            self,
            workspace_id,
            application_id,
            cache_store,
        )
        .await
    }
}

#[async_trait]
impl<T> ApplicationCompiledPlanRepository for T
where
    T: OrchestrationRuntimeRepository + Send + Sync,
{
    async fn upsert_application_compiled_plan(
        &self,
        input: &UpsertCompiledPlanInput,
    ) -> anyhow::Result<domain::CompiledPlanRecord> {
        OrchestrationRuntimeRepository::upsert_compiled_plan(self, input).await
    }

    async fn get_application_compiled_plan(
        &self,
        compiled_plan_id: Uuid,
    ) -> anyhow::Result<Option<domain::CompiledPlanRecord>> {
        OrchestrationRuntimeRepository::get_compiled_plan(self, compiled_plan_id).await
    }
}
