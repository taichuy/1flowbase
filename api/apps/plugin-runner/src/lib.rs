use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
};

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use plugin_framework::{
    data_source_contract::{
        DataSourceConfigInput, DataSourceCreateRecordInput, DataSourceCreateRecordOutput,
        DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput, DataSourceGetRecordInput,
        DataSourceGetRecordOutput, DataSourceImportSnapshotInput, DataSourceImportSnapshotOutput,
        DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourcePreviewReadInput,
        DataSourcePreviewReadOutput, DataSourceUpdateRecordInput, DataSourceUpdateRecordOutput,
    },
    error::{PluginFrameworkError, PluginFrameworkErrorKind},
    provider_contract::ProviderInvocationInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::data_source_host::{
    DataSourceCatalogOutput, DataSourceDescriptorOutput, DataSourceValueOutput,
    LoadedDataSourceSummary,
};
use crate::provider_host::{
    LoadedProviderSummary, ProviderActiveStreamsOutput, ProviderBalanceOutput, ProviderHost,
    ProviderInvokeStreamOutput, ProviderModelsOutput, ProviderValidationOutput,
};
pub use capability_host::CapabilityHost;
pub use data_source_host::DataSourceHost;

pub const DEFAULT_PLUGIN_RUNNER_ADDR: &str = "0.0.0.0:7801";
static STARTED_AT: OnceLock<OffsetDateTime> = OnceLock::new();

pub mod capability_host;
pub mod capability_stdio;
pub mod data_source_host;
pub mod data_source_stdio;
pub mod package_loader;
pub mod provider_host;
pub mod stdio_runtime;

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub service: &'static str,
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone)]
pub struct AppState {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
    runtime_profile_collector: Arc<runtime_profile::RuntimeProfileCollector>,
}

impl Default for AppState {
    fn default() -> Self {
        let started_at = *STARTED_AT.get_or_init(OffsetDateTime::now_utc);
        Self {
            provider_host: Arc::new(RwLock::new(ProviderHost::default())),
            capability_host: Arc::new(RwLock::new(CapabilityHost::default())),
            data_source_host: Arc::new(RwLock::new(DataSourceHost::default())),
            runtime_profile_collector: Arc::new(
                runtime_profile::RuntimeProfileCollector::new(
                    "plugin-runner",
                    env!("CARGO_PKG_VERSION"),
                    started_at,
                    "ok",
                )
                .expect("plugin runner runtime profile collector must initialize"),
            ),
        }
    }
}

