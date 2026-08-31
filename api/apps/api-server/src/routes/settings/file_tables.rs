use std::sync::Arc;

use access_control::{
    FILE_TABLES_CREATE_OPERATION_ID, FILE_TABLES_DELETE_OPERATION_ID,
    FILE_TABLES_LIST_OPERATION_ID, FILE_TABLES_STORAGE_BIND_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::file_management::{
    project_builtin_file_table_title, BindFileTableStorageCommand, CreateFileTableCommand,
    DeleteFileTableCommand, FileTableService, FileTableWithStorageTitle,
};
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::RuntimeRegistrySync;
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
        console_route_assembly::{console_delete, console_get, console_put, ConsoleRouteAssembly},
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFileTableBody {
    pub code: String,
    pub title: String,
}

enum FileTablesInput {
    List {
        locale: ConsoleLocaleHints,
    },
    Create(CreateFileTableBody),
    Bind {
        file_table_id: String,
        body: BindFileTableStorageBody,
    },
    Delete {
        file_table_id: String,
    },
}

impl InterfaceContract for FileTablesInput {
    const CONTRACT_ID: &'static str = "console-file-tables-input";
    const CONTRACT_VERSION: &'static str = "1";
}

enum FileTablesOutput {
    List(Vec<FileTableResponse>),
    Item(FileTableResponse),
    Deleted,
}

impl InterfaceContract for FileTablesOutput {
    const CONTRACT_ID: &'static str = "console-file-tables-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct FileTablesAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    runtime_registry_sync: Arc<dyn RuntimeRegistrySync>,
}

impl FileTablesAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FileTablesInput,
    ) -> Result<FileTablesOutput, ApiError> {
        let actor = principal.actor();
        match input {
            FileTablesInput::List { locale } => {
                let mut tables = FileTableService::new(self.store.clone())
                    .list_tables(actor.user_id)
                    .await?;
                let preferred_locale = self
                    .store
                    .find_user_by_id(actor.user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale = locale.resolve(preferred_locale);
                let resolver =
                    CatalogResolver::new(self.store.clone(), self.bootstrap_workspace_id);
                for result in &mut tables {
                    project_builtin_file_table_title(
                        &resolver,
                        self.bootstrap_workspace_id,
                        &locale,
                        &mut result.table,
                    )
                    .await?;
                }
                Ok(FileTablesOutput::List(
                    tables.into_iter().map(to_response).collect(),
                ))
            }
            FileTablesInput::Create(body) => {
                let created = FileTableService::new(self.store.clone())
                    .create_table(CreateFileTableCommand {
                        actor_user_id: actor.user_id,
                        code: body.code,
                        title: body.title,
                    })
                    .await?;
                self.runtime_registry_sync.rebuild().await?;
                Ok(FileTablesOutput::Item(to_response(created)))
            }
            FileTablesInput::Bind {
                file_table_id,
                body,
            } => {
                let updated = FileTableService::new(self.store.clone())
                    .bind_storage(BindFileTableStorageCommand {
                        actor_user_id: actor.user_id,
                        file_table_id: parse_uuid(&file_table_id, "file_table_id")?,
                        bound_storage_id: parse_uuid(&body.bound_storage_id, "bound_storage_id")?,
                    })
                    .await?;
                Ok(FileTablesOutput::Item(to_response(updated)))
            }
            FileTablesInput::Delete { file_table_id } => {
                FileTableService::new(self.store.clone())
                    .delete_table(DeleteFileTableCommand {
                        actor_user_id: actor.user_id,
                        file_table_id: parse_uuid(&file_table_id, "file_table_id")?,
                    })
                    .await?;
                self.runtime_registry_sync.rebuild().await?;
                Ok(FileTablesOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<FileTablesInput, FileTablesOutput> for FileTablesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FileTablesInput,
    ) -> ConsoleInterfaceFuture<'a, FileTablesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BindFileTableStorageBody {
    pub bound_storage_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileTableResponse {
    pub id: String,
    pub code: String,
    pub title: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub model_definition_id: String,
    pub bound_storage_id: String,
    pub bound_storage_title: Option<String>,
    pub is_builtin: bool,
    pub is_default: bool,
    pub status: String,
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn to_response(result: FileTableWithStorageTitle) -> FileTableResponse {
    let record = result.table;
    FileTableResponse {
        id: record.id.to_string(),
        code: record.code,
        title: record.title,
        scope_kind: match record.scope_kind {
            domain::FileTableScopeKind::System => "system".into(),
            domain::FileTableScopeKind::Workspace => "workspace".into(),
        },
        scope_id: record.scope_id.to_string(),
        model_definition_id: record.model_definition_id.to_string(),
        bound_storage_id: record.bound_storage_id.to_string(),
        bound_storage_title: result.bound_storage_title,
        is_builtin: record.is_builtin,
        is_default: record.is_default,
        status: record.status,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/files/tables",
            console_get(
                list_file_tables,
                ConsoleOperation(FILE_TABLES_LIST_OPERATION_ID.to_string()),
            )
            .post(
                create_file_table,
                ConsoleOperation(FILE_TABLES_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/tables/:id",
            console_delete(
                delete_file_table,
                ConsoleOperation(FILE_TABLES_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/files/tables/:id/binding",
            console_put(
                bind_file_table_storage,
                ConsoleOperation(FILE_TABLES_STORAGE_BIND_OPERATION_ID.to_string()),
            ),
        )
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "file_tables.list",
        binding_id: "http.console.settings.file-tables.list.v1",
        method: "GET",
        path: "/api/console/settings/files/tables",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_tables.create",
        binding_id: "http.console.settings.file-tables.create.v1",
        method: "POST",
        path: "/api/console/settings/files/tables",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_tables.delete",
        binding_id: "http.console.settings.file-tables.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/files/tables/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "file_tables.storage.bind",
        binding_id: "http.console.settings.file-tables.storage.bind.v1",
        method: "PUT",
        path: "/api/console/settings/files/tables/:id/binding",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    runtime_registry_sync: Arc<dyn RuntimeRegistrySync>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-file-tables",
        "graph:console-file-tables-v1",
        DECLARATIONS,
        Arc::new(FileTablesAdapter {
            store,
            bootstrap_workspace_id,
            runtime_registry_sync,
        }),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/files/tables",
    responses((status = 200, body = [FileTableResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_file_tables(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FileTableResponse>>>, ApiError> {
    let locale = ConsoleLocaleHints::from_headers(&headers);
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-tables.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        FileTablesInput::List { locale },
    )
    .await?;
    let FileTablesOutput::List(tables) = output else {
        unreachable!("file tables list binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(tables)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/files/tables",
    request_body = CreateFileTableBody,
    responses((status = 201, body = FileTableResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_file_table(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFileTableBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FileTableResponse>>), ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-tables.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileTablesInput::Create(body),
    )
    .await?;
    let FileTablesOutput::Item(created) = output else {
        unreachable!("file table create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(created))))
}

#[utoipa::path(
    put,
    path = "/api/console/settings/files/tables/{id}/binding",
    request_body = BindFileTableStorageBody,
    params(("id" = String, Path, description = "File table id")),
    responses((status = 200, body = FileTableResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn bind_file_table_storage(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_table_id): Path<String>,
    Json(body): Json<BindFileTableStorageBody>,
) -> Result<Json<ApiSuccess<FileTableResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-tables.storage.bind.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileTablesInput::Bind {
            file_table_id,
            body,
        },
    )
    .await?;
    let FileTablesOutput::Item(updated) = output else {
        unreachable!("file table bind binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(updated)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/files/tables/{id}",
    params(("id" = String, Path, description = "File table id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_file_table(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(file_table_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.settings.file-tables.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FileTablesInput::Delete { file_table_id },
    )
    .await?;
    let FileTablesOutput::Deleted = output else {
        unreachable!("file table delete binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
}
