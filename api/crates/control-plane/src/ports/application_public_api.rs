use super::*;

use crate::application_public_api::mapping::{
    ApplicationApiMappingConfig, ApplicationApiMappingDraft, ApplicationOperationBindings,
};
use crate::application_public_api::publications::ApplicationPublicationJsDependencySnapshot;
use crate::application_public_api::workflow_schedule::WorkflowScheduleTriggerRecord;

#[derive(Debug, Clone)]
pub struct ReplaceApplicationApiMappingInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping: ApplicationApiMappingConfig,
    pub operation_bindings: ApplicationOperationBindings,
}

#[derive(Debug, Clone)]
pub struct CreateApplicationPublicationVersionInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping_snapshot: ApplicationApiMappingConfig,
    pub operation_bindings: ApplicationOperationBindings,
    pub extension_slug: Option<String>,
    pub api_enabled: bool,
    pub compiled_plan_id: Uuid,
    pub flow_id: Uuid,
    pub flow_version_id: Uuid,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub document_snapshot: serde_json::Value,
    pub runtime_profile_snapshot: serde_json::Value,
    pub output_selector: serde_json::Value,
    pub dependency_snapshot: Vec<ApplicationPublicationJsDependencySnapshot>,
}

#[derive(Debug, Clone)]
pub struct SetApplicationApiEnabledInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub api_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DeactivateApplicationPublicationsInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ReplaceWorkflowScheduleTriggerInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: serde_json::Value,
}

#[async_trait]
pub trait ApplicationApiMappingRepository: Send + Sync {
    async fn get_application_api_mapping(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Option<ApplicationApiMappingDraft>>;

    async fn load_application_api_mapping_application_id_by_extension_slug(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<Uuid>>;

    async fn replace_application_api_mapping(
        &self,
        input: &ReplaceApplicationApiMappingInput,
    ) -> anyhow::Result<ApplicationApiMappingDraft>;
}

#[async_trait]
pub trait WorkflowScheduleTriggerRepository: Send + Sync {
    async fn get_workflow_schedule_trigger(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Option<WorkflowScheduleTriggerRecord>>;

    async fn list_enabled_workflow_schedule_triggers(
        &self,
    ) -> anyhow::Result<Vec<WorkflowScheduleTriggerRecord>>;

    async fn replace_workflow_schedule_trigger(
        &self,
        input: &ReplaceWorkflowScheduleTriggerInput,
    ) -> anyhow::Result<WorkflowScheduleTriggerRecord>;
}

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
}

#[async_trait]
impl<T> ApplicationCompileContextRepository for T
where
    T: ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + ApplicationJsDependencySelectionRepository
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

#[async_trait]
pub trait ApplicationPublicationRepository: Send + Sync {
    async fn create_active_application_publication_version(
        &self,
        input: &CreateApplicationPublicationVersionInput,
    ) -> anyhow::Result<
        crate::application_public_api::publications::ApplicationPublicationVersionRecord,
    >;

    async fn get_application_publication_version(
        &self,
        publication_id: Uuid,
    ) -> anyhow::Result<
        Option<crate::application_public_api::publications::ApplicationPublicationVersionRecord>,
    >;

    async fn list_application_publication_versions(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<
        Vec<crate::application_public_api::publications::ApplicationPublicationVersionRecord>,
    >;

    async fn load_active_application_publication(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<
        Option<crate::application_public_api::publications::ApplicationPublicationVersionRecord>,
    >;

    async fn load_active_application_publication_by_extension_slug(
        &self,
        slug: &str,
    ) -> anyhow::Result<
        Option<crate::application_public_api::publications::ApplicationPublicationVersionRecord>,
    >;

    async fn list_enabled_extension_publications(
        &self,
    ) -> anyhow::Result<
        Vec<crate::application_public_api::publications::ApplicationPublicationVersionRecord>,
    >;

    async fn set_application_api_enabled(
        &self,
        input: &SetApplicationApiEnabledInput,
    ) -> anyhow::Result<()>;

    async fn deactivate_application_publication_versions(
        &self,
        input: &DeactivateApplicationPublicationsInput,
    ) -> anyhow::Result<()>;
}
