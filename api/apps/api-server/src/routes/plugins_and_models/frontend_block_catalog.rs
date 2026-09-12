use std::{collections::BTreeMap, sync::Arc};

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::frontend_block_catalog::{
    FrontendBlockCatalogService, FrontendContributionBinding, ListFrontendBlockCatalogQuery,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockPermissionsResponse {
    pub network: String,
    pub storage: String,
    pub secrets: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockContextContractResponse {
    pub primitives: Vec<String>,
    #[schema(value_type = Object)]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IsolatedFrontendBlockEntryAssetResponse {
    pub media_type: String,
    pub sha256: String,
    pub url: String,
    pub integrity: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockCatalogResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_template_version: Option<String>,
    pub code_template_language: Option<String>,
    pub isolated_entry_asset: Option<IsolatedFrontendBlockEntryAssetResponse>,
    pub context_contract: FrontendBlockContextContractResponse,
    pub permissions: FrontendBlockPermissionsResponse,
    pub ui_capabilities: Vec<String>,
    pub frontend_contribution_id: String,
    pub frontend_block_id: String,
    pub frontend_block_version: String,
    pub runtime_kind: String,
    pub execution_kind: String,
    pub isolation_requirement: String,
    pub requested_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub workspace_id: String,
    pub lifecycle_kind: String,
    pub graph_fingerprint: String,
    pub provenance: FrontendContributionProvenanceResponse,
    pub disable_reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendContributionProvenanceResponse {
    pub module_id: String,
    pub module_version: String,
    pub module_kind: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/frontend-blocks",
        console_get(
            list_frontend_blocks,
            ConsoleOperation("frontend_blocks.view".to_string()),
        ),
    )
}

fn to_response(
    binding: FrontendContributionBinding,
) -> Result<FrontendBlockCatalogResponse, ApiError> {
    let asset_bindings = binding
        .assets
        .iter()
        .map(|asset| (asset.digest.as_str(), asset))
        .collect::<BTreeMap<_, _>>();
    let entry = binding.catalog_entry;
    let isolated_entry_asset = if entry.runtime == "isolated_iframe" {
        entry
            .code_modules
            .iter()
            .flat_map(|module| module.assets.iter())
            .find(|asset| asset.role == domain::FrontendModuleAssetRole::BrowserModule)
            .and_then(|asset| {
                asset_bindings.get(asset.sha256.as_str()).map(|projected| {
                    IsolatedFrontendBlockEntryAssetResponse {
                        media_type: asset.media_type.clone(),
                        sha256: asset.sha256.clone(),
                        url: projected.url.clone(),
                        integrity: projected.integrity.as_str().to_string(),
                    }
                })
            })
    } else {
        None
    };
    Ok(FrontendBlockCatalogResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        contribution_code: entry.contribution_code,
        title: entry.title,
        runtime: entry.runtime,
        entry: entry.entry,
        code_template: entry.code_template,
        code_template_version: entry.code_template_version,
        code_template_language: entry.code_template_language,
        isolated_entry_asset,
        context_contract: FrontendBlockContextContractResponse {
            primitives: entry.context_contract.primitives,
            input_schema: entry.context_contract.input_schema,
        },
        permissions: FrontendBlockPermissionsResponse {
            network: entry.permissions.network,
            storage: entry.permissions.storage,
            secrets: entry.permissions.secrets,
        },
        ui_capabilities: entry.ui_capabilities,
        frontend_contribution_id: binding.contribution_id,
        frontend_block_id: binding.block_id,
        frontend_block_version: binding.block_version,
        runtime_kind: binding.runtime_kind.as_str().to_string(),
        execution_kind: binding.execution_kind.as_str().to_string(),
        isolation_requirement: binding.isolation_requirement.as_str().to_string(),
        requested_permissions: binding.requested_permissions,
        granted_permissions: binding.granted_permissions,
        workspace_id: binding.workspace_id.to_string(),
        lifecycle_kind: binding.lifecycle.as_str().to_string(),
        graph_fingerprint: binding.graph_fingerprint,
        provenance: FrontendContributionProvenanceResponse {
            module_id: binding.provenance.module_id().as_str().to_string(),
            module_version: binding.provenance.module_version().as_str().to_string(),
            module_kind: binding.provenance.module_kind().as_str().to_string(),
        },
        disable_reason: binding
            .disable_reason
            .map(|reason| reason.as_str().to_string()),
    })
}

#[utoipa::path(
    get,
    path = "/api/console/frontend-blocks",
    responses(
        (status = 200, body = [FrontendBlockCatalogResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontend_blocks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FrontendBlockCatalogResponse>>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let output: FrontendBlocksOutput = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.frontend-blocks.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        FrontendBlocksInput,
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[derive(Clone)]
pub(crate) struct FrontendBlockDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) api_node_id: String,
    pub(crate) graph: Arc<plugin_framework::extension_bus::EffectiveExtensionGraph>,
}

pub(crate) struct FrontendBlocksInput;
impl InterfaceContract for FrontendBlocksInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("FrontendBlocksInput"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("FrontendBlocksInput".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "console-frontend-blocks-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct FrontendBlocksOutput(Vec<FrontendBlockCatalogResponse>);
impl InterfaceContract for FrontendBlocksOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("contribution_code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("runtime",mp::object_schema(&[("byte_count",mp::count_schema())])), ("entry",mp::object_schema(&[("byte_count",mp::count_schema())])), ("code_template",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("code_template_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("code_template_language",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("isolated_entry_asset",serde_json::json!({"anyOf": [mp::object_schema(&[("media_type",mp::text_schema()), ("sha256",mp::object_schema(&[("byte_count",mp::count_schema())])), ("integrity",mp::object_schema(&[("byte_count",mp::count_schema())]))]), {"type":"null"}]})), ("context_contract",mp::object_schema(&[("primitives",mp::object_schema(&[("item_count",mp::count_schema())])), ("input_schema",mp::json_summary_schema())])), ("permissions",mp::object_schema(&[("network",mp::object_schema(&[("byte_count",mp::count_schema())])), ("storage",mp::object_schema(&[("byte_count",mp::count_schema())]))])), ("ui_capabilities",mp::object_schema(&[("item_count",mp::count_schema())])), ("frontend_contribution_id",mp::text_schema()), ("frontend_block_id",mp::text_schema()), ("frontend_block_version",mp::text_schema()), ("runtime_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("execution_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("isolation_requirement",mp::object_schema(&[("byte_count",mp::count_schema())])), ("requested_permissions",mp::object_schema(&[("item_count",mp::count_schema())])), ("granted_permissions",mp::object_schema(&[("item_count",mp::count_schema())])), ("workspace_id",mp::text_schema()), ("lifecycle_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("graph_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provenance",mp::object_schema(&[("module_id",mp::text_schema()), ("module_version",mp::text_schema()), ("module_kind",mp::object_schema(&[("byte_count",mp::count_schema())]))])), ("disable_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("0", {
            if (&(self).0).len() > 32 {
                return None;
            }
            serde_json::Value::Array(
                (&(self).0)
                    .iter()
                    .map(|item| {
                        Some(mp::object_value(&[
                            ("installation_id", mp::text(&(item).installation_id)?),
                            ("provider_code", mp::text(&(item).provider_code)?),
                            ("plugin_id", mp::text(&(item).plugin_id)?),
                            ("plugin_version", mp::text(&(item).plugin_version)?),
                            ("contribution_code", mp::text(&(item).contribution_code)?),
                            (
                                "title",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).title).len()),
                                )]),
                            ),
                            (
                                "runtime",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).runtime).len()),
                                )]),
                            ),
                            (
                                "entry",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).entry).len()),
                                )]),
                            ),
                            (
                                "code_template",
                                match (&(item).code_template).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "code_template_version",
                                match (&(item).code_template_version).as_ref() {
                                    Some(item) => mp::text(item)?,
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "code_template_language",
                                match (&(item).code_template_language).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "isolated_entry_asset",
                                match (&(item).isolated_entry_asset).as_ref() {
                                    Some(item) => mp::object_value(&[
                                        ("media_type", mp::text(&(item).media_type)?),
                                        (
                                            "sha256",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).sha256).len()),
                                            )]),
                                        ),
                                        (
                                            "integrity",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).integrity).len()),
                                            )]),
                                        ),
                                    ]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "context_contract",
                                mp::object_value(&[
                                    (
                                        "primitives",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(&(item).context_contract)
                                                .primitives)
                                                .len()),
                                        )]),
                                    ),
                                    (
                                        "input_schema",
                                        mp::json_summary(&(&(item).context_contract).input_schema),
                                    ),
                                ]),
                            ),
                            (
                                "permissions",
                                mp::object_value(&[
                                    (
                                        "network",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(&(item).permissions).network).len()
                                            ),
                                        )]),
                                    ),
                                    (
                                        "storage",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(&(item).permissions).storage).len()
                                            ),
                                        )]),
                                    ),
                                ]),
                            ),
                            (
                                "ui_capabilities",
                                mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((&(item).ui_capabilities).len()),
                                )]),
                            ),
                            (
                                "frontend_contribution_id",
                                mp::text(&(item).frontend_contribution_id)?,
                            ),
                            ("frontend_block_id", mp::text(&(item).frontend_block_id)?),
                            (
                                "frontend_block_version",
                                mp::text(&(item).frontend_block_version)?,
                            ),
                            (
                                "runtime_kind",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).runtime_kind).len()),
                                )]),
                            ),
                            (
                                "execution_kind",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).execution_kind).len()),
                                )]),
                            ),
                            (
                                "isolation_requirement",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).isolation_requirement).len()),
                                )]),
                            ),
                            (
                                "requested_permissions",
                                mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((&(item).requested_permissions).len()),
                                )]),
                            ),
                            (
                                "granted_permissions",
                                mp::object_value(&[(
                                    "item_count",
                                    serde_json::json!((&(item).granted_permissions).len()),
                                )]),
                            ),
                            ("workspace_id", mp::text(&(item).workspace_id)?),
                            (
                                "lifecycle_kind",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).lifecycle_kind).len()),
                                )]),
                            ),
                            (
                                "graph_fingerprint",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).graph_fingerprint).len()),
                                )]),
                            ),
                            (
                                "provenance",
                                mp::object_value(&[
                                    ("module_id", mp::text(&(&(item).provenance).module_id)?),
                                    (
                                        "module_version",
                                        mp::text(&(&(item).provenance).module_version)?,
                                    ),
                                    (
                                        "module_kind",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(&(item).provenance).module_kind).len()
                                            ),
                                        )]),
                                    ),
                                ]),
                            ),
                            (
                                "disable_reason",
                                match (&(item).disable_reason).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                        ]))
                    })
                    .collect::<Option<Vec<_>>>()?,
            )
        })]))
    }

    const CONTRACT_ID: &'static str = "console-frontend-blocks-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct FrontendBlocksAdapter(FrontendBlockDependencies);

