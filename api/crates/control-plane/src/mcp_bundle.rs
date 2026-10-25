use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use semver::Version;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    mcp_management::{
        normalize_des_id, validate_identifier, validate_path, validate_positive,
        McpManagementService,
    },
    ports::{
        CreateMcpToolInput, McpManagementRepository, ReplaceMcpBundleGraphInput,
        SeedMcpBundleGraphInput,
    },
};

pub struct PreviewMcpBundleCommand {
    pub actor_user_id: Uuid,
    pub package: domain::McpBundlePackage,
    pub interface_catalog: Vec<domain::McpInterfaceCatalogEntry>,
    pub current_system_version: String,
}

pub struct ImportMcpBundleCommand {
    pub actor_user_id: Uuid,
    pub package: domain::McpBundlePackage,
    pub interface_catalog: Vec<domain::McpInterfaceCatalogEntry>,
    pub current_system_version: String,
}

pub struct SeedBuiltinMcpBundleCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub package: domain::McpBundlePackage,
    pub interface_catalog: Vec<domain::McpInterfaceCatalogEntry>,
}

pub struct ExportMcpBundleCommand {
    pub actor_user_id: Uuid,
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub current_system_version: String,
}

pub struct ExportMcpInstanceBundleCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub current_system_version: String,
    pub kind: McpInstanceBundleExportKind,
}

pub enum McpInstanceBundleExportKind {
    Portable,
    OfficialBuiltin {
        interface_catalog: Vec<domain::McpInterfaceCatalogEntry>,
    },
}

pub struct McpInstanceBundleExportResult {
    pub package: domain::McpBundlePackage,
    pub official_report: Option<McpOfficialBuiltinExportReport>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct McpOfficialBuiltinExportReport {
    pub excluded_tool_count: usize,
    pub exclusion_reasons: Vec<String>,
}

struct McpBundleExportRequest {
    actor_user_id: Uuid,
    organization: String,
    bundle_id: String,
    bundle_version: String,
    locale: String,
    current_system_version: String,
}

struct McpBundleExportAssembly {
    package: domain::McpBundlePackage,
    official_report: Option<McpOfficialBuiltinExportReport>,
}

enum McpBundleExportScope {
    Workspace,
    Instance {
        instance_id: String,
        kind: McpInstanceBundleExportKind,
    },
}

struct McpBundleWorkspaceSnapshot {
    tools: BTreeMap<String, domain::McpToolRecord>,
    instances: BTreeMap<String, domain::McpInstanceRecord>,
    groups: Vec<domain::McpGroupRecord>,
    bindings: Vec<domain::McpToolBindingRecord>,
    discovery_policies: BTreeMap<Uuid, domain::McpInstanceDiscoveryPolicyRecord>,
    connections: BTreeMap<Uuid, domain::McpUpstreamConnectionRecord>,
}

struct McpBundleToolPlan {
    parameter_schema: serde_json::Value,
    result_schema: serde_json::Value,
    permission_code: Option<String>,
    risk_level: domain::McpRiskLevel,
    status: domain::McpToolStatus,
}

impl<R> McpManagementService<R>
where
    R: McpManagementRepository,
{
    /// Application templates update selected instances without removing entries that
    /// only exist on the target instance.
    pub async fn preserve_unmentioned_instance_entries(
        &self,
        actor_user_id: Uuid,
        mut package: domain::McpBundlePackage,
    ) -> Result<domain::McpBundlePackage> {
        let actor = self.authorize_manage(actor_user_id).await?;
        let snapshot = self
            .load_mcp_bundle_workspace_snapshot(actor.current_workspace_id)
            .await?;
        let mut groups_by_instance: HashMap<Uuid, Vec<&domain::McpGroupRecord>> = HashMap::new();
        for group in &snapshot.groups {
            groups_by_instance
                .entry(group.instance_record_id)
                .or_default()
                .push(group);
        }
        let mut bindings_by_instance: HashMap<Uuid, Vec<&domain::McpToolBindingRecord>> =
            HashMap::new();
        for binding in &snapshot.bindings {
            bindings_by_instance
                .entry(binding.instance_record_id)
                .or_default()
                .push(binding);
        }

        for instance in &mut package.instances {
            let Some(existing) = snapshot.instances.get(&instance.instance_id) else {
                continue;
            };
            if let Some(groups) = groups_by_instance.get(&existing.id) {
                let mut group_paths = instance
                    .groups
                    .iter()
                    .map(|group| group.path.as_str())
                    .collect::<HashSet<_>>();
                let preserved_groups = groups
                    .iter()
                    .filter(|group| group_paths.insert(group.path.as_str()))
                    .map(|group| domain::McpBundleGroup {
                        path: group.path.clone(),
                        display_name: group.display_name.clone(),
                        description_short: group.description_short.clone(),
                        enabled: group.enabled,
                        sort_order: group.sort_order,
                    })
                    .collect::<Vec<_>>();
                instance.groups.extend(preserved_groups);
            }
            if let Some(bindings) = bindings_by_instance.get(&existing.id) {
                let mut binding_keys = instance
                    .bindings
                    .iter()
                    .map(|binding| (binding.group_path.as_str(), binding.tool_id.as_str()))
                    .collect::<HashSet<_>>();
                let preserved_bindings = bindings
                    .iter()
                    .filter(|binding| {
                        binding_keys.insert((binding.group_path.as_str(), binding.tool_id.as_str()))
                    })
                    .map(|binding| domain::McpBundleToolBinding {
                        group_path: binding.group_path.clone(),
                        tool_id: binding.tool_id.clone(),
                        display_alias: binding.display_alias.clone(),
                        visible: binding.visible,
                        sort_order: binding.sort_order,
                    })
                    .collect::<Vec<_>>();
                instance.bindings.extend(preserved_bindings);
            }
        }
        Ok(package)
    }
    pub async fn record_extension_bundle_import(
        &self,
        actor_user_id: Uuid,
        extension_installation_id: Uuid,
        result_status: &str,
    ) -> Result<()> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .record_mcp_extension_bundle_import(
                actor.current_workspace_id,
                extension_installation_id,
                actor_user_id,
                result_status,
            )
            .await
    }

