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
    const CONTRACT_ID: &'static str = "console-frontend-blocks-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct FrontendBlocksOutput(Vec<FrontendBlockCatalogResponse>);
impl InterfaceContract for FrontendBlocksOutput {
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
