use super::*;

pub(super) fn to_catalog_response(
    snapshot: domain::McpCatalogSnapshot,
    operations: &HashMap<String, String>,
) -> Result<McpCatalogResponse, ApiError> {
    let discovery_policies =
        discovery_policy_responses(&snapshot.instances, snapshot.discovery_policies)?;
    Ok(McpCatalogResponse {
        instances: snapshot
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
        groups: snapshot.groups.into_iter().map(to_group_response).collect(),
        tools: snapshot
            .tools
            .into_iter()
            .map(|record| to_tool_response(record, operations))
            .collect(),
        bindings: snapshot
            .bindings
            .into_iter()
            .map(to_binding_response)
            .collect(),
        discovery_policies,
    })
}

pub(super) fn to_export_response(
    export: domain::McpExportPackage,
    operations: &HashMap<String, String>,
) -> Result<McpExportPackageResponse, ApiError> {
    let discovery_policies =
        discovery_policy_responses(&export.instances, export.discovery_policies)?;
    Ok(McpExportPackageResponse {
        instances: export
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
        groups: export.groups.into_iter().map(to_group_response).collect(),
        tools: export
            .tools
            .into_iter()
            .map(|record| to_tool_response(record, operations))
            .collect(),
        bindings: export
            .bindings
            .into_iter()
            .map(to_binding_response)
            .collect(),
        discovery_policies,
    })
}

pub(super) fn to_instance_response(record: domain::McpInstanceRecord) -> McpInstanceResponse {
    McpInstanceResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        instance_id: record.instance_id,
        name: record.name,
        description_short: record.description_short,
        status: record.status.as_str().into(),
        default_entry_path: record.default_entry_path,
        created_by: record.created_by.to_string(),
        updated_by: record.updated_by.to_string(),
        created_at: record.created_at.to_string(),
        updated_at: record.updated_at.to_string(),
    }
}

pub(super) fn to_group_response(record: domain::McpGroupRecord) -> McpGroupResponse {
    McpGroupResponse {
        id: record.id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        path: record.path,
        display_name: record.display_name,
        description_short: record.description_short,
        enabled: record.enabled,
        sort_order: record.sort_order,
    }
}

pub(super) fn to_tool_response(
    record: domain::McpToolRecord,
    operations: &HashMap<String, String>,
) -> McpToolResponse {
    let (operation, availability_status) = match &record.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let available_operation = operations.get(interface_id).cloned();
            let availability_status = if available_operation.is_some() {
                domain::McpToolAvailabilityStatus::Available
            } else {
                domain::McpToolAvailabilityStatus::InterfaceMissing
            };
            (
                available_operation.unwrap_or_else(|| interface_id.clone()),
                availability_status,
            )
        }
        domain::McpToolExecutionTarget::McpProxy {
            remote_tool_name, ..
        } => (
            format!("MCP tools/call {remote_tool_name}"),
            domain::McpToolAvailabilityStatus::Available,
        ),
    };

    to_tool_response_with_operation(record, operation, availability_status)
}

pub(super) async fn to_tool_response_for_actor(
    state: &ApiState,
    actor_user_id: Uuid,
    record: domain::McpToolRecord,
    operations: &HashMap<String, String>,
) -> Result<McpToolResponse, ApiError> {
    let availability = match &record.execution_target {
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => Some(
            McpManagementService::new(state.store.clone())
                .upstream_proxy_availability(
                    actor_user_id,
                    *upstream_connection_id,
                    remote_tool_name,
                )
                .await?,
        ),
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
    };
    if let Some(availability) = availability {
        let operation = match &record.execution_target {
            domain::McpToolExecutionTarget::McpProxy {
                remote_tool_name, ..
            } => {
                format!("MCP tools/call {remote_tool_name}")
            }
            domain::McpToolExecutionTarget::InterfaceWrapper { .. } => String::new(),
        };
        Ok(to_tool_response_with_operation(
            record,
            operation,
            availability,
        ))
    } else {
        Ok(to_tool_response(record, operations))
    }
}

pub(super) fn to_tool_response_with_operation(
    record: domain::McpToolRecord,
    operation: String,
    availability_status: domain::McpToolAvailabilityStatus,
) -> McpToolResponse {
    McpToolResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        tool_id: record.tool_id,
        name: record.name,
        short_description: record.short_description,
        full_description: record.full_description,
        execution_target: match record.execution_target {
            domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                McpToolExecutionTargetDto::InterfaceWrapper { interface_id }
            }
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id,
                remote_tool_name,
                source_schema_hash,
            } => McpToolExecutionTargetDto::McpProxy {
                upstream_connection_id: upstream_connection_id.to_string(),
                remote_tool_name,
                source_schema_hash,
            },
        },
        operation,
        parameter_schema: record.parameter_schema,
        result_schema: record.result_schema,
        input_mapping: record.input_mapping,
        output_mapping: record.output_mapping,
        permission_code: record.permission_code,
        risk_level: record.risk_level.as_str().into(),
        des_id: record.des_id,
        des_id_required: record.des_id_required,
        status: record.status.as_str().into(),
        availability_status: availability_status.into(),
        availability_reason: (availability_status != domain::McpToolAvailabilityStatus::Available)
            .then(|| availability_status.as_str().to_string()),
        revision: record.revision,
    }
}

