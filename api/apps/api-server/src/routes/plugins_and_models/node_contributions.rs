use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json, Router,
};
use control_plane::{
    application::ApplicationService,
    node_contribution::{ApplicationNodeCatalogService, ListApplicationNodesQuery},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[derive(Debug, Deserialize, IntoParams, Clone, ToSchema)]
pub struct NodeContributionQuery {
    /// Target Application. Its persisted application_type selects the boundary-node family.
    pub application_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationNodeSourceKindResponse {
    /// Node contract owned by the 1flowbase backend.
    Builtin,
    /// Workspace-assigned CapabilityPlugin contribution.
    Plugin,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationNodeRuntimeStatusResponse {
    /// The current orchestration runtime executes this node type.
    Ready,
    /// The contract is discoverable but cannot currently be executed.
    Unavailable,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationNodeDependencyStatusResponse {
    /// Built-in node; no CapabilityPlugin dependency applies.
    NotApplicable,
    Ready,
    MissingPlugin,
    VersionMismatch,
    DisabledPlugin,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationNodeContractFieldResponse {
    /// Exact flow-document or runtime field path.
    pub key: String,
    /// Stable semantic description for an Agent constructing node configuration.
    pub description: String,
    /// Whether this field is required when its applicability condition is met.
    pub required: bool,
    /// Accepted flow-document or JSON value kinds.
    pub value_types: Vec<String>,
    /// Closed value set; empty when the value is open-ended.
    pub allowed_values: Vec<String>,
    /// Conditional applicability, when the field does not apply to every configuration.
    pub applicability: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationNodeFieldContractResponse {
    /// Persisted node config fields.
    pub config_fields: Vec<ApplicationNodeContractFieldResponse>,
    /// Runtime bindings or invocation inputs.
    pub input_fields: Vec<ApplicationNodeContractFieldResponse>,
    /// Runtime or declared output fields.
    pub output_fields: Vec<ApplicationNodeContractFieldResponse>,
}

/// Complete immutable identity and schema snapshot for one plugin node contribution.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationPluginNodeIdentityResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_unique_identifier: String,
    pub package_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub node_shell: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub schema_version: String,
    pub experimental: bool,
    pub icon: String,
    #[schema(value_type = Object)]
    pub schema_ui: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_schema: serde_json::Value,
    pub contribution_checksum: String,
    pub compiled_contribution_hash: String,
    #[schema(value_type = Object)]
    pub output_schema_snapshot: serde_json::Value,
    pub side_effect_policy: String,
    pub infra_contracts: Vec<String>,
    pub required_auth: Vec<String>,
    pub visibility: String,
    pub dependency_installation_kind: String,
    pub dependency_plugin_version_range: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationNodeCatalogEntryResponse {
    pub source_kind: ApplicationNodeSourceKindResponse,
    /// Flow document node.type. Plugin contributions use plugin_node and retain their exact
    /// contribution identity under plugin.
    pub node_type: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub runtime_status: ApplicationNodeRuntimeStatusResponse,
    pub runtime_status_description: String,
    pub dependency_status: ApplicationNodeDependencyStatusResponse,
    pub field_contract: ApplicationNodeFieldContractResponse,
    /// Present only when source_kind is plugin.
    pub plugin: Option<ApplicationPluginNodeIdentityResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationNodeCatalogResponse {
    /// Type-specific boundary nodes, all known built-in processing nodes, and current-workspace
    /// plugin contributions.
    pub nodes: Vec<ApplicationNodeCatalogEntryResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/node-contributions",
        console_get(
            list_node_contributions,
            ConsoleOperation("node_contributions.view".to_string()),
        ),
    )
}

fn to_contract_field(
    field: control_plane::node_contribution::ApplicationNodeContractField,
) -> ApplicationNodeContractFieldResponse {
    ApplicationNodeContractFieldResponse {
        key: field.key,
        description: field.description,
        required: field.required,
        value_types: field.value_types,
        allowed_values: field.allowed_values,
        applicability: field.applicability,
    }
}

fn to_field_contract(
    contract: control_plane::node_contribution::ApplicationNodeFieldContract,
) -> ApplicationNodeFieldContractResponse {
    ApplicationNodeFieldContractResponse {
        config_fields: contract
            .config_fields
            .into_iter()
            .map(to_contract_field)
            .collect(),
        input_fields: contract
            .input_fields
            .into_iter()
            .map(to_contract_field)
            .collect(),
        output_fields: contract
            .output_fields
            .into_iter()
            .map(to_contract_field)
            .collect(),
    }
}

fn to_plugin_identity(
    entry: domain::NodeContributionRegistryEntry,
) -> ApplicationPluginNodeIdentityResponse {
    ApplicationPluginNodeIdentityResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_unique_identifier: entry.plugin_unique_identifier,
        package_id: entry.package_id,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        contribution_code: entry.contribution_code,
        node_shell: entry.node_shell,
        category: entry.category,
        title: entry.title,
        description: entry.description,
        schema_version: entry.schema_version,
        experimental: entry.experimental,
        icon: entry.icon,
        schema_ui: entry.schema_ui,
        output_schema: entry.output_schema,
        contribution_checksum: entry.contribution_checksum,
        compiled_contribution_hash: entry.compiled_contribution_hash,
        output_schema_snapshot: entry.output_schema_snapshot,
        side_effect_policy: entry.side_effect_policy,
        infra_contracts: entry.infra_contracts,
        required_auth: entry.required_auth,
        visibility: entry.visibility,
        dependency_installation_kind: entry.dependency_installation_kind,
        dependency_plugin_version_range: entry.dependency_plugin_version_range,
    }
}

fn to_response(
    entry: control_plane::node_contribution::ApplicationNodeCatalogEntry,
) -> ApplicationNodeCatalogEntryResponse {
    ApplicationNodeCatalogEntryResponse {
        source_kind: match entry.source_kind {
            control_plane::node_contribution::ApplicationNodeSourceKind::Builtin => {
                ApplicationNodeSourceKindResponse::Builtin
            }
            control_plane::node_contribution::ApplicationNodeSourceKind::Plugin => {
                ApplicationNodeSourceKindResponse::Plugin
            }
        },
        node_type: entry.node_type,
        title: entry.title,
        description: entry.description,
        category: entry.category,
        runtime_status: match entry.runtime_status {
            control_plane::node_contribution::ApplicationNodeRuntimeStatus::Ready => {
                ApplicationNodeRuntimeStatusResponse::Ready
            }
            control_plane::node_contribution::ApplicationNodeRuntimeStatus::Unavailable => {
                ApplicationNodeRuntimeStatusResponse::Unavailable
            }
        },
        runtime_status_description: entry.runtime_status_description,
        dependency_status: match entry.dependency_status {
            control_plane::node_contribution::ApplicationNodeDependencyStatus::NotApplicable => {
                ApplicationNodeDependencyStatusResponse::NotApplicable
            }
            control_plane::node_contribution::ApplicationNodeDependencyStatus::Ready => {
                ApplicationNodeDependencyStatusResponse::Ready
            }
            control_plane::node_contribution::ApplicationNodeDependencyStatus::MissingPlugin => {
                ApplicationNodeDependencyStatusResponse::MissingPlugin
            }
            control_plane::node_contribution::ApplicationNodeDependencyStatus::VersionMismatch => {
                ApplicationNodeDependencyStatusResponse::VersionMismatch
            }
            control_plane::node_contribution::ApplicationNodeDependencyStatus::DisabledPlugin => {
                ApplicationNodeDependencyStatusResponse::DisabledPlugin
            }
        },
        field_contract: to_field_contract(entry.field_contract),
        plugin: entry.plugin.map(to_plugin_identity),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/node-contributions",
    summary = "List the unified Application node catalog",
    description = "Returns the target Application type's boundary nodes, every known built-in processing node with truthful runtime status, and CapabilityPlugin node contributions assigned to the current workspace.",
    params(NodeContributionQuery),
    responses(
        (status = 200, body = ApplicationNodeCatalogResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_node_contributions(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<NodeContributionQuery>,
) -> Result<Json<ApiSuccess<ApplicationNodeCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ApplicationService::new(state.store.clone())
        .get_application(context.user.id, query.application_id)
        .await?;
    let catalog = ApplicationNodeCatalogService::new(state.store.clone())
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: context.user.id,
            application_type: application.application_type,
        })
        .await?;

    Ok(Json(ApiSuccess::new(ApplicationNodeCatalogResponse {
        nodes: catalog.nodes.into_iter().map(to_response).collect(),
    })))
}
