use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
        NetworkEgressPoolRepository, NetworkEgressRepository, UpdateNetworkEgressPoolInput,
        UpdateNetworkEgressPoolMemberInput,
    },
};

pub struct CreateNetworkEgressPoolCommand {
    pub actor_user_id: Uuid,
    pub display_name: String,
}

pub struct UpdateNetworkEgressPoolCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub display_name: String,
}

pub struct CreateNetworkEgressPoolMemberCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub provider_id: Uuid,
    pub provider_egress_key: String,
    pub enabled: bool,
    pub sequence: i32,
}

pub struct UpdateNetworkEgressPoolMemberCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub member_id: Uuid,
    pub enabled: bool,
    pub sequence: i32,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressPoolMemberView {
    pub member: domain::NetworkEgressPoolMember,
    pub health: domain::NetworkEgressPoolMemberHealth,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressPoolView {
    pub pool: domain::NetworkEgressPool,
    pub members: Vec<NetworkEgressPoolMemberView>,
}

/// The stable member reference selected for a future runtime lease acquisition.
#[derive(Debug, Clone)]
pub struct NetworkEgressPoolSelection {
    pub pool_id: Uuid,
    pub member_id: Uuid,
    pub provider_id: Uuid,
    pub provider_egress_key: String,
}

pub struct NetworkEgressPoolService<R> {
    repository: R,
}

impl<R> NetworkEgressPoolService<R>
where
    R: NetworkEgressPoolRepository + NetworkEgressRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list(&self) -> Result<Vec<NetworkEgressPoolView>> {
        let pools = self.repository.list_network_egress_pools().await?;
        let mut views = Vec::with_capacity(pools.len());
        for pool in pools {
            views.push(self.view(pool).await?);
        }
        Ok(views)
    }

    pub async fn create(
        &self,
        command: CreateNetworkEgressPoolCommand,
    ) -> Result<NetworkEgressPoolView> {
        let pool = self
            .repository
            .create_network_egress_pool(&CreateNetworkEgressPoolInput {
                pool_id: Uuid::now_v7(),
                display_name: required_text(&command.display_name, "display_name")?,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            pool.id,
            "network_egress_pool.created",
        )
        .await?;
        Ok(NetworkEgressPoolView {
            pool,
            members: Vec::new(),
        })
    }

    pub async fn update(
        &self,
        command: UpdateNetworkEgressPoolCommand,
    ) -> Result<NetworkEgressPoolView> {
        let pool = self
            .repository
            .update_network_egress_pool(&UpdateNetworkEgressPoolInput {
                pool_id: command.pool_id,
                display_name: required_text(&command.display_name, "display_name")?,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            pool.id,
            "network_egress_pool.updated",
        )
        .await?;
        self.view(pool).await
    }

    pub async fn delete(&self, actor_user_id: Uuid, pool_id: Uuid) -> Result<()> {
        self.repository.delete_network_egress_pool(pool_id).await?;
        self.audit(actor_user_id, pool_id, "network_egress_pool.deleted")
            .await
    }

    pub async fn add_member(
        &self,
        command: CreateNetworkEgressPoolMemberCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        self.require_pool(command.pool_id).await?;
        let provider_egress_key =
            required_text(&command.provider_egress_key, "provider_egress_key")?;
        validate_sequence(command.sequence)?;
        self.require_current_descriptor(command.provider_id, &provider_egress_key)
            .await?;
        let member = self
            .repository
            .create_network_egress_pool_member(&CreateNetworkEgressPoolMemberInput {
                member_id: Uuid::now_v7(),
                pool_id: command.pool_id,
                provider_id: command.provider_id,
                provider_egress_key,
                enabled: command.enabled,
                sequence: command.sequence,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            command.pool_id,
            "network_egress_pool.member_added",
        )
        .await?;
        Ok(self.member_view(member).await?)
    }

    pub async fn update_member(
        &self,
        command: UpdateNetworkEgressPoolMemberCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        validate_sequence(command.sequence)?;
        let member = self
            .repository
            .update_network_egress_pool_member(&UpdateNetworkEgressPoolMemberInput {
                pool_id: command.pool_id,
                member_id: command.member_id,
                enabled: command.enabled,
                sequence: command.sequence,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            command.pool_id,
            "network_egress_pool.member_updated",
        )
        .await?;
        Ok(self.member_view(member).await?)
    }

    pub async fn delete_member(
        &self,
        actor_user_id: Uuid,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> Result<()> {
        self.repository
            .delete_network_egress_pool_member(pool_id, member_id)
            .await?;
        self.audit(actor_user_id, pool_id, "network_egress_pool.member_deleted")
            .await
    }

    pub async fn select_healthy_first(&self, pool_id: Uuid) -> Result<NetworkEgressPoolSelection> {
        let pool = self.require_pool(pool_id).await?;
        if pool.selection_strategy != domain::NetworkEgressPoolSelectionStrategy::HealthyFirst {
            return Err(ControlPlaneError::InvalidInput("selection_strategy").into());
        }
        for member in self
            .repository
            .list_network_egress_pool_members(pool_id)
            .await?
        {
            if member.enabled
                && self.member_health(&member).await?
                    == domain::NetworkEgressPoolMemberHealth::Healthy
            {
                return Ok(NetworkEgressPoolSelection {
                    pool_id,
                    member_id: member.id,
                    provider_id: member.provider_id,
                    provider_egress_key: member.provider_egress_key,
                });
            }
        }
        Err(ControlPlaneError::Conflict("network_egress_pool_unavailable").into())
    }

    async fn view(&self, pool: domain::NetworkEgressPool) -> Result<NetworkEgressPoolView> {
        let members = self
            .repository
            .list_network_egress_pool_members(pool.id)
            .await?;
        let mut member_views = Vec::with_capacity(members.len());
        for member in members {
            member_views.push(self.member_view(member).await?);
        }
        Ok(NetworkEgressPoolView {
            pool,
            members: member_views,
        })
    }

    async fn member_view(
        &self,
        member: domain::NetworkEgressPoolMember,
    ) -> Result<NetworkEgressPoolMemberView> {
        let health = self.member_health(&member).await?;
        Ok(NetworkEgressPoolMemberView { member, health })
    }

    async fn member_health(
        &self,
        member: &domain::NetworkEgressPoolMember,
    ) -> Result<domain::NetworkEgressPoolMemberHealth> {
        let Some(provider) = self
            .repository
            .get_network_egress_provider(member.provider_id)
            .await?
        else {
            return Ok(domain::NetworkEgressPoolMemberHealth::Invalid);
        };
        let descriptor = self
            .repository
            .list_network_egress_projections(member.provider_id)
            .await?
            .into_iter()
            .find(|item| item.provider_egress_key == member.provider_egress_key);
        let Some(descriptor) = descriptor else {
            return Ok(domain::NetworkEgressPoolMemberHealth::Invalid);
        };
        if provider.lifecycle == domain::NetworkEgressProviderLifecycle::Active
            && provider.health_status == domain::NetworkEgressHealthStatus::Healthy
            && descriptor.availability == "available"
        {
            Ok(domain::NetworkEgressPoolMemberHealth::Healthy)
        } else {
            Ok(domain::NetworkEgressPoolMemberHealth::Unhealthy)
        }
    }

    async fn require_pool(&self, pool_id: Uuid) -> Result<domain::NetworkEgressPool> {
        self.repository
            .get_network_egress_pool(pool_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("network_egress_pool").into())
    }

    async fn require_current_descriptor(&self, provider_id: Uuid, key: &str) -> Result<()> {
        let Some(_) = self
            .repository
            .get_network_egress_provider(provider_id)
            .await?
        else {
            return Err(ControlPlaneError::NotFound("network_egress_provider").into());
        };
        let exists = self
            .repository
            .list_network_egress_projections(provider_id)
            .await?
            .into_iter()
            .any(|projection| projection.provider_egress_key == key);
        if exists {
            Ok(())
        } else {
            Err(ControlPlaneError::InvalidInput("provider_egress_key").into())
        }
    }

    async fn audit(
        &self,
        actor_user_id: Uuid,
        pool_id: Uuid,
        event_type: &'static str,
    ) -> Result<()> {
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(actor_user_id),
                "network_egress_pool",
                Some(pool_id),
                event_type,
                serde_json::json!({}),
            ))
            .await
    }
}

fn required_text(value: &str, field: &'static str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(value.to_string())
}

fn validate_sequence(value: i32) -> Result<()> {
    if value < 0 {
        return Err(ControlPlaneError::InvalidInput("sequence").into());
    }
    Ok(())
}
