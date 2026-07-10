use std::collections::BTreeSet;

use anyhow::Result;
use rand_core::{OsRng, RngCore};
use regex::Regex;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        CreateMcpInstanceInput, CreateMcpToolBindingInput, CreateMcpToolInput,
        McpManagementRepository, UpdateMcpInstanceDiscoveryPolicyInput, UpdateMcpInstanceInput,
        UpdateMcpToolBindingInput, UpdateMcpToolInput, UpsertMcpGroupInput,
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

pub struct McpManagementService<R> {
    repository: R,
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
                interface_id: interface.interface_id,
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
                interface_id: interface.interface_id,
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
        let mut items = Vec::new();

        for group in groups.into_iter().filter(|group| {
            group.enabled
                && path_matches_list_query(
                    base_path,
                    &group.path,
                    discovery_policy.list_max_depth,
                    path_regex_filter.as_ref(),
                )
        }) {
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

        for binding in bindings.into_iter().filter(|binding| {
            binding.visible
                && path_matches_list_query(
                    base_path,
                    &binding.group_path,
                    discovery_policy.list_max_depth,
                    path_regex_filter.as_ref(),
                )
        }) {
            if let Some(tool) = tools
                .iter()
                .find(|tool| tool.id == binding.tool_record_id)
                .filter(|tool| tool.status == domain::McpToolStatus::Enabled)
            {
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

    async fn authorize_view(&self, actor_user_id: Uuid) -> Result<domain::ActorContext> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if actor.is_root
            || actor.has_permission("mcp_management.view.all")
            || actor.has_permission("mcp_management.manage.all")
        {
            return Ok(actor);
        }
        Err(ControlPlaneError::PermissionDenied("permission_denied").into())
    }

    async fn authorize_manage(&self, actor_user_id: Uuid) -> Result<domain::ActorContext> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        if actor.is_root || actor.has_permission("mcp_management.manage.all") {
            return Ok(actor);
        }
        Err(ControlPlaneError::PermissionDenied("permission_denied").into())
    }
}

fn validate_identifier(value: &str, field: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

fn validate_path(value: &str) -> Result<()> {
    if !value.starts_with('/') || value.len() > 255 {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(())
}

fn validate_positive(value: i32, field: &'static str) -> Result<()> {
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

fn normalize_des_id(value: Option<String>) -> String {
    let trimmed = value.unwrap_or_default().trim().to_owned();
    if trimmed.is_empty() {
        generate_short_id()
    } else {
        trimmed
    }
}

fn input_mapping_requires_des_id(input_mapping: &serde_json::Value) -> bool {
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
