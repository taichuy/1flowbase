use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
        NetworkEgressPoolRepository, NetworkEgressRepository, NetworkEgressRouteRepository,
        RecordNetworkEgressPoolMemberProbeInput, UpdateNetworkEgressPoolMemberInput,
    },
};

/// The Network Center has one system-wide proxy pool.  Keeping its id stable lets routing
/// retain a normal durable foreign key without exposing a pool choice in product UI.
pub const GLOBAL_NETWORK_EGRESS_POOL_ID: Uuid =
    Uuid::from_u128(0x1f10_0ba5_0000_4000_8000_0000_0000_1805);

pub async fn ensure_global_network_egress_pool<R>(
    repository: &R,
    actor_user_id: Uuid,
) -> Result<domain::NetworkEgressPool>
where
    R: NetworkEgressPoolRepository,
{
    if let Some(pool) = repository
        .get_network_egress_pool(GLOBAL_NETWORK_EGRESS_POOL_ID)
        .await?
    {
        return Ok(pool);
    }
    match repository
        .create_network_egress_pool(&CreateNetworkEgressPoolInput {
            pool_id: GLOBAL_NETWORK_EGRESS_POOL_ID,
            display_name: "Global proxy pool".to_string(),
            owner_provider_id: None,
            actor_user_id,
        })
        .await
    {
        Ok(pool) => Ok(pool),
        // A concurrent first write can win after the read above.  In that case the stable id
        // makes a second lookup sufficient, without pretending this is a user-created pool.
        Err(error) => repository
            .get_network_egress_pool(GLOBAL_NETWORK_EGRESS_POOL_ID)
            .await?
            .ok_or(error),
    }
}

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

/// Creates one built-in static HTTP proxy and attaches its durable egress reference to the
/// selected pool. The pool remains the user-facing composition boundary.
pub struct AddStaticHttpProxyToPoolCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub enabled: bool,
    pub sequence: i32,
}

/// Adds all current projections from one extension provider to the selected pool.
pub struct AddProviderEgressesToPoolCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub provider_id: Uuid,
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

