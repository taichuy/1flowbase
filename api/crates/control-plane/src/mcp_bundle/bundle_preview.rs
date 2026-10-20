use super::*;

pub(super) fn bindable_interfaces(
    entries: Vec<domain::McpInterfaceCatalogEntry>,
) -> BTreeMap<String, domain::McpInterfaceCatalogEntry> {
    entries
        .into_iter()
        .filter(|entry| entry.bindable)
        .map(|entry| (entry.interface_id.clone(), entry))
        .collect()
}

pub(super) fn bundle_tool_plan(
    tool: &domain::McpBundleTool,
    interfaces: &BTreeMap<String, domain::McpInterfaceCatalogEntry>,
) -> McpBundleToolPlan {
    let interface = match &tool.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            interfaces.get(interface_id)
        }
        domain::McpToolExecutionTarget::McpProxy { .. }
        | domain::McpToolExecutionTarget::AssistantClient { .. } => None,
    };
    if let Some(interface) = interface {
        McpBundleToolPlan {
            parameter_schema: interface.parameter_schema.clone(),
            result_schema: interface.result_schema.clone(),
            permission_code: interface.permission_code.clone(),
            risk_level: interface.risk_level,
            status: tool.status,
        }
    } else {
        McpBundleToolPlan {
            parameter_schema: tool.parameter_schema_snapshot.clone(),
            result_schema: tool.result_schema_snapshot.clone(),
            permission_code: tool.permission_code_snapshot.clone(),
            risk_level: tool.risk_level_snapshot,
            status: if matches!(
                tool.execution_target,
                domain::McpToolExecutionTarget::AssistantClient { .. }
            ) {
                tool.status
            } else {
                domain::McpToolStatus::Disabled
            },
        }
    }
}

pub(super) fn bundle_tool_unavailable_reason(
    tool: &domain::McpBundleTool,
    interfaces: &BTreeMap<String, domain::McpInterfaceCatalogEntry>,
    connections: &BTreeMap<Uuid, domain::McpUpstreamConnectionRecord>,
) -> Option<&'static str> {
    match &tool.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            (!interfaces.contains_key(interface_id)).then_some("interface_missing")
        }
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            ..
        } if connections.contains_key(upstream_connection_id) => Some("credentials_missing"),
        domain::McpToolExecutionTarget::McpProxy { .. } => Some("connection_missing"),
        domain::McpToolExecutionTarget::AssistantClient { .. } => None,
    }
}

pub(super) fn bundle_tool_is_already_present(
    bundle: &domain::McpBundleTool,
    existing: &domain::McpToolRecord,
    interfaces: &BTreeMap<String, domain::McpInterfaceCatalogEntry>,
) -> bool {
    let plan = bundle_tool_plan(bundle, interfaces);
    existing.name == bundle.name
        && existing.short_description == bundle.short_description
        && existing.full_description == bundle.full_description
        && existing.execution_target == bundle.execution_target
        && existing.parameter_schema == plan.parameter_schema
        && existing.result_schema == plan.result_schema
        && existing.input_mapping == bundle.input_mapping
        && existing.output_mapping == bundle.output_mapping
        && existing.max_inline_chars == bundle.max_inline_chars
        && existing.response_fields == bundle.response_fields
        && existing.permission_code == plan.permission_code
        && existing.risk_level == plan.risk_level
}

pub(super) fn bundle_connection_is_already_present(
    bundle: &domain::McpBundleUpstreamConnection,
    existing: &domain::McpUpstreamConnectionRecord,
) -> bool {
    existing.name == bundle.name
        && existing.endpoint == bundle.endpoint
        && existing.transport == bundle.transport
        && existing.auth_type == bundle.auth_type
        && existing.custom_header_name == bundle.custom_header_name
}

