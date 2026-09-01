use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::js_dependency::{JsDependencyService, ListWorkspaceJsDependenciesQuery};
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
pub struct JsDependencyPermissionsResponse {
    pub network: String,
    pub filesystem: String,
    pub env: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct JsDependencyCatalogEntryResponse {
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub integrity: String,
    pub permissions: JsDependencyPermissionsResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new().route(
        "/js-dependencies",
        console_get(
            list_js_dependencies,
            ConsoleOperation("js_dependencies.view".to_string()),
        ),
    )
}

fn to_response(entry: domain::JsDependencyRegistryEntry) -> JsDependencyCatalogEntryResponse {
    JsDependencyCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        alias: entry.alias,
        package: entry.package,
        version: entry.version,
        target: entry.target,
        artifact_path: entry.artifact_path,
        integrity: entry.integrity,
        permissions: JsDependencyPermissionsResponse {
            network: entry.permissions.network,
            filesystem: entry.permissions.filesystem,
            env: entry.permissions.env,
        },
    }
}

#[utoipa::path(
    get,
    path = "/api/console/js-dependencies",
    responses(
        (status = 200, body = [JsDependencyCatalogEntryResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_js_dependencies(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<JsDependencyCatalogEntryResponse>>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let output: JsDependenciesOutput = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.js-dependencies.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        JsDependenciesInput,
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

pub(crate) struct JsDependenciesInput;

impl InterfaceContract for JsDependenciesInput {
    const CONTRACT_ID: &'static str = "console-js-dependencies-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct JsDependenciesOutput(Vec<JsDependencyCatalogEntryResponse>);

impl InterfaceContract for JsDependenciesOutput {
    const CONTRACT_ID: &'static str = "console-js-dependencies-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct JsDependenciesAdapter(storage_durable_postgres::MainDurableStore);

impl
    crate::routes::console_interface::ConsoleInterfacePort<
        JsDependenciesInput,
        JsDependenciesOutput,
    > for JsDependenciesAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        _input: JsDependenciesInput,
    ) -> crate::routes::console_interface::ConsoleInterfaceFuture<'a, JsDependenciesOutput> {
        Box::pin(async move {
            let entries = JsDependencyService::new(self.0.for_actor(principal.actor().clone()))
                .list_workspace_js_dependencies(ListWorkspaceJsDependenciesQuery {
                    actor_user_id: principal.actor().user_id,
                })
                .await
                .map_err(ApiError)
                .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            Ok(JsDependenciesOutput(
                entries.entries.into_iter().map(to_response).collect(),
            ))
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
        "api-server.console-js-dependencies",
        "graph:console-js-dependencies-v1",
        &[
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "js_dependencies.view",
                binding_id: "http.console.js-dependencies.get.v1",
                method: "GET",
                path: "/api/console/js-dependencies",
                mutating: false,
            },
        ],
        Arc::new(JsDependenciesAdapter(store)),
    )
}
