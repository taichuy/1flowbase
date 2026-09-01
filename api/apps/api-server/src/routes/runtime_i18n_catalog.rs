use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
};
use control_plane::{errors::ControlPlaneError, i18n_catalog::RuntimeI18nCatalogService};
use domain::CatalogLocale;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

const CATALOG_CACHE_CONTROL: &str = "no-cache";

#[derive(Debug, Deserialize)]
pub struct RuntimeI18nCatalogQuery {
    pub locale: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeI18nCatalogResponse {
    pub catalog_revision: i64,
    pub locale: String,
    pub digest: String,
    pub messages: std::collections::BTreeMap<String, String>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    ConsoleRouteAssembly::new().route(
        "/i18n/catalog",
        console_get(
            get_runtime_i18n_catalog,
            ConsoleOperation("i18n.catalog.view".into()),
        ),
    )
}

pub(crate) struct RuntimeI18nInput {
    locale: String,
    if_none_match: Option<String>,
}
impl InterfaceContract for RuntimeI18nInput {
    const CONTRACT_ID: &'static str = "console-runtime-i18n-input";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct RuntimeI18nOutput {
    payload: Option<RuntimeI18nCatalogResponse>,
    etag: String,
}
impl InterfaceContract for RuntimeI18nOutput {
    const CONTRACT_ID: &'static str = "console-runtime-i18n-output";
    const CONTRACT_VERSION: &'static str = "1";
}
struct RuntimeI18nAdapter {
    store: storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: uuid::Uuid,
}
impl ConsoleInterfacePort<RuntimeI18nInput, RuntimeI18nOutput> for RuntimeI18nAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: RuntimeI18nInput,
    ) -> ConsoleInterfaceFuture<'a, RuntimeI18nOutput> {
        Box::pin(async move {
            let workspace_id = principal.actor().current_workspace_id;
            if workspace_id != self.bootstrap_workspace_id {
                return Err(ConsoleInterfaceTargetError(
                    ControlPlaneError::PermissionDenied("root_i18n_catalog_workspace").into(),
                ));
            }
            let locale = parse_locale(input.locale).map_err(ConsoleInterfaceTargetError)?;
            let manifest =
                RuntimeI18nCatalogService::new(self.store.clone(), self.bootstrap_workspace_id)
                    .manifest(workspace_id, &locale)
                    .await
                    .map_err(ApiError::from)
                    .map_err(ConsoleInterfaceTargetError)?;
            let etag = format!("\"{}\"", manifest.digest.as_str());
            let not_modified = input
                .if_none_match
                .as_deref()
                .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag));
            Ok(RuntimeI18nOutput {
                payload: (!not_modified).then(|| RuntimeI18nCatalogResponse {
                    catalog_revision: manifest.revision.value(),
                    locale: manifest.bundle.locale,
                    digest: manifest.digest.as_str().to_owned(),
                    messages: manifest.bundle.messages,
                }),
                etag,
            })
        })
    }
}

pub(crate) fn compile_registry(
    store: storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: uuid::Uuid,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
        interface_id: "i18n.catalog.view",
        binding_id: "http.console.i18n.catalog.get.v1",
        method: "GET",
        path: "/api/console/i18n/catalog",
        mutating: false,
    }];
    console_interface::compile_registry(
        "api-server.console-runtime-i18n",
        "graph:console-runtime-i18n-v1",
        DECLARATIONS,
        Arc::new(RuntimeI18nAdapter {
            store,
            bootstrap_workspace_id,
        }),
    )
}

fn parse_locale(value: String) -> Result<CatalogLocale, ApiError> {
    CatalogLocale::new(value)
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_locale").into())
}

#[utoipa::path(
    get, path = "/api/console/i18n/catalog",
    summary = "Get the resolved runtime i18n catalog",
    description = "Returns one content-addressed, globally keyed catalog for the authenticated root workspace and requested locale.",
    params(("locale" = String, Query, description = "Requested catalog locale")),
    responses((status = 200, body = RuntimeI18nCatalogResponse), (status = 304, description = "Catalog not modified"), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_runtime_i18n_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RuntimeI18nCatalogQuery>,
) -> Result<Response<Body>, ApiError> {
    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let snapshot_state = Arc::clone(&state);
    let output: RuntimeI18nOutput = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.i18n.catalog.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        RuntimeI18nInput {
            locale: query.locale,
            if_none_match,
        },
    )
    .await?;
    let mut response = Response::builder().status(if output.payload.is_none() {
        StatusCode::NOT_MODIFIED
    } else {
        StatusCode::OK
    });
    let response_headers = response
        .headers_mut()
        .ok_or_else(|| anyhow::anyhow!("runtime i18n response builder has no headers"))?;
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CATALOG_CACHE_CONTROL),
    );
    response_headers.insert(header::ETAG, HeaderValue::from_str(&output.etag)?);
    let Some(payload) = output.payload else {
        return Ok(response.body(Body::empty())?);
    };
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(response.body(Body::from(serde_json::to_vec(&payload)?))?)
}
