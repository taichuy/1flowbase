use super::*;

pub struct UpdateNetworkEgressProxyCommand {
    pub actor_user_id: Uuid,
    pub provider_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    /// Omitted or blank secret fields preserve the stored value.
    pub config: Value,
}

pub struct NetworkEgressProxyView {
    pub provider_id: Uuid,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    pub config: Value,
    pub configured_secret_fields: Vec<String>,
    pub form_schema: PluginFormSchema,
}

impl<R, H, S> NetworkEgressProviderService<R, H, S>
where
    R: NetworkEgressRepository + NetworkEgressPoolRepository + PluginRepository,
    H: NetworkEgressRuntimePort,
    S: NetworkEgressSecretResolver,
{
    async fn proxy_schema(
        &self,
        provider: &domain::NetworkEgressProviderRecord,
    ) -> Result<PluginFormSchema> {
        if provider.provider_code == "builtin_static_http" {
            return Ok(builtin_static_http_type().form_schema);
        }
        let family = provider
            .extension_family
            .as_ref()
            .ok_or(ControlPlaneError::InvalidInput("provider_code"))?;
        let installation = self
            .repository
            .get_current_local_installation(&self.node_id, family)
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_unavailable",
            ))?;
        Ok(load_egress_package(&installation)?.provider.form_schema)
    }

    pub async fn get_proxy(&self, provider_id: Uuid) -> Result<NetworkEgressProxyView> {
        let provider = self
            .repository
            .get_network_egress_provider(provider_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("network_egress_provider"))?;
        let form_schema = self.proxy_schema(&provider).await?;
        let secret = self
            .repository
            .resolve_network_egress_provider_secret_json(
                provider_id,
                &provider.secret_ref,
                &self.secret_master_key,
            )
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_secret_unavailable",
            ))?;
        let mut config = Map::new();
        let mut configured_secret_fields = Vec::new();
        for field in &form_schema.fields {
            if let Some(value) = secret.get(&field.key) {
                if proxy_field_is_secret(&provider.provider_code, &field.key) {
                    if value.as_str().is_some_and(|value| !value.is_empty()) {
                        configured_secret_fields.push(field.key.clone());
                    }
                } else {
                    // The built-in port is stored as a number; its form contract is text.
                    let text = value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string());
                    config.insert(field.key.clone(), Value::String(text));
                }
            }
        }
        Ok(NetworkEgressProxyView {
            provider_id,
            provider_code: provider.provider_code,
            display_name: provider.display_name,
            description: provider.description,
            config: Value::Object(config),
            configured_secret_fields,
            form_schema,
        })
    }

    pub async fn update_proxy(&self, command: UpdateNetworkEgressProxyCommand) -> Result<()> {
        let provider = self
            .repository
            .get_network_egress_provider(command.provider_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("network_egress_provider"))?;
        if command.provider_code != provider.provider_code {
            return Err(ControlPlaneError::InvalidInput("provider_code").into());
        }
        let schema = self.proxy_schema(&provider).await?;
        let stored = self
            .repository
            .resolve_network_egress_provider_secret_json(
                provider.id,
                &provider.secret_ref,
                &self.secret_master_key,
            )
            .await?
            .ok_or(ControlPlaneError::Conflict(
                "network_egress_provider_secret_unavailable",
            ))?;
        let mut config = command
            .config
            .as_object()
            .cloned()
            .ok_or(ControlPlaneError::InvalidInput("config"))?;
        for field in &schema.fields {
            if proxy_field_is_secret(&provider.provider_code, &field.key)
                && config
                    .get(&field.key)
                    .is_none_or(|value| value.as_str().is_some_and(|text| text.trim().is_empty()))
            {
                if let Some(value) = stored.get(&field.key) {
                    config.insert(field.key.clone(), value.clone());
                }
            }
        }
        let mut config = validate_instance_config(&schema, Value::Object(config))?;
        let display_name = required_text(&command.display_name, "display_name")?;
        let now = OffsetDateTime::now_utc();
        let egresses = if provider.provider_code == "builtin_static_http" {
            let host = required_text(config["host"].as_str().unwrap_or_default(), "host")?;
            if host.contains(['/', '@', ':']) || host.chars().any(char::is_whitespace) {
                return Err(ControlPlaneError::InvalidInput("host").into());
            }
            let port = config["port"]
                .as_str()
                .and_then(|text| text.parse::<u16>().ok())
                .filter(|port| *port > 0)
                .ok_or(ControlPlaneError::InvalidInput("port"))?;
            config["host"] = Value::String(host);
            config["port"] = serde_json::json!(port);
            vec![domain::NetworkEgressProjectionRecord {
                provider_id: provider.id,
                provider_egress_key: "static-http".into(),
                display_name: display_name.clone(),
                region: None,
                tags: vec!["static".into(), "http".into()],
                availability: "available".into(),
                synced_at: now,
            }]
        } else {
            let family = provider
                .extension_family
                .as_ref()
                .ok_or(ControlPlaneError::InvalidInput("provider_code"))?;
            let installation = self
                .repository
                .get_current_local_installation(&self.node_id, family)
                .await?
                .ok_or(ControlPlaneError::Conflict(
                    "network_egress_provider_unavailable",
                ))?;
            // Parse in a temporary runtime so a failed edit cannot replace the live configuration.
            let candidate_id = Uuid::now_v7();
            let parsed = self
                .runtime
                .sync_network_egresses(
                    candidate_id,
                    &installation,
                    crate::ports::NetworkEgressSecretMaterial {
                        secret_ref: provider.secret_ref.clone(),
                        secret_json: config.clone(),
                    },
                )
                .await;
            self.runtime
                .unload_network_egress_provider(candidate_id)
                .await?;
            parsed
                .map_err(|_| ControlPlaneError::Conflict("network_egress_proxy_parse_failed"))?
                .into_iter()
                .map(|descriptor| domain::NetworkEgressProjectionRecord {
                    provider_id: provider.id,
                    provider_egress_key: descriptor.provider_egress_key,
                    display_name: descriptor.display_name,
                    region: descriptor.region,
                    tags: descriptor.tags.unwrap_or_default(),
                    availability: match descriptor.availability {
                        EgressAvailability::Available => "available",
                        EgressAvailability::Unavailable => "unavailable",
                    }
                    .into(),
                    synced_at: now,
                })
                .collect()
        };
        self.repository
            .update_network_egress_proxy(&crate::ports::UpdateNetworkEgressProxyInput {
                provider_id: provider.id,
                expected_updated_at: provider.updated_at,
                display_name,
                description: command.description,
                plaintext_secret_json: config,
                master_key: self.secret_master_key.clone(),
                egresses,
                actor_user_id: command.actor_user_id,
                audit_event: audit_log(
                    None,
                    Some(command.actor_user_id),
                    "network_egress_provider",
                    Some(provider.id),
                    "network_egress_provider.updated",
                    serde_json::json!({}),
                ),
            })
            .await?;
        if provider.provider_code != "builtin_static_http" {
            self.runtime
                .unload_network_egress_provider(provider.id)
                .await?;
        }
        Ok(())
    }
}

fn proxy_field_is_secret(provider_code: &str, key: &str) -> bool {
    // Extension config has no public-field declaration and may contain credential-bearing URLs.
    provider_code != "builtin_static_http" || !matches!(key, "host" | "port" | "username")
}
