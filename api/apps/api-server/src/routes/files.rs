use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use control_plane::ports::{FileManagementRepository, ModelDefinitionRepository};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use storage_durable_postgres::MainDurableStore;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError,
        },
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
    },
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadedFileResponse {
    pub file_table_id: String,
    pub storage_id: String,
    #[schema(value_type = Object)]
    pub record: serde_json::Value,
}

#[derive(ToSchema)]
#[allow(dead_code)]
struct UploadFileMultipartBody {
    file_table_id: Option<String>,
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

enum BusinessFilesInput {
    Upload {
        file_table_id: Option<Uuid>,
        original_filename: String,
        content_type: Option<String>,
        bytes: Vec<u8>,
    },
    ReadContent {
        file_table_id: String,
        record_id: String,
    },
}

impl InterfaceContract for BusinessFilesInput {
    const CONTRACT_ID: &'static str = "console-business-files-input";
    const CONTRACT_VERSION: &'static str = "1";
}

struct FileContentOutput {
    content_type: String,
    bytes: Vec<u8>,
}

enum BusinessFilesOutput {
    Uploaded(UploadedFileResponse),
    Content(FileContentOutput),
}

impl InterfaceContract for BusinessFilesOutput {
    const CONTRACT_ID: &'static str = "console-business-files-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct BusinessFilesAdapter {
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route("/files/upload", console_post(upload_file, Authenticated))
        .route(
            "/files/:file_table_id/records/:record_id/content",
            console_get(read_file_content, Authenticated),
        )
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "files.upload",
        binding_id: "http.console.files.upload.v1",
        method: "POST",
        path: "/api/console/files/upload",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "files.content.read",
        binding_id: "http.console.files.content.read.v1",
        method: "GET",
        path: "/api/console/files/:file_table_id/records/:record_id/content",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-business-files",
        "graph:console-business-files-v1",
        DECLARATIONS,
        Arc::new(BusinessFilesAdapter {
            store,
            file_storage_registry,
            runtime_engine,
        }),
    )
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn invalid_input(field: &'static str) -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput(field).into()
}

fn map_runtime_error(error: anyhow::Error) -> ApiError {
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return control_plane::errors::ControlPlaneError::PermissionDenied(reason).into();
    }

    if error.to_string().contains("runtime record not found") {
        return control_plane::errors::ControlPlaneError::NotFound("runtime_record").into();
    }

    if error
        .downcast_ref::<runtime_core::runtime_engine::RuntimeModelError>()
        .is_some()
    {
        return control_plane::errors::ControlPlaneError::Conflict("runtime_model_unavailable")
            .into();
    }

    error.into()
}

fn map_file_storage_error(error: storage_object::FileStorageError) -> ApiError {
    match error {
        storage_object::FileStorageError::ObjectNotFound => {
            control_plane::errors::ControlPlaneError::NotFound("file_content").into()
        }
        storage_object::FileStorageError::ObjectChanged => {
            control_plane::errors::ControlPlaneError::Conflict("file_content_changed").into()
        }
        storage_object::FileStorageError::ObjectLengthMismatch => {
            control_plane::errors::ControlPlaneError::InvalidInput("file_size").into()
        }
        storage_object::FileStorageError::ObjectSnapshotUnavailable => {
            control_plane::errors::ControlPlaneError::Conflict("file_storage_snapshot_unavailable")
                .into()
        }
        storage_object::FileStorageError::ObjectTooLarge => {
            control_plane::errors::ControlPlaneError::InvalidInput("file_size").into()
        }
        storage_object::FileStorageError::UnsupportedDriver(_) => {
            control_plane::errors::ControlPlaneError::Conflict("storage_driver_not_registered")
                .into()
        }
        storage_object::FileStorageError::InvalidConfig(_) => {
            control_plane::errors::ControlPlaneError::Conflict("file_storage_config_invalid").into()
        }
        storage_object::FileStorageError::Other(error) => error.into(),
    }
}

