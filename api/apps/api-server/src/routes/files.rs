#[path = "files/public/mod.rs"]
pub(crate) mod public;

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use storage_durable_postgres::MainDurableStore;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    network_egress_client::NetworkEgressHttpClientResolver,
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

#[derive(Deserialize, ToSchema)]
pub struct CreateTemporaryFileLinkInput {
    pub expires_in_seconds: i64,
}
#[derive(Serialize, ToSchema)]
pub struct TemporaryFileLinkResponse {
    pub url: String,
    pub preview_url: Option<String>,
    pub expires_at: String,
}

#[derive(ToSchema)]
#[allow(dead_code)]
struct UploadFileMultipartBody {
    file_table_id: Option<String>,
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
}

/// A temporary MCP file reference is a source locator, not a persisted business file.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoteFileReference {
    pub download_url: String,
    pub file_id: String,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportRemoteFileBody {
    pub file_table_id: Option<String>,
    #[schema(inline)]
    pub file: RemoteFileReference,
}

enum BusinessFilesInput {
    Upload {
        file_table_id: Option<Uuid>,
        original_filename: String,
        content_type: Option<String>,
        bytes: Vec<u8>,
    },
    ImportRemote {
        file_table_id: Option<Uuid>,
        file: RemoteFileReference,
    },
    CreateTemporaryLink {
        file_table_id: String,
        record_id: String,
        expires_in_seconds: i64,
    },
    ReadContent {
        file_table_id: String,
        record_id: String,
        preview: bool,
    },
}

impl InterfaceContract for BusinessFilesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateTemporaryLink")),
                ("file_table_id", mp::text_schema()),
                ("record_id", mp::text_schema()),
                ("expires_in_seconds", mp::count_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Upload")),
                (
                    "file_table_id",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
                (
                    "content_type",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
                (
                    "bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportRemote")),
                (
                    "file_table_id",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
                ("file_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReadContent")),
                ("preview", serde_json::json!({"type":"boolean"})),
                ("file_table_id", mp::text_schema()),
                ("record_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::CreateTemporaryLink {
                file_table_id,
                record_id,
                expires_in_seconds,
            } => mp::object_value(&[
                ("variant", serde_json::json!("CreateTemporaryLink")),
                ("file_table_id", mp::text(file_table_id)?),
                ("record_id", mp::text(record_id)?),
                ("expires_in_seconds", serde_json::json!(expires_in_seconds)),
            ]),
            Self::Upload {
                file_table_id: _field_file_table_id,
                content_type: _field_content_type,
                bytes: _field_bytes,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Upload".to_owned())),
                (
                    "file_table_id",
                    match (_field_file_table_id).as_ref() {
                        Some(item) => serde_json::Value::String((item).to_string()),
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "content_type",
                    match (_field_content_type).as_ref() {
                        Some(item) => mp::text(item)?,
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "bytes",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_bytes).len()))]),
                ),
            ]),
            Self::ImportRemote {
                file_table_id,
                file,
            } => mp::object_value(&[
                ("variant", serde_json::json!("ImportRemote")),
                (
                    "file_table_id",
                    file_table_id
                        .as_ref()
                        .map_or(serde_json::Value::Null, |id| {
                            serde_json::json!(id.to_string())
                        }),
                ),
                ("file_id", mp::text(&file.file_id)?),
            ]),
            Self::ReadContent {
                file_table_id: _field_file_table_id,
                record_id: _field_record_id,
                preview,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReadContent".to_owned()),
                ),
                ("file_table_id", mp::text(_field_file_table_id)?),
                ("record_id", mp::text(_field_record_id)?),
                ("preview", serde_json::json!(preview)),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-business-files-input";
    const CONTRACT_VERSION: &'static str = "1";
}

type FileContentOutput = control_plane::file_management::FileContent;

enum BusinessFilesOutput {
    Uploaded(UploadedFileResponse),
    Content(FileContentOutput),
    TemporaryLink(TemporaryFileLinkResponse),
}

