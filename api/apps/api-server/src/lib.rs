#![recursion_limit = "256"]

extern crate self as api_server;
#[path = "boot_composition/generation_dispatch.rs"]
mod generation_dispatch;
#[path = "boot_composition/managed_publication.rs"]
mod managed_publication;
mod managed_service_mcp;
mod managed_services;
mod resource_import;

pub mod agent_logs_protocol;
pub mod app_state;
pub(crate) mod application_log_exchange;
pub mod application_public_docs;
mod authentication_runtime;
mod communication_runtime;
pub mod config;
#[path = "console_contracts/console_operation_compilation.rs"]
pub(crate) mod console_operation_compilation;
pub mod console_policy_migration;
#[path = "console_contracts/console_surface_registry.rs"]
pub mod console_surface_registry;
pub mod error_response;
pub mod extension_bus;
#[path = "console_contracts/external_endpoint_catalog.rs"]
pub(crate) mod external_endpoint_catalog;
#[path = "console_contracts/external_route_assembly.rs"]
pub(crate) mod external_route_assembly;
#[path = "boot_composition/host_extension_boot.rs"]
pub mod host_extension_boot;
#[path = "boot_composition/host_extension_loader.rs"]
pub mod host_extension_loader;
pub mod host_extensions;
pub mod host_infrastructure;
#[path = "boot_composition/host_route_registry.rs"]
pub mod host_route_registry;
#[path = "boot_composition/host_worker_registry.rs"]
pub mod host_worker_registry;
pub mod middleware;
#[path = "catalog_sources/model_pricing_catalog.rs"]
pub(crate) mod model_pricing_catalog;
pub mod network_egress_client;
pub mod network_egress_probe;
mod notification_runtime;
#[path = "catalog_sources/official_extension_catalog.rs"]
pub mod official_extension_catalog;
#[path = "catalog_sources/official_i18n_catalog_seed.rs"]
pub mod official_i18n_catalog_seed;
#[path = "catalog_sources/official_i18n_catalog_source.rs"]
pub mod official_i18n_catalog_source;
#[path = "catalog_sources/official_mcp_bundles.rs"]
pub mod official_mcp_bundles;
pub mod openapi;
pub mod openapi_docs;
pub mod openapi_interface;
pub mod provider_runtime;
pub mod recovery_authorization;
pub mod response;
pub mod routes;
pub mod runtime_activity;
pub mod runtime_data_model_docs;
pub mod runtime_profile_client;
pub mod runtime_registry_sync;
pub mod system_backup;
pub mod system_recovery;
#[path = "catalog_sources/ui_component_catalog_source.rs"]
pub mod ui_component_catalog_source;
pub mod workers;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use argon2::{password_hash::PasswordHasher, Argon2};
use axum::{middleware as axum_middleware, routing::get, Json, Router};
use control_plane::{
    bootstrap::{BootstrapConfig, BootstrapService},
    plugin_management::ready_current_node_plugin_installation,
    ports::{DataSourceRuntimePort, PluginRepository},
};
use serde::Serialize;
use time::OffsetDateTime;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::{Config as SwaggerUiConfig, SwaggerUi};

use crate::{
    app_state::{build_official_i18n_catalog_update_service, ApiState},
    config::{ApiConfig, ApiEnvironment},
    host_extension_loader::{
        activate_prepared_host_extensions, prepare_host_extensions_at_startup,
    },
    host_extensions::console::{
        linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
    },
    host_infrastructure::build_host_infrastructure_from_host_extensions,
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    provider_runtime::{
        ApiDataSourceRuntimeRecordBackend, ApiProviderRuntime, ApiRuntimeArtifactResolver,
        ApiRuntimeServices,
    },
    runtime_profile_client::HostApiRuntimeProfileCollector,
};

pub const DEFAULT_API_SERVER_ADDR: &str = "0.0.0.0:7800";

/// API composition root for state-free runtime MCP tool invokers.
pub(crate) async fn runtime_internal_tool_invoker_factory(
    state: &Arc<ApiState>,
    actor: &domain::ActorContext,
) -> Result<
    routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerFactory,
    error_response::ApiError,
