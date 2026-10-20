use super::*;

impl<R> McpManagementService<R>
where
    R: McpManagementRepository,
{
    pub async fn get_client_credential(
        &self,
        actor_user_id: Uuid,
        instance_id: &str,
        master_key: &str,
    ) -> Result<Option<String>> {
        let actor = self.authorize_view(actor_user_id).await?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .get_mcp_client_credential(
                actor_user_id,
                actor.current_workspace_id,
                instance.id,
                master_key,
            )
            .await
    }

    pub async fn save_client_credential(
        &self,
        command: SaveMcpClientCredentialCommand,
    ) -> Result<()> {
        if command.api_key.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput("api_key").into());
        }
        let actor = self.authorize_manage(command.actor_user_id).await?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .upsert_mcp_client_credential(&UpsertMcpClientCredentialInput {
                id: Uuid::now_v7(),
                user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                instance_record_id: instance.id,
                api_key: command.api_key,
                master_key: command.master_key,
            })
            .await
    }

    pub async fn delete_client_credential(
        &self,
        actor_user_id: Uuid,
        instance_id: &str,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .delete_mcp_client_credential(actor_user_id, actor.current_workspace_id, instance.id)
            .await
    }

    pub async fn list_upstream_connections(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::McpUpstreamConnectionRecord>> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.repository
            .list_mcp_upstream_connections(actor.current_workspace_id)
            .await
    }

    pub async fn get_upstream_connection(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.get_upstream_connection_for_actor(&actor, connection_id)
            .await
    }

    pub async fn get_upstream_connection_for_actor(
        &self,
        actor: &domain::ActorContext,
        connection_id: Uuid,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        self.repository
            .get_mcp_upstream_connection(actor.current_workspace_id, connection_id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("mcp_upstream_connection").into())
    }

    pub async fn save_upstream_connection(
        &self,
        command: SaveMcpUpstreamConnectionCommand,
    ) -> Result<domain::McpUpstreamConnectionRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_upstream_endpoint(&command.endpoint)?;
        validate_upstream_header_name(command.auth_type, command.custom_header_name.as_deref())?;
        let id = command.connection_id.unwrap_or_else(Uuid::now_v7);
        if command.connection_id.is_some() {
            let existing = self
                .repository
                .get_mcp_upstream_connection(actor.current_workspace_id, id)
                .await?
                .ok_or(ControlPlaneError::NotFound("mcp_upstream_connection"))?;
            let clear_secret = existing.auth_type != command.auth_type
                || existing.custom_header_name != command.custom_header_name
                || command.auth_type == domain::McpUpstreamAuthType::None;
            let record = self
                .repository
                .update_mcp_upstream_connection(&UpdateMcpUpstreamConnectionInput {
                    id,
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    name: command.name,
                    endpoint: command.endpoint,
                    transport: command.transport,
                    auth_type: command.auth_type,
                    custom_header_name: command.custom_header_name,
                    status: command.status,
                })
                .await?;
            if clear_secret {
                self.repository
                    .delete_mcp_upstream_secret(actor.current_workspace_id, id)
                    .await?;
            }
            Ok(record)
        } else {
            self.repository
                .create_mcp_upstream_connection(&CreateMcpUpstreamConnectionInput {
                    id,
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    name: command.name,
                    endpoint: command.endpoint,
                    transport: command.transport,
                    auth_type: command.auth_type,
                    custom_header_name: command.custom_header_name,
                    status: command.status,
                })
                .await
        }
    }

    pub async fn delete_upstream_connection(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        let referenced = self
            .repository
            .list_mcp_tools(actor.current_workspace_id)
            .await?
            .iter()
            .any(|tool| {
                matches!(
                    &tool.execution_target,
                    domain::McpToolExecutionTarget::McpProxy { upstream_connection_id, .. }
                        if *upstream_connection_id == connection_id
                )
            });
        if referenced {
            return Err(ControlPlaneError::Conflict("mcp_upstream_connection_referenced").into());
        }
        self.repository
            .delete_mcp_upstream_connection(actor.current_workspace_id, connection_id)
            .await
    }

    pub async fn save_upstream_credential(
        &self,
        command: SaveMcpUpstreamCredentialCommand,
    ) -> Result<()> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        let connection = self
            .repository
            .get_mcp_upstream_connection(actor.current_workspace_id, command.connection_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_upstream_connection"))?;
        let secret = match command.credential {
            McpUpstreamCredential::Bearer { token }
                if connection.auth_type == domain::McpUpstreamAuthType::Bearer
                    && !token.trim().is_empty() =>
            {
                serde_json::json!({"token": token})
            }
            McpUpstreamCredential::CustomHeader {
                header_name,
                header_value,
            } if connection.auth_type == domain::McpUpstreamAuthType::CustomHeader
                && connection.custom_header_name.as_deref() == Some(header_name.as_str())
                && !header_value.is_empty() =>
            {
                validate_upstream_header_name(connection.auth_type, Some(&header_name))?;
                serde_json::json!({"header_name": header_name, "header_value": header_value})
            }
            _ => return Err(ControlPlaneError::InvalidInput("credential").into()),
        };
        self.repository
            .upsert_mcp_upstream_secret(&UpsertMcpUpstreamSecretInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                upstream_connection_id: command.connection_id,
                plaintext_secret_json: secret,
                master_key: command.master_key,
            })
            .await
    }

    pub async fn delete_upstream_credential(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .delete_mcp_upstream_secret(actor.current_workspace_id, connection_id)
            .await
    }

    pub async fn upstream_secret_for_execution(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.upstream_secret_for_actor(&actor, connection_id, master_key)
            .await
    }

    pub async fn upstream_secret_for_actor(
        &self,
        actor: &domain::ActorContext,
        connection_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        self.repository
            .get_mcp_upstream_secret(actor.current_workspace_id, connection_id, master_key)
            .await
    }

    pub async fn upstream_proxy_availability(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
        remote_tool_name: &str,
    ) -> Result<domain::McpToolAvailabilityStatus> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.upstream_proxy_availability_for_actor(&actor, connection_id, remote_tool_name)
            .await
    }

    pub async fn upstream_proxy_availability_for_actor(
        &self,
        actor: &domain::ActorContext,
        connection_id: Uuid,
        remote_tool_name: &str,
    ) -> Result<domain::McpToolAvailabilityStatus> {
        let Some(connection) = self
            .repository
            .get_mcp_upstream_connection(actor.current_workspace_id, connection_id)
            .await?
        else {
            return Ok(domain::McpToolAvailabilityStatus::UpstreamDisabled);
        };
        if connection.status != domain::McpUpstreamConnectionStatus::Enabled {
            return Ok(domain::McpToolAvailabilityStatus::UpstreamDisabled);
        }
        if connection.auth_type != domain::McpUpstreamAuthType::None
            && !connection.credentials_configured
        {
            return Ok(domain::McpToolAvailabilityStatus::CredentialsMissing);
        }
        let source = self
            .repository
            .list_mcp_upstream_tool_sources(actor.current_workspace_id, connection_id)
            .await?
            .into_iter()
            .find(|source| source.remote_tool_name == remote_tool_name);
        Ok(match source.map(|source| source.source_status) {
            Some(domain::McpUpstreamSourceStatus::Imported) => {
                domain::McpToolAvailabilityStatus::Available
            }
            Some(domain::McpUpstreamSourceStatus::DefinitionChanged) => {
                domain::McpToolAvailabilityStatus::MappingInvalid
            }
            Some(domain::McpUpstreamSourceStatus::RemoteMissing) | None => {
                domain::McpToolAvailabilityStatus::UpstreamToolMissing
            }
            Some(domain::McpUpstreamSourceStatus::NotImported) => {
                domain::McpToolAvailabilityStatus::UpstreamToolMissing
            }
        })
    }

    pub async fn prepare_upstream_management_action(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
        master_key: &str,
    ) -> Result<(
        domain::McpUpstreamConnectionRecord,
        Option<serde_json::Value>,
    )> {
        let actor = self.authorize_manage(actor_user_id).await?;
        let connection = self
            .repository
            .get_mcp_upstream_connection(actor.current_workspace_id, connection_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_upstream_connection"))?;
        let secret = self
            .repository
            .get_mcp_upstream_secret(actor.current_workspace_id, connection_id, master_key)
            .await?;
        Ok((connection, secret))
    }

    pub async fn record_upstream_result(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
        connected_at: Option<time::OffsetDateTime>,
        discovered_at: Option<time::OffsetDateTime>,
        last_error: Option<&str>,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .record_mcp_upstream_connection_result(
                actor.current_workspace_id,
                connection_id,
                connected_at,
                discovered_at,
                last_error,
            )
            .await
    }

    pub async fn record_upstream_discovery(
        &self,
        command: RecordMcpUpstreamDiscoveryCommand,
    ) -> Result<Vec<domain::McpUpstreamToolSourceRecord>> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.repository
            .get_mcp_upstream_connection(actor.current_workspace_id, command.connection_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_upstream_connection"))?;
        let discovered_remote_tool_names = command
            .tools
            .iter()
            .map(|tool| tool.remote_tool_name.clone())
            .collect::<Vec<_>>();
        self.repository
            .mark_mcp_upstream_tool_sources_missing(
                actor.current_workspace_id,
                command.connection_id,
                &discovered_remote_tool_names,
            )
            .await?;
        for tool in command.tools {
            self.repository
                .upsert_mcp_upstream_tool_source(&UpsertMcpUpstreamToolSourceInput {
                    id: Uuid::now_v7(),
                    workspace_id: actor.current_workspace_id,
                    upstream_connection_id: command.connection_id,
                    remote_tool_name: tool.remote_tool_name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                    output_schema: tool.output_schema,
                    schema_hash: tool.schema_hash,
                    source_status: domain::McpUpstreamSourceStatus::NotImported,
                    discovered_at: command.discovered_at,
                })
                .await?;
        }
        self.repository
            .record_mcp_upstream_connection_result(
                actor.current_workspace_id,
                command.connection_id,
                Some(command.discovered_at),
                Some(command.discovered_at),
                None,
            )
            .await?;
        self.repository
            .list_mcp_upstream_tool_sources(actor.current_workspace_id, command.connection_id)
            .await
    }

    pub async fn import_upstream_tools(
        &self,
        actor_user_id: Uuid,
        connection_id: Uuid,
        remote_tool_names: &[String],
    ) -> Result<Vec<domain::McpToolRecord>> {
        let actor = self.authorize_manage(actor_user_id).await?;
        let sources = self
            .repository
            .list_mcp_upstream_tool_sources(actor.current_workspace_id, connection_id)
            .await?;
        let mut imported = Vec::new();
        for remote_tool_name in remote_tool_names {
            let source = sources
                .iter()
                .find(|source| source.remote_tool_name == *remote_tool_name)
                .ok_or(ControlPlaneError::NotFound("mcp_upstream_tool_source"))?;
            if let Some(tool_id) = &source.imported_tool_id {
                if let Some(tool) = self
                    .repository
                    .get_mcp_tool(actor.current_workspace_id, tool_id)
                    .await?
                {
                    imported.push(tool);
                    continue;
                }
            }
            let tool_id = proxy_tool_id(connection_id, &source.remote_tool_name);
            let tool = self
                .repository
                .create_mcp_tool(&CreateMcpToolInput {
                    id: Uuid::now_v7(),
                    actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    tool_id,
                    name: source.remote_tool_name.clone(),
                    short_description: source.description.clone().unwrap_or_default(),
                    full_description: source.description.clone().unwrap_or_default(),
                    execution_target: domain::McpToolExecutionTarget::McpProxy {
                        upstream_connection_id: connection_id,
                        remote_tool_name: source.remote_tool_name.clone(),
                        source_schema_hash: source.schema_hash.clone(),
                    },
                    parameter_schema: source.input_schema.clone(),
                    result_schema: source.output_schema.clone(),
                    input_mapping: proxy_input_mapping(&source.input_schema),
                    output_mapping: proxy_output_mapping(&source.output_schema),
                    max_inline_chars: None,
                    response_fields: None,
                    permission_code: None,
                    risk_level: domain::McpRiskLevel::High,
                    des_id: generate_short_id(),
                    des_id_required: false,
                    status: domain::McpToolStatus::Draft,
                })
                .await?;
            self.repository
                .link_mcp_upstream_tool_source(
                    actor.current_workspace_id,
                    connection_id,
                    remote_tool_name,
                    tool.id,
                )
                .await?;
            imported.push(tool);
        }
        Ok(imported)
    }
}
