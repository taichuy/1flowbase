use std::collections::BTreeSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::{
        ensure_application_non_crud_creation_operation,
        ensure_existing_application_non_crud_console_operation, ApplicationNonCrudConsoleOperation,
        ApplicationService, CreateApplicationCommand,
    },
    application_public_api::mapping::ApplicationApiMappingConfig,
    errors::ControlPlaneError,
    flow::{
        build_agent_flow_template_package, import_application_template_document,
        preview_application_template_package, validate_flow_draft_document,
        AgentFlowTemplateApplication, AgentFlowTemplateDependency, AgentFlowTemplatePackage,
        AgentFlowTemplatePreview, AgentFlowTemplateResourceSnapshot, ImportAgentFlowTemplateResult,
    },
    ports::{
        ApplicationApiMappingRepository, ApplicationRepository, CreateWorkflowTriggerConfig,
        FlowRepository, ReplaceApplicationApiMappingInput, WorkflowScheduleTriggerRepository,
    },
};

pub const APPLICATION_ARCHIVE_SCHEMA_VERSION: &str = "1flowbase.application-archive/v1";
const MAX_APPLICATION_ARCHIVE_ENTRIES: usize = 100;

#[derive(Debug, Clone)]
pub struct ExportApplicationArchiveCommand {
    pub actor_user_id: Uuid,
    pub application_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct PreviewApplicationArchiveCommand {
    pub actor_user_id: Uuid,
    pub entry: ApplicationArchiveEntry,
    pub resources: AgentFlowTemplateResourceSnapshot,
}

#[derive(Debug, Clone)]
pub struct ImportApplicationArchiveCommand {
    pub actor_user_id: Uuid,
    pub entry: ApplicationArchiveEntry,
    pub name: Option<String>,
    pub description: Option<String>,
    pub resources: AgentFlowTemplateResourceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationArchivePackage {
    pub schema_version: String,
    pub applications: Vec<ApplicationArchiveEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationArchiveEntry {
    pub application: ApplicationArchiveApplication,
    pub flow_document: serde_json::Value,
    #[serde(default)]
    pub dependencies: Vec<AgentFlowTemplateDependency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_trigger_config: Option<WorkflowTriggerTemplateConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationArchiveApplication {
    pub application_type: String,
    pub workflow_trigger_type: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowTriggerTemplateConfig {
    Schedule {
        cron: String,
        timezone: String,
        input_payload: serde_json::Value,
    },
    Extension {
        mapping: ApplicationApiMappingConfig,
    },
}

pub struct ApplicationArchiveService<R> {
    repository: R,
}

impl<R> ApplicationArchiveService<R>
where
    R: ApplicationRepository
        + FlowRepository
        + ApplicationApiMappingRepository
        + WorkflowScheduleTriggerRepository
        + Clone,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn export_archive(
        &self,
        command: ExportApplicationArchiveCommand,
    ) -> Result<ApplicationArchivePackage> {
        if command.application_ids.is_empty()
            || command.application_ids.len() > MAX_APPLICATION_ARCHIVE_ENTRIES
        {
            return Err(ControlPlaneError::InvalidInput("application_ids").into());
        }
        let unique_ids = command
            .application_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique_ids.len() != command.application_ids.len() {
            return Err(ControlPlaneError::InvalidInput("application_ids").into());
        }

        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let policies = if actor.is_root {
            Vec::new()
        } else {
            self.repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?
        };
        let mut authorized_applications = Vec::with_capacity(command.application_ids.len());
        for application_id in command.application_ids {
            let application = self
                .repository
                .get_application(actor.current_workspace_id, application_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("application"))?;
            ensure_existing_application_non_crud_console_operation(
                &actor,
                &application,
                &policies,
                ApplicationNonCrudConsoleOperation::OrchestrationTemplateExport,
            )?;
            authorized_applications.push(application);
        }

        let mut applications = Vec::with_capacity(authorized_applications.len());
        for application in authorized_applications {
            let flow_state = self
                .repository
                .get_or_create_editor_state(
                    actor.current_workspace_id,
                    application.id,
                    command.actor_user_id,
                )
                .await?;
            let template =
                build_agent_flow_template_package(&application, &flow_state.draft.document);
            let workflow_trigger_config = match application.workflow_trigger_type {
                Some(domain::WorkflowTriggerType::Schedule) => self
                    .repository
                    .get_workflow_schedule_trigger(application.id)
                    .await?
                    .map(|trigger| WorkflowTriggerTemplateConfig::Schedule {
                        cron: trigger.cron,
                        timezone: trigger.timezone,
                        input_payload: trigger.input_payload,
                    }),
                Some(domain::WorkflowTriggerType::Extension) => self
                    .repository
                    .get_application_api_mapping(application.id)
                    .await?
                    .filter(|draft| draft.mapping.extension.is_some())
                    .map(|draft| WorkflowTriggerTemplateConfig::Extension {
                        mapping: draft.mapping,
                    }),
                None => None,
            };

            applications.push(ApplicationArchiveEntry {
                application: ApplicationArchiveApplication {
                    application_type: application.application_type.as_str().to_string(),
                    workflow_trigger_type: application
                        .workflow_trigger_type
                        .map(|trigger| trigger.as_str().to_string()),
                    name: application.name,
                    description: application.description,
                    icon: application.icon,
                    icon_type: application.icon_type,
                    icon_background: application.icon_background,
                },
                flow_document: template.flow_document,
                dependencies: template.dependencies,
                workflow_trigger_config,
            });
        }

        Ok(ApplicationArchivePackage {
            schema_version: APPLICATION_ARCHIVE_SCHEMA_VERSION.to_string(),
            applications,
        })
    }
}

impl<R> ApplicationArchiveService<R>
where
    R: ApplicationRepository
        + FlowRepository
        + ApplicationApiMappingRepository
        + WorkflowScheduleTriggerRepository
        + Clone,
{
    pub async fn preview_archive(
        &self,
        command: PreviewApplicationArchiveCommand,
    ) -> Result<AgentFlowTemplatePreview> {
        self.repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        preview_application_template_package(
            archive_entry_template(command.entry),
            &command.resources,
        )
    }

    pub async fn import_archive(
        &self,
        command: ImportApplicationArchiveCommand,
    ) -> Result<ImportAgentFlowTemplateResult> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(
                    command.actor_user_id,
                    actor.current_workspace_id,
                )
                .await?;
            ensure_application_non_crud_creation_operation(
                &actor,
                &policies,
                ApplicationNonCrudConsoleOperation::OrchestrationTemplateImport,
            )?;
        }

        let template = archive_entry_template(command.entry.clone());
        let preview = preview_application_template_package(template.clone(), &command.resources)?;
        let application_type =
            parse_archive_application_type(&command.entry.application.application_type)?;
        let (workflow_trigger_type, workflow_trigger_config, extension_mapping) =
            archive_workflow_trigger_config(application_type, &command.entry)?;
        let application_name = command
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| preview.application.name.clone());
        let application_description = command
            .description
            .unwrap_or_else(|| preview.application.description.clone());
        let application = ApplicationService::new(self.repository.clone())
            .create_application_from_authorized_template_import(
                &actor,
                CreateApplicationCommand {
                    workflow_trigger_config,
                    actor_user_id: command.actor_user_id,
                    application_type,
                    workflow_trigger_type,
                    name: application_name,
                    description: application_description,
                    icon: preview.application.icon.clone(),
                    icon_type: preview.application.icon_type.clone(),
                    icon_background: preview.application.icon_background.clone(),
                },
            )
            .await?;
        if let Some(mapping) = extension_mapping {
            self.repository
                .replace_application_api_mapping(&ReplaceApplicationApiMappingInput {
                    actor_user_id: command.actor_user_id,
                    application_id: application.id,
                    mapping,
                })
                .await?;
        }
        let bootstrapped = self
            .repository
            .get_or_create_editor_state(
                actor.current_workspace_id,
                application.id,
                command.actor_user_id,
            )
            .await?;
        let (document, _) = import_application_template_document(
            &template,
            bootstrapped.flow.id,
            &command.resources,
        )?;
        validate_flow_draft_document(&document)?;
        let orchestration = self
            .repository
            .save_draft(
                actor.current_workspace_id,
                application.id,
                command.actor_user_id,
                document,
                domain::FlowChangeKind::Logical,
                "导入应用归档",
            )
            .await?;

        Ok(ImportAgentFlowTemplateResult {
            application,
            orchestration,
            preview,
        })
    }
}

fn archive_entry_template(entry: ApplicationArchiveEntry) -> AgentFlowTemplatePackage {
    AgentFlowTemplatePackage {
        schema_version: crate::flow::AGENT_FLOW_TEMPLATE_SCHEMA_VERSION.to_string(),
        application: AgentFlowTemplateApplication {
            application_type: entry.application.application_type,
            name: entry.application.name,
            description: entry.application.description,
            icon: entry.application.icon,
            icon_type: entry.application.icon_type,
            icon_background: entry.application.icon_background,
        },
        flow_document: entry.flow_document,
        dependencies: entry.dependencies,
    }
}

fn parse_archive_application_type(value: &str) -> Result<domain::ApplicationType> {
    match value {
        "agent_flow" => Ok(domain::ApplicationType::AgentFlow),
        "workflow" => Ok(domain::ApplicationType::Workflow),
        _ => Err(ControlPlaneError::InvalidInput("application.application_type").into()),
    }
}

fn archive_workflow_trigger_config(
    application_type: domain::ApplicationType,
    entry: &ApplicationArchiveEntry,
) -> Result<(
    Option<domain::WorkflowTriggerType>,
    Option<CreateWorkflowTriggerConfig>,
    Option<ApplicationApiMappingConfig>,
)> {
    if application_type == domain::ApplicationType::AgentFlow {
        if entry.application.workflow_trigger_type.is_some()
            || entry.workflow_trigger_config.is_some()
        {
            return Err(ControlPlaneError::InvalidInput("workflow_trigger_type").into());
        }
        return Ok((None, None, None));
    }

    match (
        entry.application.workflow_trigger_type.as_deref(),
        entry.workflow_trigger_config.clone(),
    ) {
        (
            Some("schedule"),
            Some(WorkflowTriggerTemplateConfig::Schedule {
                cron,
                timezone,
                input_payload,
            }),
        ) => Ok((
            Some(domain::WorkflowTriggerType::Schedule),
            Some(CreateWorkflowTriggerConfig::Schedule {
                cron,
                timezone,
                input_payload,
            }),
            None,
        )),
        (Some("extension"), Some(WorkflowTriggerTemplateConfig::Extension { mapping })) => {
            let extension = mapping
                .extension
                .as_ref()
                .ok_or(ControlPlaneError::InvalidInput("workflow_trigger_config"))?;
            Ok((
                Some(domain::WorkflowTriggerType::Extension),
                Some(CreateWorkflowTriggerConfig::Extension {
                    subpath: extension.slug.clone(),
                    http_method: extension.method.as_str().to_string(),
                    response_mode: extension.response_mode.as_str().to_string(),
                }),
                Some(mapping),
            ))
        }
        (Some("schedule"), None) => Ok((Some(domain::WorkflowTriggerType::Schedule), None, None)),
        (Some("extension"), None) => Ok((Some(domain::WorkflowTriggerType::Extension), None, None)),
        _ => Err(ControlPlaneError::InvalidInput("workflow_trigger_config").into()),
    }
}
