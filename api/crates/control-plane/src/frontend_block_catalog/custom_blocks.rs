use super::FrontendContributionBinding;
use crate::errors::ControlPlaneError;
use anyhow::Result;
use control_plane_contracts::ports::custom_frontend_blocks::{
    CustomFrontendBlock, CustomFrontendBlockBody, CustomFrontendBlockRepository,
};
use uuid::Uuid;

pub struct CustomFrontendBlockService<R>(pub R);
impl<R: CustomFrontendBlockRepository> CustomFrontendBlockService<R> {
    pub async fn list(&self, workspace_id: Uuid) -> Result<Vec<CustomFrontendBlock>> {
        self.0.list_custom_frontend_blocks(workspace_id).await
    }
    pub async fn create(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
        contribution_code: &str,
        body: CustomFrontendBlockBody,
        actor_user_id: Uuid,
        available: &[FrontendContributionBinding],
    ) -> Result<CustomFrontendBlock> {
        let body = validate(body)?;
        let base = available
            .iter()
            .find(|base| {
                base.catalog_entry.provider_code == provider_code
                    && base.catalog_entry.contribution_code == contribution_code
                    && base.disable_reason.is_none()
            })
            .ok_or(ControlPlaneError::NotFound("frontend_block"))?;
        if base.catalog_entry.code_template.is_none() {
            return Err(ControlPlaneError::InvalidInput("frontend_block_not_copyable").into());
        }
        self.0
            .create_custom_frontend_block(
                workspace_id,
                provider_code,
                contribution_code,
                &body,
                actor_user_id,
            )
            .await
    }
    pub async fn update(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        body: CustomFrontendBlockBody,
        actor_user_id: Uuid,
    ) -> Result<CustomFrontendBlock> {
        self.0
            .update_custom_frontend_block(workspace_id, id, &validate(body)?, actor_user_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("custom_frontend_block").into())
    }
    pub async fn delete(&self, workspace_id: Uuid, id: Uuid) -> Result<()> {
        if !self
            .0
            .delete_custom_frontend_block(workspace_id, id)
            .await?
        {
            return Err(ControlPlaneError::NotFound("custom_frontend_block").into());
        }
        Ok(())
    }
}
fn validate(mut body: CustomFrontendBlockBody) -> Result<CustomFrontendBlockBody> {
    body.title = body.title.trim().to_owned();
    if body.title.is_empty()
        || body.source.trim().is_empty()
        || !matches!(body.language.as_str(), "jsx" | "tsx")
    {
        return Err(ControlPlaneError::InvalidInput("invalid_custom_frontend_block").into());
    }
    Ok(body)
}

/// Definitions reuse current authority; saved source is never an execution grant.
pub fn resolve_custom_blocks(
    bases: &[FrontendContributionBinding],
    definitions: Vec<CustomFrontendBlock>,
) -> Vec<(CustomFrontendBlock, FrontendContributionBinding)> {
    definitions
        .into_iter()
        .filter_map(|definition| {
            let mut binding = bases
                .iter()
                .find(|base| {
                    base.catalog_entry.provider_code == definition.provider_code
                        && base.catalog_entry.contribution_code == definition.contribution_code
                        && base.disable_reason.is_none()
                })?
                .clone();
            binding.catalog_entry.title = definition.title.clone();
            binding.catalog_entry.code_template = Some(definition.source.clone());
            binding.catalog_entry.code_template_language = Some(definition.language.clone());
            binding.catalog_entry.code_template_version =
                Some(format!("custom-{}-r{}", definition.id, definition.revision));
            Some((definition, binding))
        })
        .collect()
}