pub(super) fn bundle_instance_is_already_present(
    bundle: &domain::McpBundleInstance,
    existing: &domain::McpInstanceRecord,
    snapshot: &McpBundleWorkspaceSnapshot,
) -> bool {
    if existing.name != bundle.name
        || existing.description_short != bundle.description_short
        || existing.default_entry_path != bundle.default_entry_path
    {
        return false;
    }
    let Some(policy) = snapshot.discovery_policies.get(&existing.id) else {
        return false;
    };
    if policy.list_default_limit != bundle.discovery_policy.list_default_limit
        || policy.list_max_depth != bundle.discovery_policy.list_max_depth
        || policy.list_regex_enabled != bundle.discovery_policy.list_regex_enabled
        || policy.list_regex_max_length != bundle.discovery_policy.list_regex_max_length
        || policy.list_return_fields != bundle.discovery_policy.list_return_fields
    {
        return false;
    }

    let existing_groups = snapshot
        .groups
        .iter()
        .filter(|group| group.instance_record_id == existing.id)
        .collect::<Vec<_>>();
    let mut matched_groups = vec![false; existing_groups.len()];
    for expected in &bundle.groups {
        let Some(index) = existing_groups
            .iter()
            .enumerate()
            .find_map(|(index, actual)| {
                (!matched_groups[index]
                    && actual.path == expected.path
                    && actual.display_name == expected.display_name
                    && actual.description_short == expected.description_short
                    && actual.enabled == expected.enabled
                    && actual.sort_order == expected.sort_order)
                    .then_some(index)
            })
        else {
            return false;
        };
        matched_groups[index] = true;
    }

    let existing_bindings = snapshot
        .bindings
        .iter()
        .filter(|binding| binding.instance_record_id == existing.id)
        .collect::<Vec<_>>();
    let mut matched_bindings = vec![false; existing_bindings.len()];
    for expected in &bundle.bindings {
        let Some(index) = existing_bindings
            .iter()
            .enumerate()
            .find_map(|(index, actual)| {
                (!matched_bindings[index]
                    && actual.group_path == expected.group_path
                    && actual.tool_id == expected.tool_id
                    && actual.display_alias == expected.display_alias
                    && actual.visible == expected.visible
                    && actual.sort_order == expected.sort_order)
                    .then_some(index)
            })
        else {
            return false;
        };
        matched_bindings[index] = true;
    }
    true
}

pub(super) fn preview_connection_reports(
    package: &domain::McpBundlePackage,
    snapshot: &McpBundleWorkspaceSnapshot,
) -> Vec<domain::McpBundleItemReport> {
    package
        .connections
        .iter()
        .map(
            |connection| match snapshot.connections.get(&connection.connection_id) {
                Some(existing) if bundle_connection_is_already_present(connection, existing) => {
                    item_report(
                        &connection.connection_id.to_string(),
                        domain::McpBundleItemEffect::AlreadyPresent,
                        "already_present",
                        None,
                    )
                }
                Some(_) => update_item_report(&connection.connection_id.to_string()),
                None => item_report(
                    &connection.connection_id.to_string(),
                    domain::McpBundleItemEffect::Create,
                    "unavailable",
                    Some("credentials_missing"),
                ),
            },
        )
        .collect()
}

pub(super) fn preview_tool_reports(
    package: &domain::McpBundlePackage,
    snapshot: &McpBundleWorkspaceSnapshot,
    interfaces: &BTreeMap<String, domain::McpInterfaceCatalogEntry>,
) -> Vec<domain::McpBundleItemReport> {
    package
        .tools
        .iter()
        .map(|tool| match snapshot.tools.get(&tool.tool_id) {
            Some(existing) if bundle_tool_is_already_present(tool, existing, interfaces) => {
                item_report(
                    &tool.tool_id,
                    domain::McpBundleItemEffect::AlreadyPresent,
                    "already_present",
                    None,
                )
            }
            Some(_) => update_item_report(&tool.tool_id),
            None => match bundle_tool_unavailable_reason(tool, interfaces, &snapshot.connections) {
                Some(reason) => item_report(
                    &tool.tool_id,
                    domain::McpBundleItemEffect::Create,
                    "unavailable",
                    Some(reason),
                ),
                None => item_report(
                    &tool.tool_id,
                    domain::McpBundleItemEffect::Create,
                    "imported",
                    None,
                ),
            },
        })
        .collect()
}

pub(super) fn preview_instance_reports(
    package: &domain::McpBundlePackage,
    snapshot: &McpBundleWorkspaceSnapshot,
) -> Vec<domain::McpBundleItemReport> {
    package
        .instances
        .iter()
        .map(
            |instance| match snapshot.instances.get(&instance.instance_id) {
                Some(existing)
                    if bundle_instance_is_already_present(instance, existing, snapshot) =>
                {
                    item_report(
                        &instance.instance_id,
                        domain::McpBundleItemEffect::AlreadyPresent,
                        "already_present",
                        None,
                    )
                }
                Some(_) => update_item_report(&instance.instance_id),
                None => item_report(
                    &instance.instance_id,
                    domain::McpBundleItemEffect::Create,
                    "imported",
                    None,
                ),
            },
        )
        .collect()
}

