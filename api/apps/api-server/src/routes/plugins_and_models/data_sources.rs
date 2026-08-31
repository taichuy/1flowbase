use std::sync::Arc;

use access_control::{
    ConsoleRouteOwnership::ConsoleOperation, AGENT_FLOW_DATA_SOURCE_OPTIONS_LIST_OPERATION_ID,
    DATA_SOURCES_SECRET_ROTATE_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::data_source::{
    CreateDataSourceInstanceCommand, DataSourceBackendView, DataSourceCatalogEntryView,
    DataSourceInstanceView, DataSourceResourcesView, DataSourceService, DataSourceView,
    DiscoverDataSourceResourcesCommand, MapDataSourceResourceToModelCommand,
    NativeSqlDataSourceOption, PreviewDataSourceReadCommand, PreviewDataSourceReadResult,
    RotateDataSourceSecretCommand, UpdateDataSourceDefaultsCommand,
    UpdateMainDataSourceDefaultsCommand, ValidateDataSourceInstanceCommand,
    ValidateDataSourceInstanceResult,
};
use control_plane::ports::RuntimeRegistrySync;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use storage_durable_postgres::MainDurableStore;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError,
        },
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
    },
};

use super::model_definitions::{to_model_definition_response, ModelDefinitionResponse};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDataSourceBody {
    pub installation_id: String,
    pub source_code: String,
    pub display_name: String,
    #[schema(value_type = Object)]
    pub config_json: serde_json::Value,
    #[schema(value_type = Object)]
    pub secret_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PreviewDataSourceReadBody {
    pub resource_key: String,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    #[schema(value_type = Object)]
    pub options_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RotateDataSourceSecretBody {
    #[schema(value_type = Object)]
    pub secret_json: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDataSourceDefaultsBody {
    pub default_data_model_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MapDataSourceResourceToModelBody {
    pub resource_key: String,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCatalogEntryResponse {
    pub installation_id: String,
    pub source_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub display_name: String,
    pub protocol: String,
    pub config_schema: Vec<DataSourceConfigFieldResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceConfigFieldOptionResponse {
    pub label: String,
    #[schema(value_type = Object)]
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceConfigFieldResponse {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub control: Option<String>,
    pub required: Option<bool>,
    pub send_mode: Option<String>,
    pub description: Option<String>,
    pub placeholder: Option<String>,
    #[schema(value_type = Object)]
    pub default_value: Option<serde_json::Value>,
    pub options: Vec<DataSourceConfigFieldOptionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCatalogResponse {
    pub entries: Vec<DataSourceCatalogEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceResourceCapabilitiesResponse {
    pub supports_list: bool,
    pub supports_get: bool,
    pub supports_create: bool,
    pub supports_update: bool,
    pub supports_delete: bool,
    pub supports_filter: bool,
    pub supports_sort: bool,
    pub supports_pagination: bool,
    pub supports_owner_filter: bool,
    pub supports_scope_filter: bool,
    pub supports_write: bool,
    pub supports_transactions: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceRemoteResourceResponse {
    pub resource_key: String,
    pub display_name: String,
    pub resource_kind: String,
    pub capabilities: DataSourceResourceCapabilitiesResponse,
    #[schema(value_type = Object)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceResourcesResponse {
    pub entries: Vec<DataSourceRemoteResourceResponse>,
    pub refresh_status: String,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceCapabilitiesResponse {
    pub can_update_defaults: bool,
    pub can_create_data_model: bool,
    pub can_validate: bool,
    pub can_discover_resources: bool,
    pub can_preview_resources: bool,
    pub can_map_resources: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataSourceBackendResponse {
    Core {
        durable_backend: String,
    },
    RuntimeExtension {
        installation_id: String,
        source_code: String,
        #[schema(value_type = Object)]
        config_json: serde_json::Value,
        secret_ref: Option<String>,
        secret_version: Option<i32>,
        catalog_refresh_status: Option<String>,
        catalog_last_error_message: Option<String>,
        catalog_refreshed_at: Option<String>,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourceResponse {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub enabled: bool,
    pub fixed: bool,
    pub default_data_model_status: String,
    pub capabilities: DataSourceCapabilitiesResponse,
    pub backend: DataSourceBackendResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ValidateDataSourceResponse {
    pub data_source: DataSourceResponse,
    #[schema(value_type = Object)]
    pub output: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataSourcePreviewOutputResponse {
    #[schema(value_type = [Object])]
    pub rows: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewDataSourceReadResponse {
    pub preview_session_id: String,
    pub expires_at: String,
    pub output: DataSourcePreviewOutputResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowDataSourceOptionResponse {
    pub data_source_instance_id: String,
    pub display_name: String,
    pub capability: String,
}

enum DataSourcesInput {
    ListAgentFlowOptions,
    ListCatalog,
    List,
    Create(CreateDataSourceBody),
    UpdateDefaults {
        data_source_id: String,
        body: UpdateDataSourceDefaultsBody,
    },
    Validate {
        data_source_id: String,
    },
    RotateSecret {
        data_source_id: String,
        body: RotateDataSourceSecretBody,
    },
    ListResources {
        data_source_id: String,
    },
    DiscoverResources {
        data_source_id: String,
    },
    PreviewRead {
        data_source_id: String,
        body: PreviewDataSourceReadBody,
    },
    MapResourceToModel {
        data_source_id: String,
        body: MapDataSourceResourceToModelBody,
    },
}

impl InterfaceContract for DataSourcesInput {
    const CONTRACT_ID: &'static str = "console-data-sources-input";
    const CONTRACT_VERSION: &'static str = "1";
}

enum DataSourcesOutput {
    AgentFlowOptions(Vec<AgentFlowDataSourceOptionResponse>),
    Catalog(DataSourceCatalogResponse),
    List(Vec<DataSourceResponse>),
    DataSource(DataSourceResponse),
    Validation(ValidateDataSourceResponse),
    Resources(DataSourceResourcesResponse),
    Preview(PreviewDataSourceReadResponse),
    Model(ModelDefinitionResponse),
}

impl InterfaceContract for DataSourcesOutput {
    const CONTRACT_ID: &'static str = "console-data-sources-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct DataSourcesAdapter {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    runtime_registry_sync: Arc<dyn RuntimeRegistrySync>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/data-sources/agent-flow-options",
            console_get(
                list_agent_flow_data_source_options,
                ConsoleOperation(AGENT_FLOW_DATA_SOURCE_OPTIONS_LIST_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/data-sources/:data_source_id/secret/rotate",
            console_post(
                rotate_secret,
                ConsoleOperation(DATA_SOURCES_SECRET_ROTATE_OPERATION_ID.to_string()),
            ),
        )
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.agent-flow-options.list",
        binding_id: "http.console.data-sources.agent-flow-options.list.v1",
        method: "GET",
        path: "/api/console/data-sources/agent-flow-options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.catalog.list",
        binding_id: "http.console.data-sources.catalog.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/data-sources/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.list",
        binding_id: "http.console.data-sources.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/data-sources",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.create",
        binding_id: "http.console.data-sources.create.v1",
        method: "POST",
        path: "/api/console/settings/data-models/data-sources",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.defaults.update",
        binding_id: "http.console.data-sources.defaults.update.v1",
        method: "PATCH",
        path: "/api/console/settings/data-models/data-sources/:data_source_id/defaults",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.validate",
        binding_id: "http.console.data-sources.validate.v1",
        method: "POST",
        path: "/api/console/settings/data-models/data-sources/:data_source_id/validate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.secret.rotate",
        binding_id: "http.console.data-sources.secret.rotate.v1",
        method: "POST",
        path: "/api/console/data-sources/:data_source_id/secret/rotate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.resources.list",
        binding_id: "http.console.data-sources.resources.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/data-sources/:data_source_id/resources",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.resources.discover",
        binding_id: "http.console.data-sources.resources.discover.v1",
        method: "POST",
        path: "/api/console/settings/data-models/data-sources/:data_source_id/resources/discover",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.preview-read",
        binding_id: "http.console.data-sources.preview-read.v1",
        method: "POST",
        path: "/api/console/settings/data-models/data-sources/:data_source_id/preview-read",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "data-sources.resources.map-to-model",
        binding_id: "http.console.data-sources.resources.map-to-model.v1",
        method: "POST",
        path:
            "/api/console/settings/data-models/data-sources/:data_source_id/resources/map-to-model",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    runtime_registry_sync: Arc<dyn RuntimeRegistrySync>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-data-sources",
        "graph:console-data-sources-v1",
        DECLARATIONS,
        Arc::new(DataSourcesAdapter {
            store,
            provider_runtime,
            provider_secret_master_key,
            api_node_id,
            provider_install_root,
            runtime_engine,
            runtime_registry_sync,
        }),
    )
}

fn to_agent_flow_option_response(
    option: NativeSqlDataSourceOption,
) -> AgentFlowDataSourceOptionResponse {
    AgentFlowDataSourceOptionResponse {
        data_source_instance_id: option.data_source_instance_id,
        display_name: option.display_name,
        capability: option.capability,
    }
}

#[utoipa::path(
    get,
    path = "/api/console/data-sources/agent-flow-options",
    responses((status = 200, body = [AgentFlowDataSourceOptionResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_agent_flow_data_source_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<AgentFlowDataSourceOptionResponse>>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.agent-flow-options.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        DataSourcesInput::ListAgentFlowOptions,
    )
    .await?;
    let DataSourcesOutput::AgentFlowOptions(options) = output else {
        unreachable!("data source options binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(options)))
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn format_time(value: time::OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn format_optional_time(value: Option<time::OffsetDateTime>) -> Option<String> {
    value.map(format_time)
}

fn to_catalog_entry_response(entry: DataSourceCatalogEntryView) -> DataSourceCatalogEntryResponse {
    DataSourceCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        source_code: entry.source_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        display_name: entry.display_name,
        protocol: entry.protocol,
        config_schema: entry
            .config_schema
            .into_iter()
            .map(|field| DataSourceConfigFieldResponse {
                key: field.key,
                label: field.label,
                field_type: field.field_type,
                control: field.control,
                required: field.required,
                send_mode: field.send_mode,
                description: field.description,
                placeholder: field.placeholder,
                default_value: field.default_value,
                options: field
                    .options
                    .into_iter()
                    .map(|option| DataSourceConfigFieldOptionResponse {
                        label: option.label,
                        value: option.value,
                        description: option.description,
                        disabled: option.disabled,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn to_resources_response(view: DataSourceResourcesView) -> DataSourceResourcesResponse {
    DataSourceResourcesResponse {
        entries: view
            .entries
            .into_iter()
            .map(|entry| {
                let capabilities = entry.capabilities;
                DataSourceRemoteResourceResponse {
                    resource_key: entry.resource_key,
                    display_name: entry.display_name,
                    resource_kind: entry.resource_kind,
                    capabilities: DataSourceResourceCapabilitiesResponse {
                        supports_list: capabilities.supports_list,
                        supports_get: capabilities.supports_get,
                        supports_create: capabilities.supports_create,
                        supports_update: capabilities.supports_update,
                        supports_delete: capabilities.supports_delete,
                        supports_filter: capabilities.supports_filter,
                        supports_sort: capabilities.supports_sort,
                        supports_pagination: capabilities.supports_pagination,
                        supports_owner_filter: capabilities.supports_owner_filter,
                        supports_scope_filter: capabilities.supports_scope_filter,
                        supports_write: capabilities.supports_write,
                        supports_transactions: capabilities.supports_transactions,
                    },
                    metadata: entry.metadata,
                }
            })
            .collect(),
        refresh_status: view.refresh_status.as_str().to_string(),
        last_error_message: view.last_error_message,
        refreshed_at: format_optional_time(view.refreshed_at),
    }
}

fn to_data_source_response(view: DataSourceView) -> DataSourceResponse {
    let capabilities = view.capabilities();
    let capabilities = DataSourceCapabilitiesResponse {
        can_update_defaults: capabilities.can_update_defaults,
        can_create_data_model: capabilities.can_create_data_model,
        can_validate: capabilities.can_validate,
        can_discover_resources: capabilities.can_discover_resources,
        can_preview_resources: capabilities.can_preview_resources,
        can_map_resources: capabilities.can_map_resources,
    };

    match view.backend {
        DataSourceBackendView::Core { defaults } => DataSourceResponse {
            id: "main".to_string(),
            display_name: "主数据源".to_string(),
            status: "ready".to_string(),
            enabled: true,
            fixed: true,
            default_data_model_status: defaults.data_model_status.as_str().to_string(),
            capabilities,
            backend: DataSourceBackendResponse::Core {
                durable_backend: "postgresql".to_string(),
            },
        },
        DataSourceBackendView::RuntimeExtension(view) => {
            let catalog = view.catalog;
            let enabled = view.instance.status != domain::DataSourceInstanceStatus::Disabled;
            DataSourceResponse {
                id: view.instance.id.to_string(),
                display_name: view.instance.display_name,
                status: view.instance.status.as_str().to_string(),
                enabled,
                fixed: false,
                default_data_model_status: view
                    .instance
                    .defaults
                    .data_model_status
                    .as_str()
                    .to_string(),
                capabilities,
                backend: DataSourceBackendResponse::RuntimeExtension {
                    installation_id: view.instance.installation_id.to_string(),
                    source_code: view.instance.source_code,
                    config_json: view.instance.config_json,
                    secret_ref: view.instance.secret_ref,
                    secret_version: view.instance.secret_version,
                    catalog_refresh_status: catalog
                        .as_ref()
                        .map(|cache| cache.refresh_status.as_str().to_string()),
                    catalog_last_error_message: catalog
                        .as_ref()
                        .and_then(|cache| cache.last_error_message.clone()),
                    catalog_refreshed_at: catalog
                        .and_then(|cache| format_optional_time(cache.refreshed_at)),
                },
            }
        }
    }
}

fn to_runtime_extension_data_source_response(view: DataSourceInstanceView) -> DataSourceResponse {
    to_data_source_response(DataSourceView {
        backend: DataSourceBackendView::RuntimeExtension(view),
    })
}

fn parse_model_status(raw: &str) -> Result<domain::DataModelStatus, ApiError> {
    match raw {
        "draft" => Ok(domain::DataModelStatus::Draft),
        "published" => Ok(domain::DataModelStatus::Published),
        "disabled" => Ok(domain::DataModelStatus::Disabled),
        "broken" => Ok(domain::DataModelStatus::Broken),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "default_data_model_status",
        )
        .into()),
    }
}

fn to_validate_response(result: ValidateDataSourceInstanceResult) -> ValidateDataSourceResponse {
    ValidateDataSourceResponse {
        data_source: to_runtime_extension_data_source_response(DataSourceInstanceView {
            instance: result.instance,
            catalog: None,
        }),
        output: result.output,
    }
}

fn to_preview_response(result: PreviewDataSourceReadResult) -> PreviewDataSourceReadResponse {
    PreviewDataSourceReadResponse {
        preview_session_id: result.preview_session.id.to_string(),
        expires_at: format_time(result.preview_session.expires_at),
        output: DataSourcePreviewOutputResponse {
            rows: result.output.rows,
            next_cursor: result.output.next_cursor,
        },
    }
}

impl DataSourcesAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        settings: bool,
    ) -> crate::app_state::ApiDataSourceService {
        let store = self.store.for_actor(actor.clone());
        let runtime = ApiProviderRuntime::new(self.provider_runtime.clone());
        let service = if settings {
            DataSourceService::for_data_model_settings(
                store,
                runtime,
                self.provider_secret_master_key.clone(),
            )
        } else {
            DataSourceService::new(store, runtime, self.provider_secret_master_key.clone())
        };
        service.with_node_artifact_context(
            self.api_node_id.clone(),
            self.provider_install_root.clone(),
        )
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: DataSourcesInput,
    ) -> Result<DataSourcesOutput, ApiError> {
        let actor = principal.actor();
        let user_id = actor.user_id;
        let workspace_id = actor.current_workspace_id;
        match input {
            DataSourcesInput::ListAgentFlowOptions => Ok(DataSourcesOutput::AgentFlowOptions(
                self.service(actor, false)
                    .list_native_sql_options(user_id, workspace_id)
                    .await?
                    .into_iter()
                    .map(to_agent_flow_option_response)
                    .collect(),
            )),
            DataSourcesInput::ListCatalog => {
                Ok(DataSourcesOutput::Catalog(DataSourceCatalogResponse {
                    entries: self
                        .service(actor, true)
                        .list_catalog(user_id, workspace_id)
                        .await?
                        .into_iter()
                        .map(to_catalog_entry_response)
                        .collect(),
                }))
            }
            DataSourcesInput::List => Ok(DataSourcesOutput::List(
                self.service(actor, true)
                    .list_data_sources(user_id, workspace_id)
                    .await?
                    .into_iter()
                    .map(to_data_source_response)
                    .collect(),
            )),
            DataSourcesInput::Create(body) => {
                let created = self
                    .service(actor, true)
                    .create_instance(CreateDataSourceInstanceCommand {
                        actor_user_id: user_id,
                        workspace_id,
                        installation_id: parse_uuid(&body.installation_id, "installation_id")?,
                        source_code: body.source_code,
                        display_name: body.display_name,
                        config_json: body.config_json,
                        secret_json: body.secret_json,
                    })
                    .await?;
                Ok(DataSourcesOutput::DataSource(
                    to_runtime_extension_data_source_response(created),
                ))
            }
            DataSourcesInput::UpdateDefaults {
                data_source_id,
                body,
            } => {
                let defaults = domain::DataSourceDefaults {
                    data_model_status: parse_model_status(&body.default_data_model_status)?,
                };
                let response = if data_source_id == "main" {
                    let defaults = self
                        .service(actor, true)
                        .update_main_data_source_defaults(UpdateMainDataSourceDefaultsCommand {
                            actor_user_id: user_id,
                            workspace_id,
                            defaults,
                        })
                        .await?;
                    to_data_source_response(DataSourceView {
                        backend: DataSourceBackendView::Core { defaults },
                    })
                } else {
                    let instance = self
                        .service(actor, true)
                        .update_defaults(UpdateDataSourceDefaultsCommand {
                            actor_user_id: user_id,
                            workspace_id,
                            instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                            defaults,
                        })
                        .await?;
                    to_runtime_extension_data_source_response(DataSourceInstanceView {
                        instance,
                        catalog: None,
                    })
                };
                Ok(DataSourcesOutput::DataSource(response))
            }
            DataSourcesInput::Validate { data_source_id } => {
                Ok(DataSourcesOutput::Validation(to_validate_response(
                    self.service(actor, true)
                        .validate_instance(ValidateDataSourceInstanceCommand {
                            actor_user_id: user_id,
                            workspace_id,
                            instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                        })
                        .await?,
                )))
            }
            DataSourcesInput::RotateSecret {
                data_source_id,
                body,
            } => Ok(DataSourcesOutput::DataSource(
                to_runtime_extension_data_source_response(
                    self.service(actor, false)
                        .rotate_secret(RotateDataSourceSecretCommand {
                            actor_user_id: user_id,
                            workspace_id,
                            instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                            secret_json: body.secret_json,
                        })
                        .await?,
                ),
            )),
            DataSourcesInput::ListResources { data_source_id } => {
                Ok(DataSourcesOutput::Resources(to_resources_response(
                    self.service(actor, true)
                        .list_resources(
                            user_id,
                            workspace_id,
                            parse_uuid(&data_source_id, "data_source_id")?,
                        )
                        .await?,
                )))
            }
            DataSourcesInput::DiscoverResources { data_source_id } => {
                Ok(DataSourcesOutput::Resources(to_resources_response(
                    self.service(actor, true)
                        .discover_resources(DiscoverDataSourceResourcesCommand {
                            actor_user_id: user_id,
                            workspace_id,
                            instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                        })
                        .await?,
                )))
            }
            DataSourcesInput::PreviewRead {
                data_source_id,
                body,
            } => Ok(DataSourcesOutput::Preview(to_preview_response(
                self.service(actor, true)
                    .preview_read(PreviewDataSourceReadCommand {
                        actor_user_id: user_id,
                        workspace_id,
                        instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                        resource_key: body.resource_key,
                        limit: body.limit,
                        cursor: body.cursor,
                        options_json: body.options_json,
                    })
                    .await?,
            ))),
            DataSourcesInput::MapResourceToModel {
                data_source_id,
                body,
            } => {
                let result = self
                    .service(actor, true)
                    .map_resource_to_model(MapDataSourceResourceToModelCommand {
                        actor_user_id: user_id,
                        workspace_id,
                        instance_id: parse_uuid(&data_source_id, "data_source_id")?,
                        resource_key: body.resource_key,
                        template_provider: body.template_provider,
                        template_code: body.template_code,
                        template_version: body.template_version,
                    })
                    .await?;
                self.runtime_registry_sync.rebuild().await?;
                let mut model = result.model;
                model.fields = result.fields;
                Ok(DataSourcesOutput::Model(to_model_definition_response(
                    model,
                    self.runtime_engine.template_catalog(),
                )))
            }
        }
    }
}

impl ConsoleInterfacePort<DataSourcesInput, DataSourcesOutput> for DataSourcesAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: DataSourcesInput,
    ) -> ConsoleInterfaceFuture<'a, DataSourcesOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/data-sources/catalog",
    operation_id = "data_source_list_catalog",
    responses((status = 200, body = DataSourceCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DataSourceCatalogResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.catalog.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        DataSourcesInput::ListCatalog,
    )
    .await?;
    let DataSourcesOutput::Catalog(response) = output else {
        unreachable!("data source catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/data-sources",
    operation_id = "data_source_list",
    responses((status = 200, body = [DataSourceResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_data_sources(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<DataSourceResponse>>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        DataSourcesInput::List,
    )
    .await?;
    let DataSourcesOutput::List(response) = output else {
        unreachable!("data sources list binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/data-sources",
    operation_id = "data_source_create",
    request_body = CreateDataSourceBody,
    responses((status = 201, body = DataSourceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_data_source(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateDataSourceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<DataSourceResponse>>), ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::Create(body),
    )
    .await?;
    let DataSourcesOutput::DataSource(response) = output else {
        unreachable!("data source create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/defaults",
    operation_id = "data_source_update_defaults",
    request_body = UpdateDataSourceDefaultsBody,
    responses((status = 200, body = DataSourceResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_defaults(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateDataSourceDefaultsBody>,
) -> Result<Json<ApiSuccess<DataSourceResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.defaults.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::UpdateDefaults {
            data_source_id,
            body,
        },
    )
    .await?;
    let DataSourcesOutput::DataSource(response) = output else {
        unreachable!("data source defaults binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/validate",
    operation_id = "data_source_validate",
    responses((status = 200, body = ValidateDataSourceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn validate_data_source(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ValidateDataSourceResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.validate.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::Validate { data_source_id },
    )
    .await?;
    let DataSourcesOutput::Validation(response) = output else {
        unreachable!("data source validate binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/data-sources/{data_source_id}/secret/rotate",
    operation_id = "data_source_rotate_secret",
    request_body = RotateDataSourceSecretBody,
    responses((status = 200, body = DataSourceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn rotate_secret(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<RotateDataSourceSecretBody>,
) -> Result<Json<ApiSuccess<DataSourceResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.secret.rotate.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::RotateSecret {
            data_source_id,
            body,
        },
    )
    .await?;
    let DataSourcesOutput::DataSource(response) = output else {
        unreachable!("data source secret rotation binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/resources",
    operation_id = "data_source_list_resources",
    responses((status = 200, body = DataSourceResourcesResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn list_resources(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DataSourceResourcesResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.resources.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        DataSourcesInput::ListResources { data_source_id },
    )
    .await?;
    let DataSourcesOutput::Resources(response) = output else {
        unreachable!("data source resources binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/resources/discover",
    operation_id = "data_source_discover_resources",
    responses((status = 200, body = DataSourceResourcesResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn discover_resources(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<DataSourceResourcesResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.resources.discover.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::DiscoverResources { data_source_id },
    )
    .await?;
    let DataSourcesOutput::Resources(response) = output else {
        unreachable!("data source discovery binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/preview-read",
    operation_id = "data_source_preview_read",
    request_body = PreviewDataSourceReadBody,
    responses((status = 200, body = PreviewDataSourceReadResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn preview_read(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<PreviewDataSourceReadBody>,
) -> Result<Json<ApiSuccess<PreviewDataSourceReadResponse>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.preview-read.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::PreviewRead {
            data_source_id,
            body,
        },
    )
    .await?;
    let DataSourcesOutput::Preview(response) = output else {
        unreachable!("data source preview binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/data-sources/{data_source_id}/resources/map-to-model",
    operation_id = "data_source_map_resource_to_model",
    request_body = MapDataSourceResourceToModelBody,
    responses((status = 201, body = ModelDefinitionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn map_resource_to_model(
    State(state): State<Arc<ApiState>>,
    Path(data_source_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<MapDataSourceResourceToModelBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelDefinitionResponse>>), ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.data-sources.resources.map-to-model.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        DataSourcesInput::MapResourceToModel {
            data_source_id,
            body,
        },
    )
    .await?;
    let DataSourcesOutput::Model(response) = output else {
        unreachable!("data source map-to-model binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}