impl AppState {
    pub fn with_capability_host(capability_host: CapabilityHost) -> Self {
        Self {
            capability_host: Arc::new(RwLock::new(capability_host)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Deserialize)]
struct LoadProviderRequest {
    package_root: String,
}

#[derive(Debug, Deserialize)]
struct ReloadProviderRequest {
    plugin_id: String,
}

#[derive(Debug, Deserialize)]
struct ValidateProviderRequest {
    plugin_id: String,
    #[serde(default)]
    provider_config: Value,
}

#[derive(Debug, Deserialize)]
struct ListModelsRequest {
    plugin_id: String,
    #[serde(default)]
    provider_config: Value,
}

#[derive(Debug, Deserialize)]
struct BalanceProviderRequest {
    plugin_id: String,
    #[serde(default)]
    provider_config: Value,
}

#[derive(Debug, Deserialize)]
struct InvokeProviderRequest {
    plugin_id: String,
    input: ProviderInvocationInput,
}

#[derive(Debug, Deserialize)]
struct ValidateCapabilityRequest {
    plugin_id: String,
    contribution_code: String,
    #[serde(default)]
    config_payload: Value,
}

#[derive(Debug, Deserialize)]
struct LoadDataSourceRequest {
    package_root: String,
}

#[derive(Debug, Deserialize)]
struct ReloadDataSourceRequest {
    plugin_id: String,
}

#[derive(Debug, Deserialize)]
struct DataSourceConnectionRequest {
    plugin_id: String,
    #[serde(flatten)]
    input: DataSourceConfigInput,
}

#[derive(Debug, Deserialize)]
struct DescribeDataSourceRequest {
    plugin_id: String,
    #[serde(flatten)]
    input: DataSourceConfigInput,
    resource_key: String,
}

#[derive(Debug, Deserialize)]
struct PreviewDataSourceRequest {
    plugin_id: String,
    input: DataSourcePreviewReadInput,
}

#[derive(Debug, Deserialize)]
struct ImportDataSourceRequest {
    plugin_id: String,
    input: DataSourceImportSnapshotInput,
}

#[derive(Debug, Deserialize)]
struct ListDataSourceRecordsRequest {
    plugin_id: String,
    input: DataSourceListRecordsInput,
}

#[derive(Debug, Deserialize)]
struct GetDataSourceRecordRequest {
    plugin_id: String,
    input: DataSourceGetRecordInput,
}

#[derive(Debug, Deserialize)]
struct CreateDataSourceRecordRequest {
    plugin_id: String,
    input: DataSourceCreateRecordInput,
}

#[derive(Debug, Deserialize)]
struct UpdateDataSourceRecordRequest {
    plugin_id: String,
    input: DataSourceUpdateRecordInput,
}

#[derive(Debug, Deserialize)]
struct DeleteDataSourceRecordRequest {
    plugin_id: String,
    input: DataSourceDeleteRecordInput,
}

#[derive(Debug, Deserialize)]
struct ExecuteCapabilityRequest {
    plugin_id: String,
    contribution_code: String,
    #[serde(default)]
    config_payload: Value,
    #[serde(default)]
    input_payload: Value,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "plugin-runner",
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn system_runtime_profile(
    State(state): State<AppState>,
) -> Result<Json<runtime_profile::RuntimeProfile>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.runtime_profile_collector.clone();
    tokio::task::spawn_blocking(move || collector.collect())
        .await
        .map_err(map_internal_error)?
        .map(Json)
        .map_err(map_internal_error)
}

async fn load_provider(
    State(state): State<AppState>,
    Json(request): Json<LoadProviderRequest>,
) -> Result<Json<LoadedProviderSummary>, (StatusCode, Json<ErrorResponse>)> {
    let mut host = state.provider_host.write().await;
    host.load(&request.package_root)
        .map(Json)
        .map_err(map_framework_error)
}

async fn reload_provider(
    State(state): State<AppState>,
    Json(request): Json<ReloadProviderRequest>,
) -> Result<Json<LoadedProviderSummary>, (StatusCode, Json<ErrorResponse>)> {
    let mut host = state.provider_host.write().await;
    host.reload(&request.plugin_id)
        .map(Json)
        .map_err(map_framework_error)
}

async fn validate_provider(
    State(state): State<AppState>,
    Json(request): Json<ValidateProviderRequest>,
) -> Result<Json<ProviderValidationOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.provider_host.read().await;
        host.validate_operation(&request.plugin_id, request.provider_config)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn list_models(
    State(state): State<AppState>,
    Json(request): Json<ListModelsRequest>,
) -> Result<Json<ProviderModelsOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.provider_host.read().await;
        host.list_models_operation(&request.plugin_id, request.provider_config)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn get_balance(
    State(state): State<AppState>,
    Json(request): Json<BalanceProviderRequest>,
) -> Result<Json<ProviderBalanceOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.provider_host.read().await;
        host.get_balance_operation(&request.plugin_id, request.provider_config)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn invoke_stream(
    State(state): State<AppState>,
    Json(request): Json<InvokeProviderRequest>,
) -> Result<Json<ProviderInvokeStreamOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.provider_host.read().await;
        host.invoke_stream_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn active_provider_streams(
    State(state): State<AppState>,
) -> Result<Json<ProviderActiveStreamsOutput>, (StatusCode, Json<ErrorResponse>)> {
    let host = state.provider_host.read().await;
    Ok(Json(host.active_stream_snapshot().await))
}

async fn validate_capability_config(
    State(state): State<AppState>,
    Json(request): Json<ValidateCapabilityRequest>,
) -> Result<Json<capability_host::CapabilityValueOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.capability_host.read().await;
        host.validate_config_operation(
            &request.plugin_id,
            &request.contribution_code,
            request.config_payload,
        )
        .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn load_data_source(
    State(state): State<AppState>,
    Json(request): Json<LoadDataSourceRequest>,
) -> Result<Json<LoadedDataSourceSummary>, (StatusCode, Json<ErrorResponse>)> {
    let mut host = state.data_source_host.write().await;
    host.load(&request.package_root)
        .map(Json)
        .map_err(map_framework_error)
}

async fn reload_data_source(
    State(state): State<AppState>,
    Json(request): Json<ReloadDataSourceRequest>,
) -> Result<Json<LoadedDataSourceSummary>, (StatusCode, Json<ErrorResponse>)> {
    let mut host = state.data_source_host.write().await;
    host.reload(&request.plugin_id)
        .map(Json)
        .map_err(map_framework_error)
}

async fn validate_data_source_config(
    State(state): State<AppState>,
    Json(request): Json<DataSourceConnectionRequest>,
) -> Result<Json<DataSourceValueOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.validate_config_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn test_data_source_connection(
    State(state): State<AppState>,
    Json(request): Json<DataSourceConnectionRequest>,
) -> Result<Json<DataSourceValueOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.test_connection_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn discover_data_source_catalog(
    State(state): State<AppState>,
    Json(request): Json<DataSourceConnectionRequest>,
) -> Result<Json<DataSourceCatalogOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.discover_catalog_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn describe_data_source_resource(
    State(state): State<AppState>,
    Json(request): Json<DescribeDataSourceRequest>,
) -> Result<Json<DataSourceDescriptorOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.describe_resource_operation(&request.plugin_id, request.input, request.resource_key)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn preview_data_source_read(
    State(state): State<AppState>,
    Json(request): Json<PreviewDataSourceRequest>,
) -> Result<Json<DataSourcePreviewReadOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.preview_read_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn import_data_source_snapshot(
    State(state): State<AppState>,
    Json(request): Json<ImportDataSourceRequest>,
) -> Result<Json<DataSourceImportSnapshotOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.import_snapshot_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn list_data_source_records(
    State(state): State<AppState>,
    Json(request): Json<ListDataSourceRecordsRequest>,
) -> Result<Json<DataSourceListRecordsOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.list_records_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn get_data_source_record(
    State(state): State<AppState>,
    Json(request): Json<GetDataSourceRecordRequest>,
) -> Result<Json<DataSourceGetRecordOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.get_record_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn create_data_source_record(
    State(state): State<AppState>,
    Json(request): Json<CreateDataSourceRecordRequest>,
) -> Result<Json<DataSourceCreateRecordOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.create_record_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn update_data_source_record(
    State(state): State<AppState>,
    Json(request): Json<UpdateDataSourceRecordRequest>,
) -> Result<Json<DataSourceUpdateRecordOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.update_record_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn delete_data_source_record(
    State(state): State<AppState>,
    Json(request): Json<DeleteDataSourceRecordRequest>,
) -> Result<Json<DataSourceDeleteRecordOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.data_source_host.read().await;
        host.delete_record_operation(&request.plugin_id, request.input)
            .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn resolve_capability_dynamic_options(
    State(state): State<AppState>,
    Json(request): Json<ValidateCapabilityRequest>,
) -> Result<Json<capability_host::CapabilityValueOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.capability_host.read().await;
        host.resolve_dynamic_options_operation(
            &request.plugin_id,
            &request.contribution_code,
            request.config_payload,
        )
        .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn resolve_capability_output_schema(
    State(state): State<AppState>,
    Json(request): Json<ValidateCapabilityRequest>,
) -> Result<Json<capability_host::CapabilityValueOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.capability_host.read().await;
        host.resolve_output_schema_operation(
            &request.plugin_id,
            &request.contribution_code,
            request.config_payload,
        )
        .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

async fn execute_capability(
    State(state): State<AppState>,
    Json(request): Json<ExecuteCapabilityRequest>,
) -> Result<Json<capability_host::CapabilityExecutionOutput>, (StatusCode, Json<ErrorResponse>)> {
    let operation = {
        let host = state.capability_host.read().await;
        host.execute_operation(
            &request.plugin_id,
            &request.contribution_code,
            request.config_payload,
            request.input_payload,
        )
        .map_err(map_framework_error)?
    };
    operation.await.map(Json).map_err(map_framework_error)
}

pub fn parse_bind_addr(candidate: Option<&str>, default_addr: &str) -> SocketAddr {
    candidate
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| default_addr.parse().unwrap())
}

pub fn app() -> Router {
    app_with_state(AppState::default())
}

pub fn app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/system/runtime-profile", get(system_runtime_profile))
        .route("/providers/load", post(load_provider))
        .route("/providers/reload", post(reload_provider))
        .route("/providers/validate", post(validate_provider))
        .route("/providers/list-models", post(list_models))
        .route("/providers/balance", post(get_balance))
        .route("/providers/invoke-stream", post(invoke_stream))
        .route("/providers/active-streams", get(active_provider_streams))
        .route("/data-sources/load", post(load_data_source))
        .route("/data-sources/reload", post(reload_data_source))
        .route(
            "/data-sources/validate-config",
            post(validate_data_source_config),
        )
        .route(
            "/data-sources/test-connection",
            post(test_data_source_connection),
        )
        .route(
            "/data-sources/discover-catalog",
            post(discover_data_source_catalog),
        )
        .route(
            "/data-sources/describe-resource",
            post(describe_data_source_resource),
        )
        .route("/data-sources/preview-read", post(preview_data_source_read))
        .route(
            "/data-sources/import-snapshot",
            post(import_data_source_snapshot),
        )
        .route("/data-sources/list-records", post(list_data_source_records))
        .route("/data-sources/get-record", post(get_data_source_record))
        .route(
            "/data-sources/create-record",
            post(create_data_source_record),
        )
        .route(
            "/data-sources/update-record",
            post(update_data_source_record),
        )
        .route(
            "/data-sources/delete-record",
            post(delete_data_source_record),
        )
        .route(
            "/capabilities/validate-config",
            post(validate_capability_config),
        )
        .route(
            "/capabilities/resolve-dynamic-options",
            post(resolve_capability_dynamic_options),
        )
        .route(
            "/capabilities/resolve-output-schema",
            post(resolve_capability_output_schema),
        )
        .route("/capabilities/execute", post(execute_capability))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

fn map_framework_error(error: PluginFrameworkError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error.kind() {
        PluginFrameworkErrorKind::Io | PluginFrameworkErrorKind::RuntimeContract => {
            StatusCode::BAD_GATEWAY
        }
        PluginFrameworkErrorKind::InvalidAssignment
        | PluginFrameworkErrorKind::InvalidProviderPackage
        | PluginFrameworkErrorKind::InvalidProviderContract
        | PluginFrameworkErrorKind::Serialization => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(ErrorResponse {
            message: error.to_string(),
        }),
    )
}

fn map_internal_error(error: impl std::fmt::Display) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            message: error.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_bind_addr, DEFAULT_PLUGIN_RUNNER_ADDR};

    #[test]
    fn parse_bind_addr_uses_runner_default_port() {
        let addr = parse_bind_addr(None, DEFAULT_PLUGIN_RUNNER_ADDR);

        assert_eq!(addr.to_string(), "0.0.0.0:7801");
    }

    #[test]
    fn parse_bind_addr_keeps_valid_override() {
        let addr = parse_bind_addr(Some("127.0.0.1:8899"), DEFAULT_PLUGIN_RUNNER_ADDR);

        assert_eq!(addr.to_string(), "127.0.0.1:8899");
    }
}
