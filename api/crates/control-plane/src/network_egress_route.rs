use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    network_egress_pool::{ensure_global_network_egress_pool, GLOBAL_NETWORK_EGRESS_POOL_ID},
    ports::{
        CreateNetworkEgressRouteInput, NetworkEgressPoolRepository, NetworkEgressRepository,
        NetworkEgressRouteRepository, UpdateNetworkEgressRouteInput,
    },
};

pub struct CreateNetworkEgressRouteCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub selector: domain::NetworkEgressConsumerSelector,
    pub pool_id: Uuid,
    pub enabled: bool,
}

pub struct UpdateNetworkEgressRouteCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub route_id: Uuid,
    pub pool_id: Uuid,
    pub enabled: bool,
}

/// The single semantic lookup used by D4 consumers. It only chooses a durable route; acquiring
/// a short-lived provider lease belongs to the API runtime boundary in the consumer packets.
pub struct NetworkEgressRouteService<R> {
    repository: R,
}

impl<R> NetworkEgressRouteService<R>
where
    R: NetworkEgressRouteRepository + NetworkEgressPoolRepository + NetworkEgressRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list(&self, workspace_id: Uuid) -> Result<Vec<domain::NetworkEgressRoute>> {
        self.repository
            .list_network_egress_routes(workspace_id)
            .await
    }

    pub async fn create(
        &self,
        command: CreateNetworkEgressRouteCommand,
    ) -> Result<domain::NetworkEgressRoute> {
        ensure_global_network_egress_pool(&self.repository, command.actor_user_id).await?;
        self.require_pool(command.pool_id).await?;
        let route = self
            .repository
            .create_network_egress_route(&CreateNetworkEgressRouteInput {
                route_id: Uuid::now_v7(),
                workspace_id: command.workspace_id,
                selector: command.selector,
                pool_id: command.pool_id,
                enabled: command.enabled,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            command.workspace_id,
            route.id,
            "network_egress_route.created",
        )
        .await?;
        Ok(route)
    }

    pub async fn update(
        &self,
        command: UpdateNetworkEgressRouteCommand,
    ) -> Result<domain::NetworkEgressRoute> {
        ensure_global_network_egress_pool(&self.repository, command.actor_user_id).await?;
        self.require_pool(command.pool_id).await?;
        let route = self
            .repository
            .update_network_egress_route(&UpdateNetworkEgressRouteInput {
                workspace_id: command.workspace_id,
                route_id: command.route_id,
                pool_id: command.pool_id,
                enabled: command.enabled,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            command.workspace_id,
            route.id,
            "network_egress_route.updated",
        )
        .await?;
        Ok(route)
    }

    pub async fn delete(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        route_id: Uuid,
    ) -> Result<()> {
        self.repository
            .delete_network_egress_route(workspace_id, route_id)
            .await?;
        self.audit(
            actor_user_id,
            workspace_id,
            route_id,
            "network_egress_route.deleted",
        )
        .await
    }

    pub async fn resolve_enabled(
        &self,
        workspace_id: Uuid,
        selector: &domain::NetworkEgressConsumerSelector,
    ) -> Result<Option<domain::NetworkEgressRoute>> {
        for candidate in matching_selectors(selector) {
            if let Some(route) = self
                .repository
                .find_enabled_network_egress_route(workspace_id, &candidate)
                .await?
            {
                return Ok(Some(route));
            }
        }
        Ok(None)
    }

    async fn require_pool(&self, pool_id: Uuid) -> Result<()> {
        if pool_id != GLOBAL_NETWORK_EGRESS_POOL_ID {
            return Err(ControlPlaneError::Conflict("network_egress_global_pool_only").into());
        }
        if self
            .repository
            .get_network_egress_pool(pool_id)
            .await?
            .is_some()
        {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("network_egress_pool").into())
        }
    }

    async fn audit(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        route_id: Uuid,
        event_type: &'static str,
    ) -> Result<()> {
        self.repository
            .append_audit_log(&audit_log(
                Some(workspace_id),
                Some(actor_user_id),
                "network_egress_route",
                Some(route_id),
                event_type,
                serde_json::json!({}),
            ))
            .await
    }
}

fn matching_selectors(
    selector: &domain::NetworkEgressConsumerSelector,
) -> Vec<domain::NetworkEgressConsumerSelector> {
    match selector {
        domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id } => vec![
            domain::NetworkEgressConsumerSelector::ModelProviderInstance {
                instance_id: *instance_id,
            },
            domain::NetworkEgressConsumerSelector::ModelProviderDefault,
        ],
        selector => vec![selector.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nc_09_selector_is_closed_and_model_provider_exact_precedes_workspace_default() {
        let instance_id = Uuid::now_v7();
        assert_eq!(
            matching_selectors(
                &domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id }
            ),
            vec![
                domain::NetworkEgressConsumerSelector::ModelProviderInstance { instance_id },
                domain::NetworkEgressConsumerSelector::ModelProviderDefault,
            ]
        );
        assert_eq!(
            matching_selectors(&domain::NetworkEgressConsumerSelector::GithubOfficialSources),
            vec![domain::NetworkEgressConsumerSelector::GithubOfficialSources]
        );
        assert!(
            domain::NetworkEgressConsumerSelector::from_storage("github", Some(instance_id))
                .is_err()
        );
    }
}
