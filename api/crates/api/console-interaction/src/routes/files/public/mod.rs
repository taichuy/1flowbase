use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::Response,
};
use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, PublicPrincipal,
};
use serde::Deserialize;

use crate::{app_state::ApiState, error_response::ApiError};

pub(crate) mod interface;
use interface::*;

#[derive(Deserialize)]
pub struct TemporaryFileQuery {
    token: String,
    #[serde(default)]
    preview: bool,
}

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        "/api/public/files/content",
        crate::external_route_assembly::get(read_temporary_file),
    )
}

struct TemporaryFileAdapter {
    service: control_plane::file_management::FileAccessService<
        storage_durable_postgres::MainDurableStore,
    >,
}

impl TemporaryFilePort for TemporaryFileAdapter {
    fn download(&self, input: TemporaryFileInput) -> TemporaryFileFuture<'_> {
        Box::pin(async move {
            self.service
                .read_temporary_with_preview(input.0, input.1)
                .await
                .map(TemporaryFileOutput)
                .map_err(|error| TemporaryFileTargetError(super::map_runtime_error(error)))
        })
    }
}

pub(crate) fn compile_registry(
    state: &ApiState,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface::compile_registry(Arc::new(TemporaryFileAdapter {
        service: control_plane::file_management::FileAccessService::new(
            state.store.clone(),
            state.file_storage_registry.clone(),
            state.runtime_engine.clone(),
            state.infrastructure.ephemeral_kv_store(),
        ),
    }))
}

#[utoipa::path(get, path = "/api/public/files/content", params(("token" = String, Query, description = "Temporary file capability"), ("preview" = Option<bool>, Query, description = "Serve a supported image inline")), responses((status = 200, body = inline(crate::openapi::OpenApiBinaryBody), content_type = "application/octet-stream"), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn read_temporary_file(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<TemporaryFileQuery>,
) -> Result<Response, ApiError> {
    let key = control_plane::file_management::FileAccessKey::from_token(&query.token)
        .map_err(super::map_runtime_error)?;
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot unavailable"))?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry unavailable"))?
        .snapshot();
    let binding = interface_runtime::BindingId::new(BINDING_ID).expect("static binding");
    let authenticated = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            snapshot.clone(),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let outcome = InterfaceInvocationKernel::new(Arc::new(TemporaryFileAuthorization))
        .invoke::<TemporaryFileInput, TemporaryFileOutput, TemporaryFileTargetError>(
            snapshot,
            authenticated.into_envelope(TemporaryFileInput(key, query.preview)),
        )
        .await
        .map_err(|failure| match failure.into_error() {
            InterfaceInvocationError::TargetFailed(error) => error
                .into_source::<TemporaryFileTargetError>()
                .map(|error| error.0)
                .unwrap_or_else(|| anyhow::anyhow!("temporary file access failed").into()),
            error => anyhow::anyhow!(error.to_string()).into(),
        })?;
    let _receipt = outcome.receipt().clone().projected();
    super::content_response(outcome.into_value().0)
}