pub(super) fn shared_tool_impacts(
    package: &domain::McpBundlePackage,
    snapshot: &McpBundleWorkspaceSnapshot,
    interfaces: &BTreeMap<String, domain::McpInterfaceCatalogEntry>,
) -> Vec<domain::McpBundleSharedToolImpact> {
    let package_instance_ids = package
        .instances
        .iter()
        .map(|instance| instance.instance_id.as_str())
        .collect::<BTreeSet<_>>();
    let instance_ids_by_record = snapshot
        .instances
        .values()
        .map(|instance| (instance.id, instance.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut impacts = Vec::new();
    for tool in &package.tools {
        let Some(existing) = snapshot.tools.get(&tool.tool_id) else {
            continue;
        };
        if bundle_tool_is_already_present(tool, existing, interfaces) {
            continue;
        }
        let mut instance_ids = snapshot
            .bindings
            .iter()
            .filter(|binding| binding.tool_record_id == existing.id)
            .filter_map(|binding| {
                instance_ids_by_record
                    .get(&binding.instance_record_id)
                    .copied()
            })
            .filter(|instance_id| !package_instance_ids.contains(instance_id))
            .map(str::to_string)
            .collect::<Vec<_>>();
        instance_ids.sort();
        instance_ids.dedup();
        if !instance_ids.is_empty() {
            impacts.push(domain::McpBundleSharedToolImpact {
                tool_id: tool.tool_id.clone(),
                instance_ids,
            });
        }
    }
    impacts.sort_by(|left, right| left.tool_id.cmp(&right.tool_id));
    impacts
}

pub(super) fn summarize_bundle_effects(
    tools: &[domain::McpBundleItemReport],
    instances: &[domain::McpBundleItemReport],
    connections: &[domain::McpBundleItemReport],
) -> domain::McpBundleEffectSummary {
    let mut summary = domain::McpBundleEffectSummary::default();
    for item in tools.iter().chain(instances).chain(connections) {
        match item.effect {
            domain::McpBundleItemEffect::Create | domain::McpBundleItemEffect::Update => {
                summary.changes += 1
            }
            domain::McpBundleItemEffect::AlreadyPresent => summary.already_present += 1,
            domain::McpBundleItemEffect::Conflict => summary.conflicts += 1,
            domain::McpBundleItemEffect::Failed => summary.failed += 1,
        }
        if item.result == "unavailable" {
            summary.unavailable += 1;
        }
    }
    summary
}

pub(super) fn validate_package(package: &domain::McpBundlePackage) -> Result<()> {
    if package.manifest.schema_version != domain::MCP_BUNDLE_SCHEMA_VERSION {
        return Err(ControlPlaneError::InvalidInput("schema_version").into());
    }
    validate_identifier(&package.manifest.organization, "organization")?;
    validate_identifier(&package.manifest.bundle_id, "bundle_id")?;
    Version::parse(&package.manifest.bundle_version)
        .map_err(|_| ControlPlaneError::InvalidInput("bundle_version"))?;
    let mut tool_ids = BTreeSet::new();
    for tool in &package.tools {
        validate_identifier(&tool.tool_id, "tool_id")?;
        domain::mcp_management::validate_mcp_return_defaults(
            tool.max_inline_chars,
            tool.response_fields.as_deref(),
        )
        .map_err(ControlPlaneError::InvalidInput)?;
        domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping)
            .map_err(ControlPlaneError::InvalidInput)?;
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
            domain::McpToolExecutionTarget::AssistantClient { capability_code } => {
                validate_identifier(capability_code, "capability_code")?;
                if !domain::is_mcp_assistant_client_capability(capability_code) {
                    return Err(ControlPlaneError::InvalidInput(
                        "assistant_client_capability_code",
                    )
                    .into());
                }
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

pub(super) fn item_report(
    id: &str,
    effect: domain::McpBundleItemEffect,
    result: &str,
    reason: Option<&str>,
) -> domain::McpBundleItemReport {
    domain::McpBundleItemReport {
        id: id.to_string(),
        effect,
        result: result.to_string(),
        reason: reason.map(str::to_string),
    }
}

pub(super) fn update_item_report(id: &str) -> domain::McpBundleItemReport {
    domain::McpBundleItemReport {
        id: id.to_string(),
        effect: domain::McpBundleItemEffect::Update,
        result: "updated".to_string(),
        reason: None,
    }
}