> {
    let owner = generation_dispatch::GenerationDispatch::acquire(state)?;
    let interface_catalog =
        routes::mcp_management::mcp_interface_catalog_entries(state.as_ref(), actor).await?;
    Ok(
        routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerFactory::new(
            routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerDependencies::new(
                state.store.clone(),
                state.infrastructure.cache_store(),
                state.provider_secret_master_key.clone(),
            )
            .with_managed_services(state.console_surface_registry.managed_services().to_vec()),
            Arc::new(
                routes::mcp_protocol::virtual_ui::McpInterfaceCatalogSnapshot::new(
                    interface_catalog,
                ),
            ),
            owner,
        ),
    )
}

#[derive(Default)]
struct ApiLifecycleDeliveryCompletion(std::sync::Weak<extension_bus::ManagedExtensionComposition>);

impl control_plane::lifecycle_outbox_dispatcher::LifecycleDeliveryCompletionPort
    for ApiLifecycleDeliveryCompletion
{
    fn complete(
        &self,
        outcome: extension_contracts::CompletionOutcome<
            control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryCompletion,
        >,
    ) {
        if let Some(composition) = self.0.upgrade() {
            composition.notify_reclamation();
        }
        tracing::info!(
            event_id = %outcome.payload().event_id,
            terminal = ?outcome.terminal(),
            "lifecycle fact delivery completed"
        );
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub service: &'static str,
    pub status: &'static str,
    pub version: &'static str,
}

#[utoipa::path(get, path = "/health", responses((status = 200, body = HealthResponse)))]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "api-server",
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn intake_active_data_source_templates_at_startup(
    store: &storage_durable_postgres::MainDurableStore,
    runtime: &ApiProviderRuntime,
    api_node_id: &str,
    provider_install_root: &str,
) -> Result<()> {
    let installations = store.list_installations().await?;
    for installation in installations.into_iter().filter(|installation| {
        installation.contract_version == "1flowbase.data_source/v1"
            && installation.desired_state == domain::PluginDesiredState::ActiveRequested
    }) {
        let local_installation = match ready_current_node_plugin_installation(
            store,
            api_node_id,
            std::path::Path::new(provider_install_root),
            installation.id,
        )
        .await
        {
            Ok(installation) => installation,
            Err(error) => {
                tracing::warn!(
                    installation_id = %installation.id,
                    error = %error,
                    "active data source template intake skipped"
                );
                continue;
            }
        };
        if let Err(error) = DataSourceRuntimePort::ensure_loaded(runtime, &local_installation).await
        {
            tracing::warn!(
                installation_id = %installation.id,
                error = %error,
                "active data source template intake failed; dependent models remain unavailable"
            );
        }
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/console/health",
    responses((status = 200, body = HealthResponse))
)]
pub(crate) async fn console_health() -> Json<HealthResponse> {
    health().await
}

pub fn parse_bind_addr(candidate: Option<&str>, default_addr: &str) -> Result<SocketAddr> {
    match candidate {
        Some(value) => value
            .parse()
            .map_err(|err| anyhow!("invalid API_SERVER_ADDR `{value}`: {err}")),
        None => default_addr
            .parse()
            .map_err(|err| anyhow!("invalid default API server address `{default_addr}`: {err}")),
    }
}

fn development_cors_layer() -> CorsLayer {
    CorsLayer::very_permissive()
}

fn cors_layer(config: &ApiConfig) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_credentials(true)
        .allow_headers(AllowHeaders::mirror_request())
        .allow_methods(AllowMethods::mirror_request());

    match &config.cors_allowed_origins {
        Some(origins) => base.allow_origin(AllowOrigin::list(origins.clone())),
        None => development_cors_layer(),
    }
}

fn base_router(include_docs_ui: bool, static_openapi: bool) -> Router {
    let router = Router::new().route("/health", get(health));

    if include_docs_ui && static_openapi {
        router.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi::ApiDoc::openapi()))
    } else if include_docs_ui {
        router.merge(SwaggerUi::new("/docs").config(SwaggerUiConfig::from("/openapi.json")))
    } else {
        router
    }
}

