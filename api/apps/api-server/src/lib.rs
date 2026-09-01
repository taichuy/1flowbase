#![recursion_limit = "256"]

extern crate self as api_server;

pub mod app_state;
pub mod application_public_docs;
pub mod config;
pub(crate) mod console_operation_compilation;
pub mod console_policy_migration;
pub mod console_surface_registry;
pub mod error_response;
pub mod extension_bootstrap;
pub mod extension_bus;
pub(crate) mod external_endpoint_catalog;
pub mod host_extension_boot;
pub mod host_extension_loader;
pub mod host_extensions;
pub mod host_infrastructure;
pub mod host_route_registry;
pub mod host_worker_registry;
pub mod middleware;
pub mod network_egress_client;
pub mod network_egress_probe;
pub mod official_extension_catalog;
pub mod official_i18n_catalog_seed;
pub mod official_i18n_catalog_source;
pub mod official_mcp_bundles;
pub mod official_plugin_registry;
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
pub mod ui_component_catalog_source;
pub mod workers;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use axum::{middleware as axum_middleware, routing::get, Json, Router};
use control_plane::{
    bootstrap::{BootstrapConfig, BootstrapService},
    plugin_management::ready_current_node_plugin_installation,
    ports::{DataSourceRuntimePort, PluginRepository},
};
use rand_core::OsRng;
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
    host_infrastructure::build_local_host_infrastructure_from_host_extensions,
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
    let interface_catalog =
        routes::mcp_management::mcp_interface_catalog_entries(state.as_ref(), actor).await?;
    Ok(
        routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerFactory::new(
            routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerDependencies::new(
                state.store.clone(),
                state.infrastructure.cache_store(),
                state.provider_secret_master_key.clone(),
            ),
            Arc::new(
                routes::mcp_protocol::virtual_ui::McpInterfaceCatalogSnapshot::new(
                    interface_catalog,
                ),
            ),
            Arc::new(
                routes::mcp_protocol::virtual_ui::ConsoleRouterMcpInterfaceDispatchPort::new(
                    console_router(state.clone(), true),
                ),
            ),
        ),
    )
}

struct ApiLifecycleDeliveryCompletion;

impl control_plane::lifecycle_outbox_dispatcher::LifecycleDeliveryCompletionPort
    for ApiLifecycleDeliveryCompletion
{
    fn complete(
        &self,
        outcome: extension_contracts::CompletionOutcome<
            control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryCompletion,
        >,
    ) {
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
    console_route_assembly: &routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
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
    for contribution in root_external_endpoint_contributions(include_docs) {
        compiler.contribute(contribution)?;
    }
    for contribution in console_route_assembly.external_endpoint_contributions() {
        compiler.contribute(contribution)?;
    }
    compiler.contribute_openapi_document("api-server.openapi", openapi_document)?;
    compiler.absorb_registry("compiled-interface-registry", registry.as_ref())?;
    compiler.contribute_mcp_protocol_surface(routes::mcp_protocol::MCP_INVOCATION_BINDING_ID)?;
    compiler.contribute_approved_controls(include_docs)?;
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
        .layer(TraceLayer::new_for_http())
}

pub fn app_with_state(state: Arc<ApiState>) -> Router {
    if let Some(snapshot) = &state.extension_boot_snapshot {
        snapshot
            .publish_complete_catalog(&state)
            .expect("complete interface catalog must publish before router construction");
    }
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
    publish_external_endpoint_catalog(&state, &assembly, true, &openapi_document)
        .expect("external endpoint catalog must publish before router construction");
    base_router(true, false)
        .merge(console_router_with_assembly(state, true, assembly))
        .layer(development_cors_layer())
        .layer(TraceLayer::new_for_http())
}

fn console_router(state: Arc<ApiState>, include_openapi: bool) -> Router {
    let interface_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot());
    let assembly = routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        interface_snapshot.as_deref(),
    );
    console_router_with_assembly(state, include_openapi, assembly)
}

