use anyhow::{anyhow, Result};
use orchestration_runtime::compiler::FlowCompiler;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::mapping::{
    validate_application_api_mapping, ApplicationApiMappingConfig, ApplicationApiMappingOutput,
};
use crate::{
    application_public_api::ensure_application_edit_permission,
    errors::ControlPlaneError,
    flow::{FlowService, SaveFlowDraftCommand, UpdateFlowVersionMetadataCommand},
    orchestration_runtime::inputs::{
        build_compiled_plan_input, flow_document_hash, flow_document_schema_version,
    },
    ports::{
        ApplicationApiMappingRepository, ApplicationCompileContextRepository,
        ApplicationCompiledPlanRepository, ApplicationJsDependencySelectionRepository,
        ApplicationPublicationRepository, ApplicationRepository,
        CreateApplicationPublicationVersionInput, FlowRepository, SetApplicationApiEnabledInput,
    },
};

#[derive(Debug, Clone)]
pub struct PublishApplicationCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping: ApplicationApiMappingConfig,
    pub api_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct LoadActiveApplicationPublicationCommand {
    pub application_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct SetApplicationApiEnabledCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub api_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationPublicationJsDependencySnapshot {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
    pub permissions: domain::JsDependencyPermissions,
}

impl From<domain::ApplicationJsDependencySelection> for ApplicationPublicationJsDependencySnapshot {
    fn from(selection: domain::ApplicationJsDependencySelection) -> Self {
        Self {
            installation_id: selection.installation_id,
            provider_code: selection.provider_code,
            plugin_id: selection.plugin_id,
            plugin_version: selection.plugin_version,
            alias: selection.alias,
            package: selection.package,
            version: selection.version,
            target: selection.target,
            artifact_path: selection.artifact_path,
            artifact_hash: selection.artifact_hash,
            integrity: selection.integrity,
            permissions: selection.permissions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationPublicationVersionRecord {
    pub id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub flow_version_id: Uuid,
    pub mapping_snapshot: ApplicationApiMappingConfig,
    pub extension_slug: Option<String>,
    pub compiled_plan_id: Uuid,
    pub version_sequence: i64,
    pub active: bool,
    pub api_enabled: bool,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub document_snapshot: serde_json::Value,
    pub runtime_profile_snapshot: serde_json::Value,
    pub output_selector: serde_json::Value,
    pub dependency_snapshot: Vec<ApplicationPublicationJsDependencySnapshot>,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
}

pub struct ApplicationPublicationService<R> {
    repository: R,
}

impl<R> ApplicationPublicationService<R>
where
    R: ApplicationRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn publish_active_version(
        &self,
        command: PublishApplicationCommand,
    ) -> Result<ApplicationPublicationVersionRecord>
    where
        R: ApplicationPublicationRepository
            + ApplicationApiMappingRepository
            + ApplicationCompiledPlanRepository
            + ApplicationCompileContextRepository
            + ApplicationJsDependencySelectionRepository
            + FlowRepository
            + Clone,
    {
        validate_application_api_mapping(&command.mapping)?;
        let output_selector = output_selector_snapshot(&command.mapping.output);
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_edit_permission(&actor, &application)?;
        let extension_slug = command.mapping.extension_slug().map(ToOwned::to_owned);
        if let Some(slug) = extension_slug.as_deref() {
            if let Some(existing_publication) = self
                .repository
                .load_active_application_publication_by_extension_slug(slug)
                .await?
            {
                if existing_publication.application_id != application.id {
                    return Err(ControlPlaneError::Conflict("extension_slug").into());
                }
            }
            if let Some(existing_application_id) = self
                .repository
                .load_application_api_mapping_application_id_by_extension_slug(slug)
                .await?
            {
                if existing_application_id != application.id {
                    return Err(ControlPlaneError::Conflict("extension_slug").into());
                }
            }
        }
        let dependency_snapshot = self
            .repository
            .list_application_js_dependency_selections(application.workspace_id, application.id)
            .await?
            .into_iter()
            .map(ApplicationPublicationJsDependencySnapshot::from)
            .collect::<Vec<_>>();

        let flow_service = FlowService::new(self.repository.clone());
        let editor_state = flow_service
            .get_or_create_editor_state(command.actor_user_id, application.id)
            .await?;
        let frozen_state = flow_service
            .save_draft(SaveFlowDraftCommand {
                actor_user_id: command.actor_user_id,
                application_id: application.id,
                document: editor_state.draft.document.clone(),
                change_kind: domain::FlowChangeKind::Logical,
                summary: "Publish application public API".to_string(),
            })
            .await?;
        let flow_version = latest_flow_version(&frozen_state)?;
        let protected_state = flow_service
            .update_version_metadata(UpdateFlowVersionMetadataCommand {
                actor_user_id: command.actor_user_id,
                application_id: application.id,
                version_id: flow_version.id,
                summary: Some("Published application public API".to_string()),
                summary_is_custom: Some(false),
                is_protected: Some(true),
            })
            .await?;
        let protected_version = protected_state
            .versions
            .iter()
            .find(|version| version.id == flow_version.id)
            .cloned()
            .ok_or(ControlPlaneError::NotFound("flow_version"))?;
        let document = protected_state.draft.document.clone();
        let compile_context = self
            .repository
            .build_application_compile_context(application.workspace_id, application.id)
            .await?;
        let compiled_plan = match application.application_type {
            domain::ApplicationType::AgentFlow => FlowCompiler::compile(
                protected_state.flow.id,
                &protected_state.draft.id.to_string(),
                &document,
                &compile_context,
            )?,
            domain::ApplicationType::Workflow => FlowCompiler::compile_workflow(
                protected_state.flow.id,
                &protected_state.draft.id.to_string(),
                &document,
                &compile_context,
            )?,
        };
        let compiled_plan = self
            .repository
            .upsert_application_compiled_plan(&build_compiled_plan_input(
                command.actor_user_id,
                &protected_state,
                &compiled_plan,
                &document,
            )?)
            .await?;

        self.repository
            .create_active_application_publication_version(
                &CreateApplicationPublicationVersionInput {
                    actor_user_id: command.actor_user_id,
                    application_id: application.id,
                    mapping_snapshot: command.mapping,
                    extension_slug,
                    api_enabled: command.api_enabled,
                    compiled_plan_id: compiled_plan.id,
                    flow_id: protected_state.flow.id,
                    flow_version_id: protected_version.id,
                    flow_schema_version: flow_document_schema_version(&protected_state, &document),
                    document_hash: flow_document_hash(&document),
                    document_snapshot: document,
                    runtime_profile_snapshot: json!({}),
                    output_selector,
                    dependency_snapshot,
                },
            )
            .await
    }

    pub async fn get_publication_version(
        &self,
        publication_id: Uuid,
    ) -> Result<Option<ApplicationPublicationVersionRecord>>
    where
        R: ApplicationPublicationRepository,
    {
        self.repository
            .get_application_publication_version(publication_id)
            .await
    }

    pub async fn list_publication_versions(
        &self,
        application_id: Uuid,
    ) -> Result<Vec<ApplicationPublicationVersionRecord>>
    where
        R: ApplicationPublicationRepository,
    {
        self.repository
            .list_application_publication_versions(application_id)
            .await
    }

    pub async fn load_active_publication(
        &self,
        command: LoadActiveApplicationPublicationCommand,
    ) -> Result<ApplicationPublicationVersionRecord>
    where
        R: ApplicationPublicationRepository,
    {
        self.repository
            .load_active_application_publication(command.application_id)
            .await?
            .ok_or_else(|| anyhow!("application_not_published"))
    }

    pub async fn set_api_enabled(&self, command: SetApplicationApiEnabledCommand) -> Result<()>
    where
        R: ApplicationPublicationRepository,
    {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_edit_permission(&actor, &application)?;

        self.repository
            .set_application_api_enabled(&SetApplicationApiEnabledInput {
                actor_user_id: command.actor_user_id,
                application_id: application.id,
                api_enabled: command.api_enabled,
            })
            .await
    }
}

fn latest_flow_version(
    editor_state: &domain::FlowEditorState,
) -> Result<domain::FlowVersionRecord> {
    editor_state
        .versions
        .iter()
        .max_by_key(|version| version.sequence)
        .cloned()
        .ok_or_else(|| ControlPlaneError::NotFound("flow_version").into())
}

fn output_selector_snapshot(output: &ApplicationApiMappingOutput) -> serde_json::Value {
    json!({
        "answer_selector": output.answer_selector,
        "usage_selector": output.usage_selector,
        "files_selector": output.files_selector,
        "error_selector": output.error_selector,
    })
}
