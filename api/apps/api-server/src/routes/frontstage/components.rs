use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::Response,
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontend_block_catalog::{FrontendModuleAssetService, GetFrontendModuleAssetQuery},
    ui_management::{ListUiComponentRecordsQuery, UiManagementService},
};
use domain::{UiComponentRecord, UiComponentRecordOrigin};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};

const COMPONENT_PAGE_SIZE: usize = 20;

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageComponentQuery {
    pub query: Option<String>,
    #[param(minimum = 0)]
    pub offset: Option<usize>,
    #[param(minimum = 1, maximum = 20)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageComponentUpstreamResponse {
    pub identity: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageComponentResponse {
    pub id: String,
    pub scope_id: String,
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    #[schema(value_type = String)]
    pub origin: UiComponentRecordOrigin,
    pub source: String,
    pub group: String,
    pub upstream: FrontstageComponentUpstreamResponse,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_updated_at: Option<String>,
    pub source_locator: Option<String>,
    pub source_checksum: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageComponentPageResponse {
    pub items: Vec<FrontstageComponentResponse>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/components",
    params(FrontstageComponentQuery),
    responses(
        (status = 200, body = FrontstageComponentPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_components(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontstageComponentQuery>,
) -> Result<Json<ApiSuccess<FrontstageComponentPageResponse>>, ApiError> {
    let FrontstageComponentsOutput::Page(value) = invoke(
        state,
        headers,
        "http.console.frontstage.components.get.v1",
        FrontstageComponentsInput::List(query),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/components/{component_id}",
    params(("component_id" = Uuid, Path, description = "Persisted component record id")),
    responses(
        (status = 200, body = FrontstageComponentResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(component_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<FrontstageComponentResponse>>, ApiError> {
    let FrontstageComponentsOutput::Component(value) = invoke(
        state,
        headers,
        "http.console.frontstage.component.get.v1",
        FrontstageComponentsInput::Get(component_id),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/component-module-assets/{sha256}",
    params(("sha256" = String, Path, description = "Registered module asset SHA-256")),
    responses(
        (status = 200, description = "Digest-verified module asset with its declared Content-Type", body = Vec<u8>),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody),
        (status = 502, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_component_module_asset(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(sha256): Path<String>,
) -> Result<Response<Body>, ApiError> {
    let FrontstageComponentsOutput::Asset(asset) = invoke(
        state,
        headers,
        "http.console.frontstage.component-asset.get.v1",
        FrontstageComponentsInput::Asset(sha256),
    )
    .await?
    else {
        unreachable!()
    };
    let mut response = Response::new(Body::from(asset.bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.media_type)
            .map_err(|_| ControlPlaneError::InvalidInput("media_type"))?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"sha256-{}\"", asset.sha256))
            .map_err(|_| ControlPlaneError::InvalidInput("sha256"))?,
    );
    Ok(response)
}

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: FrontstageComponentsInput,
) -> Result<FrontstageComponentsOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    crate::routes::console_interface::invoke(
        snapshot_state,
        binding_id,
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        input,
    )
    .await
}

pub(crate) enum FrontstageComponentsInput {
    List(FrontstageComponentQuery),
    Get(Uuid),
    Asset(String),
}
impl InterfaceContract for FrontstageComponentsInput {
    const CONTRACT_ID: &'static str = "console-frontstage-components-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed component output is projected immediately into the frontstage response"
)]
pub(crate) enum FrontstageComponentsOutput {
    Page(FrontstageComponentPageResponse),
    Component(FrontstageComponentResponse),
    Asset(FrontstageComponentAsset),
}
impl InterfaceContract for FrontstageComponentsOutput {
    const CONTRACT_ID: &'static str = "console-frontstage-components-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct FrontstageComponentAsset {
    bytes: Vec<u8>,
    media_type: String,
    sha256: String,
}

#[derive(Clone)]
pub(crate) struct FrontstageComponentsDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) api_node_id: String,
}
struct FrontstageComponentsAdapter(FrontstageComponentsDependencies);

impl
    crate::routes::console_interface::ConsoleInterfacePort<
        FrontstageComponentsInput,
        FrontstageComponentsOutput,
    > for FrontstageComponentsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstageComponentsInput,
    ) -> crate::routes::console_interface::ConsoleInterfaceFuture<'a, FrontstageComponentsOutput>
    {
        Box::pin(async move {
            let actor = principal.actor();
            require_design_permission(actor)
                .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
            let service =
                UiManagementService::new(self.0.store.clone(), self.0.api_node_id.clone());
            match input {
                FrontstageComponentsInput::List(query) => {
                    let page = service
                        .list_component_records_page(ListUiComponentRecordsQuery {
                            query: query.query,
                            offset: query.offset.unwrap_or(0),
                            limit: query
                                .limit
                                .unwrap_or(COMPONENT_PAGE_SIZE)
                                .clamp(1, COMPONENT_PAGE_SIZE),
                        })
                        .await
                        .map_err(ApiError)
                        .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
                    let items = page
                        .items
                        .into_iter()
                        .map(component_response)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
                    Ok(FrontstageComponentsOutput::Page(
                        FrontstageComponentPageResponse {
                            items,
                            total: page.total,
                            offset: page.offset,
                            limit: page.limit,
                            has_more: page.has_more,
                            next_offset: page.next_offset,
                        },
                    ))
                }
                FrontstageComponentsInput::Get(id) => {
                    let record = service
                        .get_component_record(id)
                        .await
                        .map_err(ApiError)
                        .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
                    Ok(FrontstageComponentsOutput::Component(
                        component_response(record).map_err(
                            crate::routes::console_interface::ConsoleInterfaceTargetError,
                        )?,
                    ))
                }
                FrontstageComponentsInput::Asset(sha256) => {
                    let asset = FrontendModuleAssetService::new(
                        self.0.store.clone(),
                        self.0.api_node_id.clone(),
                    )
                    .get_module_asset(GetFrontendModuleAssetQuery {
                        workspace_id: actor.current_workspace_id,
                        sha256,
                    })
                    .await
                    .map_err(ApiError)
                    .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?
                    .ok_or(ControlPlaneError::NotFound(
                        "frontend_component_module_asset",
                    ))
                    .map_err(|error| ApiError(error.into()))
                    .map_err(crate::routes::console_interface::ConsoleInterfaceTargetError)?;
                    Ok(FrontstageComponentsOutput::Asset(
                        FrontstageComponentAsset {
                            bytes: asset.bytes,
                            media_type: asset.media_type,
                            sha256: asset.sha256,
                        },
                    ))
                }
            }
        })
    }
}

pub(crate) fn compile_registry(
    dependencies: FrontstageComponentsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    crate::routes::console_interface::compile_registry(
        "api-server.console-frontstage-components",
        "graph:console-frontstage-components-v1",
        &[
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "frontstage.components.view",
                binding_id: "http.console.frontstage.components.get.v1",
                method: "GET",
                path: "/api/console/frontstage/components",
                mutating: false,
            },
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "frontstage.components.view",
                binding_id: "http.console.frontstage.component.get.v1",
                method: "GET",
                path: "/api/console/frontstage/components/:component_id",
                mutating: false,
            },
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "frontstage.components.view",
                binding_id: "http.console.frontstage.component-asset.get.v1",
                method: "GET",
                path: "/api/console/frontstage/component-module-assets/:sha256",
                mutating: false,
            },
        ],
        Arc::new(FrontstageComponentsAdapter(dependencies)),
    )
}

fn require_design_permission(actor: &domain::ActorContext) -> Result<(), ApiError> {
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    Ok(())
}

fn component_response(value: UiComponentRecord) -> Result<FrontstageComponentResponse, ApiError> {
    use time::format_description::well_known::Rfc3339;
    Ok(FrontstageComponentResponse {
        id: value.id.to_string(),
        scope_id: value.scope_id.to_string(),
        component_code: value.component_code,
        name: value.name,
        description: value.description,
        import_code: value.import_code,
        source_code: value.source_code,
        origin: value.origin,
        source: value.source,
        group: value.group,
        upstream: FrontstageComponentUpstreamResponse {
            identity: value.upstream.identity,
            version: value.upstream.version,
        },
        version: value.version,
        keywords: value.keywords,
        catalog_updated_at: value
            .catalog_updated_at
            .map(|timestamp| timestamp.format(&Rfc3339))
            .transpose()?,
        source_locator: value.source_locator,
        source_checksum: value.source_checksum,
        created_at: value.created_at.format(&Rfc3339)?,
        updated_at: value.updated_at.format(&Rfc3339)?,
    })
}