    pub async fn extension_bundle_is_imported(
        &self,
        actor_user_id: Uuid,
        extension_installation_id: Uuid,
    ) -> Result<bool> {
        let actor = self.authorize_manage(actor_user_id).await?;
        self.repository
            .has_mcp_extension_bundle_import(actor.current_workspace_id, extension_installation_id)
            .await
    }

    pub async fn authorize_bundle_management(&self, actor_user_id: Uuid) -> Result<()> {
        self.authorize_manage(actor_user_id).await?;
        Ok(())
    }

    pub async fn seed_builtin_bundle_once(
        &self,
        command: SeedBuiltinMcpBundleCommand,
    ) -> Result<()> {
        validate_package(&command.package)?;
        if !command.package.connections.is_empty() {
            return Err(ControlPlaneError::InvalidInput("builtin_mcp_connections").into());
        }
        let source = domain::McpManagedBundleSource {
            organization: command.package.manifest.organization.clone(),
            bundle_id: command.package.manifest.bundle_id.clone(),
            bundle_version: command.package.manifest.bundle_version.clone(),
        };
        let interfaces = bindable_interfaces(command.interface_catalog);
        let tools = command
            .package
            .tools
            .iter()
            .map(|tool| {
                let plan = bundle_tool_plan(tool, &interfaces);
                CreateMcpToolInput {
                    id: Uuid::now_v7(),
                    actor_user_id: command.actor_user_id,
                    workspace_id: command.workspace_id,
                    tool_id: tool.tool_id.clone(),
                    name: tool.name.clone(),
                    short_description: tool.short_description.clone(),
                    full_description: tool.full_description.clone(),
                    execution_target: tool.execution_target.clone(),
                    parameter_schema: plan.parameter_schema,
                    result_schema: plan.result_schema,
                    input_mapping: tool.input_mapping.clone(),
                    output_mapping: tool.output_mapping.clone(),
                    max_inline_chars: tool.max_inline_chars,
                    response_fields: tool.response_fields.clone(),
                    permission_code: plan.permission_code,
                    risk_level: plan.risk_level,
                    des_id: normalize_des_id(None),
                    des_id_required: domain::mcp_management::mcp_des_id_required(
                        &tool.input_mapping,
                    ),
                    status: plan.status,
                }
            })
            .collect();
        self.repository
            .seed_mcp_bundle_graph_once_atomically(&SeedMcpBundleGraphInput {
                kind: crate::ports::McpBundleSeedKind::BundleVersion,
                actor_user_id: command.actor_user_id,
                workspace_id: command.workspace_id,
                source,
                tools,
                instances: command.package.instances,
            })
            .await
    }