impl BusinessFilesAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: BusinessFilesInput,
    ) -> Result<BusinessFilesOutput, ApiError> {
        let actor = principal.actor().clone();
        match input {
            BusinessFilesInput::Upload {
                file_table_id,
                original_filename,
                content_type,
                bytes,
            } => {
                let uploaded = control_plane::file_management::FileUploadService::new(
                    self.store.clone(),
                    self.file_storage_registry.clone(),
                    self.runtime_engine.clone(),
                )
                .upload(control_plane::file_management::UploadFileCommand {
                    actor,
                    target: file_table_id.map_or(
                        control_plane::file_management::FileUploadTarget::Default,
                        control_plane::file_management::FileUploadTarget::Table,
                    ),
                    original_filename,
                    content_type,
                    bytes,
                })
                .await
                .map_err(map_runtime_error)?;
                Ok(BusinessFilesOutput::Uploaded(UploadedFileResponse {
                    file_table_id: uploaded.file_table_id.to_string(),
                    storage_id: uploaded.storage_id.to_string(),
                    record: uploaded.record,
                }))
            }
            BusinessFilesInput::ReadContent {
                file_table_id,
                record_id,
            } => {
                let file_table = self
                    .store
                    .get_file_table(parse_uuid(&file_table_id, "file_table_id")?)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "file_table",
                    ))?;
                let model = self
                    .store
                    .get_model_definition(
                        actor.current_workspace_id,
                        file_table.model_definition_id,
                    )
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "model_definition",
                    ))?;
                let scope_grant = control_plane::model_definition::ModelDefinitionService::new(
                    self.store.clone(),
                )
                .load_runtime_scope_grant(
                    &actor,
                    model.id,
                    runtime_core::runtime_acl::RuntimeDataAction::View,
                )
                .await?;
                let record = self
                    .runtime_engine
                    .get_record(runtime_core::runtime_engine::RuntimeGetInput {
                        actor,
                        model_code: model.code,
                        record_id,
                        scope_grant,
                    })
                    .await
                    .map_err(map_runtime_error)?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "runtime_record",
                    ))?;
                let storage_id = record
                    .get("storage_id")
                    .and_then(|value| value.as_str())
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "storage_id",
                    ))?;
                let object_path = record.get("path").and_then(|value| value.as_str()).ok_or(
                    control_plane::errors::ControlPlaneError::InvalidInput("path"),
                )?;
                let storage = self
                    .store
                    .get_file_storage(parse_uuid(storage_id, "storage_id")?)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                        "file_storage",
                    ))?;
                let driver = self.file_storage_registry.get(&storage.driver_type).ok_or(
                    control_plane::errors::ControlPlaneError::Conflict(
                        "storage_driver_not_registered",
                    ),
                )?;
                let open = driver
                    .open_read(storage_object::OpenReadInput {
                        config_json: &storage.config_json,
                        object_path,
                    })
                    .await
                    .map_err(map_file_storage_error)?;
                Ok(BusinessFilesOutput::Content(FileContentOutput {
                    content_type: open
                        .content_type
                        .or_else(|| {
                            record
                                .get("mimetype")
                                .and_then(|value| value.as_str())
                                .map(str::to_string)
                        })
                        .unwrap_or_else(|| "application/octet-stream".into()),
                    bytes: open.bytes,
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<BusinessFilesInput, BusinessFilesOutput> for BusinessFilesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: BusinessFilesInput,
    ) -> ConsoleInterfaceFuture<'a, BusinessFilesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

#[utoipa::path(
    post,
    path = "/api/console/files/upload",
    request_body(content = inline(UploadFileMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = UploadedFileResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn upload_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<UploadedFileResponse>>), ApiError> {
    let mut file_table_id = None;
    let mut filename = None;
    let mut content_type = None;
    let mut bytes = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("file_table_id") => {
                file_table_id = Some(field.text().await.map_err(ApiError::from)?)
            }
            Some("file") => {
                filename = field.file_name().map(str::to_string);
                content_type = field.content_type().map(str::to_string);
                bytes = Some(field.bytes().await.map_err(ApiError::from)?.to_vec());
            }
            _ => {}
        }
    }

    let file_table_id = file_table_id
        .as_deref()
        .map(|value| parse_uuid(value, "file_table_id"))
        .transpose()?;
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.files.upload.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        BusinessFilesInput::Upload {
            file_table_id,
            original_filename: filename.unwrap_or_else(|| "upload.bin".into()),
            content_type,
            bytes: bytes.ok_or_else(|| invalid_input("file"))?,
        },
    )
    .await?;
    let BusinessFilesOutput::Uploaded(response) = output else {
        unreachable!("file upload binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    get,
    path = "/api/console/files/{file_table_id}/records/{record_id}/content",
    params(
        ("file_table_id" = String, Path, description = "File table id"),
        ("record_id" = String, Path, description = "Runtime record id")
    ),
    responses((status = 200, body = inline(crate::openapi::OpenApiBinaryBody), content_type = "application/octet-stream"), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn read_file_content(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((file_table_id, record_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.files.content.read.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        BusinessFilesInput::ReadContent {
            file_table_id,
            record_id,
        },
    )
    .await?;
    let BusinessFilesOutput::Content(content) = output else {
        unreachable!("file content binding returned a different output")
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content.content_type)
        .body(Body::from(content.bytes))
        .unwrap())
}
