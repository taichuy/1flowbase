use anyhow::Result;
use plugin_framework::{
    EgressAvailability, NetworkEgressProviderPackage, PluginFormSchema,
    NETWORK_EGRESS_PROVIDER_CONTRACT,
};
use serde_json::{Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateNetworkEgressProviderInput, NetworkEgressPoolRepository, NetworkEgressRepository,
        NetworkEgressRuntimePort, NetworkEgressSecretResolver, PluginRepository,
        RecordNetworkEgressSyncFailureInput, ReplaceNetworkEgressProjectionInput,
        UpdateNetworkEgressProviderLifecycleInput,
    },
};

pub struct CreateNetworkEgressProviderCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
    pub display_name: String,
    pub description: String,
    /// Plugin-defined configuration. It is immediately encrypted by the registry and never
    /// projected back into a DTO.
    pub secret_json: Value,
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

#[derive(Debug, Clone)]
pub struct NetworkEgressProviderTypeView {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
    pub form_schema: PluginFormSchema,
}

pub struct NetworkEgressProviderService<R, H, S> {
    repository: R,
    runtime: H,
    secret_resolver: S,
    secret_master_key: String,
    node_id: String,
}

impl<R, H, S> NetworkEgressProviderService<R, H, S>
where
    R: NetworkEgressRepository + NetworkEgressPoolRepository + PluginRepository,
    H: NetworkEgressRuntimePort,
    S: NetworkEgressSecretResolver,
{
    pub fn new(
        repository: R,
        runtime: H,
        secret_resolver: S,
        secret_master_key: String,
        node_id: String,
    ) -> Self {
        Self {
            repository,
            runtime,
            secret_resolver,
            secret_master_key,
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

    /// The catalog is derived from installed artifacts, never from the frontend or registry
    /// listing. A package remains selectable even while its extension desired-state is disabled:
    /// an egress instance owns its own lifecycle.
    pub async fn list_types(&self) -> Result<Vec<NetworkEgressProviderTypeView>> {
        let mut types = Vec::new();
        for installation in self.repository.list_installations().await? {
            if installation.contract_version != NETWORK_EGRESS_PROVIDER_CONTRACT
                || installation.metadata_json["plugin_type"] != "network_egress_provider"
            {
                continue;
            }
            let Some(local) = self
                .repository
                .get_local_installation(&self.node_id, installation.id)
                .await?
            else {
                continue;
            };
            let package = load_egress_package(&local)?;
            types.push(NetworkEgressProviderTypeView {
                installation_id: installation.id,
                provider_code: package.provider.provider_code,
                display_name: package.provider.display_name,
                form_schema: package.provider.form_schema,
            });
        }
        types.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(types)
    }

    pub async fn create(
        &self,
        command: CreateNetworkEgressProviderCommand,
    ) -> Result<NetworkEgressProviderView> {
        let display_name = required_text(&command.display_name, "display_name")?;
        let description = command.description.trim().to_string();
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        if installation.contract_version != NETWORK_EGRESS_PROVIDER_CONTRACT
            || installation.metadata_json["plugin_type"] != "network_egress_provider"
        {
            return Err(ControlPlaneError::InvalidInput("installation_id").into());
        }
        let local = self
            .repository
            .get_local_installation(&self.node_id, command.installation_id)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_unavailable",
            ))?;
        let package = load_egress_package(&local)?;
        let secret_json =
            validate_instance_config(&package.provider.form_schema, command.secret_json)?;
        let provider_id = Uuid::now_v7();
        let secret_ref = format!("secret://system/network-egress/{provider_id}");

        let provider = self
            .repository
            .create_network_egress_provider(&CreateNetworkEgressProviderInput {
                provider_id,
                installation_id: command.installation_id,
                provider_code: package.provider.provider_code,
                display_name,
                description,
                secret_ref,
                lifecycle: domain::NetworkEgressProviderLifecycle::Active,
                actor_user_id: command.actor_user_id,
            })
            .await?;
        self.repository
            .upsert_network_egress_provider_secret(
                &crate::ports::UpsertNetworkEgressProviderSecretInput {
                    provider_id,
                    secret_ref: provider.secret_ref.clone(),
                    plaintext_secret_json: secret_json,
                    master_key: self.secret_master_key.clone(),
                    secret_version: 1,
                },
            )
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
        self.sync(command.actor_user_id, provider.id).await
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
        let secret = self
            .secret_resolver
            .resolve_for_runner(&provider)
            .await
            .map_err(|_| ControlPlaneError::Conflict("network_egress_provider_secret_unavailable"))?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_secret_unavailable",
            ))?;
        let descriptors = match self
            .runtime
            .sync_network_egresses(&local_installation, secret)
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
        self.sync_derived_pool(
            provider.id,
            &provider.display_name,
            &egresses,
            actor_user_id,
        )
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

    async fn sync_derived_pool(
        &self,
        provider_id: Uuid,
        provider_name: &str,
        egresses: &[domain::NetworkEgressProjectionRecord],
        actor_user_id: Uuid,
    ) -> Result<()> {
        let pool = self
            .repository
            .list_network_egress_pools()
            .await?
            .into_iter()
            .find(|pool| pool.owner_provider_id == Some(provider_id));
        let pool = match pool {
            Some(pool) => pool,
            None => {
                self.repository
                    .create_network_egress_pool(&crate::ports::CreateNetworkEgressPoolInput {
                        pool_id: Uuid::now_v7(),
                        display_name: format!("{provider_name} exits"),
                        owner_provider_id: Some(provider_id),
                        actor_user_id,
                    })
                    .await?
            }
        };
        for member in self
            .repository
            .list_network_egress_pool_members(pool.id)
            .await?
        {
            self.repository
                .delete_network_egress_pool_member(pool.id, member.id)
                .await?;
        }
        for (sequence, egress) in egresses.iter().enumerate() {
            self.repository
                .create_network_egress_pool_member(
                    &crate::ports::CreateNetworkEgressPoolMemberInput {
                        member_id: Uuid::now_v7(),
                        pool_id: pool.id,
                        provider_id,
                        provider_egress_key: egress.provider_egress_key.clone(),
                        enabled: egress.availability == "available",
                        sequence: sequence as i32,
                        actor_user_id,
                    },
                )
                .await?;
        }
        Ok(())
    }
}

fn required_text(value: &str, field: &'static str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(value.to_string())
}

fn load_egress_package(
    installation: &domain::LocalPluginInstallationRecord,
) -> Result<NetworkEgressProviderPackage> {
    let path = installation
        .local_path()
        .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?;
    NetworkEgressProviderPackage::load_from_dir(path)
        .map_err(|_| ControlPlaneError::InvalidInput("network_egress_provider_package").into())
}

fn validate_instance_config(schema: &PluginFormSchema, value: Value) -> Result<Value> {
    let object = value
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config"))?;
    let declared = schema
        .fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<std::collections::HashSet<_>>();
    if object.keys().any(|key| !declared.contains(key.as_str())) {
        return Err(ControlPlaneError::InvalidInput("config").into());
    }
    let mut validated = Map::new();
    for field in &schema.fields {
        let value = object.get(&field.key);
        if field.required.unwrap_or(false) && value.is_none() {
            return Err(ControlPlaneError::InvalidInput("config").into());
        }
        let Some(value) = value else { continue };
        let text = value
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .ok_or(ControlPlaneError::InvalidInput("config"))?;
        if text.len() > 4096 {
            return Err(ControlPlaneError::InvalidInput("config").into());
        }
        validated.insert(field.key.clone(), Value::String(text.to_string()));
    }
    Ok(Value::Object(validated))
}
