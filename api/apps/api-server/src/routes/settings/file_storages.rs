use std::sync::Arc;

use access_control::{
    FILE_STORAGES_CREATE_OPERATION_ID, FILE_STORAGES_DELETE_OPERATION_ID,
    FILE_STORAGES_LIST_OPERATION_ID, FILE_STORAGES_UPDATE_OPERATION_ID,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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
        console_route_assembly::{ConsoleRouteAssembly, console_get, console_put},
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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        ("code", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("driver_type", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("is_default", serde_json::json!({"type":"boolean"})),
                        ("config_json", mp::json_summary_schema()),
                        ("rule_json", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("file_storage_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("is_default", serde_json::json!({"type":"boolean"})),
                        ("config_json", mp::json_summary_schema()),
                        ("rule_json", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("file_storage_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List { .. } => {
                mp::object_value(&[("variant", serde_json::Value::String("List".to_owned()))])
            }
            Self::Create(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("code", mp::text(&(_field_0).code)?),
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).title).len()),
                            )]),
                        ),
                        ("driver_type", mp::text(&(_field_0).driver_type)?),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "is_default",
                            serde_json::Value::Bool(*(&(_field_0).is_default)),
                        ),
                        ("config_json", mp::json_summary(&(_field_0).config_json)),
                        ("rule_json", mp::json_summary(&(_field_0).rule_json)),
                    ]),
                ),
            ]),
            Self::Update {
                file_storage_id: _field_file_storage_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Update".to_owned())),
                ("file_storage_id", mp::text(_field_file_storage_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        (
                            "is_default",
                            serde_json::Value::Bool(*(&(_field_body).is_default)),
                        ),
                        ("config_json", mp::json_summary(&(_field_body).config_json)),
                        ("rule_json", mp::json_summary(&(_field_body).rule_json)),
                    ]),
                ),
            ]),
            Self::Delete {
                file_storage_id: _field_file_storage_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                ("file_storage_id", mp::text(_field_file_storage_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-file-storages-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed storage output is projected immediately into the console response"
)]
enum FileStoragesOutput {
    List(Vec<FileStorageResponse>),
    Item(FileStorageResponse),
    Deleted,
}

impl InterfaceContract for FileStoragesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("driver_type",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("is_default",serde_json::json!({"type":"boolean"})), ("config_json",mp::json_summary_schema()), ("rule_json",mp::json_summary_schema()), ("health_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("last_health_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Item")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("code", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("driver_type", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("is_default", serde_json::json!({"type":"boolean"})),
                        ("config_json", mp::json_summary_schema()),
                        ("rule_json", mp::json_summary_schema()),
                        (
                            "health_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_health_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", mp::text(&(item).id)?),
                                    ("code", mp::text(&(item).code)?),
                                    (
                                        "title",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).title).len()),
                                        )]),
                                    ),
                                    ("driver_type", mp::text(&(item).driver_type)?),
                                    ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                    ("is_default", serde_json::Value::Bool(*(&(item).is_default))),
                                    ("config_json", mp::json_summary(&(item).config_json)),
                                    ("rule_json", mp::json_summary(&(item).rule_json)),
                                    (
                                        "health_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).health_status).len()),
                                        )]),
                                    ),
                                    (
                                        "last_health_error",
                                        match (&(item).last_health_error).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Item(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Item".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("code", mp::text(&(_field_0).code)?),
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).title).len()),
                            )]),
                        ),
                        ("driver_type", mp::text(&(_field_0).driver_type)?),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "is_default",
                            serde_json::Value::Bool(*(&(_field_0).is_default)),
                        ),
                        ("config_json", mp::json_summary(&(_field_0).config_json)),
                        ("rule_json", mp::json_summary(&(_field_0).rule_json)),
                        (
                            "health_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).health_status).len()),
                            )]),
                        ),
                        (
                            "last_health_error",
                            match (&(_field_0).last_health_error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Deleted => {
                mp::object_value(&[("variant", serde_json::Value::String("Deleted".to_owned()))])
            }
        })
    }

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
        interface_id: "file_storages.list",
        binding_id: "http.console.settings.file-storages.list.v1",
        method: "GET",
        path: "/api/console/settings/files/storages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_storages.create",
        binding_id: "http.console.settings.file-storages.create.v1",
        method: "POST",
        path: "/api/console/settings/files/storages",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_storages.update",
        binding_id: "http.console.settings.file-storages.update.v1",
        method: "PUT",
        path: "/api/console/settings/files/storages/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_storages.delete",
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

/// Constructs private owner values without widening their production visibility.
#[cfg(test)]
pub(crate) fn root_2014_projection_fixture(
    sentinel: &str,
) -> (
    interface_runtime::ManagedInterfaceProjection,
    interface_runtime::ManagedInterfaceProjection,
) {
    let config = serde_json::json!({"innocent": {"access_key": sentinel, "secret_key": sentinel},
        "endpoint": format!("https://user:{sentinel}@example.invalid"),
        "schema": {"default": sentinel, "examples": [sentinel]}});
    let input = FileStoragesInput::Create(CreateFileStorageBody {
        code: "projection-fixture".into(),
        title: "Storage".into(),
        driver_type: "rustfs".into(),
        enabled: true,
        is_default: false,
        config_json: config.clone(),
        rule_json: config.clone(),
    });
    let output = FileStoragesOutput::Item(FileStorageResponse {
        id: uuid::Uuid::nil().to_string(),
        code: "projection-fixture".into(),
        title: "Storage".into(),
        driver_type: "rustfs".into(),
        enabled: true,
        is_default: false,
        config_json: config.clone(),
        rule_json: config.clone(),
        health_status: "healthy".into(),
        last_health_error: Some(sentinel.into()),
    });
    let input_view = interface_runtime::ManagedInterfaceProjection::from_contract(&input).unwrap();
    let output_view =
        interface_runtime::ManagedInterfaceProjection::from_contract(&output).unwrap();
    match input {
        FileStoragesInput::Create(original) => assert_eq!(original.config_json, config),
        _ => unreachable!(),
    }
    match output {
        FileStoragesOutput::Item(original) => assert_eq!(original.config_json, config),
        _ => unreachable!(),
    }
    (input_view, output_view)
}