impl InterfaceContract for BusinessFilesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("TemporaryLink"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Uploaded")),
                (
                    "0",
                    mp::object_schema(&[
                        ("file_table_id", mp::text_schema()),
                        ("storage_id", mp::text_schema()),
                        ("record", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Content"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::TemporaryLink(_) => {
                mp::object_value(&[("variant", serde_json::json!("TemporaryLink"))])
            }
            Self::Uploaded(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Uploaded".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("file_table_id", mp::text(&(_field_0).file_table_id)?),
                        ("storage_id", mp::text(&(_field_0).storage_id)?),
                        ("record", mp::json_summary(&(_field_0).record)),
                    ]),
                ),
            ]),
            Self::Content(_) => {
                mp::object_value(&[("variant", serde_json::Value::String("Content".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-business-files-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct BusinessFilesAdapter {
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    grants: Arc<dyn control_plane::ports::EphemeralKvStore>,
    network_egress: NetworkEgressHttpClientResolver,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/files/:file_table_id/records/:record_id/access-links",
            console_post(create_temporary_file_link, Authenticated),
        )
        .route("/files/upload", console_post(upload_file, Authenticated))
        .route(
            "/files/import-remote",
            console_post(import_remote_file, Authenticated),
        )
        .route(
            "/files/:file_table_id/records/:record_id/content",
            console_get(read_file_content, Authenticated),
        )
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "files.access_links.create",
        binding_id: "http.console.files.access-links.create.v1",
        method: "POST",
        path: "/api/console/files/:file_table_id/records/:record_id/access-links",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "files.upload",
        binding_id: "http.console.files.upload.v1",
        method: "POST",
        path: "/api/console/files/upload",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "files.import_remote",
        binding_id: "http.console.files.import-remote.v1",
        method: "POST",
        path: "/api/console/files/import-remote",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "files.content.download",
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
    grants: Arc<dyn control_plane::ports::EphemeralKvStore>,
    network_egress: NetworkEgressHttpClientResolver,
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
            grants,
            network_egress,
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
    let error = match error.downcast::<storage_object::FileStorageError>() {
        Ok(error) => return map_file_storage_error(error),
        Err(error) => error,
    };
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
    fn access_service(
        &self,
    ) -> control_plane::file_management::FileAccessService<MainDurableStore> {
        control_plane::file_management::FileAccessService::new(
            self.store.clone(),
            self.file_storage_registry.clone(),
            self.runtime_engine.clone(),
            self.grants.clone(),
        )
    }

    async fn upload_bytes(
        &self,
        actor: domain::ActorContext,
        file_table_id: Option<Uuid>,
        original_filename: String,
        content_type: Option<String>,
        bytes: Vec<u8>,
    ) -> Result<BusinessFilesOutput, ApiError> {
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
                self.upload_bytes(actor, file_table_id, original_filename, content_type, bytes)
                    .await
            }
            BusinessFilesInput::ImportRemote {
                file_table_id,
                file,
            } => {
                let request = self
                    .network_egress
                    .resolve_http_request(
                        actor.current_workspace_id,
                        domain::NetworkEgressConsumerSelector::HttpNodeDefault,
                    )
                    .await
                    .map_err(map_runtime_error)?;
                let result = async {
                    let response = request
                        .http_client()
                        .get(&file.download_url)
                        .send()
                        .await?
                        .error_for_status()?;
                    let content_type = response
                        .headers()
                        .get(CONTENT_TYPE)
                        .and_then(|header| header.to_str().ok())
                        .map(str::to_owned);
                    let bytes = response.bytes().await?.to_vec();
                    Ok::<_, anyhow::Error>((bytes, content_type))
                }
                .await;
                let (bytes, remote_content_type) =
                    request.finish(result).await.map_err(map_runtime_error)?;
                self.upload_bytes(
                    actor,
                    file_table_id,
                    file.file_name.unwrap_or_else(|| "download.bin".to_owned()),
                    file.mime_type.or(remote_content_type),
                    bytes,
                )
                .await
            }
            BusinessFilesInput::ReadContent {
                file_table_id,
                record_id,
                preview,
            } => {
                let content = self
                    .access_service()
                    .read_with_preview(
                        actor,
                        parse_uuid(&file_table_id, "file_table_id")?,
                        parse_uuid(&record_id, "record_id")?,
                        preview,
                    )
                    .await
                    .map_err(map_runtime_error)?;
                Ok(BusinessFilesOutput::Content(content))
            }
            BusinessFilesInput::CreateTemporaryLink {
                file_table_id,
                record_id,
                expires_in_seconds,
            } => {
                let link = self
                    .access_service()
                    .create_temporary_link(
                        actor,
                        parse_uuid(&file_table_id, "file_table_id")?,
                        parse_uuid(&record_id, "record_id")?,
                        expires_in_seconds,
                    )
                    .await
                    .map_err(map_runtime_error)?;
                Ok(BusinessFilesOutput::TemporaryLink(
                    TemporaryFileLinkResponse {
                        url: link.url,
                        preview_url: link.preview_url,
                        expires_at: link.expires_at,
                    },
                ))
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
    post,
    path = "/api/console/files/import-remote",
    request_body = ImportRemoteFileBody,
    responses((status = 201, body = UploadedFileResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn import_remote_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ImportRemoteFileBody>,
) -> Result<(StatusCode, Json<ApiSuccess<UploadedFileResponse>>), ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.files.import-remote.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        BusinessFilesInput::ImportRemote {
            file_table_id: body
                .file_table_id
                .as_deref()
                .map(|id| parse_uuid(id, "file_table_id"))
                .transpose()?,
            file: body.file,
        },
    )
    .await?;
    let BusinessFilesOutput::Uploaded(response) = output else {
        unreachable!("file import binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[derive(Deserialize)]
pub struct FileContentQuery {
    #[serde(default)]
    pub preview: bool,
}

#[utoipa::path(
    get,
    path = "/api/console/files/{file_table_id}/records/{record_id}/content",
    params(
        ("file_table_id" = String, Path, description = "File table id"),
        ("record_id" = String, Path, description = "Runtime record id"),
        ("preview" = Option<bool>, Query, description = "Serve a supported image inline")
    ),
    responses((status = 200, body = inline(crate::openapi::OpenApiBinaryBody), content_type = "application/octet-stream"), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn read_file_content(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((file_table_id, record_id)): Path<(String, String)>,
    Query(query): Query<FileContentQuery>,
) -> Result<Response, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.files.content.read.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        BusinessFilesInput::ReadContent {
            file_table_id,
            record_id,
            preview: query.preview,
        },
    )
    .await?;
    let BusinessFilesOutput::Content(content) = output else {
        unreachable!("file content binding returned a different output")
    };
    content_response(content)
}

#[utoipa::path(post, path = "/api/console/files/{file_table_id}/records/{record_id}/access-links",
    params(("file_table_id" = String, Path), ("record_id" = String, Path)),
    request_body = CreateTemporaryFileLinkInput,
    responses((status = 201, body = TemporaryFileLinkResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn create_temporary_file_link(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((file_table_id, record_id)): Path<(String, String)>,
    Json(input): Json<CreateTemporaryFileLinkInput>,
) -> Result<
    (
        StatusCode,
        HeaderMap,
        Json<ApiSuccess<TemporaryFileLinkResponse>>,
    ),
    ApiError,
> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.files.access-links.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        BusinessFilesInput::CreateTemporaryLink {
            file_table_id,
            record_id,
            expires_in_seconds: input.expires_in_seconds,
        },
    )
    .await?;
    let BusinessFilesOutput::TemporaryLink(link) = output else {
        unreachable!("file link binding returned a different output")
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok((StatusCode::CREATED, headers, Json(ApiSuccess::new(link))))
}

pub(crate) fn content_response(content: FileContentOutput) -> Result<Response, ApiError> {
    use axum::http::header;
    // A user-controlled filename cannot create response headers or executable same-origin content.
    let encoded_filename: String =
        url::form_urlencoded::byte_serialize(content.filename.as_bytes()).collect();
    let reader = content
        .reader
        .into_inner()
        .map_err(|_| anyhow::anyhow!("file reader transfer failed"))?;
    let stream = futures_util::stream::try_unfold(reader, |mut reader| async move {
        let mut buffer = vec![0u8; storage_object::FILE_STORAGE_STREAM_BUFFER_BYTES];
        let read = tokio::io::AsyncReadExt::read(&mut reader, &mut buffer).await?;
        if read == 0 {
            return Ok::<_, std::io::Error>(None);
        }
        buffer.truncate(read);
        Ok(Some((axum::body::Bytes::from(buffer), reader)))
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content.content_type)
        .header(header::CONTENT_LENGTH, content.content_length)
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "{}; filename*=UTF-8''{}",
                if content.preview {
                    "inline"
                } else {
                    "attachment"
                },
                encoded_filename.replace('+', "%20")
            ),
        )
        .header(header::CACHE_CONTROL, "private, no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_SECURITY_POLICY, "sandbox")
        .header(header::REFERRER_POLICY, "no-referrer")
        .body(Body::from_stream(stream))
        .map_err(Into::into)
}