fn console_router_with_assembly(
    state: Arc<ApiState>,
    include_openapi: bool,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
) -> Router {
    let maintenance_classifier =
        middleware::system_maintenance::SystemMaintenanceRequestClassifier::new(
            console_route_assembly.maintenance_control_routes().to_vec(),
        );
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
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::system_maintenance::fence_mutating_requests,
        ))
        .layer(axum_middleware::from_fn_with_state(
            maintenance_classifier,
            middleware::system_maintenance::classify_system_maintenance_request,
        ))
        .with_state(state)
}

pub fn app_with_state_and_config(state: Arc<ApiState>, config: &ApiConfig) -> Router {
    if let Some(snapshot) = &state.extension_boot_snapshot {
        snapshot
            .publish_complete_catalog(&state)
            .expect("complete interface catalog must publish before router construction");
    }
    let interface_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot());
    let assembly = routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations_and_plugin_upload_max_bytes(
        interface_snapshot.as_deref(),
        config.plugin_upload_max_bytes,
    );
    let openapi_document = serde_json::to_value(openapi::ApiDoc::openapi())
        .expect("static OpenAPI document must serialize");
    app_with_state_and_config_and_console_route_assembly(state, config, assembly, &openapi_document)
}

fn app_with_state_and_config_and_console_route_assembly(
    state: Arc<ApiState>,
    config: &ApiConfig,
    console_route_assembly: routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>>,
    openapi_document: &serde_json::Value,
) -> Router {
    let include_docs = config.env != ApiEnvironment::Production;
    publish_external_endpoint_catalog(
        &state,
        &console_route_assembly,
        include_docs,
        openapi_document,
    )
    .expect("external endpoint catalog must publish before router construction");
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
    app_and_runtime_host_from_config(config)
        .await
        .map(|(router, _)| router)
}

pub async fn app_and_runtime_host_from_env(
) -> Result<(Router, Arc<runtime_extension_host::RuntimeExtensionHost>)> {
    let config = ApiConfig::from_env()?;
    app_and_runtime_host_from_config(&config).await
}

