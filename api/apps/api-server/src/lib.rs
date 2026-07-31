#![recursion_limit = "256"]

extern crate self as api_server;

pub mod app_state;
pub mod application_public_docs;
pub mod config;
pub mod console_policy_migration;
pub mod console_surface_registry;
pub mod error_response;
pub mod host_extension_boot;
pub mod host_extension_loader;
pub mod host_extensions;
pub mod host_infrastructure;
pub mod host_route_registry;
pub mod host_worker_registry;
pub mod middleware;
pub mod official_agent_flow_templates;
pub mod official_i18n_catalog_seed;
pub mod official_i18n_catalog_source;
pub mod official_mcp_bundles;
pub mod official_plugin_registry;
pub mod openapi;
pub mod openapi_docs;
pub mod openapi_interface;
pub mod provider_runtime;
pub mod response;
pub mod routes;
pub mod runtime_activity;
pub mod runtime_data_model_docs;
pub mod runtime_profile_client;
pub mod runtime_registry_sync;
pub mod workers;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use axum::{middleware as axum_middleware, routing::get, Json, Router};
use control_plane::bootstrap::{BootstrapConfig, BootstrapService};
use rand_core::OsRng;
use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tower_http::{
    cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::{Config as SwaggerUiConfig, SwaggerUi};

use crate::{
    app_state::{build_official_i18n_catalog_update_service, compile_console_boot_plan, ApiState},
    config::{ApiConfig, ApiEnvironment},
    host_extension_loader::{
        activate_prepared_host_extensions, prepare_host_extensions_at_startup,
    },
    host_extensions::console::{
        linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
    },
    host_infrastructure::build_local_host_infrastructure_from_host_extensions,
    provider_runtime::{ApiDataSourceRuntimeRecordBackend, ApiProviderRuntime, ApiRuntimeServices},
    runtime_profile_client::{HostApiRuntimeProfileCollector, HttpPluginRunnerSystemClient},
};

pub const DEFAULT_API_SERVER_ADDR: &str = "0.0.0.0:7800";

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

pub fn app() -> Router {
    base_router(true, true)
        .layer(development_cors_layer())
        .layer(TraceLayer::new_for_http())
}

pub fn app_with_state(state: Arc<ApiState>) -> Router {
    base_router(true, false)
        .merge(console_router(state, true))
        .layer(development_cors_layer())
        .layer(TraceLayer::new_for_http())
}

fn console_router(state: Arc<ApiState>, include_openapi: bool) -> Router {
    console_router_with_assembly(
        state,
        include_openapi,
        routes::console_route_assembly::migrated_core_console_route_assembly(),
    )
}

fn console_router_with_assembly(
    state: Arc<ApiState>,
    include_openapi: bool,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
) -> Router {
    let router = Router::new()
        .merge(routes::application_public_api::compatible_router())
        .nest("/api/agent/v1", routes::application_public_api::router())
        .nest("/api/ex", routes::application_public_api::ex::router())
        .nest("/api", routes::mcp_protocol::router())
        .nest("/api/console", console_route_assembly.into_router())
        .nest("/api/runtime", routes::runtime_models::router())
        .nest("/api/public/auth", routes::auth::router());

    let router = if include_openapi {
        router.route("/openapi.json", get(openapi::dynamic_openapi))
    } else {
        router
    };

    router
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_settings_feature_permission::require_settings_feature_permission,
        ))
        .with_state(state)
}

pub fn app_with_state_and_config(state: Arc<ApiState>, config: &ApiConfig) -> Router {
    app_with_state_and_config_and_console_route_assembly(
        state,
        config,
        routes::console_route_assembly::migrated_core_console_route_assembly(),
    )
}

fn app_with_state_and_config_and_console_route_assembly(
    state: Arc<ApiState>,
    config: &ApiConfig,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
) -> Router {
    let include_docs = config.env != ApiEnvironment::Production;
    base_router(include_docs, false)
        .merge(console_router_with_assembly(
            state,
            include_docs,
            console_route_assembly,
        ))
        .layer(cors_layer(config))
        .layer(TraceLayer::new_for_http())
}

pub async fn app_from_env() -> Result<Router> {
    let config = ApiConfig::from_env()?;
    app_from_config(&config).await
}

