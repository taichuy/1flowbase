use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Result;
use rand::rngs::SysRng;
use rand_core::{Rng, UnwrapErr};
use regex::Regex;
use uuid::Uuid;

mod commands;
mod description_check;
mod llm_registration;
mod upstream_contract;
pub use commands::{
    CopyMcpInstanceCommand, CreateMcpInstanceCommand, CreateMcpToolBindingCommand,
    CreateMcpToolCommand, MoveMcpGroupCommand, RefreshMcpToolDescriptionCommand,
    SaveMcpClientCredentialCommand, UpdateMcpInstanceDiscoveryPolicyCommand,
    UpdateMcpToolBindingCommand, UpdateMcpToolCommand, UpsertMcpGroupCommand,
};
pub use llm_registration::{
    mcp_llm_instance_registration, mcp_llm_registrations, McpLlmOperation, McpLlmRegistration,
    McpLlmRegistrationSource,
};
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
        CreateMcpInstanceGraphInput, CreateMcpInstanceInput, CreateMcpToolBindingInput,
        CreateMcpToolInput, CreateMcpUpstreamConnectionInput, McpManagementRepository,
        UpdateMcpInstanceDiscoveryPolicyInput, UpdateMcpInstanceInput, UpdateMcpToolBindingInput,
        UpdateMcpToolInput, UpdateMcpUpstreamConnectionInput, UpsertMcpClientCredentialInput,
        UpsertMcpGroupInput, UpsertMcpUpstreamSecretInput, UpsertMcpUpstreamToolSourceInput,
    },
};

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

    pub async fn read_workspace_catalog(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::McpCatalogSnapshot> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.read_catalog_for_actor(&actor).await
    }

    pub async fn read_catalog_for_actor(
        &self,
        actor: &domain::ActorContext,
    ) -> Result<domain::McpCatalogSnapshot> {
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
        let mut tools = self.repository.list_mcp_tools(workspace_id).await?;
        for tool in &mut tools {
            tool.des_id_required = domain::mcp_management::mcp_des_id_required(&tool.input_mapping);
        }
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
        self.create_instance_for_actor(&actor, command).await
    }

    pub async fn create_instance_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: CreateMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
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
                webmcp_exposure: command.webmcp_exposure,
            })
            .await
    }

    pub async fn copy_instance(
        &self,
        command: CopyMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.copy_instance_for_actor(&actor, command).await
    }

    pub async fn copy_instance_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: CopyMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        validate_identifier(&command.source_instance_id, "source_instance_id")?;
        validate_identifier(&command.instance_id, "instance_id")?;
        if command.name.trim().is_empty() {
            return Err(ControlPlaneError::InvalidInput("name").into());
        }
        let source = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, &command.source_instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        let source_groups = self.repository.list_mcp_groups(&[source.id]).await?;
        let source_bindings = self.repository.list_mcp_tool_bindings(&[source.id]).await?;
        let source_policy = self
            .repository
            .get_mcp_instance_discovery_policy(source.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance_discovery_policy"))?;
        let copied_instance_id = Uuid::now_v7();

        self.repository
            .create_mcp_instance_graph_atomically(&CreateMcpInstanceGraphInput {
                instance: CreateMcpInstanceInput {
                    id: copied_instance_id,
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    instance_id: command.instance_id,
                    name: command.name,
                    description_short: source.description_short,
                    status: domain::McpInstanceStatus::Draft,
                    default_entry_path: source.default_entry_path,
                    webmcp_exposure: domain::WebMcpExposure::Disabled,
                },
                groups: source_groups
                    .into_iter()
                    .map(|group| UpsertMcpGroupInput {
                        id: Uuid::now_v7(),
                        actor_user_id: command.actor_user_id,
                        instance_record_id: copied_instance_id,
                        path: group.path,
                        display_name: group.display_name,
                        description_short: group.description_short,
                        enabled: group.enabled,
                        sort_order: group.sort_order,
                    })
                    .collect(),
                bindings: source_bindings
                    .into_iter()
                    .map(|binding| CreateMcpToolBindingInput {
                        id: Uuid::now_v7(),
                        actor_user_id: command.actor_user_id,
                        instance_record_id: copied_instance_id,
                        tool_record_id: binding.tool_record_id,
                        group_path: binding.group_path,
                        display_alias: binding.display_alias,
                        visible: binding.visible,
                        sort_order: binding.sort_order,
                    })
                    .collect(),
                discovery_policy: UpdateMcpInstanceDiscoveryPolicyInput {
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    instance_record_id: copied_instance_id,
                    list_default_limit: source_policy.list_default_limit,
                    list_max_depth: source_policy.list_max_depth,
                    list_regex_enabled: source_policy.list_regex_enabled,
                    list_regex_max_length: source_policy.list_regex_max_length,
                    list_return_fields: source_policy.list_return_fields,
                },
            })
            .await
    }

    pub async fn update_instance(
        &self,
        command: CreateMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.update_instance_for_actor(&actor, command).await
    }

    pub async fn update_instance_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: CreateMcpInstanceCommand,
    ) -> Result<domain::McpInstanceRecord> {
        validate_identifier(&command.instance_id, "instance_id")?;
        validate_path(&command.default_entry_path)?;
        self.repository
            .get_mcp_instance(actor.current_workspace_id, &command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .update_mcp_instance(&UpdateMcpInstanceInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                instance_id: command.instance_id,
                name: command.name,
                description_short: command.description_short,
                status: command.status,
                default_entry_path: command.default_entry_path,
                webmcp_exposure: command.webmcp_exposure,
            })
            .await
    }

    pub async fn delete_instance(&self, actor_user_id: Uuid, instance_id: &str) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.delete_instance_for_actor(&actor, instance_id).await
    }

    pub async fn delete_instance_for_actor(
        &self,
        actor: &domain::ActorContext,
        instance_id: &str,
    ) -> Result<()> {
        self.repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .delete_mcp_instance(actor.current_workspace_id, instance_id)
            .await
    }

    pub async fn upsert_group(
        &self,
        command: UpsertMcpGroupCommand,
    ) -> Result<domain::McpGroupRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.upsert_group_for_actor(&actor, command).await
    }

    pub async fn upsert_group_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: UpsertMcpGroupCommand,
    ) -> Result<domain::McpGroupRecord> {
        validate_group_path(&command.path)?;
        let display_name = normalize_group_display_name(&command.path, &command.display_name)?;
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
                display_name,
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
        self.delete_group_for_actor(&actor, instance_id, path).await
    }

    pub async fn delete_group_for_actor(
        &self,
        actor: &domain::ActorContext,
        instance_id: &str,
        path: &str,
    ) -> Result<()> {
        validate_path(path)?;
        let instance = self
            .repository
            .get_mcp_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_instance"))?;
        self.repository
            .delete_mcp_group_subtree(instance.id, path)
            .await
    }

    pub async fn move_group(&self, command: MoveMcpGroupCommand) -> Result<domain::McpGroupRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.move_group_for_actor(&actor, command).await
    }

    pub async fn move_group_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: MoveMcpGroupCommand,
    ) -> Result<domain::McpGroupRecord> {
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
        self.create_tool_for_actor(&actor, command).await
    }

    pub async fn create_tool_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: CreateMcpToolCommand,
    ) -> Result<domain::McpToolRecord> {
        validate_identifier(&command.tool_id, "tool_id")?;
        domain::mcp_management::validate_mcp_return_defaults(
            command.max_inline_chars,
            command.response_fields.as_deref(),
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        domain::mcp_management::validate_mcp_call_parameters(&command.input_mapping)
            .map_err(ControlPlaneError::InvalidInput)?;
        let des_id = normalize_des_id(command.des_id);
        let interface = bindable_interface(command.interface_entry)?;
        domain::mcp_management::input_defaults::validate_input_defaults(
            &command.input_mapping,
            &interface.parameter_descriptors,
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        let des_id_required = domain::mcp_management::mcp_des_id_required(&command.input_mapping);
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
                max_inline_chars: command.max_inline_chars,
                response_fields: command.response_fields,
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
        self.update_tool_for_actor(&actor, command).await
    }

    pub async fn update_tool_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: UpdateMcpToolCommand,
    ) -> Result<domain::McpToolRecord> {
        validate_identifier(&command.tool_id, "tool_id")?;
        domain::mcp_management::validate_mcp_return_defaults(
            command.max_inline_chars,
            command.response_fields.as_deref(),
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        domain::mcp_management::validate_mcp_call_parameters(&command.input_mapping)
            .map_err(ControlPlaneError::InvalidInput)?;
        self.repository
            .get_mcp_tool(actor.current_workspace_id, &command.tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        let des_id = normalize_des_id(command.des_id);
        let interface = bindable_interface(command.interface_entry)?;
        domain::mcp_management::input_defaults::validate_input_defaults(
            &command.input_mapping,
            &interface.parameter_descriptors,
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        let des_id_required = domain::mcp_management::mcp_des_id_required(&command.input_mapping);
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
                max_inline_chars: command.max_inline_chars,
                response_fields: command.response_fields,
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
        self.update_proxy_tool_for_actor(&actor, command).await
    }

    pub async fn update_proxy_tool_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: UpdateMcpProxyToolCommand,
    ) -> Result<domain::McpToolRecord> {
        domain::mcp_management::validate_mcp_return_defaults(
            command.max_inline_chars,
            command.response_fields.as_deref(),
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        domain::mcp_management::validate_mcp_call_parameters(&command.input_mapping)
            .map_err(ControlPlaneError::InvalidInput)?;
        let existing = self
            .repository
            .get_mcp_tool(actor.current_workspace_id, &command.tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        if existing.execution_target != command.execution_target {
            return Err(ControlPlaneError::InvalidInput("execution_target").into());
        }
        if matches!(
            command.execution_target,
            domain::McpToolExecutionTarget::McpProxy { .. }
        ) {
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
        }
        let des_id_required = domain::mcp_management::mcp_des_id_required(&command.input_mapping);
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
                max_inline_chars: command.max_inline_chars,
                response_fields: command.response_fields,
                permission_code: None,
                risk_level: command.risk_level,
                des_id: normalize_des_id(command.des_id),
                des_id_required,
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
        let mut tool = self
            .repository
            .get_mcp_tool(actor.current_workspace_id, tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        tool.des_id_required = domain::mcp_management::mcp_des_id_required(&tool.input_mapping);
        Ok(tool)
    }

    pub async fn refresh_tool_description(
        &self,
        command: RefreshMcpToolDescriptionCommand,
    ) -> Result<domain::McpToolRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.refresh_tool_description_for_actor(&actor, command)
            .await
    }

    pub async fn refresh_tool_description_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: RefreshMcpToolDescriptionCommand,
    ) -> Result<domain::McpToolRecord> {
        self.repository
            .get_mcp_tool(actor.current_workspace_id, &command.tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
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
        self.delete_tool_for_actor(&actor, tool_id).await
    }

    pub async fn delete_tool_for_actor(
        &self,
        actor: &domain::ActorContext,
        tool_id: &str,
    ) -> Result<()> {
        self.repository
            .get_mcp_tool(actor.current_workspace_id, tool_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
        self.repository
            .delete_mcp_tool(actor.current_workspace_id, tool_id)
            .await
    }

    pub async fn create_tool_binding(
        &self,
        command: CreateMcpToolBindingCommand,
    ) -> Result<domain::McpToolBindingRecord> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        self.create_tool_binding_for_actor(&actor, command).await
    }

    pub async fn create_tool_binding_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: CreateMcpToolBindingCommand,
    ) -> Result<domain::McpToolBindingRecord> {
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
        self.update_tool_binding_for_actor(&actor, command).await
    }

    pub async fn update_tool_binding_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: UpdateMcpToolBindingCommand,
    ) -> Result<domain::McpToolBindingRecord> {
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
        self.delete_tool_binding_for_actor(&actor, binding_id).await
    }

    pub async fn delete_tool_binding_for_actor(
        &self,
        actor: &domain::ActorContext,
        binding_id: Uuid,
    ) -> Result<()> {
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
        self.get_instance_discovery_policy_for_actor(&actor, instance_id)
            .await
    }

    pub async fn get_instance_discovery_policy_for_actor(
        &self,
        actor: &domain::ActorContext,
        instance_id: &str,
    ) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
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
        self.update_instance_discovery_policy_for_actor(&actor, command)
            .await
    }

    pub async fn update_instance_discovery_policy_for_actor(
        &self,
        actor: &domain::ActorContext,
        command: UpdateMcpInstanceDiscoveryPolicyCommand,
    ) -> Result<domain::McpInstanceDiscoveryPolicyRecord> {
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

    pub async fn list_items(
        &self,
        actor_user_id: Uuid,
        query: McpListItemsQuery<'_>,
    ) -> Result<Vec<domain::McpListItemSummary>> {
        let actor = self.authorize_view(actor_user_id).await?;
        self.list_items_for_actor(&actor, query).await
    }

    pub async fn list_items_for_actor(
        &self,
        actor: &domain::ActorContext,
        query: McpListItemsQuery<'_>,
    ) -> Result<Vec<domain::McpListItemSummary>> {
        let McpListItemsQuery {
            instance_id,
            path,
            path_regex,
            keywords,
            depth,
            limit,
        } = query;
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
        let groups = self.repository.list_mcp_groups(&[instance.id]).await?;
        let bindings = self
            .repository
            .list_mcp_tool_bindings(&[instance.id])
            .await?;
        let tools = self.repository.list_mcp_tools(workspace_id).await?;
        let selected_id = instance.instance_id.clone();
        let catalog = domain::McpCatalogSnapshot {
            instances: vec![instance],
            groups,
            tools,
            bindings,
            discovery_policies: vec![discovery_policy],
        };
        list_catalog_items(
            &catalog,
            &selected_id,
            path,
            path_regex,
            keywords,
            depth,
            limit,
        )
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
        let actor = self.authorize_view(actor_user_id).await?;
        self.export_catalog_for_actor(&actor).await
    }

    pub async fn export_catalog_for_actor(
        &self,
        actor: &domain::ActorContext,
    ) -> Result<domain::McpExportPackage> {
        let snapshot = self.read_catalog_for_actor(actor).await?;
        Ok(domain::McpExportPackage {
            instances: snapshot.instances,
            groups: snapshot.groups,
            tools: snapshot.tools,
            bindings: snapshot.bindings,
            discovery_policies: snapshot.discovery_policies,
        })
    }

    pub(crate) async fn authorize_view(&self, actor_user_id: Uuid) -> Result<domain::ActorContext> {
        self.repository
            .load_actor_context_for_user(actor_user_id)
            .await
    }

    pub(crate) async fn authorize_manage(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        self.repository
            .load_actor_context_for_user(actor_user_id)
            .await
    }
}

mod query;
pub use query::McpListItemsQuery;
use query::{
    bindable_interface, generate_short_id, normalize_group_display_name, validate_group_path,
    validate_list_return_fields,
};
pub(crate) use query::{normalize_des_id, validate_identifier, validate_path, validate_positive};

pub use query::list_catalog_items;

#[path = "mcp_management/upstream_management.rs"]
mod upstream_management;
