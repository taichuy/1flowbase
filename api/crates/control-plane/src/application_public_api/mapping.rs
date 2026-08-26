use anyhow::Result;
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

pub use control_plane_contracts::application_public_api::{
    ApplicationApiMappingConfig, ApplicationApiMappingDraft, ApplicationApiMappingInput,
    ApplicationApiMappingOutput, WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
    WorkflowExtensionResponseMode,
};

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
        ensure_application_view_permission(&self.repository, &actor, &application).await?;

        Ok(self
            .get_mapping_draft_for_application(application.id)
            .await?
            .mapping)
    }

    pub async fn get_mapping_draft(
        &self,
        command: GetApplicationApiMappingCommand,
    ) -> Result<ApplicationApiMappingDraft> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_view_permission(&self.repository, &actor, &application).await?;

        self.get_mapping_draft_for_application(application.id).await
    }

    pub async fn replace_mapping(
        &self,
        command: ReplaceApplicationApiMappingCommand,
    ) -> Result<ApplicationApiMappingConfig> {
        Ok(self.replace_mapping_draft(command).await?.mapping)
    }

    pub async fn replace_mapping_draft(
        &self,
        command: ReplaceApplicationApiMappingCommand,
    ) -> Result<ApplicationApiMappingDraft> {
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
        ensure_application_edit_permission(&self.repository, &actor, &application).await?;
        let current_draft = self
            .repository
            .get_application_api_mapping(application.id)
            .await?;
        // The read model supplies a native default for an absent draft, but no
        // extension registration exists until a draft has been persisted.
        if let Some(current_draft) = current_draft.as_ref() {
            ensure_extension_registration_unchanged(&current_draft.mapping, &command.mapping)?;
        }
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

    async fn get_mapping_draft_for_application(
        &self,
        application_id: Uuid,
    ) -> Result<ApplicationApiMappingDraft> {
        Ok(self
            .repository
            .get_application_api_mapping(application_id)
            .await?
            .unwrap_or_else(ApplicationApiMappingDraft::default_native))
    }
}

pub(crate) fn ensure_extension_registration_unchanged(
    current: &ApplicationApiMappingConfig,
    requested: &ApplicationApiMappingConfig,
) -> Result<()> {
    let unchanged = match (&current.extension, &requested.extension) {
        (None, None) => true,
        (Some(current), Some(requested)) => {
            current.slug == requested.slug
                && current.method == requested.method
                && current.response_mode == requested.response_mode
        }
        _ => false,
    };

    if unchanged {
        Ok(())
    } else {
        Err(ControlPlaneError::Conflict("workflow_extension_registration_immutable").into())
    }
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
    Ok(())
}

fn validate_extension_slug(slug: &str) -> Result<()> {
    let valid = !slug.is_empty()
        && slug.len() <= 255
        && slug.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.len() <= 63
                && (valid_extension_literal_segment(segment)
                    || valid_extension_placeholder_segment(segment))
        });

    if valid {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("extension.slug").into())
    }
}

fn valid_extension_literal_segment(segment: &str) -> bool {
    segment.chars().enumerate().all(|(index, character)| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || (index > 0 && matches!(character, '-' | '_'))
    })
}

fn valid_extension_placeholder_segment(segment: &str) -> bool {
    let Some(name) = segment
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
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
