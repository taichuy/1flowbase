use std::collections::BTreeSet;

use anyhow::Result;
use rand_core::{OsRng, RngCore};
use regex::Regex;
use uuid::Uuid;

mod upstream_contract;
use upstream_contract::{
    proxy_input_mapping, proxy_output_mapping, proxy_tool_id, validate_proxy_mapping_contract,
    validate_upstream_endpoint, validate_upstream_header_name,
};
pub use upstream_contract::{
    McpRemoteToolDefinition, McpUpstreamCredential, RecordMcpUpstreamDiscoveryCommand,
    SaveMcpUpstreamConnectionCommand, SaveMcpUpstreamCredentialCommand, UpdateMcpProxyToolCommand,
};

use crate::{
    errors::ControlPlaneError,
    ports::{
        CreateMcpInstanceInput, CreateMcpToolBindingInput, CreateMcpToolInput,
        CreateMcpUpstreamConnectionInput, McpManagementRepository,
        UpdateMcpInstanceDiscoveryPolicyInput, UpdateMcpInstanceInput, UpdateMcpToolBindingInput,
        UpdateMcpToolInput, UpdateMcpUpstreamConnectionInput, UpsertMcpClientCredentialInput,
        UpsertMcpGroupInput, UpsertMcpUpstreamSecretInput, UpsertMcpUpstreamToolSourceInput,
    },
};

pub struct CreateMcpInstanceCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: domain::McpInstanceStatus,
    pub default_entry_path: String,
}

pub struct UpsertMcpGroupCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

pub struct MoveMcpGroupCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub source_path: String,
    pub target_parent_path: String,
    pub sort_order: i32,
}

pub struct CreateMcpToolCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_entry: domain::McpInterfaceCatalogEntry,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub status: domain::McpToolStatus,
}

pub struct UpdateMcpToolCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_entry: domain::McpInterfaceCatalogEntry,
    pub input_mapping: serde_json::Value,
    pub output_mapping: serde_json::Value,
    pub status: domain::McpToolStatus,
}

pub struct RefreshMcpToolDescriptionCommand {
    pub actor_user_id: Uuid,
    pub tool_id: String,
}

pub struct CreateMcpToolBindingCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

pub struct UpdateMcpToolBindingCommand {
    pub actor_user_id: Uuid,
    pub binding_id: Uuid,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

pub struct UpdateMcpInstanceDiscoveryPolicyCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    pub list_return_fields: serde_json::Value,
}

pub struct SaveMcpClientCredentialCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub api_key: String,
    pub master_key: String,
}

pub struct McpManagementService<R> {
    pub(crate) repository: R,
}