    pub async fn export_bundle(
        &self,
        command: ExportMcpBundleCommand,
    ) -> Result<domain::McpBundlePackage> {
        Ok(self
            .export_bundle_for_scope(
                McpBundleExportRequest {
                    actor_user_id: command.actor_user_id,
                    organization: command.organization,
                    bundle_id: command.bundle_id,
                    bundle_version: command.bundle_version,
                    locale: command.locale,
                    current_system_version: command.current_system_version,
                },
                McpBundleExportScope::Workspace,
            )
            .await?
            .package)
    }

    pub async fn export_instance_bundle(
        &self,
        command: ExportMcpInstanceBundleCommand,
    ) -> Result<McpInstanceBundleExportResult> {
        let assembly = self
            .export_bundle_for_scope(
                McpBundleExportRequest {
                    actor_user_id: command.actor_user_id,
                    organization: command.organization,
                    bundle_id: command.bundle_id,
                    bundle_version: command.bundle_version,
                    locale: command.locale,
                    current_system_version: command.current_system_version,
                },
                McpBundleExportScope::Instance {
                    instance_id: command.instance_id,
                    kind: command.kind,
                },
            )
            .await?;
        Ok(McpInstanceBundleExportResult {
            package: assembly.package,
            official_report: assembly.official_report,
        })
    }

