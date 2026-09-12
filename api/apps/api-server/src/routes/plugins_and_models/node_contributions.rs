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
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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
pub enum ApplicationNodeAuthoringStatusResponse {
    /// Published in the Application node picker and available for new flow documents.
    Published,
    /// Retained for runtime and contract discovery, but not offered for new authoring.
    Hidden,
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
    /// Whether this field is required.
    pub required: bool,
    /// Accepted flow-document or JSON value kinds.
    pub value_types: Vec<String>,
    /// Closed value set; empty when the value is open-ended.
    pub allowed_values: Vec<String>,
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
    pub category: String,
    pub authoring_status: ApplicationNodeAuthoringStatusResponse,
    pub runtime_status: ApplicationNodeRuntimeStatusResponse,
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
        required: field.required,
        value_types: field.value_types,
        allowed_values: field.allowed_values,
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
        category: entry.category,
        authoring_status: match entry.authoring_status {
            control_plane::node_contribution::ApplicationNodeAuthoringStatus::Published => {
                ApplicationNodeAuthoringStatusResponse::Published
            }
            control_plane::node_contribution::ApplicationNodeAuthoringStatus::Hidden => {
                ApplicationNodeAuthoringStatusResponse::Hidden
            }
        },
        runtime_status: match entry.runtime_status {
            control_plane::node_contribution::ApplicationNodeRuntimeStatus::Ready => {
                ApplicationNodeRuntimeStatusResponse::Ready
            }
            control_plane::node_contribution::ApplicationNodeRuntimeStatus::Unavailable => {
                ApplicationNodeRuntimeStatusResponse::Unavailable
            }
        },
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
    let snapshot_state = Arc::clone(&state);
    let output: NodeContributionsOutput = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.node-contributions.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        NodeContributionsInput(query),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

pub(crate) struct NodeContributionsInput(NodeContributionQuery);
impl InterfaceContract for NodeContributionsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[("application_id", mp::text_schema())]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[(
                "application_id",
                serde_json::Value::String((&(&(self).0).application_id).to_string()),
            )]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-node-contributions-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NodeContributionsOutput(ApplicationNodeCatalogResponse);
impl InterfaceContract for NodeContributionsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[(
                "nodes",
                serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source_kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Builtin"))]), mp::object_schema(&[("variant",mp::tag_schema("Plugin"))])])), ("node_type",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("category",mp::object_schema(&[("byte_count",mp::count_schema())])), ("authoring_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Published"))]), mp::object_schema(&[("variant",mp::tag_schema("Hidden"))])])), ("runtime_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Ready"))]), mp::object_schema(&[("variant",mp::tag_schema("Unavailable"))])])), ("dependency_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("NotApplicable"))]), mp::object_schema(&[("variant",mp::tag_schema("Ready"))]), mp::object_schema(&[("variant",mp::tag_schema("MissingPlugin"))]), mp::object_schema(&[("variant",mp::tag_schema("VersionMismatch"))]), mp::object_schema(&[("variant",mp::tag_schema("DisabledPlugin"))])])), ("field_contract",mp::object_schema(&[("config_fields",mp::object_schema(&[("item_count",mp::count_schema())])), ("input_fields",mp::object_schema(&[("item_count",mp::count_schema())])), ("output_fields",mp::object_schema(&[("item_count",mp::count_schema())]))])), ("plugin",serde_json::json!({"anyOf": [mp::object_schema(&[("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("package_id",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("contribution_code",mp::text_schema()), ("node_shell",mp::object_schema(&[("byte_count",mp::count_schema())])), ("category",mp::object_schema(&[("byte_count",mp::count_schema())])), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("schema_version",mp::text_schema()), ("experimental",serde_json::json!({"type":"boolean"})), ("icon",mp::object_schema(&[("byte_count",mp::count_schema())])), ("schema_ui",mp::json_summary_schema()), ("output_schema",mp::json_summary_schema()), ("contribution_checksum",mp::object_schema(&[("byte_count",mp::count_schema())])), ("compiled_contribution_hash",mp::object_schema(&[("byte_count",mp::count_schema())])), ("output_schema_snapshot",mp::json_summary_schema()), ("side_effect_policy",mp::object_schema(&[("byte_count",mp::count_schema())])), ("infra_contracts",mp::object_schema(&[("item_count",mp::count_schema())])), ("required_auth",mp::object_schema(&[("item_count",mp::count_schema())])), ("visibility",mp::object_schema(&[("byte_count",mp::count_schema())])), ("dependency_installation_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("dependency_plugin_version_range",mp::object_schema(&[("byte_count",mp::count_schema())]))]), {"type":"null"}]}))])}),
            )]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[("nodes", {
                if (&(&(self).0).nodes).len() > 32 {
                    return None;
                }
                serde_json::Value::Array((&(&(self).0).nodes).iter().map(|item| Some(mp::object_value(&[("source_kind",match &(item).source_kind {ApplicationNodeSourceKindResponse::Builtin => mp::object_value(&[("variant",serde_json::Value::String("Builtin".to_owned()))]), ApplicationNodeSourceKindResponse::Plugin => mp::object_value(&[("variant",serde_json::Value::String("Plugin".to_owned()))])}), ("node_type",mp::text(&(item).node_type)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(item).category).len()))])), ("authoring_status",match &(item).authoring_status {ApplicationNodeAuthoringStatusResponse::Published => mp::object_value(&[("variant",serde_json::Value::String("Published".to_owned()))]), ApplicationNodeAuthoringStatusResponse::Hidden => mp::object_value(&[("variant",serde_json::Value::String("Hidden".to_owned()))])}), ("runtime_status",match &(item).runtime_status {ApplicationNodeRuntimeStatusResponse::Ready => mp::object_value(&[("variant",serde_json::Value::String("Ready".to_owned()))]), ApplicationNodeRuntimeStatusResponse::Unavailable => mp::object_value(&[("variant",serde_json::Value::String("Unavailable".to_owned()))])}), ("dependency_status",match &(item).dependency_status {ApplicationNodeDependencyStatusResponse::NotApplicable => mp::object_value(&[("variant",serde_json::Value::String("NotApplicable".to_owned()))]), ApplicationNodeDependencyStatusResponse::Ready => mp::object_value(&[("variant",serde_json::Value::String("Ready".to_owned()))]), ApplicationNodeDependencyStatusResponse::MissingPlugin => mp::object_value(&[("variant",serde_json::Value::String("MissingPlugin".to_owned()))]), ApplicationNodeDependencyStatusResponse::VersionMismatch => mp::object_value(&[("variant",serde_json::Value::String("VersionMismatch".to_owned()))]), ApplicationNodeDependencyStatusResponse::DisabledPlugin => mp::object_value(&[("variant",serde_json::Value::String("DisabledPlugin".to_owned()))])}), ("field_contract",mp::object_value(&[("config_fields",mp::object_value(&[("item_count",serde_json::json!((&(&(item).field_contract).config_fields).len()))])), ("input_fields",mp::object_value(&[("item_count",serde_json::json!((&(&(item).field_contract).input_fields).len()))])), ("output_fields",mp::object_value(&[("item_count",serde_json::json!((&(&(item).field_contract).output_fields).len()))]))])), ("plugin",match (&(item).plugin).as_ref() { Some(item) => mp::object_value(&[("installation_id",mp::text(&(item).installation_id)?), ("provider_code",mp::text(&(item).provider_code)?), ("package_id",mp::text(&(item).package_id)?), ("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_version",mp::text(&(item).plugin_version)?), ("contribution_code",mp::text(&(item).contribution_code)?), ("node_shell",mp::object_value(&[("byte_count",serde_json::json!((&(item).node_shell).len()))])), ("category",mp::object_value(&[("byte_count",serde_json::json!((&(item).category).len()))])), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("schema_version",mp::text(&(item).schema_version)?), ("experimental",serde_json::Value::Bool(*(&(item).experimental))), ("icon",mp::object_value(&[("byte_count",serde_json::json!((&(item).icon).len()))])), ("schema_ui",mp::json_summary(&(item).schema_ui)), ("output_schema",mp::json_summary(&(item).output_schema)), ("contribution_checksum",mp::object_value(&[("byte_count",serde_json::json!((&(item).contribution_checksum).len()))])), ("compiled_contribution_hash",mp::object_value(&[("byte_count",serde_json::json!((&(item).compiled_contribution_hash).len()))])), ("output_schema_snapshot",mp::json_summary(&(item).output_schema_snapshot)), ("side_effect_policy",mp::object_value(&[("byte_count",serde_json::json!((&(item).side_effect_policy).len()))])), ("infra_contracts",mp::object_value(&[("item_count",serde_json::json!((&(item).infra_contracts).len()))])), ("required_auth",mp::object_value(&[("item_count",serde_json::json!((&(item).required_auth).len()))])), ("visibility",mp::object_value(&[("byte_count",serde_json::json!((&(item).visibility).len()))])), ("dependency_installation_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).dependency_installation_kind).len()))])), ("dependency_plugin_version_range",mp::object_value(&[("byte_count",serde_json::json!((&(item).dependency_plugin_version_range).len()))]))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
            })]),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-node-contributions-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct NodeContributionsAdapter(storage_durable_postgres::MainDurableStore);

impl
    crate::routes::console_interface::ConsoleInterfacePort<
        NodeContributionsInput,
        NodeContributionsOutput,
    > for NodeContributionsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NodeContributionsInput,
    ) -> crate::routes::console_interface::ConsoleInterfaceFuture<'a, NodeContributionsOutput> {
        Box::pin(async move {
            let actor = principal.actor();
            let application = ApplicationService::new(self.0.for_actor(actor.clone()))
                .get_application(actor.user_id, input.0.application_id)
                .await
                .map_err(ApiError)
                .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            let catalog = ApplicationNodeCatalogService::new(self.0.clone())
                .list_application_nodes(ListApplicationNodesQuery {
                    actor_user_id: actor.user_id,
                    application_type: application.application_type,
                })
                .await
                .map_err(ApiError)
                .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            Ok(NodeContributionsOutput(ApplicationNodeCatalogResponse {
                nodes: catalog.nodes.into_iter().map(to_response).collect(),
            }))
        })
    }
}

pub(crate) fn compile_registry(
    store: storage_durable_postgres::MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    crate::routes::console_interface::compile_registry(
        "api-server.console-node-contributions",
        "graph:console-node-contributions-v1",
        &[
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "node_contributions.view",
                binding_id: "http.console.node-contributions.get.v1",
                method: "GET",
                path: "/api/console/node-contributions",
                mutating: false,
            },
        ],
        Arc::new(NodeContributionsAdapter(store)),
    )
}