impl<R> McpManagementService<R>
where
    R: McpManagementRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

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

    pub async fn read_workspace_catalog(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::McpCatalogSnapshot> {
        let actor = self.authorize_view(actor_user_id).await?;
        let workspace_id = actor.current_workspace_id;
        let instances = self.repository.list_mcp_instances(workspace_id).await?;
        let instance_record_ids = instances
            .iter()
            .map(|instance| instance.id)
            .collect::<Vec<_>>();
        let groups = self
            .repository
            .list_mcp_groups(&instance_record_ids)
            .await?;
        let bindings = self
            .repository
            .list_mcp_tool_bindings(&instance_record_ids)
            .await?;
        let tools = self.repository.list_mcp_tools(workspace_id).await?;
        let discovery_policies = self
            .repository
            .list_mcp_instance_discovery_policies(&instance_record_ids)
            .await?;

        Ok(domain::McpCatalogSnapshot {
            instances,
            groups,
            tools,
            bindings,
            discovery_policies,
        })
    }

    pub async fn create_instance(
        &self,
        command: CreateMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_identifier(&command.instance_id, "instance_id")?;
        validate_path(&command.default_entry_path)?;
        self.repository
            .create_mcp_instance(&CreateMcpInstanceInput {
                id: Uuid::now_v7(),
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                instance_id: command.instance_id,
                name: command.name,
                description_short: command.description_short,
                status: command.status,
                default_entry_path: command.default_entry_path,
            })
            .await
    }

    pub async fn update_instance(
        &self,
        command: CreateMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_identifier(&command.instance_id, "instance_id")?;
        validate_path(&command.default_entry_path)?;
        self.repository
            .update_mcp_instance(&UpdateMcpInstanceInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                instance_id: command.instance_id,
                name: command.name,
                description_short: command.description_short,
                status: command.status,
                default_entry_path: command.default_entry_path,
            })
            .await
    }

    pub async fn delete_instance(&self, actor_user_id: Uuid, instance_id: &str) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .delete_mcp_instance(actor.current_workspace_id, instance_id)
            .await
    }

    pub async fn upsert_group(
        &self,
        command: UpsertMcpGroupCommand,
    ) -> Result<domain::McpGroupRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_path(&command.path)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .upsert_mcp_group(&UpsertMcpGroupInput {
                id: Uuid::now_v7(),
                actor_user_id: command.actor_user_id,
                instance_record_id: instance.id,
                path: command.path,
                display_name: command.display_name,
                description_short: command.description_short,
                enabled: command.enabled,
                sort_order: command.sort_order,
            })
            .await
    }

    pub async fn delete_group(
        &self,
        actor_user_id: Uuid,
        instance_id: &str,
        path: &str,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        validate_path(path)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        let group = self
            .repository
            .list_mcp_groups(&[instance.id])
            .await?
            .into_iter()
            .find(|group| group.path == path)
            .ok_or(ControlPlaneError::NotFound("mcp_group"))?;
        self.repository.delete_mcp_group(group.id).await
    }

    pub async fn move_group(&self, command: MoveMcpGroupCommand) -> Result<domain::McpGroupRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_path(&command.source_path)?;
        validate_path(&command.target_parent_path)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        let groups = self.repository.list_mcp_groups(&[instance.id]).await?;
        if !groups.iter().any(|group| group.path == command.source_path) {
            return Err(ControlPlaneError::NotFound("mcp_group").into());
        }
        if command.target_parent_path != "/"
            && !groups
                .iter()
                .any(|group| group.path == command.target_parent_path)
        {
            return Err(ControlPlaneError::NotFound("mcp_group_parent").into());
        }
        if command.target_parent_path == command.source_path
            || command
                .target_parent_path
                .starts_with(&format!("{}/", command.source_path))
        {
            return Err(ControlPlaneError::InvalidInput("target_parent_path").into());
        }
        let leaf = command
            .source_path
            .rsplit('/')
            .next()
            .filter(|value| !value.is_empty())
            .ok_or(ControlPlaneError::InvalidInput("source_path"))?;
        let target_path = if command.target_parent_path == "/" {
            format!("/{leaf}")
        } else {
            format!("{}/{leaf}", command.target_parent_path)
        };
        if target_path != command.source_path
            && groups.iter().any(|group| group.path == target_path)
        {
            return Err(ControlPlaneError::Conflict("mcp_group_path").into());
        }
        self.repository
            .move_mcp_group(
                command.actor_user_id,
                instance.id,
                &command.source_path,
                &target_path,
                command.sort_order,
            )
            .await
    }

    pub async fn create_tool(
        &self,
        command: CreateMcpToolCommand,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_identifier(&command.tool_id, "tool_id")?;
        let des_id = normalize_des_id(command.des_id);
        let interface = bindable_interface(command.interface_entry)?;
        let des_id_required = input_mapping_requires_des_id(&command.input_mapping);
        self.repository
            .create_mcp_tool(&CreateMcpToolInput {
                id: Uuid::now_v7(),
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                tool_id: command.tool_id,
                name: command.name,
                short_description: command.short_description,
                full_description: command.full_description,
                execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                    interface_id: interface.interface_id,
                },
                parameter_schema: interface.parameter_schema,
                result_schema: interface.result_schema,
                input_mapping: command.input_mapping,
                output_mapping: command.output_mapping,
                permission_code: interface.permission_code,
                risk_level: interface.risk_level,
                des_id,
                des_id_required,
                status: command.status,
            })
            .await
    }

    pub async fn update_tool(
        &self,
        command: UpdateMcpToolCommand,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_identifier(&command.tool_id, "tool_id")?;
        let des_id = normalize_des_id(command.des_id);
        let interface = bindable_interface(command.interface_entry)?;
        let des_id_required = input_mapping_requires_des_id(&command.input_mapping);
        self.repository
            .update_mcp_tool(&UpdateMcpToolInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                tool_id: command.tool_id,
                name: command.name,
                short_description: command.short_description,
                full_description: command.full_description,
                execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                    interface_id: interface.interface_id,
                },
                parameter_schema: interface.parameter_schema,
                result_schema: interface.result_schema,
                input_mapping: command.input_mapping,
                output_mapping: command.output_mapping,
                permission_code: interface.permission_code,
                risk_level: interface.risk_level,
                des_id,
                des_id_required,
                status: command.status,
            })
            .await
    }

    pub async fn update_proxy_tool(
        &self,
        command: UpdateMcpProxyToolCommand,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        let existing = self
            .repository
            .get_mcp_tool(actor.current_workspace_id, &command.tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        if !matches!(
            existing.execution_target,
            domain::McpToolExecutionTarget::McpProxy { .. }
        ) || existing.execution_target != command.execution_target
        {
            return Err(ControlPlaneError::InvalidInput("execution_target").into());
        }
        validate_proxy_mapping_contract(
            &command.input_mapping,
            "local_path",
            "remote_path",
            "input_mapping",
        )?;
        validate_proxy_mapping_contract(
            &command.output_mapping,
            "remote_path",
            "local_path",
            "output_mapping",
        )?;
        self.repository
            .update_mcp_tool(&UpdateMcpToolInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                tool_id: command.tool_id,
                name: command.name,
                short_description: command.short_description,
                full_description: command.full_description,
                execution_target: command.execution_target,
                parameter_schema: command.parameter_schema,
                result_schema: command.result_schema,
                input_mapping: command.input_mapping,
                output_mapping: command.output_mapping,
                permission_code: None,
                risk_level: command.risk_level,
                des_id: normalize_des_id(command.des_id),
                des_id_required: false,
                status: command.status,
            })
            .await
    }

    pub async fn get_tool(
        &self,
        actor_user_id: Uuid,
        tool_id: &str,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_view(actor_user_id).await?;
        Ok(self
            .repository
            .get_mcp_tool(actor.current_workspace_id, tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?)
    }

    pub async fn refresh_tool_description(
        &self,
        command: RefreshMcpToolDescriptionCommand,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.repository
            .refresh_mcp_tool_des_id(
                actor.current_workspace_id,
                command.actor_user_id,
                &command.tool_id,
                &generate_short_id(),
            )
            .await
    }

    pub async fn delete_tool(&self, actor_user_id: Uuid, tool_id: &str) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .delete_mcp_tool(actor.current_workspace_id, tool_id)
            .await
    }

    pub async fn create_tool_binding(
        &self,
        command: CreateMcpToolBindingCommand,
    ) -> Result<domain::McpToolBindingRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_path(&command.group_path)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        let tool = self
            .repository
            .get_mcp_tool(actor.current_workspace_id, &command.tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        self.repository
            .create_mcp_tool_binding(&CreateMcpToolBindingInput {
                id: Uuid::now_v7(),
                actor_user_id: command.actor_user_id,
                instance_record_id: instance.id,
                tool_record_id: tool.id,
                group_path: command.group_path,
                display_alias: command.display_alias,
                visible: command.visible,
                sort_order: command.sort_order,
            })
            .await
    }

    pub async fn update_tool_binding(
        &self,
        command: UpdateMcpToolBindingCommand,
    ) -> Result<domain::McpToolBindingRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_path(&command.group_path)?;
        self.repository
            .update_mcp_tool_binding(&UpdateMcpToolBindingInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                binding_id: command.binding_id,
                group_path: command.group_path,
                display_alias: command.display_alias,
                visible: command.visible,
                sort_order: command.sort_order,
            })
            .await
    }

    pub async fn delete_tool_binding(&self, actor_user_id: Uuid, binding_id: Uuid) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .delete_mcp_tool_binding(actor.current_workspace_id, binding_id)
            .await
    }

    pub async fn get_instance_discovery_policy(
        &self,
        actor_user_id: Uuid,
        instance_id: &str,
    ) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
        let actor = self.authorize_view(actor_user_id).await?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .get_mcp_instance_discovery_policy(instance.id)
            .await?
            .ok_or_else(|| ControlPlaneError::NotFound("mcp_instance_discovery_policy").into())
    }

    pub async fn update_instance_discovery_policy(
        &self,
        command: UpdateMcpInstanceDiscoveryPolicyCommand,
    ) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_positive(command.list_default_limit, "list_default_limit")?;
        validate_positive(command.list_max_depth, "list_max_depth")?;
        validate_positive(command.list_regex_max_length, "list_regex_max_length")?;
        validate_list_return_fields(&command.list_return_fields)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .update_mcp_instance_discovery_policy(&UpdateMcpInstanceDiscoveryPolicyInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                instance_record_id: instance.id,
                list_default_limit: command.list_default_limit,
                list_max_depth: command.list_max_depth,
                list_regex_enabled: command.list_regex_enabled,
                list_regex_max_length: command.list_regex_max_length,
                list_return_fields: command.list_return_fields,
            })
            .await
    }

    pub async fn description_check(
        &self,
        actor_user_id: Uuid,
        tool_id: &str,
        des_id: Option<&str>,
    ) -> Result<domain::McpDescriptionCheckResult> {
        let actor = self.authorize_view(actor_user_id).await?;
        let tool = self
            .repository
            .get_mcp_tool(actor.current_workspace_id, tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        let accepted = !tool.des_id_required || des_id.is_some_and(|value| value == tool.des_id);
        Ok(domain::McpDescriptionCheckResult {
            accepted,
            current_des_id: Some(tool.des_id),
        })
    }

    pub async fn list_items(
        &self,
        actor_user_id: Uuid,
        instance_id: Option<&str>,
        path: Option<&str>,
        path_regex: Option<&str>,
        keywords: Option<&[String]>,
        depth: Option<i32>,
        limit: Option<usize>,
    ) -> Result<Vec<domain::McpListItemSummary>> {
        let actor = self.authorize_view(actor_user_id).await?;
        let workspace_id = actor.current_workspace_id;
        let instance = match instance_id {
            Some(instance_id) => {
                self.repository
                    .get_mcp_instance(workspace_id, instance_id)
                    .await?
            }
            None => return Err(ControlPlaneError::InvalidInput("instance_id").into()),
        }
        .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        if instance.status != domain::McpInstanceStatus::Enabled {
            return Err(ControlPlaneError::NotFound("mcp_instance").into());
        }
        let discovery_policy = self
            .repository
            .get_mcp_instance_discovery_policy(instance.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance_discovery_policy"))?;
        let path_regex_filter = compile_list_path_regex(
            path_regex,
            discovery_policy.list_regex_enabled,
            discovery_policy.list_regex_max_length,
        )?;

        let groups = self.repository.list_mcp_groups(&[instance.id]).await?;
        let bindings = self
            .repository
            .list_mcp_tool_bindings(&[instance.id])
            .await?;
        let tools = self.repository.list_mcp_tools(workspace_id).await?;
        let base_path = path.unwrap_or(instance.default_entry_path.as_str());
        let max_depth = depth
            .unwrap_or(discovery_policy.list_max_depth)
            .clamp(0, discovery_policy.list_max_depth);
        let mut items = Vec::new();

        for group in groups.into_iter().filter(|group| {
            group.enabled
                && path_matches_list_query(
                    base_path,
                    &group.path,
                    max_depth,
                    path_regex_filter.as_ref(),
                )
        }) {
            if list_item_matches_keywords(
                keywords,
                &group.path,
                &group.display_name,
                group.description_short.as_deref(),
            ) {
                items.push(domain::McpListItemSummary {
                    id: group.id.to_string(),
                    item_kind: domain::McpListItemKind::Group,
                    path: group.path,
                    name: group.display_name,
                    description_short: group.description_short,
                    children_count: 0,
                    risk_level: None,
                });
            }
        }

        for binding in bindings.into_iter().filter(|binding| {
            binding.visible
                && path_matches_list_query(
                    base_path,
                    &binding.group_path,
                    max_depth,
                    path_regex_filter.as_ref(),
                )
        }) {
            if let Some(tool) = tools
                .iter()
                .find(|tool| tool.id == binding.tool_record_id)
                .filter(|tool| tool.status == domain::McpToolStatus::Enabled)
            {
                let display_name = binding
                    .display_alias
                    .as_deref()
                    .unwrap_or(tool.name.as_str());
                if list_item_matches_keywords(
                    keywords,
                    &binding.group_path,
                    display_name,
                    Some(&tool.short_description),
                ) {
                    items.push(domain::McpListItemSummary {
                        id: tool.tool_id.clone(),
                        item_kind: domain::McpListItemKind::Tool,
                        path: binding.group_path,
                        name: binding
                            .display_alias
                            .clone()
                            .unwrap_or_else(|| tool.name.clone()),
                        description_short: Some(tool.short_description.clone()),
                        children_count: 0,
                        risk_level: Some(tool.risk_level),
                    });
                }
            }
        }

        let limit = limit.unwrap_or(discovery_policy.list_default_limit as usize);
        items.truncate(limit);
        Ok(items)
    }

    pub async fn authorize_interface_catalog_view(&self, actor_user_id: Uuid) -> Result<()> {
        self.authorize_view(actor_user_id).await?;
        Ok(())
    }

    pub async fn authorize_debug_execute(&self, actor_user_id: Uuid) -> Result<()> {
        self.authorize_manage(actor_user_id).await?;
        Ok(())
    }

    pub async fn export_workspace_catalog(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::McpExportPackage> {
        let snapshot = self.read_workspace_catalog(actor_user_id).await?;
        Ok(domain::McpExportPackage {
            instances: snapshot.instances,
            groups: snapshot.groups,
            tools: snapshot.tools,
            bindings: snapshot.bindings,
            discovery_policies: snapshot.discovery_policies,
        })
    }

    pub async fn export_instance_directory(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::McpInstanceDirectoryExportPackage> {
        let snapshot = self.read_workspace_catalog(actor_user_id).await?;
        Ok(domain::McpInstanceDirectoryExportPackage {
            instances: snapshot.instances,
            groups: snapshot.groups,
            bindings: snapshot.bindings,
            discovery_policies: snapshot.discovery_policies,
        })
    }

    pub(crate) async fn authorize_view(&self, actor_user_id: Uuid) -> Result<domain::ActorContext> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if actor.is_root
            || actor.has_permission("mcp_management.view.all")
            || actor.has_permission("mcp_management.manage.all")
            || actor
                .has_permission(access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION)
        {
            return Ok(actor);
        }
        Err(ControlPlaneError::PermissionDenied("permission_denied").into())
    }

    pub(crate) async fn authorize_manage(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if actor.is_root
            || actor.has_permission("mcp_management.manage.all")
            || actor
                .has_permission(access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION)
        {
            return Ok(actor);
        }
        Err(ControlPlaneError::PermissionDenied("permission_denied").into())
    }
}

pub(crate) fn validate_identifier(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

pub(crate) fn validate_path(value: &str) -> Result<()> {
    if !value.starts_with('/') || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(())
}

pub(crate) fn validate_positive(value: i32, field: &'static str) -> Result<()> {
    if value <= 0 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

fn validate_list_return_fields(value: &serde_json::Value) -> Result<()> {
    let Some(fields) = value.as_array() else {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    };
    if fields.is_empty() {
        return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
    }

    let mut seen = BTreeSet::new();
    for field in fields {
        let Some(field) = field.as_str() else {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        };
        if ![
            "id",
            "type",
            "item_kind",
            "path",
            "name",
            "description_short",
            "children_count",
            "risk_level",
        ]
        .contains(&field)
            || !seen.insert(field)
        {
            return Err(ControlPlaneError::InvalidInput("list_return_fields").into());
        }
    }
    Ok(())
}

fn generate_short_id() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    let mut output = String::with_capacity(8);
    for _ in 0..8 {
        let index = (OsRng.next_u32() as usize) % ALPHABET.len();
        output.push(ALPHABET[index] as char);
    }
    output
}

pub(crate) fn normalize_des_id(value: Option<String>) -> String {
    let trimmed = value.unwrap_or_default().trim().to_owned();
    if trimmed.is_empty() {
        generate_short_id()
    } else {
        trimmed
    }
}

pub(crate) fn input_mapping_requires_des_id(input_mapping: &serde_json::Value) -> bool {
    const DES_ID: &str = "des_id";

    let Some(mapping) = input_mapping.as_object() else {
        return false;
    };

    let interface_parameter_required = mapping
        .get("interface_parameters")
        .and_then(serde_json::Value::as_array)
        .and_then(|parameters| {
            parameters.iter().find_map(|parameter| {
                let parameter = parameter.as_object()?;
                (parameter.get("name").and_then(serde_json::Value::as_str) == Some(DES_ID))
                    .then(|| {
                        parameter
                            .get("required")
                            .and_then(serde_json::Value::as_bool)
                    })
                    .flatten()
            })
        });

    mapping
        .get("mappings")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let entry = entry.as_object()?;
                let maps_des_id = entry
                    .get("interface_param")
                    .and_then(serde_json::Value::as_str)
                    == Some(DES_ID)
                    || entry.get("mcp_param").and_then(serde_json::Value::as_str) == Some(DES_ID);
                maps_des_id
                    .then(|| {
                        entry
                            .get("required")
                            .and_then(serde_json::Value::as_bool)
                            .or(interface_parameter_required)
                    })
                    .flatten()
            })
        })
        .or(interface_parameter_required)
        .unwrap_or(false)
}