pub struct RecordNetworkEgressPoolMemberProbeCommand {
    pub actor_user_id: Uuid,
    pub pool_id: Uuid,
    pub member_id: Uuid,
    pub status: domain::NetworkEgressPoolMemberProbeStatus,
    pub http_status: domain::NetworkEgressPoolMemberProbeStatus,
    pub https_status: domain::NetworkEgressPoolMemberProbeStatus,
    pub latency_ms: i32,
    pub exit_ip: Option<String>,
    pub exit_region: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressPoolMemberView {
    pub member: domain::NetworkEgressPoolMember,
    pub health: domain::NetworkEgressPoolMemberHealth,
    pub provider_code: String,
    pub display_name: String,
    pub address_summary: Option<String>,
    pub region: Option<String>,
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
    secret_master_key: Option<String>,
}

impl<R> NetworkEgressPoolService<R>
where
    R: NetworkEgressPoolRepository + NetworkEgressRepository + NetworkEgressRouteRepository,
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            secret_master_key: None,
        }
    }

    pub fn with_secret_master_key(repository: R, secret_master_key: String) -> Self {
        Self {
            repository,
            secret_master_key: Some(secret_master_key),
        }
    }

    pub async fn list_global(&self, actor_user_id: Uuid) -> Result<Vec<NetworkEgressPoolView>> {
        Ok(vec![
            self.view(ensure_global_network_egress_pool(&self.repository, actor_user_id).await?)
                .await?,
        ])
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
        _command: CreateNetworkEgressPoolCommand,
    ) -> Result<NetworkEgressPoolView> {
        Err(ControlPlaneError::Conflict("network_egress_global_pool_only").into())
    }

    pub async fn update(
        &self,
        command: UpdateNetworkEgressPoolCommand,
    ) -> Result<NetworkEgressPoolView> {
        let _ = command;
        Err(ControlPlaneError::Conflict("network_egress_global_pool_only").into())
    }

    pub async fn delete(&self, actor_user_id: Uuid, pool_id: Uuid) -> Result<()> {
        let _ = (actor_user_id, pool_id);
        Err(ControlPlaneError::Conflict("network_egress_global_pool_only").into())
    }

    pub async fn add_member(
        &self,
        command: CreateNetworkEgressPoolMemberCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        self.require_global_pool(command.pool_id).await?;
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
        self.member_view(member).await
    }

    pub async fn add_static_http_proxy(
        &self,
        command: AddStaticHttpProxyToPoolCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        self.require_global_pool(command.pool_id).await?;
        let display_name = required_text(&command.display_name, "display_name")?;
        let host = static_http_host(&command.host)?;
        if command.port == 0 {
            return Err(ControlPlaneError::InvalidInput("port").into());
        }
        validate_sequence(command.sequence)?;
        let secret_master_key =
            self.secret_master_key
                .as_deref()
                .ok_or(ControlPlaneError::Conflict(
                    "network_egress_static_http_unavailable",
                ))?;
        let provider_id = Uuid::now_v7();
        let secret_ref = format!("secret://system/network-egress/{provider_id}");
        let synchronized_at = OffsetDateTime::now_utc();
        let member = self
            .repository
            .create_static_http_proxy_pool_member(
                &crate::ports::CreateStaticHttpProxyPoolMemberInput {
                    provider_id,
                    member_id: Uuid::now_v7(),
                    pool_id: command.pool_id,
                    display_name,
                    description: String::new(),
                    secret_ref,
                    plaintext_secret_json: json!({
                        "host": host,
                        "port": command.port,
                        "username": command.username.trim(),
                        "password": command.password,
                    }),
                    master_key: secret_master_key.to_string(),
                    enabled: command.enabled,
                    sequence: command.sequence,
                    synchronized_at,
                    actor_user_id: command.actor_user_id,
                },
            )
            .await?;
        self.audit(
            command.actor_user_id,
            command.pool_id,
            "network_egress_pool.member_added",
        )
        .await?;
        self.member_view(member).await
    }

    pub async fn add_provider_egresses(
        &self,
        command: AddProviderEgressesToPoolCommand,
    ) -> Result<Vec<NetworkEgressPoolMemberView>> {
        self.require_global_pool(command.pool_id).await?;
        validate_sequence(command.sequence)?;
        let provider = self
            .repository
            .get_network_egress_provider(command.provider_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("network_egress_provider"))?;
        if provider.lifecycle != domain::NetworkEgressProviderLifecycle::Active {
            return Err(ControlPlaneError::Conflict("network_egress_provider_not_active").into());
        }
        let members = self
            .repository
            .list_network_egress_pool_members(command.pool_id)
            .await?;
        let projections = self
            .repository
            .list_network_egress_projections(command.provider_id)
            .await?;
        let mut created = Vec::new();
        for (offset, projection) in projections.iter().enumerate() {
            if members.iter().any(|member| {
                member.provider_id == command.provider_id
                    && member.provider_egress_key == projection.provider_egress_key
            }) {
                continue;
            }
            created.push(
                self.add_member(CreateNetworkEgressPoolMemberCommand {
                    actor_user_id: command.actor_user_id,
                    pool_id: command.pool_id,
                    provider_id: command.provider_id,
                    provider_egress_key: projection.provider_egress_key.clone(),
                    enabled: command.enabled && projection.availability == "available",
                    sequence: command.sequence + offset as i32,
                })
                .await?,
            );
        }
        Ok(created)
    }

    pub async fn update_member(
        &self,
        command: UpdateNetworkEgressPoolMemberCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        self.require_global_pool(command.pool_id).await?;
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
        self.member_view(member).await
    }

    pub async fn delete_member(
        &self,
        actor_user_id: Uuid,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> Result<()> {
        self.require_global_pool(pool_id).await?;
        if self
            .repository
            .is_network_egress_pool_member_referenced(member_id)
            .await?
        {
            return Err(ControlPlaneError::Conflict("network_egress_pool_member_in_use").into());
        }
        self.repository
            .delete_network_egress_pool_member(pool_id, member_id)
            .await?;
        self.audit(actor_user_id, pool_id, "network_egress_pool.member_deleted")
            .await
    }

    pub async fn record_probe(
        &self,
        command: RecordNetworkEgressPoolMemberProbeCommand,
    ) -> Result<NetworkEgressPoolMemberView> {
        self.require_global_pool(command.pool_id).await?;
        let member = self
            .repository
            .record_network_egress_pool_member_probe(&RecordNetworkEgressPoolMemberProbeInput {
                pool_id: command.pool_id,
                member_id: command.member_id,
                status: command.status,
                http_status: command.http_status,
                https_status: command.https_status,
                latency_ms: command.latency_ms,
                exit_ip: command.exit_ip,
                exit_region: command.exit_region,
                error_code: command.error_code,
                probed_at: OffsetDateTime::now_utc(),
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.audit(
            command.actor_user_id,
            command.pool_id,
            "network_egress_pool.member_connection_tested",
        )
        .await?;
        self.member_view(member).await
    }

    pub async fn member(
        &self,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> Result<domain::NetworkEgressPoolMember> {
        self.require_global_pool(pool_id).await?;
        self.repository
            .list_network_egress_pool_members(pool_id)
            .await?
            .into_iter()
            .find(|member| member.id == member_id)
            .ok_or_else(|| ControlPlaneError::NotFound("network_egress_pool_member").into())
    }

    pub async fn select_healthy_first(&self, pool_id: Uuid) -> Result<NetworkEgressPoolSelection> {
        let member_ids = self
            .repository
            .list_network_egress_pool_members(pool_id)
            .await?
            .into_iter()
            .map(|member| member.id)
            .collect::<Vec<_>>();
        self.select_healthy_first_from(pool_id, &member_ids).await
    }

    pub async fn select_healthy_first_from(
        &self,
        pool_id: Uuid,
        member_ids: &[Uuid],
    ) -> Result<NetworkEgressPoolSelection> {
        let pool = self.require_pool(pool_id).await?;
        if pool.selection_strategy != domain::NetworkEgressPoolSelectionStrategy::HealthyFirst {
            return Err(ControlPlaneError::InvalidInput("selection_strategy").into());
        }
        let mut members = self
            .repository
            .list_network_egress_pool_members(pool_id)
            .await?
            .into_iter()
            .map(|member| (member.id, member))
            .collect::<HashMap<_, _>>();
        let mut untested_fallback = None;
        for member_id in member_ids {
            let Some(member) = members.remove(member_id) else {
                return Err(ControlPlaneError::InvalidInput("pool_member_ids").into());
            };
            if !member.enabled {
                continue;
            }
            match self.member_health(&member).await? {
                domain::NetworkEgressPoolMemberHealth::Healthy => {
                    return Ok(NetworkEgressPoolSelection {
                        pool_id,
                        member_id: member.id,
                        provider_id: member.provider_id,
                        provider_egress_key: member.provider_egress_key,
                    });
                }
                domain::NetworkEgressPoolMemberHealth::NotTested => {
                    untested_fallback.get_or_insert(member);
                }
                domain::NetworkEgressPoolMemberHealth::Unhealthy
                | domain::NetworkEgressPoolMemberHealth::Invalid => {}
            }
        }
        untested_fallback
            .map(|member| NetworkEgressPoolSelection {
                pool_id,
                member_id: member.id,
                provider_id: member.provider_id,
                provider_egress_key: member.provider_egress_key,
            })
            .ok_or_else(|| ControlPlaneError::Conflict("network_egress_pool_unavailable").into())
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

    async fn require_global_pool(&self, pool_id: Uuid) -> Result<()> {
        if pool_id != GLOBAL_NETWORK_EGRESS_POOL_ID {
            return Err(ControlPlaneError::Conflict("network_egress_global_pool_only").into());
        }
        self.require_pool(pool_id).await.map(|_| ())
    }

    async fn member_view(
        &self,
        member: domain::NetworkEgressPoolMember,
    ) -> Result<NetworkEgressPoolMemberView> {
        let provider = self
            .repository
            .get_network_egress_provider(member.provider_id)
            .await?;
        let projection = self
            .repository
            .list_network_egress_projections(member.provider_id)
            .await?
            .into_iter()
            .find(|item| item.provider_egress_key == member.provider_egress_key);
        let address_summary = match provider.as_ref() {
            Some(provider) => self.safe_address_summary(provider).await?,
            None => None,
        };
        let health = member_health_from_snapshot(
            provider.as_ref(),
            projection.as_ref(),
            member.probe_status,
        );
        let region = projection
            .as_ref()
            .and_then(|projection| projection.region.clone())
            .or(member.probe_exit_region.clone());
        Ok(NetworkEgressPoolMemberView {
            provider_code: provider
                .as_ref()
                .map(|provider| provider.provider_code.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            display_name: projection
                .as_ref()
                .map(|projection| projection.display_name.clone())
                .or_else(|| {
                    provider
                        .as_ref()
                        .map(|provider| provider.display_name.clone())
                })
                .unwrap_or_else(|| member.provider_egress_key.clone()),
            member,
            health,
            address_summary,
            region,
        })
    }

    async fn safe_address_summary(
        &self,
        provider: &domain::NetworkEgressProviderRecord,
    ) -> Result<Option<String>> {
        if provider.provider_code != "builtin_static_http" {
            return Ok(None);
        }
        let Some(master_key) = self.secret_master_key.as_deref() else {
            return Ok(None);
        };
        let Some(secret) = self
            .repository
            .resolve_network_egress_provider_secret_json(
                provider.id,
                &provider.secret_ref,
                master_key,
            )
            .await?
        else {
            return Ok(None);
        };
        let host = secret["host"].as_str().filter(|value| !value.is_empty());
        let port = secret["port"]
            .as_u64()
            .filter(|value| (1..=65535).contains(value));
        Ok(host.zip(port).map(|(host, port)| format!("{host}:{port}")))
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
        Ok(member_health_from_snapshot(
            Some(&provider),
            Some(&descriptor),
            member.probe_status,
        ))
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

fn member_health_from_snapshot(
    provider: Option<&domain::NetworkEgressProviderRecord>,
    projection: Option<&domain::NetworkEgressProjectionRecord>,
    probe_status: domain::NetworkEgressPoolMemberProbeStatus,
) -> domain::NetworkEgressPoolMemberHealth {
    match (provider, projection) {
        (Some(provider), Some(projection))
            if provider.lifecycle == domain::NetworkEgressProviderLifecycle::Active
                && provider.health_status == domain::NetworkEgressHealthStatus::Healthy
                && projection.availability == "available" =>
        {
            match probe_status {
                domain::NetworkEgressPoolMemberProbeStatus::NotTested => {
                    domain::NetworkEgressPoolMemberHealth::NotTested
                }
                domain::NetworkEgressPoolMemberProbeStatus::Succeeded => {
                    domain::NetworkEgressPoolMemberHealth::Healthy
                }
                domain::NetworkEgressPoolMemberProbeStatus::Failed => {
                    domain::NetworkEgressPoolMemberHealth::Unhealthy
                }
            }
        }
        (Some(_), Some(_)) => domain::NetworkEgressPoolMemberHealth::Unhealthy,
        _ => domain::NetworkEgressPoolMemberHealth::Invalid,
    }
}

#[cfg(test)]
mod health_tests {
    use super::member_health_from_snapshot;
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn active_provider() -> domain::NetworkEgressProviderRecord {
        domain::NetworkEgressProviderRecord {
            id: Uuid::now_v7(),
            extension_family: None,
            provider_code: "builtin_static_http".to_string(),
            display_name: "Proxy".to_string(),
            description: String::new(),
            secret_ref: "secret://system/network-egress/test".to_string(),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            health_status: domain::NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            last_synced_at: None,
            created_by: Uuid::now_v7(),
            updated_by: Uuid::now_v7(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn available_projection(provider_id: Uuid) -> domain::NetworkEgressProjectionRecord {
        domain::NetworkEgressProjectionRecord {
            provider_id,
            provider_egress_key: "static-http".to_string(),
            display_name: "Proxy".to_string(),
            region: None,
            tags: vec![],
            availability: "available".to_string(),
            synced_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn ac_proxy_save_reports_untested_until_a_connection_test_completes() {
        let provider = active_provider();
        let projection = available_projection(provider.id);

        assert_eq!(
            member_health_from_snapshot(
                Some(&provider),
                Some(&projection),
                domain::NetworkEgressPoolMemberProbeStatus::NotTested,
            ),
            domain::NetworkEgressPoolMemberHealth::NotTested
        );
        assert_eq!(
            member_health_from_snapshot(
                Some(&provider),
                Some(&projection),
                domain::NetworkEgressPoolMemberProbeStatus::Succeeded,
            ),
            domain::NetworkEgressPoolMemberHealth::Healthy
        );
        assert_eq!(
            member_health_from_snapshot(
                Some(&provider),
                Some(&projection),
                domain::NetworkEgressPoolMemberProbeStatus::Failed,
            ),
            domain::NetworkEgressPoolMemberHealth::Unhealthy
        );
    }
}

fn required_text(value: &str, field: &'static str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(value.to_string())
}

fn static_http_host(value: &str) -> Result<String> {
    let host = required_text(value, "host")?;
    if host.contains(['/', '@', ':']) || host.chars().any(char::is_whitespace) {
        return Err(ControlPlaneError::InvalidInput("host").into());
    }
    Ok(host)
}

fn validate_sequence(value: i32) -> Result<()> {
    if value < 0 {
        return Err(ControlPlaneError::InvalidInput("sequence").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::static_http_host;

    /// AC-NC16: credentials and port are separate secret fields; the pool write path accepts
    /// only a host, never a pasted URL or credential-bearing proxy string.
    #[test]
    fn ac_nc16_static_http_host_rejects_proxy_urls_and_credentials() {
        assert_eq!(static_http_host("198.65.36.212").unwrap(), "198.65.36.212");
        assert!(static_http_host("http://198.65.36.212").is_err());
        assert!(static_http_host("user@198.65.36.212").is_err());
        assert!(static_http_host("198.65.36.212:37867").is_err());
    }
}
