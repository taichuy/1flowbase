use anyhow::Result;
use plugin_framework::{
    EgressAvailability, NetworkEgressProviderPackage, PluginFormFieldSchema, PluginFormSchema,
    NETWORK_EGRESS_PROVIDER_CONTRACT,
};
use serde_json::{Map, Value};
use std::collections::HashSet;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    network_egress_pool::{ensure_global_network_egress_pool, GLOBAL_NETWORK_EGRESS_POOL_ID},
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

/// A user-facing proxy creation.  It deliberately accepts a proxy type rather than an
/// installation id: Core resolves the installed extension and attaches every parsed egress to
/// the single global pool as one server-side action.
pub struct CreateNetworkEgressProxyCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    pub config: Value,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressProviderView {
    pub provider: domain::NetworkEgressProviderRecord,
    pub egresses: Vec<domain::NetworkEgressProjectionRecord>,
}

#[derive(Debug, Clone)]
pub struct NetworkEgressProviderTypeView {
    /// Built-in types are supplied by Core; extension types point to their installed artifact.
    pub installation_id: Option<Uuid>,
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
        let mut types = vec![builtin_static_http_type()];
        let mut installations = self.repository.list_installations().await?;
        installations.sort_by(|left, right| {
            plugin_version_order(&right.plugin_version, &left.plugin_version)
                .then_with(|| right.id.cmp(&left.id))
        });
        let mut selected_provider_codes = HashSet::new();
        for installation in installations {
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
            if !local.artifact.artifact_status.is_ready() || !local.artifact.is_current {
                continue;
            }
            let package = load_egress_package(&local)?;
            if !selected_provider_codes.insert(package.provider.provider_code.clone()) {
                continue;
            }
            types.push(NetworkEgressProviderTypeView {
                installation_id: Some(installation.id),
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
        if !local.artifact.artifact_status.is_ready() || !local.artifact.is_current {
            return Err(ControlPlaneError::Conflict("network_egress_provider_not_current").into());
        }
        let package = load_egress_package(&local)?;
        let secret_json =
            validate_instance_config(&package.provider.form_schema, command.secret_json)?;
        let provider_id = Uuid::now_v7();
        let secret_ref = format!("secret://system/network-egress/{provider_id}");

        let provider = self
            .repository
            .create_network_egress_provider(&CreateNetworkEgressProviderInput {
                provider_id,
                installation_id: Some(command.installation_id),
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

    pub async fn create_proxy(
        &self,
        command: CreateNetworkEgressProxyCommand,
    ) -> Result<NetworkEgressProviderView> {
        let provider_code = required_text(&command.provider_code, "provider_code")?;
        if provider_code == "builtin_static_http" {
            return self.create_static_http_proxy(command).await;
        }
        let proxy_type = self
            .list_types()
            .await?
            .into_iter()
            .find(|proxy_type| proxy_type.provider_code == provider_code)
            .ok_or(ControlPlaneError::InvalidInput("provider_code"))?;
        let installation_id = proxy_type
            .installation_id
            .ok_or(ControlPlaneError::InvalidInput("provider_code"))?;
        let provider = self
            .create(CreateNetworkEgressProviderCommand {
                actor_user_id: command.actor_user_id,
                installation_id,
                display_name: command.display_name,
                description: command.description,
                secret_json: command.config,
            })
            .await?;
        if provider.provider.health_status != domain::NetworkEgressHealthStatus::Healthy {
            return Err(ControlPlaneError::Conflict("network_egress_proxy_parse_failed").into());
        }
        let pool =
            ensure_global_network_egress_pool(&self.repository, command.actor_user_id).await?;
        for (sequence, egress) in provider.egresses.iter().enumerate() {
            self.repository
                .create_network_egress_pool_member(
                    &crate::ports::CreateNetworkEgressPoolMemberInput {
                        member_id: Uuid::now_v7(),
                        pool_id: pool.id,
                        provider_id: provider.provider.id,
                        provider_egress_key: egress.provider_egress_key.clone(),
                        enabled: egress.availability == "available",
                        sequence: sequence as i32,
                        actor_user_id: command.actor_user_id,
                    },
                )
                .await?;
        }
        self.repository
            .append_audit_log(&audit_log(
                None,
                Some(command.actor_user_id),
                "network_egress_pool",
                Some(GLOBAL_NETWORK_EGRESS_POOL_ID),
                "network_egress_pool.members_added",
                serde_json::json!({ "provider_id": provider.provider.id }),
            ))
            .await?;
        Ok(provider)
    }

    async fn create_static_http_proxy(
        &self,
        command: CreateNetworkEgressProxyCommand,
    ) -> Result<NetworkEgressProviderView> {
        let config =
            validate_instance_config(&builtin_static_http_type().form_schema, command.config)?;
        let host = required_text(config["host"].as_str().unwrap_or_default(), "host")?;
        if host.contains(['/', '@', ':']) || host.chars().any(char::is_whitespace) {
            return Err(ControlPlaneError::InvalidInput("host").into());
        }
        let port = config["port"]
            .as_str()
            .ok_or(ControlPlaneError::InvalidInput("port"))?
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or(ControlPlaneError::InvalidInput("port"))?;
        let display_name = required_text(&command.display_name, "display_name")?;
        let provider_id = Uuid::now_v7();
        let pool =
            ensure_global_network_egress_pool(&self.repository, command.actor_user_id).await?;
        self.repository
            .create_static_http_proxy_pool_member(
                &crate::ports::CreateStaticHttpProxyPoolMemberInput {
                    provider_id,
                    member_id: Uuid::now_v7(),
                    pool_id: pool.id,
                    display_name,
                    description: command.description,
                    secret_ref: format!("secret://system/network-egress/{provider_id}"),
                    plaintext_secret_json: serde_json::json!({
                        "host": host,
                        "port": port,
                        "username": config["username"].as_str().unwrap_or_default(),
                        "password": config["password"].as_str().unwrap_or_default(),
                    }),
                    master_key: self.secret_master_key.clone(),
                    enabled: true,
                    sequence: 0,
                    synchronized_at: OffsetDateTime::now_utc(),
                    actor_user_id: command.actor_user_id,
                },
            )
            .await?;
        let provider = self
            .repository
            .get_network_egress_provider(provider_id)
            .await?
            .expect("static HTTP proxy is persisted with its provider");
        let egresses = self
            .repository
            .list_network_egress_projections(provider_id)
            .await?;
        Ok(NetworkEgressProviderView { provider, egresses })
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
        let installation_id = provider
            .installation_id
            .ok_or(ControlPlaneError::InvalidInput(
                "network_egress_provider_type",
            ))?;
        let local_installation = self
            .repository
            .get_local_installation(&self.node_id, installation_id)
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

fn plugin_version_order(left: &str, right: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn builtin_static_http_type() -> NetworkEgressProviderTypeView {
    let required_text = |key: &str, label: &str| PluginFormFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        field_type: "string".to_string(),
        control: None,
        group: None,
        order: None,
        advanced: None,
        required: Some(true),
        send_mode: None,
        enabled_by_default: None,
        description: None,
        placeholder: None,
        default_value: None,
        min: None,
        max: None,
        step: None,
        precision: None,
        unit: None,
        options: Vec::new(),
        visible_when: Vec::new(),
        disabled_when: Vec::new(),
    };
    let mut username = required_text("username", "Username");
    username.required = Some(false);
    let mut password = required_text("password", "Password");
    password.required = Some(false);
    NetworkEgressProviderTypeView {
        installation_id: None,
        provider_code: "builtin_static_http".to_string(),
        display_name: "HTTP proxy".to_string(),
        form_schema: PluginFormSchema {
            schema_version: "1flowbase.plugin.form/v1".to_string(),
            title: None,
            description: None,
            fields: vec![
                required_text("host", "Hostname or IP"),
                required_text("port", "Port"),
                username,
                password,
            ],
        },
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

#[cfg(test)]
mod tests {
    use super::builtin_static_http_type;

    #[test]
    fn ac_nc_global_pool_catalog_includes_the_builtin_http_proxy_type() {
        let proxy_type = builtin_static_http_type();
        assert_eq!(proxy_type.provider_code, "builtin_static_http");
        assert!(proxy_type.installation_id.is_none());
        assert_eq!(
            proxy_type
                .form_schema
                .fields
                .iter()
                .map(|field| field.key.as_str())
                .collect::<Vec<_>>(),
            vec!["host", "port", "username", "password"]
        );
    }
}
