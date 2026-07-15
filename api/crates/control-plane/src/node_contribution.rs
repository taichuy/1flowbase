use anyhow::Result;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, NodeContributionRepository, RoleConsolePolicyReader},
};

const NODE_CONTRIBUTIONS_VIEW_OPERATION_ID: &str = "node_contributions.view";

pub struct ListNodeContributionsQuery {
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct NodeContributionListView {
    pub entries: Vec<domain::NodeContributionRegistryEntry>,
}

pub struct NodeContributionService<R> {
    repository: R,
}

impl<R> NodeContributionService<R>
where
    R: AuthRepository + NodeContributionRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_node_contributions(
        &self,
        query: ListNodeContributionsQuery,
    ) -> Result<NodeContributionListView> {
        let actor = self
            .repository
            .load_actor_context_for_user(query.actor_user_id)
            .await?;
        if !actor.is_root {
            let policies = self
                .repository
                .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
                .await?;
            let group = domain::ConsolePolicyGroup::other("other.node-contributions")
                .expect("compiled node contribution policy group must be valid");
            let operation_id =
                domain::ConsoleOperationId::try_from(NODE_CONTRIBUTIONS_VIEW_OPERATION_ID)
                    .expect("compiled node contribution operation id must be valid");
            if !domain::effective_console_simple_operation(&policies, &group, &operation_id) {
                return Err(ControlPlaneError::PermissionDenied("permission_denied").into());
            }
        }

        Ok(NodeContributionListView {
            entries: self
                .repository
                .list_node_contributions(actor.current_workspace_id)
                .await?,
        })
    }
}
