use std::{collections::HashSet, sync::Arc};

use access_control::{FILES_CONTENT_DOWNLOAD_OPERATION_ID, FILES_UPLOAD_OPERATION_ID};
use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use control_plane::ports::{FileManagementRepository, ModelDefinitionRepository};
use control_plane::resource_action::{
    ActionDefinition, ResourceActionKernel, ResourceActionRegistry, ResourceDefinition,
    ResourceScopeKind,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadedFileResponse {
    pub storage_id: String,
    #[schema(value_type = Object)]
    pub record: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct UploadFileActorInput {
    user_id: Uuid,
    tenant_id: Uuid,
    current_workspace_id: Uuid,
    effective_display_role: String,
    is_root: bool,
    permissions: Vec<String>,
}

impl From<domain::ActorContext> for UploadFileActorInput {
    fn from(actor: domain::ActorContext) -> Self {
        Self {
            user_id: actor.user_id,
            tenant_id: actor.tenant_id,
            current_workspace_id: actor.current_workspace_id,
            effective_display_role: actor.effective_display_role,
            is_root: actor.is_root,
            permissions: actor.permissions.into_iter().collect(),
        }
    }
}

impl UploadFileActorInput {
    fn into_actor(self) -> domain::ActorContext {
        domain::ActorContext {
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            current_workspace_id: self.current_workspace_id,
            effective_display_role: self.effective_display_role,
            is_root: self.is_root,
            permissions: HashSet::from_iter(self.permissions),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct UploadFileActionInput {
    actor: UploadFileActorInput,
    file_table_id: Uuid,
    original_filename: String,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/files/upload",
            console_post(
                upload_file,
                ConsoleOperation(FILES_UPLOAD_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/files/:file_table_id/records/:record_id/content",
            console_get(
                read_file_content,
                ConsoleOperation(FILES_CONTENT_DOWNLOAD_OPERATION_ID.to_string()),
            ),
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

fn upload_file_action_kernel(state: Arc<ApiState>) -> Result<ResourceActionKernel, ApiError> {
    let mut registry = ResourceActionRegistry::default();
    registry.register_resource(ResourceDefinition::core(
        "files",
        ResourceScopeKind::Workspace,
    ))?;
    registry.register_action(ActionDefinition::core("files", "upload"))?;

    let mut kernel = ResourceActionKernel::new(registry);
    kernel.register_json_handler("files", "upload", move |input| {
        let state = state.clone();
        async move {
            let input: UploadFileActionInput = serde_json::from_value(input).map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput("file_upload_action")
            })?;
            let uploaded = control_plane::file_management::FileUploadService::new(
                state.store.clone(),
                state.file_storage_registry.clone(),
                state.runtime_engine.clone(),
            )
            .upload(control_plane::file_management::UploadFileCommand {
                actor: input.actor.into_actor(),
                file_table_id: input.file_table_id,
                original_filename: input.original_filename,
                content_type: input.content_type,
                bytes: input.bytes,
            })
            .await
            .map_err(|error| map_runtime_error(error).0)?;

            Ok(serde_json::to_value(UploadedFileResponse {
                storage_id: uploaded.storage_id.to_string(),
                record: uploaded.record,
            })?)
        }
    })?;

    Ok(kernel)
}

#[utoipa::path(
    post,
    path = "/api/console/files/upload",
    responses((status = 201, body = UploadedFileResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn upload_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<UploadedFileResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

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

    let file_table_id = parse_uuid(
        file_table_id
            .as_deref()
            .ok_or_else(|| invalid_input("file_table_id"))?,
        "file_table_id",
    )?;
    let output = upload_file_action_kernel(state.clone())?
        .dispatch_json(
            "files",
            "upload",
            serde_json::json!({
                "actor": UploadFileActorInput::from(context.actor),
                "file_table_id": file_table_id,
                "original_filename": filename.unwrap_or_else(|| "upload.bin".into()),
                "content_type": content_type,
                "bytes": bytes.ok_or_else(|| invalid_input("file"))?,
            }),
        )
        .await?;
    let response = serde_json::from_value(output).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("file_upload_result")
    })?;

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    get,
    path = "/api/console/files/{file_table_id}/records/{record_id}/content",
    params(
        ("file_table_id" = String, Path, description = "File table id"),
        ("record_id" = String, Path, description = "Runtime record id")
    ),
    responses((status = 200), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn read_file_content(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((file_table_id, record_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    let file_table = state
        .store
        .get_file_table(parse_uuid(&file_table_id, "file_table_id")?)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "file_table",
        ))?;
    let model = state
        .store
        .get_model_definition(
            context.actor.current_workspace_id,
            file_table.model_definition_id,
        )
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "model_definition",
        ))?;
    let scope_grant =
        control_plane::model_definition::ModelDefinitionService::new(state.store.clone())
            .load_runtime_scope_grant(
                &context.actor,
                model.id,
                runtime_core::runtime_acl::RuntimeDataAction::View,
            )
            .await?;
    let record = state
        .runtime_engine
        .get_record(runtime_core::runtime_engine::RuntimeGetInput {
            actor: context.actor,
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
    let storage = state
        .store
        .get_file_storage(parse_uuid(storage_id, "storage_id")?)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "file_storage",
        ))?;
    let driver = state
        .file_storage_registry
        .get(&storage.driver_type)
        .ok_or(control_plane::errors::ControlPlaneError::Conflict(
            "storage_driver_not_registered",
        ))?;
    let open = driver
        .open_read(storage_object::OpenReadInput {
            config_json: &storage.config_json,
            object_path,
        })
        .await
        .map_err(map_file_storage_error)?;
    let content_type = open
        .content_type
        .or_else(|| {
            record
                .get("mimetype")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "application/octet-stream".into());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(open.bytes))
        .unwrap())
}
