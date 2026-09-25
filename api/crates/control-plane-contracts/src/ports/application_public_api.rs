use super::*;
use std::{future::Future, pin::Pin, sync::Arc};

use crate::application_public_api::{
    ApplicationApiMappingConfig, ApplicationApiMappingDraft,
    ApplicationPublicationJsDependencySnapshot, ApplicationPublicationVersionRecord,
    WorkflowScheduleTriggerRecord,
};

pub type PublishedPlanLoader = Box<
    dyn FnOnce() -> Pin<
            Box<dyn Future<Output = anyhow::Result<Option<domain::CompiledPlanRecord>>> + Send>,
        > + Send,
>;

pub type PublishedPublicationLoader = Box<
    dyn FnOnce() -> Pin<
            Box<
                dyn Future<Output = anyhow::Result<Option<ApplicationPublicationVersionRecord>>>
                    + Send,
            >,
        > + Send,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveApplicationPublication {
    pub publication_id: Uuid,
    pub compiled_plan_id: Uuid,
    pub api_enabled: bool,
}

#[async_trait]
pub trait PublishedPublicationCache: Send + Sync {
    /// The publication row ID is reused on republish; the compiled plan ID is new.
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPublicationLoader,
    ) -> anyhow::Result<Option<Arc<ApplicationPublicationVersionRecord>>>;
}

/// Cache only immutable compiled plans, keyed by their publication-bound ID.
/// The caller must still resolve the active publication from durable storage.
#[async_trait]
pub trait PublishedPlanCache: Send + Sync {
    async fn get_or_load(
        &self,
        compiled_plan_id: Uuid,
        loader: PublishedPlanLoader,
    ) -> anyhow::Result<Option<Arc<domain::CompiledPlanRecord>>>;
}

#[derive(Debug, Clone)]
pub struct ReplaceApplicationApiMappingInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping: ApplicationApiMappingConfig,
}

#[derive(Debug, Clone)]
pub struct CreateApplicationPublicationVersionInput {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping_snapshot: ApplicationApiMappingConfig,
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
pub trait ApplicationPublicationRepository: Send + Sync {
    async fn load_active_application_publication_identity(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Option<ActiveApplicationPublication>> {
        Ok(self
            .load_active_application_publication(application_id)
            .await?
            .map(|publication| ActiveApplicationPublication {
                publication_id: publication.id,
                compiled_plan_id: publication.compiled_plan_id,
                api_enabled: publication.api_enabled,
            }))
    }

    async fn create_active_application_publication_version(
        &self,
        input: &CreateApplicationPublicationVersionInput,
    ) -> anyhow::Result<ApplicationPublicationVersionRecord>;

    async fn get_application_publication_version(
        &self,
        publication_id: Uuid,
    ) -> anyhow::Result<Option<ApplicationPublicationVersionRecord>>;

    async fn list_application_publication_versions(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Vec<ApplicationPublicationVersionRecord>>;

    async fn load_active_application_publication(
        &self,
        application_id: Uuid,
    ) -> anyhow::Result<Option<ApplicationPublicationVersionRecord>>;

    async fn load_active_application_publication_by_extension_slug(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<ApplicationPublicationVersionRecord>>;

    async fn list_enabled_extension_publications(
        &self,
    ) -> anyhow::Result<Vec<ApplicationPublicationVersionRecord>>;

    async fn set_application_api_enabled(
        &self,
        input: &SetApplicationApiEnabledInput,
    ) -> anyhow::Result<()>;

    async fn deactivate_application_publication_versions(
        &self,
        input: &DeactivateApplicationPublicationsInput,
    ) -> anyhow::Result<()>;
}
