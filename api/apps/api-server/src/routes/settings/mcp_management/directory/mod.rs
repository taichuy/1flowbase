mod typed;
pub(crate) use typed::{compile_registry, McpDirectoryDependencies};

use super::*;
use typed::{McpDirectoryInput, McpDirectoryOutput};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct McpInstanceDirectoryResponse {
    pub instance_record_id: String,
    pub groups: Vec<McpGroupResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceMcpInstanceDirectoryBody {
    pub expected_snapshot: McpInstanceDirectoryResponse,
    pub groups: Vec<McpDirectoryGroupBody>,
    pub bindings: Vec<McpDirectoryBindingBody>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDirectoryGroupBody {
    pub id: Option<String>,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDirectoryBindingBody {
    pub id: Option<String>,
    pub tool_id: String,
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

/// Read an instance directory snapshot.
///
/// Returns the groups and tool bindings from one consistent workspace-scoped snapshot.
#[utoipa::path(get, path = "/api/console/mcp/instances/{instance_id}/directory",
    params(("instance_id" = String, Path, description = "Workspace-scoped instance identifier")),
    responses((status = 200, body = McpInstanceDirectoryResponse), (status = 403), (status = 404)))]
pub async fn get_instance_directory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<Json<ApiSuccess<McpInstanceDirectoryResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let McpDirectoryOutput(snapshot) = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.mcp.directory.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        McpDirectoryInput::Read(instance_id),
    )
    .await?;
    Ok(Json(ApiSuccess::new(snapshot)))
}

/// Replace an instance directory atomically.
///
/// Replaces all groups and bindings only when the expected snapshot still matches; conflicts leave the directory unchanged.
#[utoipa::path(put, path = "/api/console/mcp/instances/{instance_id}/directory",
    request_body = ReplaceMcpInstanceDirectoryBody,
    params(("instance_id" = String, Path, description = "Workspace-scoped instance identifier")),
    responses((status = 200, body = McpInstanceDirectoryResponse), (status = 403), (status = 404), (status = 409), (status = 400), (status = 422)))]
pub async fn replace_instance_directory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<ReplaceMcpInstanceDirectoryBody>,
) -> Result<Json<ApiSuccess<McpInstanceDirectoryResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let McpDirectoryOutput(snapshot) = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.mcp.directory.put.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        McpDirectoryInput::Replace(instance_id, body),
    )
    .await?;
    Ok(Json(ApiSuccess::new(snapshot)))
}

fn to_response(
    snapshot: domain::mcp_management::McpInstanceDirectory,
) -> McpInstanceDirectoryResponse {
    McpInstanceDirectoryResponse {
        instance_record_id: snapshot.instance_record_id.to_string(),
        groups: snapshot
            .groups
            .into_iter()
            .map(|group| McpGroupResponse {
                id: group.id.to_string(),
                instance_record_id: group.instance_record_id.to_string(),
                path: group.path,
                display_name: group.display_name,
                description_short: group.description_short,
                enabled: group.enabled,
                sort_order: group.sort_order,
            })
            .collect(),
        bindings: snapshot
            .bindings
            .into_iter()
            .map(|binding| McpToolBindingResponse {
                id: binding.id.to_string(),
                instance_record_id: binding.instance_record_id.to_string(),
                tool_record_id: binding.tool_record_id.to_string(),
                tool_id: binding.tool_id,
                group_path: binding.group_path,
                display_alias: binding.display_alias,
                visible: binding.visible,
                sort_order: binding.sort_order,
            })
            .collect(),
    }
}

fn to_command(
    instance_id: String,
    body: ReplaceMcpInstanceDirectoryBody,
) -> Result<control_plane::mcp_management::ReplaceMcpInstanceDirectoryCommand, ApiError> {
    use domain::mcp_management::*;
    let snapshot = body.expected_snapshot;
    Ok(
        control_plane::mcp_management::ReplaceMcpInstanceDirectoryCommand {
            instance_id,
            expected_snapshot: McpInstanceDirectory {
                instance_record_id: parse_uuid(&snapshot.instance_record_id, "instance_record_id")?,
                groups: snapshot
                    .groups
                    .into_iter()
                    .map(|group| {
                        Ok(McpDirectoryGroupSnapshot {
                            id: parse_uuid(&group.id, "group_id")?,
                            instance_record_id: parse_uuid(
                                &group.instance_record_id,
                                "instance_record_id",
                            )?,
                            path: group.path,
                            display_name: group.display_name,
                            description_short: group.description_short,
                            enabled: group.enabled,
                            sort_order: group.sort_order,
                        })
                    })
                    .collect::<Result<_, ApiError>>()?,
                bindings: snapshot
                    .bindings
                    .into_iter()
                    .map(|binding| {
                        Ok(McpDirectoryBindingSnapshot {
                            id: parse_uuid(&binding.id, "binding_id")?,
                            instance_record_id: parse_uuid(
                                &binding.instance_record_id,
                                "instance_record_id",
                            )?,
                            tool_record_id: parse_uuid(&binding.tool_record_id, "tool_record_id")?,
                            tool_id: binding.tool_id,
                            group_path: binding.group_path,
                            display_alias: binding.display_alias,
                            visible: binding.visible,
                            sort_order: binding.sort_order,
                        })
                    })
                    .collect::<Result<_, ApiError>>()?,
            },
            groups: body
                .groups
                .into_iter()
                .map(|group| {
                    Ok(McpDirectoryGroup {
                        id: group
                            .id
                            .as_deref()
                            .map(|id| parse_uuid(id, "group_id"))
                            .transpose()?,
                        path: group.path,
                        display_name: group.display_name,
                        description_short: group.description_short,
                        enabled: group.enabled,
                        sort_order: group.sort_order,
                    })
                })
                .collect::<Result<_, ApiError>>()?,
            bindings: body
                .bindings
                .into_iter()
                .map(|binding| {
                    Ok(McpDirectoryBinding {
                        id: binding
                            .id
                            .as_deref()
                            .map(|id| parse_uuid(id, "binding_id"))
                            .transpose()?,
                        tool_id: binding.tool_id,
                        group_path: binding.group_path,
                        display_alias: binding.display_alias,
                        visible: binding.visible,
                        sort_order: binding.sort_order,
                    })
                })
                .collect::<Result<_, ApiError>>()?,
        },
    )
}