pub async fn app_from_config(config: &ApiConfig) -> Result<Router> {
    let durable = storage_durable::build_main_durable_postgres_with_max_connections(
        &config.database_url,
        config.database_pool_max_connections,
    )
    .await?;
    let store = durable.store.clone();
    console_policy_migration::require_runtime_console_policy_cutover(&store).await?;
    let builtin_host_extensions =
        host_extensions::builtin::load_builtin_host_extension_manifests(api_workspace_root()?)?;
    let mut host_extension_registry =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &builtin_host_extensions,
        )?;
    let infrastructure = Arc::new(build_local_host_infrastructure_from_host_extensions(
        &host_extension_registry,
    )?);
    let session_store = infrastructure
        .session_store()
        .expect("storage-ephemeral default provider must provide session store");
    let runtime_event_stream = infrastructure
        .runtime_event_stream()
        .expect("runtime-event-stream default provider must be registered");
    let salt = SaltString::generate(&mut OsRng);
    let root_password_hash = Argon2::default()
        .hash_password(config.bootstrap_root_password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("failed to hash bootstrap root password: {err}"))?
        .to_string();
    let file_storage_registry = Arc::new(storage_object::builtin_driver_registry());

    let bootstrap_result = BootstrapService::new(store.clone())
        .run_with_official_catalog_loader(
            &BootstrapConfig {
                workspace_name: config.bootstrap_workspace_name.clone(),
                root_account: config.bootstrap_root_account.clone(),
                root_email: config.bootstrap_root_email.clone(),
                root_password_hash,
                root_name: config.bootstrap_root_name.clone(),
                root_nickname: config.bootstrap_root_nickname.clone(),
            },
            official_i18n_catalog_seed::load_official_i18n_catalog_seed,
        )
        .await?;
    let default_storage = if let Some(existing) =
        <storage_durable::MainDurableStore as control_plane::ports::FileManagementRepository>::get_default_file_storage(&store)
            .await?
    {
        existing
    } else {
        control_plane::file_management::FileStorageService::new(store.clone())
            .create_storage(control_plane::file_management::CreateFileStorageCommand {
                actor_user_id: bootstrap_result.root_user_id,
                code: "local_default".into(),
                title: "Local".into(),
                driver_type: "local".into(),
                enabled: true,
                is_default: true,
                config_json: serde_json::json!({
                    "root_path": config.business_file_local_root.clone(),
                    "public_base_url": null
                }),
                rule_json: serde_json::json!({}),
            })
            .await?
    };
    control_plane::file_management::FileManagementBootstrapService::new(store.clone())
        .ensure_builtin_attachments(
            bootstrap_result.root_user_id,
            default_storage.id,
            "attachments",
        )
        .await?;
    let system_metadata_bootstrap =
        control_plane::system_metadata::SystemMetadataBootstrapService::new(store.clone());
    system_metadata_bootstrap
        .ensure_builtin_user_and_role_models(bootstrap_result.root_user_id)
        .await?;
    system_metadata_bootstrap
        .ensure_builtin_runtime_read_model_grants(
            bootstrap_result.root_user_id,
            bootstrap_result.workspace_id,
        )
        .await?;
    control_plane::mcp_management::McpManagementService::new(store.clone())
        .read_workspace_catalog(bootstrap_result.root_user_id)
        .await?;
    let provider_runtime = Arc::new(ApiRuntimeServices::new(
        Arc::new(RwLock::new(
            plugin_runner::provider_host::ProviderHost::default(),
        )),
        Arc::new(RwLock::new(
            plugin_runner::capability_host::CapabilityHost::default(),
        )),
        Arc::new(RwLock::new(
            plugin_runner::data_source_host::DataSourceHost::default(),
        )),
    ));
    let api_provider_runtime = ApiProviderRuntime::new(provider_runtime.clone());
    let runtime_registry = runtime_core::runtime_model_registry::RuntimeModelRegistry::default();
    let runtime_metadata = store.list_runtime_model_metadata().await?;
    runtime_registry.rebuild(runtime_metadata);
    let runtime_engine = Arc::new(
        runtime_core::runtime_engine::RuntimeEngine::new_with_data_source_backend(
            runtime_registry,
            Arc::new(store.clone()),
            Arc::new(ApiDataSourceRuntimeRecordBackend::new(
                store.clone(),
                api_provider_runtime.clone(),
                config.provider_secret_master_key.clone(),
            )),
        ),
    );
    let api_docs = Arc::new(
        openapi_docs::build_default_api_docs_registry_with_cookie_name(&config.cookie_name)?,
    );
    let resolved_official_source = config.resolve_official_plugin_source();
    let resolved_official_agent_flow_template_source =
        config.resolve_official_agent_flow_template_source();
    let resolved_official_mcp_bundle_source = config.resolve_official_mcp_bundle_source();
    let official_agent_flow_template_cache = infrastructure.cache_store();
    let trusted_public_keys = config.official_plugin_trusted_public_keys()?;
    let official_plugin_source =
        Arc::new(official_plugin_registry::ApiOfficialPluginRegistry::new(
            resolved_official_source,
            trusted_public_keys,
        ));
    let official_agent_flow_template_source = Arc::new(
        official_agent_flow_templates::ApiOfficialAgentFlowTemplateRegistry::new(
            resolved_official_agent_flow_template_source,
            official_agent_flow_template_cache,
        ),
    );
    let official_mcp_bundle_source =
        Arc::new(official_mcp_bundles::ApiOfficialMcpBundleRegistry::new(
            resolved_official_mcp_bundle_source,
        ));
    let official_i18n_catalog_update_service =
        build_official_i18n_catalog_update_service(store.clone(), config);
    let plugin_management = control_plane::plugin_management::PluginManagementService::new(
        store.clone(),
        ApiProviderRuntime::new(provider_runtime.clone()),
        official_plugin_source.clone(),
        config.provider_install_root.clone(),
    )
    .with_node_id(config.api_node_id.clone())
    .with_allow_uploaded_host_extensions(config.allow_uploaded_host_extensions);
    plugin_management
        .ensure_builtin_plugin(
            control_plane::plugin_management::EnsureBuiltinPluginCommand {
                actor_user_id: bootstrap_result.root_user_id,
                package_root: builtin_jsx_block_package_root()?.display().to_string(),
            },
        )
        .await?;
    plugin_management.reconcile_all_installations().await?;
    let mut prepared_host_extensions = prepare_host_extensions_at_startup(
        &store,
        &config.api_node_id,
        &config.provider_install_root,
        &config.host_extension_dropin_root,
        config.allow_unverified_filesystem_dropins,
    )
    .await?;
    let mut console_host_extensions = builtin_host_extensions
        .iter()
        .map(|(_, contribution)| {
            resolve_linked_host_extension_console_contribution(
                contribution.clone(),
                linked_host_console_route_sources(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let prepared_contributions = prepared_host_extensions.take_contributions();
    for resolved in &prepared_contributions {
        control_plane::host_extension_boot::register_host_extension_contribution(
            &mut host_extension_registry,
            &resolved.contribution,
        )?;
    }
    console_host_extensions.extend(prepared_contributions);
    let authenticator_registry = Arc::new(
        control_plane::auth::AuthenticatorRegistry::from_host_extensions(&host_extension_registry)?,
    );
    let compiled_console_plan = compile_console_boot_plan(console_host_extensions)?;
    activate_prepared_host_extensions(&store, &config.api_node_id, prepared_host_extensions)
        .await?;
    let process_started_at = OffsetDateTime::now_utc();
    let runtime_activity = Arc::new(runtime_activity::ApplicationRuntimeActivityTracker::default());

    let state = Arc::new(ApiState {
        #[cfg(test)]
        test_resources: None,
        store,
        authenticator_registry,
        settings_feature_registry: compiled_console_plan.settings_feature_registry.clone(),
        console_operation_registry: compiled_console_plan.console_operation_registry.clone(),
        infrastructure,
        console_surface_registry: compiled_console_plan.console_surface_registry.clone(),
        file_storage_registry,
        runtime_engine,
        provider_runtime,
        process_started_at,
        runtime_activity,
        api_runtime_profile: Arc::new(HostApiRuntimeProfileCollector::new(process_started_at)?),
        plugin_runner_system: Arc::new(HttpPluginRunnerSystemClient::new(
            config.plugin_runner_internal_base_url.clone(),
        )),
        official_plugin_source,
        official_agent_flow_template_source,
        official_mcp_bundle_source,
        official_i18n_catalog_update_service,
        api_node_id: config.api_node_id.clone(),
        provider_install_root: config.provider_install_root.clone(),
        provider_secret_master_key: config.provider_secret_master_key.clone(),
        host_extension_dropin_root: config.host_extension_dropin_root.clone(),
        allow_unverified_filesystem_dropins: config.allow_unverified_filesystem_dropins,
        allow_uploaded_host_extensions: config.allow_uploaded_host_extensions,
        session_store,
        runtime_event_stream,
        api_docs,
        cookie_name: config.cookie_name.clone(),
        cookie_secure: config.cookie_secure,
        session_ttl_days: config.session_ttl_days,
        bootstrap_workspace_id: bootstrap_result.workspace_id,
        bootstrap_workspace_name: config.bootstrap_workspace_name.clone(),
    });
    crate::workers::workflow_schedule::spawn_workflow_schedule_loops(state.clone());
    crate::workers::provider_request_logs::spawn_provider_request_log_worker(state.clone());

    Ok(app_with_state_and_config_and_console_route_assembly(
        state,
        config,
        compiled_console_plan.route_assembly,
    ))
}

fn api_workspace_root() -> Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        current_dir.clone(),
        current_dir.join("api"),
    ];

    for candidate in candidates {
        if candidate.join("plugins/host-extensions").is_dir() {
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "api workspace root with plugins/host-extensions was not found"
    ))
}

fn builtin_jsx_block_package_root() -> Result<PathBuf> {
    let package_root = api_workspace_root()?.join("plugins/capability-plugins/1flowbase");
    if package_root.join("manifest.yaml").is_file() {
        return Ok(package_root);
    }

    Err(anyhow!(
        "builtin JSX block package manifest was not found at {}",
        package_root.join("manifest.yaml").display()
    ))
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
        api_workspace_root, builtin_jsx_block_package_root, parse_bind_addr,
        DEFAULT_API_SERVER_ADDR,
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
    fn api_workspace_root_contains_builtin_jsx_block_package() {
        let package_root = builtin_jsx_block_package_root().unwrap();

        assert!(package_root.join("manifest.yaml").is_file());
    }
}

#[cfg(test)]
mod _tests;