fn path_matches(base_path: &str, candidate: &str) -> bool {
    base_path == "/" || candidate == base_path || candidate.starts_with(&format!("{base_path}/"))
}

fn list_item_matches_keywords(
    keywords: Option<&[String]>,
    path: &str,
    name: &str,
    description_short: Option<&str>,
) -> bool {
    let searchable = format!(
        "{} {} {}",
        path,
        name,
        description_short.unwrap_or_default()
    )
    .to_lowercase();
    keywords
        .unwrap_or_default()
        .iter()
        .filter(|keyword| !keyword.trim().is_empty())
        .all(|keyword| searchable.contains(&keyword.to_lowercase()))
}

fn path_matches_list_query(
    base_path: &str,
    candidate: &str,
    max_depth: i32,
    path_regex_filter: Option<&Regex>,
) -> bool {
    let Some(depth) = list_relative_depth(base_path, candidate) else {
        return false;
    };
    if depth > max_depth {
        return false;
    }
    path_regex_filter
        .map(|path_regex_filter| path_regex_filter.is_match(candidate))
        .unwrap_or(true)
}

fn list_relative_depth(base_path: &str, candidate: &str) -> Option<i32> {
    if !path_matches(base_path, candidate) {
        return None;
    }
    if candidate == base_path {
        return Some(0);
    }
    let relative_path = if base_path == "/" {
        candidate.trim_start_matches('/')
    } else {
        candidate.strip_prefix(base_path)?.trim_start_matches('/')
    };
    Some(
        relative_path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .count() as i32,
    )
}

