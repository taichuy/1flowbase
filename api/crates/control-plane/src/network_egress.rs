use anyhow::Result;
use plugin_framework::{EgressAvailability, NETWORK_EGRESS_PROVIDER_CONTRACT};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateNetworkEgressProviderInput, NetworkEgressRepository, NetworkEgressRuntimePort,
        PluginRepository, RecordNetworkEgressSyncFailureInput, ReplaceNetworkEgressProjectionInput,
        UpdateNetworkEgressProviderLifecycleInput,
    },
};

pub struct CreateNetworkEgressProviderCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
    pub display_name: String,
    pub secret_ref: String,
}

pub struct UpdateNetworkEgressProviderLifecycleCommand {
    pub actor_user_id: Uuid,
    pub provider_id: Uuid,
    pub lifecycle: domain::NetworkEgressProviderLifecycle,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressProviderView {
    pub provider: domain::NetworkEgressProviderRecord,
    pub egresses: Vec<domain::NetworkEgressProjectionRecord>,
}

pub struct NetworkEgressProviderService<R, H> {
    repository: R,
    runtime: H,
    node_id: String,
}

impl<R, H> NetworkEgressProviderService<R, H>
where
    R: NetworkEgressRepository + PluginRepository,
    H: NetworkEgressRuntimePort,
{
    pub fn new(repository: R, runtime: H, node_id: String) -> Self {
        Self {
            repository,
            runtime,
            node_id,
        }
    }

    pub async fn list(&self) -> Result<Vec<NetworkEgressProviderView>> {
        let providers = self.repository.list_network_egress_providers().await?;
        let mut views = Vec::with_capacity(providers.len());
        for provider in providers {
            let egresses = self
                .repository
                .list_network_egress_projections(provider.id)
                .await?;
            views.push(NetworkEgressProviderView { provider, egresses });
        }
        Ok(views)
    }

    pub async fn create(
        &self,
        command: CreateNetworkEgressProviderCommand,
    ) -> Result<NetworkEgressProviderView> {
        let display_name = required_text(&command.display_name, "display_name")?;
        let secret_ref = valid_secret_ref(&command.secret_ref)?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        if installation.contract_version != NETWORK_EGRESS_PROVIDER_CONTRACT {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }

        let provider = self
            .repository
            .create_network_egress_provider(&CreateNetworkEgressProviderInput {
                provider_id: Uuid::now_v7(),
                installation_id: command.installation_id,
                provider_code: installation.provider_code,
                display_name,
                secret_ref,
                lifecycle: domain::NetworkEgressProviderLifecycle::Draft,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(command.actor_user_id),
                "network_egress_provider",
                Some(provider.id),
                "network_egress_provider.created",
                serde_json::json!({ "installation_id": provider.installation_id, "lifecycle": provider.lifecycle.as_str() }),
            ))
            .await?;
        Ok(NetworkEgressProviderView {
            provider,
            egresses: Vec::new(),
        })
    }

    pub async fn update_lifecycle(
        &self,
        command: UpdateNetworkEgressProviderLifecycleCommand,
    ) -> Result<NetworkEgressProviderView> {
        let provider = self
            .repository
            .update_network_egress_provider_lifecycle(&UpdateNetworkEgressProviderLifecycleInput {
                provider_id: command.provider_id,
                lifecycle: command.lifecycle,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(command.actor_user_id),
                "network_egress_provider",
                Some(provider.id),
                "network_egress_provider.lifecycle_updated",
                serde_json::json!({ "lifecycle": provider.lifecycle.as_str() }),
            ))
            .await?;
        let egresses = self
            .repository
            .list_network_egress_projections(provider.id)
            .await?;
        Ok(NetworkEgressProviderView { provider, egresses })
    }

    pub async fn sync(
        &self,
        actor_user_id: Uuid,
        provider_id: Uuid,
    ) -> Result<NetworkEgressProviderView> {
        let provider = self
            .repository
            .get_network_egress_provider(provider_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("network_egress_provider"))?;
        if provider.lifecycle != domain::NetworkEgressProviderLifecycle::Active {
            return Err(ControlPlaneError::Conflict("network_egress_provider_not_active").into());
        }
        let sync_at = OffsetDateTime::now_utc();
        let local_installation = self
            .repository
            .get_local_installation(&self.node_id, provider.installation_id)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_unavailable",
            ))?;
        if local_installation.contract_version != NETWORK_EGRESS_PROVIDER_CONTRACT {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }
        let descriptors = match self
            .runtime
            .sync_network_egresses(&local_installation)
            .await
        {
            Ok(descriptors) => descriptors,
            Err(_) => {
                let failed = self
                    .repository
                    .record_network_egress_sync_failure(&RecordNetworkEgressSyncFailureInput {
                        provider_id,
                        last_sync_error: "network_egress_sync_failed".to_string(),
                        synchronized_at: sync_at,
                        actor_user_id,
                    })
                    .await?;
                self.repository
                    .append_audit_log(&audit_log(
                        None,
                        Some(actor_user_id),
                        "network_egress_provider",
                        Some(provider_id),
                        "network_egress_provider.sync_failed",
                        serde_json::json!({}),
                    ))
                    .await?;
                let egresses = self
                    .repository
                    .list_network_egress_projections(provider_id)
                    .await?;
                return Ok(NetworkEgressProviderView {
                    provider: failed,
                    egresses,
                });
            }
        };
        let egresses = descriptors
            .into_iter()
            .map(|descriptor| domain::NetworkEgressProjectionRecord {
                provider_id,
                provider_egress_key: descriptor.provider_egress_key,
                display_name: descriptor.display_name,
                region: descriptor.region,
                tags: descriptor.tags.unwrap_or_default(),
                availability: match descriptor.availability {
                    EgressAvailability::Available => "available".to_string(),
                    EgressAvailability::Unavailable => "unavailable".to_string(),
                },
                synced_at: sync_at,
            })
            .collect::<Vec<_>>();
        let provider = self
            .repository
            .replace_network_egress_projection(&ReplaceNetworkEgressProjectionInput {
                provider_id,
                health_status: domain::NetworkEgressHealthStatus::Healthy,
                last_sync_error: None,
                synchronized_at: sync_at,
                egresses: egresses.clone(),
                actor_user_id,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(actor_user_id),
                "network_egress_provider",
                Some(provider_id),
                "network_egress_provider.synced",
                serde_json::json!({ "egress_count": egresses.len() }),
            ))
            .await?;
        Ok(NetworkEgressProviderView { provider, egresses })
    }
}

fn required_text(value: &str, field: &'static str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(value.to_string())
}

fn valid_secret_ref(value: &str) -> Result<String> {
    let value = required_text(value, "secret_ref")?;
    if !value.starts_with("secret://") || value.chars().any(char::is_whitespace) {
        return Err(ControlPlaneError::InvalidInput("secret_ref").into());
    }
    Ok(value)
}
