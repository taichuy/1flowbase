use crate::openapi_interface::json_contract::{JsonApiInput, JsonApiOutput};
use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    Router,
};
use control_plane::{
    errors::ControlPlaneError,
    frontend_block_catalog::{
        custom_blocks::{resolve_custom_blocks, CustomFrontendBlockService},
        FrontendBlockCatalogService, FrontendBlockTypeDeclarationEntry,
        FrontendContributionBinding, ListFrontendBlockCatalogQuery,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::Serialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_interface::{
            self, ConsoleInterfaceFuture, ConsoleInterfacePort, ConsoleInterfaceTargetError,
        },
        console_route_assembly::{console_get, ConsoleRouteAssembly},
    },
};

const FRONTEND_BLOCKS_CACHE_CONTROL: &str = "private, no-cache";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct FrontendBlockRuntimeCodeModuleResponse {
    pub source: String,
    pub version: String,
    pub exports: Vec<String>,
    #[schema(value_type = String)]
    pub binding: domain::FrontendModuleBinding,
    #[schema(value_type = Vec<Object>)]
    pub assets: Vec<domain::FrontendModuleAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct FrontendBlockTypeDeclarationResponse {
    pub installation_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub source: String,
    pub version: String,
    #[schema(value_type = String)]
    pub binding: domain::FrontendModuleBinding,
    pub type_declarations: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontendBlockCatalogResponse {
    pub custom_block_id: Option<String>,
    pub registration_targets: Vec<String>,
    pub installation_id: String,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
    pub title: String,
    pub runtime: String,
    pub entry: String,
    pub code_template: Option<String>,
    pub code_modules: Vec<FrontendBlockRuntimeCodeModuleResponse>,
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

    ConsoleRouteAssembly::new()
        .route(
            "/frontend-blocks",
            console_get(
                list_frontend_blocks,
                ConsoleOperation("frontend_blocks.view".to_string()),
            ),
        )
        .route(
            "/frontend-blocks/type-declarations",
            console_get(
                list_frontend_block_type_declarations,
                ConsoleOperation("frontend_blocks.type_declarations.view".to_string()),
            ),
        )
}

test_support_item! { pub(crate), []
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
    let registration_targets = entry.registration_targets();
    let code_modules = entry
        .code_modules
        .into_iter()
        .map(|module| FrontendBlockRuntimeCodeModuleResponse {
            source: module.source,
            version: module.version,
            exports: module.exports,
            binding: module.binding,
            assets: module.assets,
        })
        .collect();
    Ok(FrontendBlockCatalogResponse {
        custom_block_id: None,
        registration_targets,
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        contribution_code: entry.contribution_code,
        title: entry.title,
        runtime: entry.runtime,
        entry: entry.entry,
        code_template: entry.code_template,
        code_modules,
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
}

fn to_type_declaration_response(
    entry: FrontendBlockTypeDeclarationEntry,
) -> FrontendBlockTypeDeclarationResponse {
    FrontendBlockTypeDeclarationResponse {
        installation_id: entry.installation_id.to_string(),
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        source: entry.source,
        version: entry.version,
        binding: entry.binding,
        type_declarations: entry.type_declarations,
    }
}

fn extract_if_none_match(headers: &[HeaderValue]) -> Option<String> {
    let values = headers
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.join(","))
}

fn if_none_match_matches(if_none_match: Option<&str>, etag: &str) -> bool {
    let Some(header_value) = if_none_match else {
        return false;
    };
    header_value.split(',').any(|candidate| {
        let trimmed = candidate.trim();
        if trimmed == "*" {
            return true;
        }
        let normalized = trimmed.strip_prefix("W/").unwrap_or(trimmed).trim();
        normalized == etag
    })
}

fn conditional_json_response<T: Serialize>(
    if_none_match: Option<&str>,
    data: T,
) -> Result<Response<Body>, ApiError> {
    let body_bytes = serde_json::to_vec(&ApiSuccess::new(data))?;
    let etag = format!(
        "\"{:x}\"",
        base16ct::HexDisplay(&Sha256::digest(&body_bytes))
    );
    let not_modified = if_none_match_matches(if_none_match, &etag);
    let mut response = if not_modified {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NOT_MODIFIED;
        response
    } else {
        let mut response = Response::new(Body::from(body_bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(FRONTEND_BLOCKS_CACHE_CONTROL),
    );
    headers.insert(header::ETAG, HeaderValue::from_str(&etag)?);
    Ok(response)
}

#[utoipa::path(
    get,
    path = "/api/console/frontend-blocks",
    summary = "List runtime frontend block contributions",
    description = "Returns verified, workspace-authorized frontend block contributions and runtime module metadata without inline TypeScript declaration sources.",
    params(FrontendBlocksInput),
    responses(
        (status = 200, body = [FrontendBlockCatalogResponse]),
        (status = 304, description = "Frontend block catalog not modified"),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontend_blocks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontendBlocksInput>,
) -> Result<Response<Body>, ApiError> {
    invoke_frontend_blocks(state, headers, FrontendBlocksRequest::Catalog(query)).await
}

#[utoipa::path(
    get,
    path = "/api/console/frontend-blocks/type-declarations",
    summary = "List frontend block module type declarations",
    description = "Returns deduplicated TypeScript declaration sources for active, workspace-authorized frontend block modules on demand for code editors.",
    responses(
        (status = 200, body = [FrontendBlockTypeDeclarationResponse]),
        (status = 304, description = "Frontend block type declarations not modified"),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontend_block_type_declarations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Response<Body>, ApiError> {
    invoke_frontend_blocks(state, headers, FrontendBlocksRequest::TypeDeclarations).await
}

async fn invoke_frontend_blocks(
    state: Arc<ApiState>,
    headers: HeaderMap,
    request: FrontendBlocksRequest,
) -> Result<Response<Body>, ApiError> {
    let catalog = matches!(request, FrontendBlocksRequest::Catalog(_));
    let binding_id = if catalog {
        "http.console.frontend-blocks.get.v1"
    } else {
        "http.console.frontend-blocks.type-declarations.get.v1"
    };
    // Retain only the conditional headers when transferring the request headers to authentication.
    let conditional_headers: Vec<_> = headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .cloned()
        .collect();
    // Authentication and the per-operation ACL must run even for a matching ETag.
    let output = console_interface::invoke(
        Arc::clone(&state),
        binding_id,
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        request,
    )
    .await?;
    let if_none_match = extract_if_none_match(&conditional_headers);
    match (catalog, output) {
        (true, FrontendBlocksOutput::Catalog(items)) => {
            conditional_json_response(if_none_match.as_deref(), items)
        }
        (false, FrontendBlocksOutput::TypeDeclarations(items)) => {
            conditional_json_response(if_none_match.as_deref(), items)
        }
        (true, _) => {
            Err(anyhow::anyhow!("unexpected frontend blocks catalog output variant").into())
        }
        (false, _) => Err(anyhow::anyhow!(
            "unexpected frontend block type declarations output variant"
        )
        .into()),
    }
}

pub use api_host_kernel::frontend_block_dependencies::FrontendBlockDependencies;

#[derive(Default, serde::Serialize, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FrontendBlocksInput {
    /// Filter by an explicitly registered target: block, component or code_template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_target: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub(crate) enum FrontendBlocksRequest {
    #[serde(serialize_with = "crate::openapi_interface::json_contract::query")]
    Catalog(FrontendBlocksInput),
    TypeDeclarations,
}
impl JsonApiInput for FrontendBlocksRequest {}

impl InterfaceContract for FrontendBlocksRequest {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("Catalog")),
                (
                    "registration_target",
                    serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]}),
                ),
            ]),
            mp::object_schema([("variant", mp::tag_schema("TypeDeclarations"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog(query) => mp::object_value([
                ("variant", serde_json::json!("Catalog")),
                (
                    "registration_target",
                    match &query.registration_target {
                        Some(value) => mp::text(value)?,
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::TypeDeclarations => {
                mp::object_value([("variant", serde_json::json!("TypeDeclarations"))])
            }
        })
    }
    const CONTRACT_ID: &'static str = "console-frontend-blocks-input";
    const CONTRACT_VERSION: &'static str = "3";
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub(crate) enum FrontendBlocksOutput {
    Catalog(Vec<FrontendBlockCatalogResponse>),
    TypeDeclarations(Vec<FrontendBlockTypeDeclarationResponse>),
}
impl JsonApiOutput for FrontendBlocksOutput {}

impl InterfaceContract for FrontendBlocksOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("Catalog")),
                (
                    "items",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("registration_targets",mp::object_schema([("item_count",mp::count_schema())])), ("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("contribution_code",mp::text_schema()), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("runtime",mp::object_schema([("byte_count",mp::count_schema())])), ("entry",mp::object_schema([("byte_count",mp::count_schema())])), ("code_template",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("code_template_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("code_template_language",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("isolated_entry_asset",serde_json::json!({"anyOf": [mp::object_schema([("media_type",mp::text_schema()), ("sha256",mp::object_schema([("byte_count",mp::count_schema())])), ("integrity",mp::object_schema([("byte_count",mp::count_schema())]))]), {"type":"null"}]})), ("context_contract",mp::object_schema([("primitives",mp::object_schema([("item_count",mp::count_schema())])), ("input_schema",mp::json_summary_schema())])), ("permissions",mp::object_schema([("network",mp::object_schema([("byte_count",mp::count_schema())])), ("storage",mp::object_schema([("byte_count",mp::count_schema())]))])), ("ui_capabilities",mp::object_schema([("item_count",mp::count_schema())])), ("frontend_contribution_id",mp::text_schema()), ("frontend_block_id",mp::text_schema()), ("frontend_block_version",mp::text_schema()), ("runtime_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("execution_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("isolation_requirement",mp::object_schema([("byte_count",mp::count_schema())])), ("requested_permissions",mp::object_schema([("item_count",mp::count_schema())])), ("granted_permissions",mp::object_schema([("item_count",mp::count_schema())])), ("workspace_id",mp::text_schema()), ("lifecycle_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("graph_fingerprint",mp::object_schema([("byte_count",mp::count_schema())])), ("provenance",mp::object_schema([("module_id",mp::text_schema()), ("module_version",mp::text_schema()), ("module_kind",mp::object_schema([("byte_count",mp::count_schema())]))])), ("disable_reason",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("TypeDeclarations")),
                ("item_count", mp::count_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog(items) => {
                if items.len() > 32 {
                    return None;
                }
                mp::object_value([
                    ("variant", serde_json::json!("Catalog")),
                    (
                        "items",
                        serde_json::Value::Array(
                            items
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value([
                                        (
                                            "registration_targets",
                                            mp::object_value([(
                                                "item_count",
                                                serde_json::json!((item)
                                                    .registration_targets
                                                    .len()),
                                            )]),
                                        ),
                                        ("installation_id", mp::text(&(item).installation_id)?),
                                        ("provider_code", mp::text(&(item).provider_code)?),
                                        ("plugin_id", mp::text(&(item).plugin_id)?),
                                        ("plugin_version", mp::text(&(item).plugin_version)?),
                                        ("contribution_code", mp::text(&(item).contribution_code)?),
                                        (
                                            "title",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).title.len()),
                                            )]),
                                        ),
                                        (
                                            "runtime",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).runtime.len()),
                                            )]),
                                        ),
                                        (
                                            "entry",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).entry.len()),
                                            )]),
                                        ),
                                        (
                                            "code_template",
                                            match (item).code_template.as_ref() {
                                                Some(item) => mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "code_template_version",
                                            match (item).code_template_version.as_ref() {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "code_template_language",
                                            match (item).code_template_language.as_ref() {
                                                Some(item) => mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "isolated_entry_asset",
                                            match (item).isolated_entry_asset.as_ref() {
                                                Some(item) => mp::object_value([
                                                    ("media_type", mp::text(&(item).media_type)?),
                                                    (
                                                        "sha256",
                                                        mp::object_value([(
                                                            "byte_count",
                                                            serde_json::json!((item).sha256.len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "integrity",
                                                        mp::object_value([(
                                                            "byte_count",
                                                            serde_json::json!((item)
                                                                .integrity
                                                                .len()),
                                                        )]),
                                                    ),
                                                ]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "context_contract",
                                            mp::object_value([
                                                (
                                                    "primitives",
                                                    mp::object_value([(
                                                        "item_count",
                                                        serde_json::json!((item)
                                                            .context_contract
                                                            .primitives
                                                            .len()),
                                                    )]),
                                                ),
                                                (
                                                    "input_schema",
                                                    mp::json_summary(
                                                        &(item).context_contract.input_schema,
                                                    ),
                                                ),
                                            ]),
                                        ),
                                        (
                                            "permissions",
                                            mp::object_value([
                                                (
                                                    "network",
                                                    mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item)
                                                            .permissions
                                                            .network
                                                            .len()),
                                                    )]),
                                                ),
                                                (
                                                    "storage",
                                                    mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item)
                                                            .permissions
                                                            .storage
                                                            .len()),
                                                    )]),
                                                ),
                                            ]),
                                        ),
                                        (
                                            "ui_capabilities",
                                            mp::object_value([(
                                                "item_count",
                                                serde_json::json!((item).ui_capabilities.len()),
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
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).runtime_kind.len()),
                                            )]),
                                        ),
                                        (
                                            "execution_kind",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).execution_kind.len()),
                                            )]),
                                        ),
                                        (
                                            "isolation_requirement",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item)
                                                    .isolation_requirement
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "requested_permissions",
                                            mp::object_value([(
                                                "item_count",
                                                serde_json::json!((item)
                                                    .requested_permissions
                                                    .len()),
                                            )]),
                                        ),
                                        (
                                            "granted_permissions",
                                            mp::object_value([(
                                                "item_count",
                                                serde_json::json!((item).granted_permissions.len()),
                                            )]),
                                        ),
                                        ("workspace_id", mp::text(&(item).workspace_id)?),
                                        (
                                            "lifecycle_kind",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).lifecycle_kind.len()),
                                            )]),
                                        ),
                                        (
                                            "graph_fingerprint",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).graph_fingerprint.len()),
                                            )]),
                                        ),
                                        (
                                            "provenance",
                                            mp::object_value([
                                                (
                                                    "module_id",
                                                    mp::text(&(item).provenance.module_id)?,
                                                ),
                                                (
                                                    "module_version",
                                                    mp::text(&(item).provenance.module_version)?,
                                                ),
                                                (
                                                    "module_kind",
                                                    mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item)
                                                            .provenance
                                                            .module_kind
                                                            .len()),
                                                    )]),
                                                ),
                                            ]),
                                        ),
                                        (
                                            "disable_reason",
                                            match (item).disable_reason.as_ref() {
                                                Some(item) => mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((item).len()),
                                                )]),
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        ),
                    ),
                ])
            }
            Self::TypeDeclarations(items) => mp::object_value([
                ("variant", serde_json::json!("TypeDeclarations")),
                ("item_count", serde_json::json!(items.len())),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-frontend-blocks-output";
    const CONTRACT_VERSION: &'static str = "2";
}

struct FrontendBlocksAdapter(FrontendBlockDependencies);

impl FrontendBlocksAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontendBlocksRequest,
    ) -> Result<FrontendBlocksOutput, ApiError> {
        if matches!(&input, FrontendBlocksRequest::Catalog(query)
            if query.registration_target.as_deref().is_some_and(|target| {
                !matches!(target, "block" | "component" | "code_template")
            })
        ) {
            return Err(
                ControlPlaneError::InvalidInput("invalid_frontend_registration_target").into(),
            );
        }

        let actor = principal.actor();
        let store = self.0.store.for_actor(actor.clone());
        let service = FrontendBlockCatalogService::new(
            store.clone(),
            self.0.api_node_id.clone(),
            Arc::clone(&self.0.graph),
        )?;
        let query = ListFrontendBlockCatalogQuery {
            actor_user_id: actor.user_id,
        };
        match input {
            FrontendBlocksRequest::Catalog(input) => {
                let entries = service.list_frontend_blocks(query).await?;
                let definitions = CustomFrontendBlockService(store)
                    .list(actor.current_workspace_id)
                    .await?;
                let custom = resolve_custom_blocks(&entries.entries, definitions);
                let mut output = entries
                    .entries
                    .into_iter()
                    .filter(|binding| {
                        input.registration_target.as_ref().is_none_or(|target| {
                            binding
                                .catalog_entry
                                .registration_targets()
                                .contains(target)
                        })
                    })
                    .map(to_response)
                    .collect::<Result<Vec<_>, _>>()?;
                for (definition, binding) in custom {
                    if input
                        .registration_target
                        .as_deref()
                        .is_some_and(|target| target != "block")
                    {
                        continue;
                    }
                    let mut response = to_response(binding)?;
                    response.custom_block_id = Some(definition.id.to_string());
                    response.registration_targets = vec!["block".to_owned()];
                    output.push(response);
                }
                Ok(FrontendBlocksOutput::Catalog(output))
            }
            FrontendBlocksRequest::TypeDeclarations => Ok(FrontendBlocksOutput::TypeDeclarations(
                service
                    .list_frontend_block_type_declarations(query)
                    .await?
                    .entries
                    .into_iter()
                    .map(to_type_declaration_response)
                    .collect(),
            )),
        }
    }
}

impl ConsoleInterfacePort<FrontendBlocksRequest, FrontendBlocksOutput> for FrontendBlocksAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontendBlocksRequest,
    ) -> ConsoleInterfaceFuture<'a, FrontendBlocksOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub fn compile_registry(
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
            crate::routes::console_interface::ConsoleInterfaceDeclaration {
                interface_id: "frontend_blocks.type_declarations.view",
                binding_id: "http.console.frontend-blocks.type-declarations.get.v1",
                method: "GET",
                path: "/api/console/frontend-blocks/type-declarations",
                mutating: false,
            },
        ],
        Arc::new(FrontendBlocksAdapter(dependencies)),
    )
}
