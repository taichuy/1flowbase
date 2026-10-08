use crate::{app_state::ApiState, error_response::ApiError};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue},
    response::Response,
};
use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, PublicPrincipal,
};
use std::sync::Arc;
pub(crate) mod interface;
use interface::*;

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        "/api/public/client-collectors/:organization/:artifact_id/:version/assets/:asset",
        crate::external_route_assembly::get(download),
    )
}

struct CollectorAssetAdapter {
    store: storage_durable_postgres::MainDurableStore,
    install_root: String,
    node_id: String,
}
impl CollectorAssetPort for CollectorAssetAdapter {
    fn download(&self, input: CollectorAssetInput) -> CollectorAssetFuture<'_> {
        Box::pin(async move {
            let identity = domain::ExtensionInstallationIdentity {
                category: domain::ExtensionCategory::RuntimeExtensions,
                organization: input.organization,
                artifact_id: input.artifact_id,
                version: input.version,
            };
            let download = control_plane::plugin_management::ExtensionInstallationService::new(
                self.store.clone(),
                &self.install_root,
            )
            .download_client_collector(&self.node_id, &identity, &input.asset)
            .await
            .map_err(|error| CollectorAssetTargetError(ApiError(error)))?;
            Ok(CollectorAssetOutput(download))
        })
    }
}
pub(crate) fn compile_registry(
    state: &ApiState,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface::compile_registry(Arc::new(CollectorAssetAdapter {
        store: state.store.clone(),
        install_root: state.provider_install_root.clone(),
        node_id: state.api_node_id.clone(),
    }))
}

#[utoipa::path(get, path = "/api/public/client-collectors/{organization}/{artifact_id}/{version}/assets/{asset}", params(("organization" = String, Path), ("artifact_id" = String, Path), ("version" = String, Path), ("asset" = String, Path)), responses((status = 200, content_type = "application/octet-stream", body = String), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn download(
    State(state): State<Arc<ApiState>>,
    Path((organization, artifact_id, version, asset)): Path<(String, String, String, String)>,
) -> Result<Response, ApiError> {
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
    let outcome = InterfaceInvocationKernel::new(Arc::new(CollectorAssetAuthorization))
        .invoke::<CollectorAssetInput, CollectorAssetOutput, CollectorAssetTargetError>(
            snapshot,
            authenticated.into_envelope(CollectorAssetInput {
                organization,
                artifact_id,
                version,
                asset,
            }),
        )
        .await
        .map_err(|failure| match failure.into_error() {
            InterfaceInvocationError::TargetFailed(error) => error
                .into_source::<CollectorAssetTargetError>()
                .map(|error| error.0)
                .unwrap_or_else(|| anyhow::anyhow!("collector download failed").into()),
            error => anyhow::anyhow!(error.to_string()).into(),
        })?;
    let _receipt = outcome.receipt().clone().projected();
    let download = outcome.into_value().0;
    let stream = asset_stream(download.reader, download.completion, download.size);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(download.size));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", download.name)).map_err(
            |_| control_plane::errors::ControlPlaneError::InvalidInput("client_collector_asset"),
        )?,
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

fn asset_stream(
    reader: tokio::io::DuplexStream,
    completion: tokio::sync::oneshot::Receiver<std::io::Result<()>>,
    size: u64,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures_util::stream::try_unfold(
        (reader, completion, size),
        |(mut reader, completion, remaining)| async move {
            let mut buffer = vec![0u8; 64 * 1024];
            let read = tokio::io::AsyncReadExt::read(&mut reader, &mut buffer).await?;
            if read == 0 {
                completion
                    .await
                    .map_err(|_| std::io::Error::other("collector streaming completion lost"))??;
                if remaining != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "collector asset stream truncated",
                    ));
                }
                return Ok(None);
            }
            let remaining = remaining.checked_sub(read as u64).ok_or_else(|| {
                std::io::Error::other("collector asset stream exceeded declared size")
            })?;
            buffer.truncate(read);
            Ok::<_, std::io::Error>(Some((
                bytes::Bytes::from(buffer),
                (reader, completion, remaining),
            )))
        },
    )
}

#[cfg(test)]
mod _tests;