async fn app_and_runtime_host_from_config(
    config: &ApiConfig,
) -> Result<(Router, Arc<runtime_extension_host::RuntimeExtensionHost>)> {
    let durable = storage_durable_postgres::build_main_durable_postgres_with_max_connections(
        &config.database_url,
        config.database_pool_max_connections,
    )
    .await?;
    let store = durable
        .store
        .clone()
        .with_runtime_table_name_policy(config.runtime_table_name_policy.clone());
    console_policy_migration::require_runtime_console_policy_cutover(&store).await?;
    let prepared_host_extensions = prepare_host_extensions_at_startup(
        &store,
        &config.api_node_id,
        &config.provider_install_root,
        &config.host_extension_dropin_root,
        config.allow_unverified_filesystem_dropins,
    )
    .await?;
    let mut extension_assembly = extension_bus::assemble_extension_graph_input(
        api_workspace_root()?,
        extension_bus::DEFAULT_PLUGIN_SET_PATH,
        Vec::new(),
    )?;
    extension_assembly
        .extend_active_host_extensions(prepared_host_extensions.graph_extensions())?;
    let extension_graph = Arc::new(extension_assembly.compile_graph()?);
    let lifecycle_plan = extension_assembly.compile_lifecycle_subscriber_plan(&extension_graph)?;
    let extension_boot_snapshot = Arc::new(extension_bus::compile_extension_boot_snapshot(
        Arc::clone(&extension_graph),
        &extension_assembly,
        store.clone(),
        config.api_node_id.clone(),
    )?);
    let active_host_extensions = extension_assembly.into_host_extension_manifests();
    let host_extension_registry =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &active_host_extensions,
        )?;
    let infrastructure = Arc::new(build_local_host_infrastructure_from_host_extensions(
        &host_extension_registry,
        &extension_graph,
    )?);
    let (lifecycle_delivery, lifecycle_publication_catalog) =
        host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(
            &lifecycle_plan,
            host_extensions::lifecycle::production_lifecycle_handler_factories(
                infrastructure.event_bus(),
            )?
            .activate(&active_host_extensions)?,
        )?;
    let store = store.with_lifecycle_publication_catalog(lifecycle_publication_catalog);
    tokio::spawn(
        control_plane::lifecycle_outbox_dispatcher::LifecycleOutboxDispatcher::new(
            store.clone(),
            Arc::new(lifecycle_delivery),
            Arc::new(ApiLifecycleDeliveryCompletion),
        )
        .run(),
    );
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
        <storage_durable_postgres::MainDurableStore as control_plane::ports::FileManagementRepository>::get_default_file_storage(&store)
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
        .ensure_builtin_model_pricing_rules(bootstrap_result.root_user_id)
        .await?;
    routes::billing::sync_bundled_pricing_catalog(&store, bootstrap_result.root_user_id)
        .await
        .map_err(|error| error.0)?;
    let catalog_url = format!(
        "https://raw.githubusercontent.com/{}/main/model-pricing/catalog/v1/index.json",
        config.official_plugin_repository
    );
    if let Err(error) = routes::billing::sync_remote_pricing_catalog(
        &store,
        bootstrap_result.root_user_id,
        &catalog_url,
    )
    .await
    {
        tracing::warn!(
            catalog_url,
            error = %error.0,
            "remote model pricing catalog unavailable; bundled snapshot remains active"
        );
    }
    system_metadata_bootstrap
        .ensure_builtin_runtime_read_model_grants(
            bootstrap_result.root_user_id,
            bootstrap_result.workspace_id,
        )
        .await?;
    control_plane::mcp_management::McpManagementService::new(store.clone())
        .read_workspace_catalog(bootstrap_result.root_user_id)
        .await?;
    let process_started_at = OffsetDateTime::now_utc();
    let runtime_artifact_resolver = Arc::new(ApiRuntimeArtifactResolver::new(
        store.clone(),
        config.api_node_id.clone(),
        config.provider_install_root.clone(),
    ));
    let runtime_extension_host = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver_and_plugin_data(
            process_started_at,
            runtime_artifact_resolver,
            Arc::new(store.clone()),
        )?,
    );
    let mut runtime_backend_slot = runtime_core::runtime_backend::RuntimeBackendSlot::default();
    runtime_backend_slot.bind(runtime_extension_host.clone())?;
    let runtime_backend = runtime_backend_slot.backend()?;
    let provider_runtime = Arc::new(ApiRuntimeServices::new_with_runtime_backend(
        runtime_backend,
        Arc::clone(&extension_graph),
    )?);
    let api_provider_runtime = ApiProviderRuntime::new(provider_runtime.clone());
    let data_model_template_catalog = provider_runtime.data_model_template_catalog();
    let runtime_registry = runtime_core::runtime_model_registry::RuntimeModelRegistry::default();
    let runtime_metadata = store.list_runtime_model_metadata().await?;
    runtime_registry.rebuild(runtime_metadata);
    let runtime_engine = Arc::new(
        runtime_core::runtime_engine::RuntimeEngine::new_with_data_source_backend_templates_and_ordered_tree(
            runtime_registry,
            Arc::new(store.clone()),
            Arc::new(store.clone()),
            Arc::new(ApiDataSourceRuntimeRecordBackend::new(
                store.clone(),
                api_provider_runtime.clone(),
                config.provider_secret_master_key.clone(),
                config.api_node_id.clone(),
            )),
            data_model_template_catalog,
        ),
    );
    let api_docs = Arc::new(
        openapi_docs::build_default_api_docs_registry_with_cookie_name(&config.cookie_name)?,
    );
    let resolved_official_mcp_bundle_source = config.resolve_official_mcp_bundle_source();
    let trusted_public_keys = config.official_plugin_trusted_public_keys()?;
    let network_egress_http_clients =
        Arc::new(network_egress_client::NetworkEgressHttpClientResolver::new(
            store.clone(),
            api_provider_runtime.clone(),
            config.provider_secret_master_key.clone(),
            config.api_node_id.clone(),
        ));
    let api_provider_runtime =
        api_provider_runtime.with_network_egress(Arc::clone(&network_egress_http_clients));
    let official_extension_catalog_source = Arc::new(
        official_extension_catalog::ApiOfficialExtensionCatalogSource::from_config(config)
            .with_network_egress(network_egress_http_clients.as_ref().clone()),
    );
    let official_plugin_source = Arc::new(
        official_extension_catalog::ApiOfficialRuntimeExtensionSource::new(
            official_extension_catalog_source.clone(),
            if config.official_plugin_signature_required {
                "signature_required".to_string()
            } else {
                "allow_unsigned".to_string()
            },
            trusted_public_keys.clone(),
        ),
    );
    let official_mcp_bundle_source =
        Arc::new(official_mcp_bundles::ApiOfficialMcpBundleRegistry::new(
            resolved_official_mcp_bundle_source,
            std::path::PathBuf::from(&config.mcp_template_library_root),
            Arc::new(store.clone()),
            config.api_node_id.clone(),
            bootstrap_result.root_user_id,
            trusted_public_keys,
        ));
    if let Err(error) = official_mcp_bundle_source
        .reconcile_local_installations()
        .await
    {
        tracing::warn!(
            error = %error,
            "MCP template installation reconciliation unavailable; core startup continues"
        );
    }
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
    match extension_bootstrap::load_locked_extension_bootstrap(&api_workspace_root()?) {
        Ok(entries) => {
            for result in plugin_management
                .bootstrap_locked_extensions(bootstrap_result.root_user_id, &entries)
                .await
            {
                if let Some(warning) = result.warning {
                    tracing::warn!(
                        extension_id = %warning.extension_id,
                        version = %warning.version,
                        stage = warning.stage,
                        error = %warning.message,
                        "default extension bootstrap warning; core startup continues"
                    );
                }
            }
        }
        Err(error) => tracing::warn!(
            error = %error,
            "default extension bootstrap manifest unavailable; core startup continues"
        ),
    }
    plugin_management
        .ensure_builtin_plugin(
            control_plane::plugin_management::EnsureBuiltinPluginCommand {
                actor_user_id: bootstrap_result.root_user_id,
                package_root: builtin_frontstage_code_block_package_root()?
                    .display()
                    .to_string(),
            },
        )
        .await?;
    plugin_management.reconcile_all_installations().await?;
    intake_active_data_source_templates_at_startup(
        &store,
        &api_provider_runtime,
        &config.api_node_id,
        &config.provider_install_root,
    )
    .await?;
    let console_host_extensions = active_host_extensions
        .iter()
        .map(|(_, contribution)| {
            resolve_linked_host_extension_console_contribution(
                contribution.clone(),
                linked_host_console_route_sources(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let authenticator_registry = Arc::new(
        control_plane::auth::AuthenticatorRegistry::from_host_extensions(&host_extension_registry)?,
    );
    let interface_snapshot = extension_boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is absent at production boot"))?
        .snapshot();
    let compiled_console_plan =
        app_state::compile_console_boot_plan_with_interface_operations_and_plugin_upload_max_bytes(
            console_host_extensions,
            Some(interface_snapshot.as_ref()),
            config.plugin_upload_max_bytes,
        )?;
    routes::host_infrastructure::interface_operation::validate_console_registry(
        interface_snapshot.as_ref(),
        &compiled_console_plan.console_operation_registry,
    )?;
    activate_prepared_host_extensions(&store, &config.api_node_id, prepared_host_extensions)
        .await?;
    let runtime_activity = Arc::new(runtime_activity::ApplicationRuntimeActivityTracker::default());
    let assistant_conversation_events =
        Arc::new(routes::assistant::conversation_events::AssistantConversationEventHub::default());
    let system_maintenance = Arc::new(control_plane::system_recovery::SystemMaintenance::default());
    let system_backup = resolve_system_backup_startup(
        system_backup::SystemBackupRuntime::open(
            store.clone(),
            file_storage_registry.clone(),
            system_maintenance.clone(),
            config,
        )
        .await
        .map(Arc::new),
    )?;
    let api_runtime_profile = Arc::new(HostApiRuntimeProfileCollector::new(process_started_at)?);
    if !provider_runtime
        .model_provider_extension_graph()
        .is_some_and(|graph| Arc::ptr_eq(graph, extension_boot_snapshot.graph_arc()))
    {
        anyhow::bail!("model provider slot resolver must use the published extension graph");
    }
    runtime_extension_host.mark_ready()?;

    let state = Arc::new(ApiState {
        #[cfg(test)]
        test_resources: None,
        console_policy_reader: Arc::new(store.clone()),
        store,
        system_backup,
        system_maintenance,
        authenticator_registry,
        settings_feature_registry: compiled_console_plan.settings_feature_registry.clone(),
        console_operation_registry: compiled_console_plan.console_operation_registry.clone(),
        infrastructure,
        extension_boot_snapshot: Some(Arc::clone(&extension_boot_snapshot)),
        console_surface_registry: compiled_console_plan.console_surface_registry.clone(),
        file_storage_registry,
        runtime_engine,
        provider_runtime,
        process_started_at,
        runtime_activity,
        assistant_conversation_events,
        assistant_executions: Default::default(),
        assistant_client_sessions: Default::default(),
        api_runtime_profile,
        runtime_host_system: runtime_extension_host.clone(),
        official_plugin_source,
        official_mcp_bundle_source,
        official_extension_catalog_source,
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
    extension_boot_snapshot.publish_complete_catalog(&state)?;
    let builtin_mcp_interfaces =
        openapi_interface::build_openapi_capability_catalog(&state, bootstrap_result.workspace_id)
            .await
            .map_err(|error| error.0)?
            .into_iter()
            .map(routes::mcp_management::mcp_interface_entry_from_capability)
            .collect();
    control_plane::mcp_management::McpManagementService::new(state.store.clone())
        .seed_builtin_bundle_once(
            control_plane::mcp_bundle::SeedBuiltinMcpBundleCommand {
                actor_user_id: bootstrap_result.root_user_id,
                workspace_id: bootstrap_result.workspace_id,
                package: official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package()?,
                interface_catalog: builtin_mcp_interfaces,
            },
        )
        .await?;
    crate::workers::workflow_schedule::spawn_workflow_schedule_loops(state.clone());
    crate::workers::provider_request_logs::spawn_provider_request_log_worker(state.clone());
    crate::workers::billing::spawn_billing_worker(state.clone());
    #[cfg(not(test))]
    spawn_default_ui_component_catalog_bootstrap(
        state.store.clone(),
        bootstrap_result.root_user_id,
    );

    let external_openapi_document = openapi::dynamic_openapi_document(&state)
        .await
        .map_err(|error| error.0)?;
    Ok((
        app_with_state_and_config_and_console_route_assembly(
            state,
            config,
            compiled_console_plan.route_assembly,
            &external_openapi_document,
        ),
        runtime_extension_host,
    ))
}

#[cfg(not(test))]
fn spawn_default_ui_component_catalog_bootstrap(
    store: storage_durable_postgres::MainDurableStore,
    actor_user_id: uuid::Uuid,
) {
    tokio::spawn(async move {
        let service = control_plane::ui_component_catalog::UiComponentCatalogService::new(
            store,
            ui_component_catalog_source::ApiUiComponentCatalogSource::default_taichuy(),
        );
        match service.bootstrap_empty_system(actor_user_id).await {
            Ok(control_plane::ui_component_catalog::UiComponentBootstrapOutcome::Imported {
                records,
            }) => tracing::info!(
                records,
                "default UI component catalog bootstrapped into an empty system"
            ),
            Ok(
                control_plane::ui_component_catalog::UiComponentBootstrapOutcome::SkippedNonEmpty,
            ) => {}
            Err(error) => tracing::warn!(
                error = %error,
                "default UI component catalog bootstrap failed; startup and existing records are unchanged"
            ),
        }
    });
}

fn resolve_system_backup_startup(
    result: Result<
        Arc<system_backup::SystemBackupRuntime>,
        system_backup::SystemBackupRuntimeError,
    >,
) -> Result<Option<Arc<system_backup::SystemBackupRuntime>>, system_backup::SystemBackupRuntimeError>
{
    match result {
        Ok(runtime) => Ok(Some(runtime)),
        Err(error) => {
            tracing::warn!(
                capability = "system_backup",
                error_code = "system_backup_unavailable",
                error = %error,
                "system backup initialization failed; system backup APIs are disabled"
            );
            Ok(None)
        }
    }
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

fn builtin_frontstage_code_block_package_root() -> Result<PathBuf> {
    let package_root = api_workspace_root()?.join("plugins/capability-plugins/1flowbase");
    if package_root.join("manifest.yaml").is_file() {
        return Ok(package_root);
    }

    Err(anyhow!(
        "builtin Frontstage code block package manifest was not found at {}",
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
