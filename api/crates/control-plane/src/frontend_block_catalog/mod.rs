use anyhow::Result;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, FrontendBlockCatalogRepository, RoleConsolePolicyReader},
};

const FRONTEND_BLOCKS_VIEW_OPERATION_ID: &str = "frontend_blocks.view";

pub struct ListFrontendBlockCatalogQuery {
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct FrontendBlockCatalogView {
    pub entries: Vec<domain::FrontendBlockCatalogEntry>,
}

pub struct FrontendBlockCatalogService<R> {
    repository: R,
}

impl<R> FrontendBlockCatalogService<R>
where
    R: AuthRepository + FrontendBlockCatalogRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_frontend_blocks(
        &self,
        query: ListFrontendBlockCatalogQuery,
    ) -> Result<FrontendBlockCatalogView> {
        let actor = self
            .repository
            .load_actor_context_for_user(query.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
                .await?;
            let group = domain::ConsolePolicyGroup::other("other.frontend-blocks")
                .expect("compiled frontend block policy group must be valid");
            let operation_id =
                domain::ConsoleOperationId::try_from(FRONTEND_BLOCKS_VIEW_OPERATION_ID)
                    .expect("compiled frontend block operation id must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        Ok(FrontendBlockCatalogView {
            entries: self
                .repository
                .list_workspace_frontend_blocks(actor.current_workspace_id)
                .await?,
        })
    }
}