    async fn export_bundle_for_scope(
        &self,
        request: McpBundleExportRequest,
        scope: McpBundleExportScope,
    ) -> Result<McpBundleExportAssembly> {
        validate_identifier(&request.organization, "organization")?;
        validate_identifier(&request.bundle_id, "bundle_id")?;
        Version::parse(&request.bundle_version)
            .map_err(|_| ControlPlaneError::InvalidInput("bundle_version"))?;
        Version::parse(&request.current_system_version)
            .map_err(|_| ControlPlaneError::InvalidInput("current_system_version"))?;
        if !matches!(request.locale.as_str(), "zh_Hans" | "en_US") {
            return Err(ControlPlaneError::InvalidInput("locale").into());
        }
        let actor = self.authorize_manage(request.actor_user_id).await?;
        let instances = match &scope {
            McpBundleExportScope::Workspace => {
                self.repository
                    .list_mcp_instances(actor.current_workspace_id)
                    .await?
            }
            McpBundleExportScope::Instance { instance_id, .. } => vec![self
                .repository
                .get_mcp_instance(actor.current_workspace_id, instance_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("mcp_instance"))?],
        };
        let instance_record_ids = instances
            .iter()
            .map(|instance| instance.id)
            .collect::<Vec<_>>();
        let groups = self
            .repository
            .list_mcp_groups(&instance_record_ids)
            .await?;
        let mut bindings = self
            .repository
            .list_mcp_tool_bindings(&instance_record_ids)
            .await?;
        let policies = self
            .repository
            .list_mcp_instance_discovery_policies(&instance_record_ids)
            .await?;
        let mut tools = self
            .repository
            .list_mcp_tools(actor.current_workspace_id)
            .await?;
        if matches!(&scope, McpBundleExportScope::Instance { .. }) {
            let referenced_tool_ids = bindings
                .iter()
                .map(|binding| binding.tool_record_id)
                .collect::<BTreeSet<_>>();
            tools.retain(|tool| referenced_tool_ids.contains(&tool.id));
        }
        let mut official_report = None;
        if let McpBundleExportScope::Instance {
            kind: McpInstanceBundleExportKind::OfficialBuiltin { interface_catalog },
            ..
        } = &scope
        {
            official_report = Some(retain_official_builtin_tools(
                &mut tools,
                &mut bindings,
                interface_catalog,
            )?);
        }
        let referenced_connection_ids = tools
            .iter()
            .filter_map(|tool| match &tool.execution_target {
                domain::McpToolExecutionTarget::McpProxy {
                    upstream_connection_id,
                    ..
                } => Some(*upstream_connection_id),
                domain::McpToolExecutionTarget::InterfaceWrapper { .. }
                | domain::McpToolExecutionTarget::AssistantClient { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let mut connections = self
            .repository
            .list_mcp_upstream_connections(actor.current_workspace_id)
            .await?;
        if matches!(&scope, McpBundleExportScope::Instance { .. }) {
            connections.retain(|connection| referenced_connection_ids.contains(&connection.id));
        }

        let portable_tools = tools
            .into_iter()
            .map(|tool| domain::McpBundleTool {
                tool_id: tool.tool_id,
                name: tool.name,
                short_description: tool.short_description,
                full_description: tool.full_description,
                execution_target: tool.execution_target,
                parameter_schema_snapshot: tool.parameter_schema,
                result_schema_snapshot: tool.result_schema,
                input_mapping: tool.input_mapping,
                output_mapping: tool.output_mapping,
                max_inline_chars: tool.max_inline_chars,
                response_fields: tool.response_fields,
                permission_code_snapshot: tool.permission_code,
                risk_level_snapshot: tool.risk_level,
                status: tool.status,
            })
            .collect();
        let mut portable_instances = Vec::with_capacity(instances.len());
        for instance in instances {
            let policy = policies
                .iter()
                .find(|policy| policy.instance_record_id == instance.id)
                .ok_or(ControlPlaneError::NotFound("mcp_instance_discovery_policy"))?;
            portable_instances.push(domain::McpBundleInstance {
                instance_id: instance.instance_id,
                name: instance.name,
                description_short: instance.description_short,
                status: instance.status,
                default_entry_path: instance.default_entry_path,
                groups: groups
                    .iter()
                    .filter(|group| group.instance_record_id == instance.id)
                    .map(|group| domain::McpBundleGroup {
                        path: group.path.clone(),
                        display_name: group.display_name.clone(),
                        description_short: group.description_short.clone(),
                        enabled: group.enabled,
                        sort_order: group.sort_order,
                    })
                    .collect(),
                bindings: bindings
                    .iter()
                    .filter(|binding| binding.instance_record_id == instance.id)
                    .map(|binding| domain::McpBundleToolBinding {
                        group_path: binding.group_path.clone(),
                        tool_id: binding.tool_id.clone(),
                        display_alias: binding.display_alias.clone(),
                        visible: binding.visible,
                        sort_order: binding.sort_order,
                    })
                    .collect(),
                discovery_policy: domain::McpBundleInstanceDiscoveryPolicy {
                    list_default_limit: policy.list_default_limit,
                    list_max_depth: policy.list_max_depth,
                    list_regex_enabled: policy.list_regex_enabled,
                    list_regex_max_length: policy.list_regex_max_length,
                    list_return_fields: policy.list_return_fields.clone(),
                },
            });
        }
        let exported_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| ControlPlaneError::InvalidInput("exported_at"))?;
        let package = domain::McpBundlePackage {
            manifest: domain::McpBundleManifest {
                schema_version: domain::MCP_BUNDLE_SCHEMA_VERSION.into(),
                organization: request.organization,
                bundle_id: request.bundle_id,
                bundle_version: request.bundle_version,
                locale: request.locale,
                minimum_host_version: request.current_system_version.clone(),
                exported_from_system_version: request.current_system_version,
                exported_at,
                files: Vec::new(),
            },
            tools: portable_tools,
            instances: portable_instances,
            connections: connections
                .into_iter()
                .map(|connection| domain::McpBundleUpstreamConnection {
                    connection_id: connection.id,
                    name: connection.name,
                    endpoint: connection.endpoint,
                    transport: connection.transport,
                    auth_type: connection.auth_type,
                    custom_header_name: connection.custom_header_name,
                    status: connection.status,
                })
                .collect(),
        };
        Ok(McpBundleExportAssembly {
            package,
            official_report,
        })
    }

    async fn load_mcp_bundle_workspace_snapshot(
        &self,
        workspace_id: Uuid,
    ) -> Result<McpBundleWorkspaceSnapshot> {
        let tools = self
            .repository
            .list_mcp_tools(workspace_id)
            .await?
            .into_iter()
            .map(|tool| (tool.tool_id.clone(), tool))
            .collect();
        let instance_records = self.repository.list_mcp_instances(workspace_id).await?;
        let instance_record_ids = instance_records
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
        let discovery_policies = self
            .repository
            .list_mcp_instance_discovery_policies(&instance_record_ids)
            .await?
            .into_iter()
            .map(|policy| (policy.instance_record_id, policy))
            .collect();
        let connections = self
            .repository
            .list_mcp_upstream_connections(workspace_id)
            .await?
            .into_iter()
            .map(|connection| (connection.id, connection))
            .collect();

        Ok(McpBundleWorkspaceSnapshot {
            tools,
            instances: instance_records
                .into_iter()
                .map(|instance| (instance.instance_id.clone(), instance))
                .collect(),
            groups,
            bindings,
            discovery_policies,
            connections,
        })
    }

    pub async fn preview_bundle(
        &self,
        command: PreviewMcpBundleCommand,
    ) -> Result<domain::McpBundlePreview> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_package(&command.package)?;
        let snapshot = self
            .load_mcp_bundle_workspace_snapshot(actor.current_workspace_id)
            .await?;
        let interfaces = bindable_interfaces(command.interface_catalog);

        let tools: Vec<domain::McpBundleItemReport> = command
            .package
            .tools
            .iter()
            .map(|tool| {
                if let Some(existing) = snapshot.tools.get(&tool.tool_id) {
                    if bundle_tool_is_already_present(tool, existing, &interfaces) {
                        item_report(
                            &tool.tool_id,
                            domain::McpBundleItemEffect::AlreadyPresent,
                            "already_present",
                            None,
                        )
                    } else {
                        update_item_report(&tool.tool_id)
                    }
                } else {
                    match &tool.execution_target {
                        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id }
                            if interfaces.contains_key(interface_id) =>
                        {
                            item_report(
                                &tool.tool_id,
                                domain::McpBundleItemEffect::Create,
                                "imported",
                                None,
                            )
                        }
                        domain::McpToolExecutionTarget::McpProxy {
                            upstream_connection_id,
                            ..
                        } if snapshot.connections.contains_key(upstream_connection_id)
                            || command.package.connections.iter().any(|connection| {
                                connection.connection_id == *upstream_connection_id
                            }) =>
                        {
                            item_report(
                                &tool.tool_id,
                                domain::McpBundleItemEffect::Create,
                                "unavailable",
                                Some("credentials_missing"),
                            )
                        }
                        domain::McpToolExecutionTarget::McpProxy { .. } => item_report(
                            &tool.tool_id,
                            domain::McpBundleItemEffect::Create,
                            "unavailable",
                            Some("connection_missing"),
                        ),
                        domain::McpToolExecutionTarget::AssistantClient { .. } => item_report(
                            &tool.tool_id,
                            domain::McpBundleItemEffect::Create,
                            "imported",
                            None,
                        ),
                        _ => item_report(
                            &tool.tool_id,
                            domain::McpBundleItemEffect::Create,
                            "unavailable",
                            Some("interface_missing"),
                        ),
                    }
                }
            })
            .collect();
        let instances: Vec<domain::McpBundleItemReport> = command
            .package
            .instances
            .iter()
            .map(|instance| {
                if let Some(existing) = snapshot.instances.get(&instance.instance_id) {
                    if bundle_instance_is_already_present(instance, existing, &snapshot) {
                        item_report(
                            &instance.instance_id,
                            domain::McpBundleItemEffect::AlreadyPresent,
                            "already_present",
                            None,
                        )
                    } else {
                        update_item_report(&instance.instance_id)
                    }
                } else if instance.bindings.iter().any(|binding| {
                    !snapshot.tools.contains_key(&binding.tool_id)
                        && !command
                            .package
                            .tools
                            .iter()
                            .any(|tool| tool.tool_id == binding.tool_id)
                }) {
                    item_report(
                        &instance.instance_id,
                        domain::McpBundleItemEffect::Failed,
                        "failed",
                        Some("binding_tool_missing"),
                    )
                } else {
                    item_report(
                        &instance.instance_id,
                        domain::McpBundleItemEffect::Create,
                        "imported",
                        None,
                    )
                }
            })
            .collect();
        let connections: Vec<domain::McpBundleItemReport> = command
            .package
            .connections
            .iter()
            .map(|connection| {
                if let Some(existing) = snapshot.connections.get(&connection.connection_id) {
                    if bundle_connection_is_already_present(connection, existing) {
                        item_report(
                            &connection.connection_id.to_string(),
                            domain::McpBundleItemEffect::AlreadyPresent,
                            "already_present",
                            None,
                        )
                    } else {
                        update_item_report(&connection.connection_id.to_string())
                    }
                } else {
                    item_report(
                        &connection.connection_id.to_string(),
                        domain::McpBundleItemEffect::Create,
                        "unavailable",
                        Some("credentials_missing"),
                    )
                }
            })
            .collect();
        let effect_summary = summarize_bundle_effects(&tools, &instances, &connections);
        let shared_tool_impacts = shared_tool_impacts(&command.package, &snapshot, &interfaces);

        Ok(domain::McpBundlePreview {
            version_status: compare_system_versions(
                &command.package.manifest.exported_from_system_version,
                &command.current_system_version,
            ),
            manifest: command.package.manifest,
            current_system_version: command.current_system_version,
            effect_summary,
            tools,
            instances,
            connections,
            shared_tool_impacts,
        })
    }