#[cfg(test)]
pub(crate) fn root_external_endpoint_contributions(
    include_docs: bool,
) -> Vec<external_endpoint_catalog::ExternalEndpointContribution> {
    let mut contributions = vec![
        external_endpoint_catalog::ExternalEndpointContribution::unclassified_http(
            "api-server.root-router",
            "GET",
            "/health",
        ),
        external_endpoint_catalog::ExternalEndpointContribution::unclassified_http(
            "api-server.root-router",
            "GET",
            "/openapi.json",
        ),
    ];
    if include_docs {
        contributions.push(
            external_endpoint_catalog::ExternalEndpointContribution::unclassified_http(
                "api-server.root-router",
                "GET",
                "/docs",
            ),
        );
    }
    contributions
}

fn publish_external_endpoint_catalog(
    state: &Arc<ApiState>,
    mounted: &[external_endpoint_catalog::ExternalEndpointContribution],
    include_docs: bool,
    openapi_document: &serde_json::Value,
) -> Result<()> {
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow!("extension boot snapshot is unavailable"))?;
    let registry = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow!("compiled interface registry is unavailable"))?
        .snapshot();
    let mut compiler = external_endpoint_catalog::ExternalEndpointCatalogCompiler::default();
    compiler.contribute_openapi_document("api-server.openapi", openapi_document)?;
    compiler.absorb_registry("compiled-interface-registry", registry.as_ref())?;
    compiler.contribute_managed_stream_controls(
        state.console_surface_registry.managed_services(),
        registry.as_ref(),
    )?;
    compiler.contribute_mcp_protocol_surface(routes::mcp_protocol::MCP_INVOCATION_BINDING_ID)?;
    compiler.contribute_approved_controls(include_docs)?;
    compiler.contribute_mounted_routes(mounted)?;
    let catalog = compiler.compile_complete(registry.as_ref())?;
    tracing::info!(
        total = catalog.rows().len(),
        canonical_business = catalog.classification_count(
            external_endpoint_catalog::ExternalEndpointClassification::CanonicalBusinessInterface,
        ),
        protocol_control = catalog.classification_count(
            external_endpoint_catalog::ExternalEndpointClassification::ProtocolControl,
        ),
        operational_control = catalog.classification_count(
            external_endpoint_catalog::ExternalEndpointClassification::OperationalControl,
        ),
        unclassified = catalog.classification_count(
            external_endpoint_catalog::ExternalEndpointClassification::Unclassified,
        ),
        "published complete external endpoint catalog"
    );
    boot_snapshot.publish_external_endpoint_catalog(catalog);
    Ok(())
}

pub fn app() -> Router {
    base_router(true, true)
        .layer(development_cors_layer())
        .layer(TraceLayer::new_for_http().make_span_with(safe_http_trace_span))
}

pub fn app_with_state(state: Arc<ApiState>) -> Router {
    let owner = state.extension_boot_snapshot.as_ref().map(|snapshot| {
        snapshot
            .publish_complete_catalog(&state)
            .expect("complete interface catalog must publish before router construction")
    });
    let interface_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot());
    let assembly = routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        interface_snapshot.as_deref(),
    );
    let openapi_document = serde_json::to_value(openapi::ApiDoc::openapi())
        .expect("static OpenAPI document must serialize");
    let (router, mounted) = console_router_with_assembly(state.clone(), true, assembly);
    publish_external_endpoint_catalog(&state, &mounted, true, &openapi_document)
        .expect("external endpoint catalog must publish before router construction");
    let router = match owner {
        Some(owner) => generation_dispatch::serving_router(router, owner),
        None => router,
    };
    router
        .layer(development_cors_layer())
        .layer(TraceLayer::new_for_http().make_span_with(safe_http_trace_span))
}

#[cfg(test)]
fn console_router(state: Arc<ApiState>, include_openapi: bool) -> Router {
    let interface_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot());
    console_router_for_registry(state, include_openapi, interface_snapshot.as_deref())
}

fn console_router_for_registry(
    state: Arc<ApiState>,
    include_openapi: bool,
    interface_registry: Option<&interface_runtime::CompiledInterfaceRegistry>,
) -> Router {
    #[cfg(test)]
    if let Some(boot) = &state.extension_boot_snapshot {
        boot.record_dispatch_assembly();
    }
    let assembly = routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        interface_registry,
    );
    let assembly = assembly.merge(crate::managed_services::route_assembly(
        state.console_surface_registry.managed_services(),
    ));
    console_router_with_assembly(state, include_openapi, assembly).0
}

