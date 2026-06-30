use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::ControlPlaneError;
use crate::{
    application_public_api::{
        ensure_application_edit_permission, ensure_application_view_permission,
    },
    ports::{
        ApplicationApiMappingRepository, ApplicationPublicationRepository, ApplicationRepository,
        ReplaceApplicationApiMappingInput,
    },
};

#[derive(Debug, Clone)]
pub struct GetApplicationApiMappingCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct ReplaceApplicationApiMappingCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
    pub mapping: ApplicationApiMappingConfig,
}

pub struct ApplicationApiMappingService<R> {
    repository: R,
}

impl<R> ApplicationApiMappingService<R>
where
    R: ApplicationRepository + ApplicationApiMappingRepository + ApplicationPublicationRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_mapping(
        &self,
        command: GetApplicationApiMappingCommand,
    ) -> Result<ApplicationApiMappingConfig> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_view_permission(&actor, &application)?;

        Ok(self
            .repository
            .get_application_api_mapping(application.id)
            .await?
            .unwrap_or_else(ApplicationApiMappingConfig::default_native))
    }

    pub async fn replace_mapping(
        &self,
        command: ReplaceApplicationApiMappingCommand,
    ) -> Result<ApplicationApiMappingConfig> {
        validate_application_api_mapping(&command.mapping)?;
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
        if let Some(slug) = command.mapping.extension_slug() {
            if let Some(existing_application_id) = self
                .repository
                .load_application_api_mapping_application_id_by_extension_slug(slug)
                .await?
            {
                if existing_application_id != application.id {
                    return Err(ControlPlaneError::Conflict("extension_slug").into());
                }
            }
            if let Some(existing_publication) = self
                .repository
                .load_active_application_publication_by_extension_slug(slug)
                .await?
            {
                if existing_publication.application_id != application.id {
                    return Err(ControlPlaneError::Conflict("extension_slug").into());
                }
            }
        }

        self.repository
            .replace_application_api_mapping(&ReplaceApplicationApiMappingInput {
                actor_user_id: command.actor_user_id,
                application_id: application.id,
                mapping: command.mapping,
            })
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingConfig {
    pub input: ApplicationApiMappingInput,
    pub output: ApplicationApiMappingOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<WorkflowExtensionApiConfig>,
}

impl ApplicationApiMappingConfig {
    pub fn default_native() -> Self {
        Self {
            input: ApplicationApiMappingInput {
                query_target: "node-start.query".to_string(),
                model_target: Some("node-start.model".to_string()),
                inputs_target: Some("node-start".to_string()),
                history_target: Some("node-start.history".to_string()),
                attachments_target: Some("node-start.files".to_string()),
            },
            output: ApplicationApiMappingOutput::default(),
            extension: None,
        }
    }

    pub fn extension_slug(&self) -> Option<&str> {
        self.extension
            .as_ref()
            .map(|extension| extension.slug.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingInput {
    pub query_target: String,
    pub model_target: Option<String>,
    pub inputs_target: Option<String>,
    pub history_target: Option<String>,
    pub attachments_target: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationApiMappingOutput {
    pub answer_selector: Option<String>,
    pub usage_selector: Option<String>,
    pub files_selector: Option<String>,
    pub error_selector: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExtensionApiConfig {
    pub slug: String,
    pub method: WorkflowExtensionHttpMethod,
    pub response_mode: WorkflowExtensionResponseMode,
    #[serde(default)]
    pub parameters: Vec<WorkflowExtensionParameterMapping>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkflowExtensionHttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl WorkflowExtensionHttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExtensionResponseMode {
    Sync,
    Async,
}

impl WorkflowExtensionResponseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::Async => "async",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExtensionParameterMapping {
    pub name: String,
    pub source: WorkflowExtensionParameterSource,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExtensionParameterSource {
    Path,
    Query,
    Form,
    Body,
}

pub fn validate_application_api_mapping(mapping: &ApplicationApiMappingConfig) -> Result<()> {
    validate_required_selector("query_target", &mapping.input.query_target)?;
    validate_optional_selector("model_target", mapping.input.model_target.as_deref())?;
    validate_optional_selector("inputs_target", mapping.input.inputs_target.as_deref())?;
    validate_optional_selector("history_target", mapping.input.history_target.as_deref())?;
    validate_optional_selector(
        "attachments_target",
        mapping.input.attachments_target.as_deref(),
    )?;
    validate_optional_selector("answer_selector", mapping.output.answer_selector.as_deref())?;
    validate_optional_selector("usage_selector", mapping.output.usage_selector.as_deref())?;
    validate_optional_selector("files_selector", mapping.output.files_selector.as_deref())?;
    validate_optional_selector("error_selector", mapping.output.error_selector.as_deref())?;
    if let Some(extension) = &mapping.extension {
        validate_extension_api_config(extension)?;
    }
    Ok(())
}

fn validate_extension_api_config(extension: &WorkflowExtensionApiConfig) -> Result<()> {
    validate_extension_slug(&extension.slug)?;
    let mut seen = std::collections::BTreeSet::new();
    for parameter in &extension.parameters {
        validate_parameter_name(&parameter.name)?;
        validate_required_selector("parameter_target", &parameter.target)?;
        let source_key = (parameter.source, parameter.name.as_str());
        if !seen.insert(source_key) {
            return Err(ControlPlaneError::InvalidInput("parameter").into());
        }
    }

    Ok(())
}

fn validate_extension_slug(slug: &str) -> Result<()> {
    let valid = !slug.is_empty()
        && slug.len() <= 63
        && slug.chars().enumerate().all(|(index, character)| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || (index > 0 && matches!(character, '-' | '_'))
        });

    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("extension.slug").into())
    }
}

fn validate_parameter_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("parameter.name").into())
    }
}

fn validate_required_selector(field: &'static str, selector: &str) -> Result<()> {
    if selector.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    validate_selector_syntax(selector)
}

fn validate_optional_selector(field: &'static str, selector: Option<&str>) -> Result<()> {
    let Some(selector) = selector else {
        return Ok(());
    };
    if selector.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    validate_selector_syntax(selector)
}

fn validate_selector_syntax(selector: &str) -> Result<()> {
    if selector.trim() != selector
        || selector.contains('*')
        || selector.contains('[')
        || selector.contains(']')
        || selector.contains('(')
        || selector.contains(')')
        || selector.contains('?')
    {
        return Err(ControlPlaneError::InvalidInput("selector").into());
    }

    let valid = selector.split('.').all(|part| {
        !part.is_empty()
            && part.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
    });
    if !valid {
        return Err(ControlPlaneError::InvalidInput("selector").into());
    }

    Ok(())
}
