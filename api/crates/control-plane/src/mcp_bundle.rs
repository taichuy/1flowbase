use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use semver::Version;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    mcp_management::{
        input_mapping_requires_des_id, normalize_des_id, validate_identifier, validate_path,
        validate_positive, McpManagementService,
    },
    ports::{
        CreateMcpInstanceGraphInput, CreateMcpInstanceInput, CreateMcpToolBindingInput,
        CreateMcpToolInput, CreateMcpUpstreamConnectionInput, McpManagementRepository,
        UpdateMcpInstanceDiscoveryPolicyInput, UpsertMcpGroupInput,
        UpsertMcpUpstreamToolSourceInput,
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

pub struct ExportMcpBundleCommand {
    pub actor_user_id: Uuid,
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub current_system_version: String,
}

pub struct ExportMcpInstanceBundleCommand {
    pub actor_user_id: Uuid,
    pub instance_id: String,
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub current_system_version: String,
}

struct McpBundleExportRequest {
    actor_user_id: Uuid,
    organization: String,
    bundle_id: String,
    bundle_version: String,
    locale: String,
    minimum_host_version: String,
    current_system_version: String,
}

enum McpBundleExportScope {
    Workspace,
    Instance(String),
}

impl<R> McpManagementService<R>
where
    R: McpManagementRepository,
{
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

    pub async fn export_bundle(
        &self,
        command: ExportMcpBundleCommand,
    ) -> Result<domain::McpBundlePackage> {
        self.export_bundle_for_scope(
            McpBundleExportRequest {
                actor_user_id: command.actor_user_id,
                organization: command.organization,
                bundle_id: command.bundle_id,
                bundle_version: command.bundle_version,
                locale: command.locale,
                minimum_host_version: command.minimum_host_version,
                current_system_version: command.current_system_version,
            },
            McpBundleExportScope::Workspace,
        )
        .await
    }

    pub async fn export_instance_bundle(
        &self,
        command: ExportMcpInstanceBundleCommand,
    ) -> Result<domain::McpBundlePackage> {
        self.export_bundle_for_scope(
            McpBundleExportRequest {
                actor_user_id: command.actor_user_id,
                organization: command.organization,
                bundle_id: command.bundle_id,
                bundle_version: command.bundle_version,
                locale: command.locale,
                minimum_host_version: command.minimum_host_version,
                current_system_version: command.current_system_version,
            },
            McpBundleExportScope::Instance(command.instance_id),
        )
        .await
    }

    async fn export_bundle_for_scope(
        &self,
        request: McpBundleExportRequest,
        scope: McpBundleExportScope,
    ) -> Result<domain::McpBundlePackage> {
        validate_identifier(&request.organization, "organization")?;
        validate_identifier(&request.bundle_id, "bundle_id")?;
        Version::parse(&request.bundle_version)
            .map_err(|_| ControlPlaneError::InvalidInput("bundle_version"))?;
        Version::parse(&request.minimum_host_version)
            .map_err(|_| ControlPlaneError::InvalidInput("minimum_host_version"))?;
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
            McpBundleExportScope::Instance(instance_id) => vec![self
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
        let bindings = self
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
        if matches!(&scope, McpBundleExportScope::Instance(_)) {
            let referenced_tool_ids = bindings
                .iter()
                .map(|binding| binding.tool_record_id)
                .collect::<BTreeSet<_>>();
            tools.retain(|tool| referenced_tool_ids.contains(&tool.id));
        }
        let referenced_connection_ids = tools
            .iter()
            .filter_map(|tool| match &tool.execution_target {
                domain::McpToolExecutionTarget::McpProxy {
                    upstream_connection_id,
                    ..
                } => Some(*upstream_connection_id),
                domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let mut connections = self
            .repository
            .list_mcp_upstream_connections(actor.current_workspace_id)
            .await?;
        if matches!(&scope, McpBundleExportScope::Instance(_)) {
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
        Ok(domain::McpBundlePackage {
            manifest: domain::McpBundleManifest {
                schema_version: domain::MCP_BUNDLE_SCHEMA_VERSION.into(),
                organization: request.organization,
                bundle_id: request.bundle_id,
                bundle_version: request.bundle_version,
                locale: request.locale,
                minimum_host_version: request.minimum_host_version,
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
        })
    }

    pub async fn preview_bundle(
        &self,
        command: PreviewMcpBundleCommand,
    ) -> Result<domain::McpBundlePreview> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_package(&command.package)?;
        let snapshot = self
            .repository
            .list_mcp_tools(actor.current_workspace_id)
            .await?;
        let existing_tool_ids = snapshot
            .into_iter()
            .map(|tool| tool.tool_id)
            .collect::<BTreeSet<_>>();
        let existing_instance_ids = self
            .repository
            .list_mcp_instances(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect::<BTreeSet<_>>();
        let interfaces = bindable_interfaces(command.interface_catalog);
        let existing_connection_ids = self
            .repository
            .list_mcp_upstream_connections(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|connection| connection.id)
            .collect::<BTreeSet<_>>();

        let tools = command
            .package
            .tools
            .iter()
            .map(|tool| {
                if existing_tool_ids.contains(&tool.tool_id) {
                    item_report(&tool.tool_id, "skipped", Some("tool_id_conflict"))
                } else {
                    match &tool.execution_target {
                        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id }
                            if interfaces.contains_key(interface_id) =>
                        {
                            item_report(&tool.tool_id, "imported", None)
                        }
                        domain::McpToolExecutionTarget::McpProxy {
                            upstream_connection_id,
                            ..
                        } if existing_connection_ids.contains(upstream_connection_id)
                            || command.package.connections.iter().any(|connection| {
                                connection.connection_id == *upstream_connection_id
                            }) =>
                        {
                            item_report(&tool.tool_id, "unavailable", Some("credentials_missing"))
                        }
                        domain::McpToolExecutionTarget::McpProxy { .. } => {
                            item_report(&tool.tool_id, "unavailable", Some("connection_missing"))
                        }
                        _ => item_report(&tool.tool_id, "unavailable", Some("interface_missing")),
                    }
                }
            })
            .collect();
        let instances = command
            .package
            .instances
            .iter()
            .map(|instance| {
                if existing_instance_ids.contains(&instance.instance_id) {
                    item_report(
                        &instance.instance_id,
                        "skipped",
                        Some("instance_id_conflict"),
                    )
                } else {
                    item_report(&instance.instance_id, "imported", None)
                }
            })
            .collect();
        let connections = command
            .package
            .connections
            .iter()
            .map(|connection| {
                if existing_connection_ids.contains(&connection.connection_id) {
                    item_report(
                        &connection.connection_id.to_string(),
                        "skipped",
                        Some("connection_id_conflict"),
                    )
                } else {
                    item_report(
                        &connection.connection_id.to_string(),
                        "unavailable",
                        Some("credentials_missing"),
                    )
                }
            })
            .collect();

        Ok(domain::McpBundlePreview {
            version_status: compare_system_versions(
                &command.package.manifest.exported_from_system_version,
                &command.current_system_version,
            ),
            manifest: command.package.manifest,
            current_system_version: command.current_system_version,
            tools,
            instances,
            connections,
        })
    }

    pub async fn import_bundle(
        &self,
        command: ImportMcpBundleCommand,
    ) -> Result<domain::McpBundleImportReport> {
        let actor = self.authorize_manage(command.actor_user_id).await?;
        validate_package(&command.package)?;
        let interfaces = bindable_interfaces(command.interface_catalog);
        let mut tool_records = self
            .repository
            .list_mcp_tools(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|tool| (tool.tool_id.clone(), tool))
            .collect::<BTreeMap<_, _>>();
        let existing_instance_ids = self
            .repository
            .list_mcp_instances(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|instance| instance.instance_id)
            .collect::<BTreeSet<_>>();
        let mut existing_connection_ids = self
            .repository
            .list_mcp_upstream_connections(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|connection| connection.id)
            .collect::<BTreeSet<_>>();
        let mut connection_reports = Vec::with_capacity(command.package.connections.len());
        for connection in &command.package.connections {
            if existing_connection_ids.contains(&connection.connection_id) {
                connection_reports.push(item_report(
                    &connection.connection_id.to_string(),
                    "skipped",
                    Some("connection_id_conflict"),
                ));
                continue;
            }
            let result = self
                .repository
                .create_mcp_upstream_connection(&CreateMcpUpstreamConnectionInput {
                    id: connection.connection_id,
                    actor_user_id: command.actor_user_id,
                    workspace_id: actor.current_workspace_id,
                    name: connection.name.clone(),
                    endpoint: connection.endpoint.clone(),
                    transport: connection.transport,
                    auth_type: connection.auth_type,
                    custom_header_name: connection.custom_header_name.clone(),
                    status: domain::McpUpstreamConnectionStatus::Disabled,
                })
                .await;
            match result {
                Ok(_) => {
                    existing_connection_ids.insert(connection.connection_id);
                    connection_reports.push(item_report(
                        &connection.connection_id.to_string(),
                        "unavailable",
                        Some("credentials_missing"),
                    ));
                }
                Err(_) => connection_reports.push(item_report(
                    &connection.connection_id.to_string(),
                    "failed",
                    Some("connection_write_failed"),
                )),
            }
        }

        let mut tool_reports = Vec::with_capacity(command.package.tools.len());
        for tool in &command.package.tools {
            if tool_records.contains_key(&tool.tool_id) {
                tool_reports.push(item_report(
                    &tool.tool_id,
                    "skipped",
                    Some("tool_id_conflict"),
                ));
                continue;
            }

            let interface = match &tool.execution_target {
                domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                    interfaces.get(interface_id)
                }
                domain::McpToolExecutionTarget::McpProxy { .. } => None,
            };
            let unavailable = interface.is_none();
            let (parameter_schema, result_schema, permission_code, risk_level, status) =
                if let Some(interface) = interface {
                    (
                        interface.parameter_schema.clone(),
                        interface.result_schema.clone(),
                        interface.permission_code.clone(),
                        interface.risk_level,
                        tool.status,
                    )
                } else {
                    (
                        tool.parameter_schema_snapshot.clone(),
                        tool.result_schema_snapshot.clone(),
                        tool.permission_code_snapshot.clone(),
                        tool.risk_level_snapshot,
                        domain::McpToolStatus::Disabled,
                    )
                };
            let input = CreateMcpToolInput {
                id: Uuid::now_v7(),
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                tool_id: tool.tool_id.clone(),
                name: tool.name.clone(),
                short_description: tool.short_description.clone(),
                full_description: tool.full_description.clone(),
                execution_target: tool.execution_target.clone(),
                parameter_schema,
                result_schema,
                input_mapping: tool.input_mapping.clone(),
                output_mapping: tool.output_mapping.clone(),
                permission_code,
                risk_level,
                des_id: normalize_des_id(None),
                des_id_required: input_mapping_requires_des_id(&tool.input_mapping),
                status,
            };
            match self.repository.create_mcp_tool(&input).await {
                Ok(record) => {
                    if let domain::McpToolExecutionTarget::McpProxy {
                        upstream_connection_id,
                        remote_tool_name,
                        source_schema_hash,
                    } = &tool.execution_target
                    {
                        let discovered_at = OffsetDateTime::now_utc();
                        self.repository
                            .upsert_mcp_upstream_tool_source(&UpsertMcpUpstreamToolSourceInput {
                                id: Uuid::now_v7(),
                                workspace_id: actor.current_workspace_id,
                                upstream_connection_id: *upstream_connection_id,
                                remote_tool_name: remote_tool_name.clone(),
                                description: Some(tool.full_description.clone()),
                                input_schema: tool.parameter_schema_snapshot.clone(),
                                output_schema: tool.result_schema_snapshot.clone(),
                                schema_hash: source_schema_hash.clone(),
                                source_status: domain::McpUpstreamSourceStatus::NotImported,
                                discovered_at,
                            })
                            .await?;
                        self.repository
                            .link_mcp_upstream_tool_source(
                                actor.current_workspace_id,
                                *upstream_connection_id,
                                remote_tool_name,
                                record.id,
                            )
                            .await?;
                    }
                    tool_records.insert(tool.tool_id.clone(), record);
                    tool_reports.push(if unavailable {
                        let reason = if matches!(
                            tool.execution_target,
                            domain::McpToolExecutionTarget::McpProxy { .. }
                        ) {
                            "credentials_missing"
                        } else {
                            "interface_missing"
                        };
                        item_report(&tool.tool_id, "unavailable", Some(reason))
                    } else {
                        item_report(&tool.tool_id, "imported", None)
                    });
                }
                Err(_) => tool_reports.push(item_report(
                    &tool.tool_id,
                    "failed",
                    Some("tool_write_failed"),
                )),
            }
        }

        let mut instance_reports = Vec::with_capacity(command.package.instances.len());
        for instance in &command.package.instances {
            if existing_instance_ids.contains(&instance.instance_id) {
                instance_reports.push(item_report(
                    &instance.instance_id,
                    "skipped",
                    Some("instance_id_conflict"),
                ));
                continue;
            }
            let missing_binding_tool = instance
                .bindings
                .iter()
                .any(|binding| !tool_records.contains_key(&binding.tool_id));
            if missing_binding_tool {
                instance_reports.push(item_report(
                    &instance.instance_id,
                    "failed",
                    Some("binding_tool_missing"),
                ));
                continue;
            }

            match self
                .import_bundle_instance(
                    command.actor_user_id,
                    actor.current_workspace_id,
                    instance,
                    &tool_records,
                )
                .await
            {
                Ok(()) => {
                    instance_reports.push(item_report(&instance.instance_id, "imported", None))
                }
                Err(_) => instance_reports.push(item_report(
                    &instance.instance_id,
                    "failed",
                    Some("instance_write_failed"),
                )),
            }
        }

        let has_warnings = tool_reports
            .iter()
            .chain(instance_reports.iter())
            .chain(connection_reports.iter())
            .any(|item| item.result != "imported");
        Ok(domain::McpBundleImportReport {
            version_status: compare_system_versions(
                &command.package.manifest.exported_from_system_version,
                &command.current_system_version,
            ),
            manifest: command.package.manifest,
            current_system_version: command.current_system_version,
            status: if has_warnings {
                "completed_with_warnings".into()
            } else {
                "completed".into()
            },
            tools: tool_reports,
            instances: instance_reports,
            connections: connection_reports,
        })
    }

    async fn import_bundle_instance(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        bundle: &domain::McpBundleInstance,
        tools: &BTreeMap<String, domain::McpToolRecord>,
    ) -> Result<()> {
        let instance_record_id = Uuid::now_v7();
        let groups = bundle
            .groups
            .iter()
            .map(|group| UpsertMcpGroupInput {
                id: Uuid::now_v7(),
                actor_user_id,
                instance_record_id,
                path: group.path.clone(),
                display_name: group.display_name.clone(),
                description_short: group.description_short.clone(),
                enabled: group.enabled,
                sort_order: group.sort_order,
            })
            .collect();
        let bindings = bundle
            .bindings
            .iter()
            .map(|binding| {
                let tool = tools
                    .get(&binding.tool_id)
                    .ok_or(ControlPlaneError::NotFound("mcp_tool"))?;
                Ok(CreateMcpToolBindingInput {
                    id: Uuid::now_v7(),
                    actor_user_id,
                    instance_record_id,
                    tool_record_id: tool.id,
                    group_path: binding.group_path.clone(),
                    display_alias: binding.display_alias.clone(),
                    visible: binding.visible,
                    sort_order: binding.sort_order,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let policy = &bundle.discovery_policy;
        self.repository
            .create_mcp_instance_graph_atomically(&CreateMcpInstanceGraphInput {
                instance: CreateMcpInstanceInput {
                    id: instance_record_id,
                    actor_user_id,
                    workspace_id,
                    instance_id: bundle.instance_id.clone(),
                    name: bundle.name.clone(),
                    description_short: bundle.description_short.clone(),
                    status: bundle.status,
                    default_entry_path: bundle.default_entry_path.clone(),
                },
                groups,
                bindings,
                discovery_policy: UpdateMcpInstanceDiscoveryPolicyInput {
                    actor_user_id,
                    workspace_id,
                    instance_record_id,
                    list_default_limit: policy.list_default_limit,
                    list_max_depth: policy.list_max_depth,
                    list_regex_enabled: policy.list_regex_enabled,
                    list_regex_max_length: policy.list_regex_max_length,
                    list_return_fields: policy.list_return_fields.clone(),
                },
            })
            .await?;
        Ok(())
    }
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

fn bindable_interfaces(
    entries: Vec<domain::McpInterfaceCatalogEntry>,
) -> BTreeMap<String, domain::McpInterfaceCatalogEntry> {
    entries
        .into_iter()
        .filter(|entry| entry.bindable)
        .map(|entry| (entry.interface_id.clone(), entry))
        .collect()
}

fn validate_package(package: &domain::McpBundlePackage) -> Result<()> {
    if package.manifest.schema_version != domain::MCP_BUNDLE_SCHEMA_VERSION
        && package.manifest.schema_version != "1flowbase.mcp.bundle/v1"
    {
        return Err(ControlPlaneError::InvalidInput("schema_version").into());
    }
    validate_identifier(&package.manifest.organization, "organization")?;
    validate_identifier(&package.manifest.bundle_id, "bundle_id")?;
    Version::parse(&package.manifest.bundle_version)
        .map_err(|_| ControlPlaneError::InvalidInput("bundle_version"))?;
    let mut tool_ids = BTreeSet::new();
    for tool in &package.tools {
        validate_identifier(&tool.tool_id, "tool_id")?;
        match &tool.execution_target {
            domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                validate_identifier(interface_id, "interface_id")?;
            }
            domain::McpToolExecutionTarget::McpProxy {
                remote_tool_name,
                source_schema_hash,
                ..
            } => {
                validate_identifier(remote_tool_name, "remote_tool_name")?;
                validate_identifier(source_schema_hash, "source_schema_hash")?;
            }
        }
        if !tool_ids.insert(tool.tool_id.as_str()) {
            return Err(ControlPlaneError::InvalidInput("duplicate_tool_id").into());
        }
    }
    let mut instance_ids = BTreeSet::new();
    for instance in &package.instances {
        validate_identifier(&instance.instance_id, "instance_id")?;
        validate_path(&instance.default_entry_path)?;
        validate_positive(
            instance.discovery_policy.list_default_limit,
            "list_default_limit",
        )?;
        validate_positive(instance.discovery_policy.list_max_depth, "list_max_depth")?;
        validate_positive(
            instance.discovery_policy.list_regex_max_length,
            "list_regex_max_length",
        )?;
        if !instance_ids.insert(instance.instance_id.as_str()) {
            return Err(ControlPlaneError::InvalidInput("duplicate_instance_id").into());
        }
        for group in &instance.groups {
            validate_path(&group.path)?;
        }
        for binding in &instance.bindings {
            validate_path(&binding.group_path)?;
            validate_identifier(&binding.tool_id, "tool_id")?;
        }
    }
    let mut connection_ids = BTreeSet::new();
    for connection in &package.connections {
        if !connection_ids.insert(connection.connection_id) {
            return Err(ControlPlaneError::InvalidInput("duplicate_connection_id").into());
        }
        validate_identifier(&connection.name, "connection_name")?;
        if !connection.endpoint.starts_with("https://") {
            return Err(ControlPlaneError::InvalidInput("endpoint").into());
        }
    }
    Ok(())
}

fn item_report(id: &str, result: &str, reason: Option<&str>) -> domain::McpBundleItemReport {
    domain::McpBundleItemReport {
        id: id.to_string(),
        result: result.to_string(),
        reason: reason.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::compare_system_versions;

    #[test]
    fn compares_export_source_and_current_system_versions() {
        assert_eq!(
            compare_system_versions("0.2.5", "0.2.6"),
            domain::McpBundleVersionStatus::ExportedFromOlderSystem
        );
        assert_eq!(
            compare_system_versions("0.3.0", "0.2.6"),
            domain::McpBundleVersionStatus::ExportedFromNewerSystem
        );
        assert_eq!(
            compare_system_versions("latest", "0.2.6"),
            domain::McpBundleVersionStatus::UnknownSystemVersion
        );
    }
}
