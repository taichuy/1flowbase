use std::{
    path::{Component, Path as FsPath},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
};
use control_plane::model_provider::ModelProviderService;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) struct ProviderIconInput(pub(crate) String);
impl InterfaceContract for ProviderIconInput {
    const CONTRACT_ID: &'static str = "console-provider-icon-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderIconOutput {
    pub(crate) content_type: &'static str,
    pub(crate) content: Vec<u8>,
}
impl InterfaceContract for ProviderIconOutput {
    const CONTRACT_ID: &'static str = "console-provider-icon-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ProviderIconAdapter {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    secret_key: String,
    api_node_id: String,
    install_root: String,
}

fn provider_icon_content_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

async fn path_is_file(path: &FsPath) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
}

async fn resolve_provider_icon_path(
    installed_path: &str,
    icon_path: &str,
) -> Result<std::path::PathBuf, ApiError> {
    let icon_relative_path = FsPath::new(icon_path);

    if icon_relative_path.is_absolute()
        || icon_relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("icon").into());
    }

    let installed_root = FsPath::new(installed_path);
    let resolved_path = installed_root.join(icon_relative_path);
    if path_is_file(&resolved_path).await {
        return Ok(resolved_path);
    }

    // Official provider packages often store manifest file-name icons under _assets/.
    if icon_relative_path.components().count() == 1 {
        let assets_path = installed_root.join("_assets").join(icon_relative_path);
        if path_is_file(&assets_path).await {
            return Ok(assets_path);
        }
    }

    Ok(resolved_path)
}

async fn installed_manifest_icon(installed_path: &str) -> Option<String> {
    let manifest_path = FsPath::new(installed_path).join("manifest.yaml");
    let manifest_raw = tokio::fs::read_to_string(manifest_path).await.ok()?;
    let manifest = plugin_framework::parse_plugin_manifest(&manifest_raw).ok()?;
    let icon = manifest.icon?;
    let icon = icon.trim().to_string();
    if icon.is_empty() {
        return None;
    }
    Some(icon)
}

async fn installation_icon_path(
    installed_path: &str,
    metadata_json: &serde_json::Value,
) -> Option<String> {
    if let Some(icon) = metadata_json
        .get("icon")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    {
        return Some(icon);
    }

    installed_manifest_icon(installed_path).await
}

pub async fn read_provider_icon(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(provider_code): Path<String>,
) -> Result<Response, ApiError> {
    let output: ProviderIconOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.icons.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        ProviderIconInput(provider_code),
    )
    .await?;
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, output.content_type)
        .body(Body::from(output.content))
        .map_err(ApiError::from)
}

impl ConsoleInterfacePort<ProviderIconInput, ProviderIconOutput> for ProviderIconAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderIconInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderIconOutput> {
        Box::pin(async move {
            let source = ModelProviderService::for_console_operation(
                self.store.for_actor(principal.actor().clone()),
                ApiProviderRuntime::new(self.provider_runtime.clone()),
                self.secret_key.clone(),
                domain::ConsolePolicyGroup::other("other.model-providers")
                    .expect("compiled model-provider group must be valid"),
                "model_providers.icons.view",
            )
            .with_node_artifact_context(self.api_node_id.clone(), self.install_root.clone())
            .provider_icon_source(principal.actor().user_id, &input.0)
            .await
            .map_err(|error| ConsoleInterfaceTargetError(error.into()))?;
            let icon_path = installation_icon_path(&source.installed_path, &source.metadata_json)
                .await
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "plugin_icon",
                ))
                .map_err(|error| ConsoleInterfaceTargetError(error.into()))?;
            let resolved_path = resolve_provider_icon_path(&source.installed_path, &icon_path)
                .await
                .map_err(ConsoleInterfaceTargetError)?;
            let content = tokio::fs::read(&resolved_path)
                .await
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::NotFound => {
                        control_plane::errors::ControlPlaneError::NotFound("plugin_icon").into()
                    }
                    _ => ApiError::from(anyhow::Error::from(error)),
                })
                .map_err(ConsoleInterfaceTargetError)?;
            Ok(ProviderIconOutput {
                content_type: provider_icon_content_type(&resolved_path),
                content,
            })
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "model-providers.icons.view",
    binding_id: "http.console.model-providers.icons.view.v1",
    method: "GET",
    path: "/api/console/model-providers/providers/:provider_code/icon",
    mutating: false,
}];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    secret_key: String,
    api_node_id: String,
    install_root: String,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-icons",
        "graph:console-provider-icons-v1",
        DECLARATIONS,
        Arc::new(ProviderIconAdapter {
            store,
            provider_runtime,
            secret_key,
            api_node_id,
            install_root,
        }),
    )
}