fn console_router_with_assembly(
    state: Arc<ApiState>,
    include_openapi: bool,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
) -> (
    Router,
    Vec<external_endpoint_catalog::ExternalEndpointContribution>,
) {
    let maintenance_classifier =
        middleware::system_maintenance::SystemMaintenanceRequestClassifier::new(
            console_route_assembly.maintenance_control_routes().to_vec(),
        );
    let router = external_route_assembly::ExternalRouteAssembly::new()
        .route("/health", external_route_assembly::get(health))
        .merge(routes::application_public_api::compatible_route_assembly())
        .merge(routes::application_public_api::agent_logs::route_assembly())
        .merge(routes::application_public_api::agent_logs::otlp::route_assembly())
        .nest(
            "/api/agent/v1",
            routes::application_public_api::route_assembly(),
        )
        .nest(
            "/api/ex",
            routes::application_public_api::ex::route_assembly(),
        )
        .merge(routes::branding::public_interface::route_assembly())
        .merge(routes::client_collectors::route_assembly())
        .merge(routes::files::public::route_assembly())
        .merge(routes::mcp_oauth::route_assembly())
        .nest("/api", routes::mcp_protocol::route_assembly())
        .nest("/api", routes::webmcp::route_assembly())
        .nest(
            "/api/console",
            external_route_assembly::ExternalRouteAssembly::console(console_route_assembly),
        )
        .nest("/api/runtime", routes::runtime_models::route_assembly())
        .nest("/api/public/auth", routes::auth::route_assembly());

    let router = if include_openapi {
        router
            .route(
                "/openapi.json",
                external_route_assembly::get(openapi::dynamic_openapi),
            )
            .merge(external_route_assembly::ExternalRouteAssembly::docs())
    } else {
        router
    };

    let mounted = router.contributions();
    let router = router
        .into_router()
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_settings_feature_permission::require_settings_feature_permission,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::system_maintenance::fence_mutating_requests,
        ))
        .layer(axum_middleware::from_fn_with_state(
            maintenance_classifier,
            middleware::system_maintenance::classify_system_maintenance_request,
        ))
        .with_state(state);
    (router, mounted)
}

pub fn app_with_state_and_config(state: Arc<ApiState>, config: &ApiConfig) -> Router {
    let interface_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot());
    let assembly = routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
        interface_snapshot.as_deref(),
        config.plugin_upload_max_bytes,
    );
    let owner = state.extension_boot_snapshot.as_ref().map(|snapshot| {
        snapshot
            .publish_complete_catalog_with_route_assembly(&state, assembly.clone())
            .expect("complete interface catalog must publish before router construction")
    });
    let openapi_document = serde_json::to_value(openapi::ApiDoc::openapi())
        .expect("static OpenAPI document must serialize");
    let router = app_with_state_and_config_and_console_route_assembly(
        state,
        config,
        assembly,
        &openapi_document,
    );
    match owner {
        Some(owner) => generation_dispatch::serving_router(router, owner),
        None => router,
    }
}

fn app_with_state_and_config_and_console_route_assembly(
    state: Arc<ApiState>,
    config: &ApiConfig,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
    openapi_document: &serde_json::Value,
) -> Router {
    let include_docs = config.env != ApiEnvironment::Production;
    let (router, mounted) =
        console_router_with_assembly(state.clone(), include_docs, console_route_assembly);
    publish_external_endpoint_catalog(&state, &mounted, include_docs, openapi_document)
        .expect("external endpoint catalog must publish before router construction");
    router
        .layer(cors_layer(config))
        .layer(TraceLayer::new_for_http().make_span_with(safe_http_trace_span))
}

fn safe_http_trace_span(request: &axum::http::Request<axum::body::Body>) -> tracing::Span {
    let uri = safe_http_trace_uri(request.uri());
    tracing::debug_span!("request", method = %request.method(), uri = %uri, version = ?request.version())
}

