use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::Response,
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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "query",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Get")), ("0", mp::text_schema())]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Asset")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "query",
                            match (&(_field_0).query).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_0).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_0).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Get(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Get".to_owned())),
                ("0", serde_json::Value::String((_field_0).to_string())),
            ]),
            Self::Asset(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Asset".to_owned())),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
        })
    }

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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Page")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("scope_id",mp::text_schema()), ("component_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("import_code",mp::text_schema()), ("source_code",mp::text_schema()), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("upstream",mp::object_schema(&[("identity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema())])), ("version",mp::text_schema()), ("keywords",mp::object_schema(&[("item_count",mp::count_schema())])), ("catalog_updated_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source_locator",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("offset", serde_json::json!({"type":"integer"})),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                        (
                            "next_offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Component")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("scope_id", mp::text_schema()),
                        ("component_code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("import_code", mp::text_schema()),
                        ("source_code", mp::text_schema()),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "group",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "upstream",
                            mp::object_schema(&[
                                (
                                    "identity",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("version", mp::text_schema()),
                            ]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "keywords",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "catalog_updated_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "source_locator",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "source_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Asset"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Page(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Page".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("scope_id", mp::text(&(item).scope_id)?),
                                            ("component_code", mp::text(&(item).component_code)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).description).len()),
                                                )]),
                                            ),
                                            ("import_code", mp::text(&(item).import_code)?),
                                            ("source_code", mp::text(&(item).source_code)?),
                                            (
                                                "source",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source).len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group).len()),
                                                )]),
                                            ),
                                            (
                                                "upstream",
                                                mp::object_value(&[
                                                    (
                                                        "identity",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!(
                                                                (&(&(item).upstream).identity)
                                                                    .len()
                                                            ),
                                                        )]),
                                                    ),
                                                    (
                                                        "version",
                                                        mp::text(&(&(item).upstream).version)?,
                                                    ),
                                                ]),
                                            ),
                                            ("version", mp::text(&(item).version)?),
                                            (
                                                "keywords",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).keywords).len()),
                                                )]),
                                            ),
                                            (
                                                "catalog_updated_at",
                                                match (&(item).catalog_updated_at).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_locator",
                                                match (&(item).source_locator).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_checksum",
                                                match (&(item).source_checksum).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                            ("updated_at", mp::text(&(item).updated_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total", serde_json::json!(*(&(_field_0).total))),
                        ("offset", serde_json::json!(*(&(_field_0).offset))),
                        ("limit", serde_json::json!(*(&(_field_0).limit))),
                        ("has_more", serde_json::Value::Bool(*(&(_field_0).has_more))),
                        (
                            "next_offset",
                            match (&(_field_0).next_offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Component(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Component".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("scope_id", mp::text(&(_field_0).scope_id)?),
                        ("component_code", mp::text(&(_field_0).component_code)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "description",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).description).len()),
                            )]),
                        ),
                        ("import_code", mp::text(&(_field_0).import_code)?),
                        ("source_code", mp::text(&(_field_0).source_code)?),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source).len()),
                            )]),
                        ),
                        (
                            "group",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).group).len()),
                            )]),
                        ),
                        (
                            "upstream",
                            mp::object_value(&[
                                (
                                    "identity",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).upstream).identity).len()),
                                    )]),
                                ),
                                ("version", mp::text(&(&(_field_0).upstream).version)?),
                            ]),
                        ),
                        ("version", mp::text(&(_field_0).version)?),
                        (
                            "keywords",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).keywords).len()),
                            )]),
                        ),
                        (
                            "catalog_updated_at",
                            match (&(_field_0).catalog_updated_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_locator",
                            match (&(_field_0).source_locator).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_checksum",
                            match (&(_field_0).source_checksum).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::Asset(_) => {
                mp::object_value(&[("variant", serde_json::Value::String("Asset".to_owned()))])
            }
        })
    }

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
