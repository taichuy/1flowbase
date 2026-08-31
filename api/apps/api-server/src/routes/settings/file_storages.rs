use std::sync::Arc;

use access_control::{
    FILE_STORAGES_CREATE_OPERATION_ID, FILE_STORAGES_DELETE_OPERATION_ID,
    FILE_STORAGES_LIST_OPERATION_ID, FILE_STORAGES_UPDATE_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::file_management::{
    CreateFileStorageCommand, DeleteFileStorageCommand, FileStorageService,
    UpdateFileStorageCommand,
};
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
            ConsoleInterfaceTargetError, ConsoleLocaleHints,
        },
        console_route_assembly::{console_get, console_put, ConsoleRouteAssembly},
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFileStorageBody {
    pub code: String,
    pub title: String,
    pub driver_type: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFileStorageBody {
    pub title: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileStorageResponse {
    pub id: String,
    pub code: String,
    pub title: String,
    pub driver_type: String,
    pub enabled: bool,
    pub is_default: bool,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub rule_json: serde_json::Value,
    pub health_status: String,
    pub last_health_error: Option<String>,
}

enum FileStoragesInput {
    List {
        locale: ConsoleLocaleHints,
    },
    Create(CreateFileStorageBody),
    Update {
        file_storage_id: String,
        body: UpdateFileStorageBody,
    },
    Delete {
        file_storage_id: String,
    },
}

impl InterfaceContract for FileStoragesInput {
    const CONTRACT_ID: &'static str = "console-file-storages-input";
    const CONTRACT_VERSION: &'static str = "1";
}

enum FileStoragesOutput {
    List(Vec<FileStorageResponse>),
    Item(FileStorageResponse),
    Deleted,
}

impl InterfaceContract for FileStoragesOutput {
    const CONTRACT_ID: &'static str = "console-file-storages-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct FileStoragesAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

impl FileStoragesAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FileStoragesInput,
    ) -> Result<FileStoragesOutput, ApiError> {
        let actor = principal.actor();
        match input {
            FileStoragesInput::List { locale } => {
                let mut storages = FileStorageService::new(self.store.clone())
                    .list_storages(actor.user_id)
                    .await?;
                let preferred_locale = self
                    .store
                    .find_user_by_id(actor.user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale = locale.resolve(preferred_locale);
                for storage in &mut storages {
                    if storage.code == "local_default" && storage.is_default {
                        storage.title = crate::app_state::project_canonical_display_with(
                            &self.store,
                            self.bootstrap_workspace_id,
                            &locale,
                            "Local",
                            &storage.title,
                        )
                        .await?;
                    }
                }
                Ok(FileStoragesOutput::List(
                    storages.into_iter().map(to_response).collect(),
                ))
            }
            FileStoragesInput::Create(body) => Ok(FileStoragesOutput::Item(to_response(
                FileStorageService::new(self.store.clone())
                    .create_storage(CreateFileStorageCommand {
                        actor_user_id: actor.user_id,
                        code: body.code,
                        title: body.title,
                        driver_type: body.driver_type,
                        enabled: body.enabled,
                        is_default: body.is_default,
                        config_json: body.config_json,
                        rule_json: body.rule_json,
                    })
                    .await?,
            ))),
            FileStoragesInput::Update {
                file_storage_id,
                body,
            } => Ok(FileStoragesOutput::Item(to_response(
                FileStorageService::new(self.store.clone())
                    .update_storage(UpdateFileStorageCommand {
                        actor_user_id: actor.user_id,
                        file_storage_id: parse_uuid(&file_storage_id, "file_storage_id")?,
                        title: body.title,
                        enabled: body.enabled,
                        is_default: body.is_default,
                        config_json: body.config_json,
                        rule_json: body.rule_json,
                    })
                    .await?,
            ))),
            FileStoragesInput::Delete { file_storage_id } => {
                FileStorageService::new(self.store.clone())
                    .delete_storage(DeleteFileStorageCommand {
                        actor_user_id: actor.user_id,
                        file_storage_id: parse_uuid(&file_storage_id, "file_storage_id")?,
                    })
                    .await?;
                Ok(FileStoragesOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<FileStoragesInput, FileStoragesOutput> for FileStoragesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FileStoragesInput,
    ) -> ConsoleInterfaceFuture<'a, FileStoragesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn to_response(record: domain::FileStorageRecord) -> FileStorageResponse {
    FileStorageResponse {
        id: record.id.to_string(),
        code: record.code,
        title: record.title,
        driver_type: record.driver_type,
        enabled: record.enabled,
        is_default: record.is_default,
        config_json: record.config_json,
        rule_json: record.rule_json,
        health_status: match record.health_status {
            domain::FileStorageHealthStatus::Unknown => "unknown".into(),
            domain::FileStorageHealthStatus::Ready => "ready".into(),
            domain::FileStorageHealthStatus::Failed => "failed".into(),
        },
        last_health_error: record.last_health_error,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/files/storages",
            console_get(
                list_file_storages,
                ConsoleOperation(FILE_STORAGES_LIST_OPERATION_ID.to_string()),
            )
            .post(
                create_file_storage,
                ConsoleOperation(FILE_STORAGES_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/storages/:id",
            console_put(
                update_file_storage,
                ConsoleOperation(FILE_STORAGES_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                delete_file_storage,
                ConsoleOperation(FILE_STORAGES_DELETE_OPERATION_ID.to_string()),
            ),
        )
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "settings.file-storages.list",
        binding_id: "http.console.settings.file-storages.list.v1",
        method: "GET",
        path: "/api/console/settings/files/storages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "settings.file-storages.create",
        binding_id: "http.console.settings.file-storages.create.v1",
        method: "POST",
        path: "/api/console/settings/files/storages",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "settings.file-storages.update",
        binding_id: "http.console.settings.file-storages.update.v1",
        method: "PUT",
        path: "/api/console/settings/files/storages/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "settings.file-storages.delete",
        binding_id: "http.console.settings.file-storages.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/files/storages/:id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-file-storages",
        "graph:console-file-storages-v1",
        DECLARATIONS,
        Arc::new(FileStoragesAdapter {
            store,
            bootstrap_workspace_id,
        }),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/files/storages",
    responses((status = 200, body = [FileStorageResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_file_storages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FileStorageResponse>>>, ApiError> {
    let locale = ConsoleLocaleHints::from_headers(&headers);
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-storages.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        FileStoragesInput::List { locale },
    )
    .await?;
    let FileStoragesOutput::List(storages) = output else {
        unreachable!("file storages list binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(storages)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/files/storages",
    request_body = CreateFileStorageBody,
    responses((status = 201, body = FileStorageResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFileStorageBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FileStorageResponse>>), ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-storages.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileStoragesInput::Create(body),
    )
    .await?;
    let FileStoragesOutput::Item(created) = output else {
        unreachable!("file storage create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(created))))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/files/storages/{id}",
    request_body = UpdateFileStorageBody,
    params(("id" = String, Path, description = "File storage id")),
    responses((status = 200, body = FileStorageResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_storage_id): Path<String>,
    Json(body): Json<UpdateFileStorageBody>,
) -> Result<Json<ApiSuccess<FileStorageResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-storages.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileStoragesInput::Update {
            file_storage_id,
            body,
        },
    )
    .await?;
    let FileStoragesOutput::Item(updated) = output else {
        unreachable!("file storage update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(updated)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/files/storages/{id}",
    params(("id" = String, Path, description = "File storage id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_file_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_storage_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-storages.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileStoragesInput::Delete { file_storage_id },
    )
    .await?;
    let FileStoragesOutput::Deleted = output else {
        unreachable!("file storage delete binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
}