    pub async fn import_bundle(
        &self,
        command: ImportMcpBundleCommand,
    ) -> Result<domain::McpBundleImportReport> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_package(&command.package)?;
        let interfaces = bindable_interfaces(command.interface_catalog);
        let snapshot = self
            .load_mcp_bundle_workspace_snapshot(actor.current_workspace_id)
            .await?;
        let tools = command
            .package
            .tools
            .iter()
            .map(|tool| {
                let plan = bundle_tool_plan(tool, &interfaces);
                CreateMcpToolInput {
                    id: Uuid::now_v7(),
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    tool_id: tool.tool_id.clone(),
                    name: tool.name.clone(),
                    short_description: tool.short_description.clone(),
                    full_description: tool.full_description.clone(),
                    execution_target: tool.execution_target.clone(),
                    parameter_schema: plan.parameter_schema,
                    result_schema: plan.result_schema,
                    input_mapping: tool.input_mapping.clone(),
                    output_mapping: tool.output_mapping.clone(),
                    max_inline_chars: tool.max_inline_chars,
                    response_fields: tool.response_fields.clone(),
                    permission_code: plan.permission_code,
                    risk_level: plan.risk_level,
                    des_id: normalize_des_id(None),
                    des_id_required: domain::mcp_management::mcp_des_id_required(
                        &tool.input_mapping,
                    ),
                    status: plan.status,
                }
            })
            .collect::<Vec<_>>();