impl
    crate::routes::console_interface::ConsoleInterfacePort<
        FrontendBlocksInput,
        FrontendBlocksOutput,
    > for FrontendBlocksAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        _input: FrontendBlocksInput,
    ) -> crate::routes::console_interface::ConsoleInterfaceFuture<'a, FrontendBlocksOutput> {
        Box::pin(async move {
            let entries = FrontendBlockCatalogService::new(
                self.0.store.for_actor(principal.actor().clone()),
                self.0.api_node_id.clone(),
                Arc::clone(&self.0.graph),
            )
            .map_err(ApiError)
            .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?
            .list_frontend_blocks(ListFrontendBlockCatalogQuery {
                actor_user_id: principal.actor().user_id,
            })
            .await
            .map_err(ApiError)
            .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            let output = entries
                .entries
                .into_iter()
                .map(to_response)
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            Ok(FrontendBlocksOutput(output))
        })
    }
}

pub(crate) fn compile_registry(
    dependencies: FrontendBlockDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    crate::routes::console_interface::compile_registry(
        "api-server.console-frontend-blocks",
        "graph:console-frontend-blocks-v1",
        &[
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "frontend_blocks.view",
                binding_id: "http.console.frontend-blocks.get.v1",
                method: "GET",
                path: "/api/console/frontend-blocks",
                mutating: false,
            },
        ],
        Arc::new(FrontendBlocksAdapter(dependencies)),
    )
}