fn safe_http_trace_uri(uri: &axum::http::Uri) -> String {
    if matches!(
        uri.path(),
        "/api/public/files/content"
            | "/api/public/auth/external/callback"
            | "/api/public/auth/external/start"
    ) {
        uri.path().to_owned()
    } else {
        uri.to_string()
    }
}

pub async fn app_from_env() -> Result<Router> {
    let config = ApiConfig::from_env()?;
    app_from_config(&config).await
}

pub async fn app_from_config(config: &ApiConfig) -> Result<Router> {
    app_and_runtime_host_from_config(config)
        .await
        .map(|(router, owner)| router.layer(axum::Extension(Arc::new(owner))))
}

pub async fn app_and_runtime_host_from_env() -> Result<(Router, ApiRuntimeShutdown)> {
    let config = ApiConfig::from_env()?;
    app_and_runtime_host_from_config(&config).await
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::{
        api_workspace_root, parse_bind_addr, resolve_system_backup_startup, DEFAULT_API_SERVER_ADDR,
    };

    #[test]
    fn parse_bind_addr_uses_new_default_api_port() {
        let addr = parse_bind_addr(None, DEFAULT_API_SERVER_ADDR).unwrap();

        assert_eq!(addr.to_string(), "0.0.0.0:7800");
    }

    #[test]
    fn parse_bind_addr_rejects_invalid_value() {
        let error = parse_bind_addr(Some("not-an-addr"), DEFAULT_API_SERVER_ADDR).unwrap_err();

        assert!(error.to_string().contains("API_SERVER_ADDR"));
    }

    #[test]
    fn api_workspace_root_contains_builtin_host_extensions() {
        let root = api_workspace_root().unwrap();

        assert!(root.join("plugins/host-extensions").is_dir());
    }

    #[test]
    fn unavailable_postgresql_toolchain_does_not_block_api_startup() {
        let resolved = resolve_system_backup_startup(Err(
            crate::system_backup::SystemBackupRuntimeError::PostgreSqlToolchainUnavailable,
        ))
        .expect("PostgreSQL backup toolchain failure should degrade the optional capability");

        assert!(resolved.is_none());
    }

    #[test]
    fn system_backup_initialization_failure_does_not_block_api_startup() {
        let result = resolve_system_backup_startup(Err(
            crate::system_backup::SystemBackupRuntimeError::PostgreSqlPreflight,
        ))
        .expect("optional system backup failure should degrade startup");

        assert!(result.is_none());
    }
}

#[cfg(all(test, feature = "root-1805-consumer-fixture"))]
#[path = "_tests/root_1805_consumer_fixture.rs"]
mod _tests;

#[cfg(all(test, not(feature = "root-1805-consumer-fixture")))]
mod _tests;

/// Actual server shutdown owner. A timeout preserves closed admission and all unfinished owners.
pub struct ApiRuntimeShutdown {
    host: Arc<runtime_extension_host::RuntimeExtensionHost>,
    services: Arc<ApiRuntimeServices>,
    operations: Arc<dyn control_plane_contracts::ports::ManagedOperationLifetime>,
    lifecycle_worker: control_plane::lifecycle_outbox_dispatcher::LifecycleOutboxWorker,
    communication_worker: communication_runtime::CommunicationWorker,
}
impl ApiRuntimeShutdown {
    pub async fn stop(&self) -> Result<()> {
        self.communication_worker.stop().await;
        crate::managed_services::stream::revoke_all();
        self.services
            .shutdown_transport_sessions(std::time::Duration::from_secs(10))
            .await;
        self.lifecycle_worker.close();
        self.lifecycle_worker
            .wait(std::time::Duration::from_secs(15))
            .await?;
        let composition = self.services.managed_composition()?;
        composition.close_owned_admission();
        composition
            .wait_owned_shutdown(std::time::Duration::from_secs(15))
            .await?;
        self.operations.close();
        self.operations
            .wait(std::time::Duration::from_secs(15))
            .await?;
        composition
            .wait_for_shutdown(std::time::Duration::from_secs(15))
            .await?;
        self.host.stop().await?;
        composition.cleanup_after_shutdown().await;
        Ok(())
    }
}

#[path = "internal/startup_assembly.rs"]
mod startup_assembly;

use startup_assembly::*;