pub(super) fn to_binding_response(record: domain::McpToolBindingRecord) -> McpToolBindingResponse {
    McpToolBindingResponse {
        id: record.id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        tool_record_id: record.tool_record_id.to_string(),
        group_path: record.group_path,
        tool_id: record.tool_id,
        display_alias: record.display_alias,
        visible: record.visible,
        sort_order: record.sort_order,
    }
}

pub(super) fn to_discovery_policy_response(
    record: domain::McpInstanceDiscoveryPolicyRecord,
    instance_id: String,
) -> McpInstanceDiscoveryPolicyResponse {
    McpInstanceDiscoveryPolicyResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        instance_id,
        list_default_limit: record.list_default_limit,
        list_max_depth: record.list_max_depth,
        list_regex_enabled: record.list_regex_enabled,
        list_regex_max_length: record.list_regex_max_length,
        list_return_fields: record.list_return_fields,
    }
}

pub(super) fn discovery_policy_responses(
    instances: &[domain::McpInstanceRecord],
    policies: Vec<domain::McpInstanceDiscoveryPolicyRecord>,
) -> Result<Vec<McpInstanceDiscoveryPolicyResponse>, ApiError> {
    let instance_ids = instances
        .iter()
        .map(|instance| (instance.id, instance.instance_id.clone()))
        .collect::<HashMap<_, _>>();
    policies
        .into_iter()
        .map(|policy| {
            let instance_id = instance_ids
                .get(&policy.instance_record_id)
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "MCP discovery policy references missing instance record {}",
                        policy.instance_record_id
                    )
                })?;
            Ok(to_discovery_policy_response(policy, instance_id))
        })
        .collect()
}

pub(super) fn to_interface_response(
    entry: domain::McpInterfaceCatalogEntry,
) -> McpInterfaceCatalogEntryResponse {
    McpInterfaceCatalogEntryResponse {
        interface_id: entry.interface_id,
        method: entry.method,
        path: entry.path,
        name: entry.name,
        short_description: entry.short_description,
        parameter_descriptors: entry
            .parameter_descriptors
            .into_iter()
            .map(to_parameter_descriptor_response)
            .collect(),
        parameter_schema: entry.parameter_schema,
        result_schema: entry.result_schema,
        permission_code: entry.permission_code,
        security: entry.security,
        risk_level: entry.risk_level.as_str().into(),
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason,
    }
}

pub(super) fn to_parameter_descriptor_response(
    descriptor: McpParameterDescriptor,
) -> McpParameterDescriptorResponse {
    McpParameterDescriptorResponse {
        name: descriptor.name,
        field_type: descriptor.field_type,
        parameter_type: descriptor.parameter_type.as_str().into(),
        description: descriptor.description,
        required: descriptor.required,
        schema: descriptor.schema,
    }
}

pub(super) fn list_response_field_set(
    value: &serde_json::Value,
) -> Result<BTreeSet<String>, ApiError> {
    let Some(fields) = value.as_array() else {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("list_return_fields").into(),
        );
    };
    let mut field_set = BTreeSet::new();
    for field in fields {
        let Some(field) = field.as_str() else {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "list_return_fields",
            )
            .into());
        };
        field_set.insert(field.to_string());
    }
    Ok(field_set)
}

pub(super) fn includes_list_response_field(fields: &BTreeSet<String>, field: &str) -> bool {
    fields.contains(field) || (field == "item_kind" && fields.contains("type"))
}

pub(super) fn to_list_item_response(
    item: domain::McpListItemSummary,
    fields: &BTreeSet<String>,
) -> McpListItemSummaryResponse {
    let item_kind = match item.item_kind {
        domain::McpListItemKind::Group => "group".to_string(),
        domain::McpListItemKind::Tool => "tool".to_string(),
    };
    McpListItemSummaryResponse {
        id: if includes_list_response_field(fields, "id") {
            Some(item.id)
        } else {
            None
        },
        item_kind: if includes_list_response_field(fields, "item_kind") {
            Some(item_kind)
        } else {
            None
        },
        path: if includes_list_response_field(fields, "path") {
            Some(item.path)
        } else {
            None
        },
        name: if includes_list_response_field(fields, "name") {
            Some(item.name)
        } else {
            None
        },
        description_short: if includes_list_response_field(fields, "description_short") {
            item.description_short
        } else {
            None
        },
        children_count: if includes_list_response_field(fields, "children_count") {
            Some(item.children_count)
        } else {
            None
        },
        risk_level: if includes_list_response_field(fields, "risk_level") {
            item.risk_level.map(|risk| risk.as_str().into())
        } else {
            None
        },
    }
}