fn compile_list_path_regex(
    pattern: Option<&str>,
    regex_enabled: bool,
    regex_max_length: i32,
) -> Result<Option<Regex>> {
    let Some(pattern) = pattern else {
        return Ok(None);
    };
    if !regex_enabled {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    let regex_max_length = usize::try_from(regex_max_length)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex"))?;
    if pattern.chars().count() > regex_max_length {
        return Err(ControlPlaneError::InvalidInput("path_regex").into());
    }
    Regex::new(pattern)
        .map(Some)
        .map_err(|_| ControlPlaneError::InvalidInput("path_regex").into())
}

fn bindable_interface(
    entry: domain::McpInterfaceCatalogEntry,
) -> Result<domain::McpInterfaceCatalogEntry> {
    if !entry.bindable {
        return Err(ControlPlaneError::InvalidInput("interface_id").into());
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::input_mapping_requires_des_id;

    #[test]
    fn input_mapping_des_id_required_is_derived_from_parameter_mapping() {
        assert!(!input_mapping_requires_des_id(&json!({})));

        assert!(input_mapping_requires_des_id(&json!({
            "interface_parameters": [
                {
                    "name": "des_id",
                    "field_type": "string",
                    "parameter_type": "json_body",
                    "description": "des_id",
                    "required": true
                }
            ],
            "mappings": [
                {
                    "interface_param": "des_id",
                    "mcp_param": "des_id",
                    "description": "des_id",
                    "required": true
                }
            ]
        })));

        assert!(!input_mapping_requires_des_id(&json!({
            "interface_parameters": [
                {
                    "name": "des_id",
                    "field_type": "string",
                    "parameter_type": "json_body",
                    "description": "des_id",
                    "required": false
                }
            ],
            "mappings": [
                {
                    "interface_param": "des_id",
                    "mcp_param": "des_id",
                    "description": "des_id",
                    "required": false
                }
            ]
        })));
    }
}