        let connection_reports = preview_connection_reports(&command.package, &snapshot);
        let tool_reports = preview_tool_reports(&command.package, &snapshot, &interfaces);
        let instance_reports = preview_instance_reports(&command.package, &snapshot);
        let effect_summary =
            summarize_bundle_effects(&tool_reports, &instance_reports, &connection_reports);

        if effect_summary.changes > 0 || effect_summary.failed > 0 {
            self.repository
                .replace_mcp_bundle_graph_atomically(&ReplaceMcpBundleGraphInput {
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    source: domain::McpManagedBundleSource {
                        organization: command.package.manifest.organization.clone(),
                        bundle_id: command.package.manifest.bundle_id.clone(),
                        bundle_version: command.package.manifest.bundle_version.clone(),
                    },
                    connections: command.package.connections.clone(),
                    tools,
                    instances: command.package.instances.clone(),
                })
                .await?;
        }

        let status = if effect_summary.changes == 0 {
            "already_applied"
        } else if effect_summary.unavailable > 0 {
            "completed_with_warnings"
        } else {
            "completed"
        };
        Ok(domain::McpBundleImportReport {
            version_status: compare_system_versions(
                &command.package.manifest.exported_from_system_version,
                &command.current_system_version,
            ),
            manifest: command.package.manifest,
            current_system_version: command.current_system_version,
            status: status.into(),
            effect_summary,
            tools: tool_reports,
            instances: instance_reports,
            connections: connection_reports,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OfficialBuiltinInterfaceDisposition {
    Include,
    Exclude(&'static str),
}

pub(crate) fn official_builtin_interface_disposition(
    interface_id: &str,
    sources: &BTreeMap<String, domain::McpInterfaceCatalogSource>,
) -> Result<OfficialBuiltinInterfaceDisposition> {
    let source = sources
        .get(interface_id)
        .ok_or(ControlPlaneError::Conflict(
            "mcp_official_export_interface_source",
        ))?;
    Ok(match source {
        domain::McpInterfaceCatalogSource::StaticApi
        | domain::McpInterfaceCatalogSource::BuiltinDataModelCrud => {
            OfficialBuiltinInterfaceDisposition::Include
        }
        domain::McpInterfaceCatalogSource::PublishedWorkflow => {
            OfficialBuiltinInterfaceDisposition::Exclude("published_workflow")
        }
        domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud => {
            OfficialBuiltinInterfaceDisposition::Exclude("workspace_data_model_crud")
        }
    })
}

pub(crate) fn retain_official_builtin_tools(
    tools: &mut Vec<domain::McpToolRecord>,
    bindings: &mut Vec<domain::McpToolBindingRecord>,
    interface_catalog: &[domain::McpInterfaceCatalogEntry],
) -> Result<McpOfficialBuiltinExportReport> {
    let sources = interface_catalog
        .iter()
        .map(|entry| (entry.interface_id.clone(), entry.source))
        .collect::<BTreeMap<_, _>>();
    let mut retained_tool_record_ids = BTreeSet::new();
    let mut excluded_tool_count = 0;
    let mut exclusion_reasons = BTreeSet::new();
    for tool in tools.iter() {
        let exportable = match &tool.execution_target {
            domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                match official_builtin_interface_disposition(interface_id, &sources)? {
                    OfficialBuiltinInterfaceDisposition::Include => true,
                    OfficialBuiltinInterfaceDisposition::Exclude(reason) => {
                        exclusion_reasons.insert(reason.to_string());
                        false
                    }
                }
            }
            domain::McpToolExecutionTarget::McpProxy { .. } => true,
            domain::McpToolExecutionTarget::AssistantClient { .. } => true,
        };
        if exportable {
            retained_tool_record_ids.insert(tool.id);
        } else {
            excluded_tool_count += 1;
        }
    }
    tools.retain(|tool| retained_tool_record_ids.contains(&tool.id));
    bindings.retain(|binding| retained_tool_record_ids.contains(&binding.tool_record_id));
    Ok(McpOfficialBuiltinExportReport {
        excluded_tool_count,
        exclusion_reasons: exclusion_reasons.into_iter().collect(),
    })
}

pub fn compare_system_versions(source: &str, current: &str) -> domain::McpBundleVersionStatus {
    let parsed = |value: &str| Version::parse(value.trim().trim_start_matches('v')).ok();
    match (parsed(source), parsed(current)) {
        (Some(source), Some(current)) if source < current => {
            domain::McpBundleVersionStatus::ExportedFromOlderSystem
        }
        (Some(source), Some(current)) if source > current => {
            domain::McpBundleVersionStatus::ExportedFromNewerSystem
        }
        (Some(_), Some(_)) => domain::McpBundleVersionStatus::SameSystemVersion,
        _ => domain::McpBundleVersionStatus::UnknownSystemVersion,
    }
}

#[path = "mcp_bundle/bundle_preview.rs"]
mod bundle_preview;

use bundle_preview::*;
