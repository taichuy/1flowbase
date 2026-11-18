pub use crate::event_center::catalog::EventSourceCatalog;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::http::{header::ACCEPT_LANGUAGE, HeaderMap};
use control_plane::i18n_catalog::CatalogResolver;
use control_plane::ports::{
    OfficialPluginSourcePort, RoleConsolePolicyReader, RuntimeEventStream, SessionStore,
};
use domain::{CatalogLocale, CatalogMessageIdentity};
use runtime_core::runtime_engine::RuntimeEngine;
use storage_durable_postgres::MainDurableStore;

/// Durable store selected by the API composition root.
///
/// Protocol modules depend on this state-owned name instead of importing a concrete adapter.
pub type ApiNotificationService =
    control_plane::notifications::NotificationService<MainDurableStore>;

pub type ApiModelProviderService = control_plane::model_provider::ModelProviderService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiPluginManagementService = control_plane::plugin_management::PluginManagementService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiExtensionInstallationService =
    control_plane::plugin_management::ExtensionInstallationService<MainDurableStore>;
pub type ApiDataSourceService = control_plane::data_source::DataSourceService<
    MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
>;
pub type ApiModelDefinitionMutationService =
    control_plane::runtime_registry_sync::ModelDefinitionMutationService<
        MainDurableStore,
        crate::runtime_registry_sync::ApiRuntimeRegistrySync,
    >;
pub type ApiModelDefinitionService =
    control_plane::model_definition::ModelDefinitionService<MainDurableStore>;
pub type ApiNetworkEgressProviderService =
    control_plane::network_egress::NetworkEgressProviderService<
        MainDurableStore,
        crate::provider_runtime::ApiProviderRuntime,
        control_plane::network_egress_secret::ProviderRegistryNetworkEgressSecretResolver<
            MainDurableStore,
        >,
    >;
pub type ApiNetworkEgressRouteService =
    control_plane::network_egress_route::NetworkEgressRouteService<MainDurableStore>;
pub type ApiNetworkEgressPoolService =
    control_plane::network_egress_pool::NetworkEgressPoolService<MainDurableStore>;
pub type ApiUiComponentCatalogService =
    control_plane::ui_component_catalog::UiComponentCatalogService<
        MainDurableStore,
        crate::ui_component_catalog_source::ApiUiComponentCatalogSource,
    >;
use time::OffsetDateTime;

use crate::error_response::ApiError;
use crate::openapi_docs::ApiDocsRegistry;
use crate::{
    config::ApiConfig, console_surface_registry::ConsoleSurfaceRegistry,
    extension_bus::ExtensionBootSnapshot, host_infrastructure::HostInfrastructureRegistry,
};
use crate::{
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    official_i18n_catalog_source::ApiOfficialI18nCatalogSource,
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    provider_runtime::ApiRuntimeServices,
    routes::assistant::conversation_events::AssistantConversationEventHub,
    runtime_activity::ApplicationRuntimeActivityTracker,
    runtime_profile_client::{ApiRuntimeProfilePort, RuntimeHostSystemPort},
};

pub fn build_official_i18n_catalog_update_service(
    store: MainDurableStore,
    config: &ApiConfig,
    network_egress: crate::network_egress_client::NetworkEgressHttpClientResolver,
    workspace_id: uuid::Uuid,
) -> Arc<control_plane::i18n_catalog::OfficialI18nCatalogUpdateService<MainDurableStore>> {
    let source = Arc::new(ApiOfficialI18nCatalogSource::new(
        config.resolve_official_i18n_catalog_source(),
        network_egress,
        workspace_id,
    ));
    Arc::new(control_plane::i18n_catalog::OfficialI18nCatalogUpdateService::new(store, source))
}

pub fn request_catalog_locale(
    headers: &HeaderMap,
    preferred_locale: Option<String>,
) -> CatalogLocale {
    let resolved = runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale: None,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale: preferred_locale,
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    });
    CatalogLocale::new(resolved.resolved_locale)
        .expect("runtime profile must resolve a supported catalog locale")
}

pub async fn resolve_request_text_with(
    store: &MainDurableStore,
    bootstrap_workspace_id: uuid::Uuid,
    locale: &CatalogLocale,
    key: &str,
) -> Result<String, ApiError> {
    let identity =
        CatalogMessageIdentity::new(key).expect("backend display catalog key must be valid");
    let resolver = CatalogResolver::new(store.clone(), bootstrap_workspace_id);
    Ok(resolver
        .resolve(bootstrap_workspace_id, &identity, locale)
        .await?
        .value)
}

pub async fn project_canonical_display_with(
    store: &MainDurableStore,
    bootstrap_workspace_id: uuid::Uuid,
    locale: &CatalogLocale,
    key: &'static str,
    stored: &str,
) -> Result<String, ApiError> {
    if !stored.trim().is_empty() && stored != key {
        return Ok(stored.to_owned());
    }
    resolve_request_text_with(store, bootstrap_workspace_id, locale, key).await
}

pub async fn resolve_official_source_label_with(
    store: &MainDurableStore,
    bootstrap_workspace_id: uuid::Uuid,
    locale: &CatalogLocale,
    source_kind: &str,
    fallback: String,
) -> Result<String, ApiError> {
    let key = match source_kind {
        "official_registry" => "Official source",
        "mirror_registry" => "Mirror source",
        _ => return Ok(fallback),
    };
    resolve_request_text_with(store, bootstrap_workspace_id, locale, key).await
}

#[derive(Clone)]

pub struct ApiState {
    #[cfg(any(test, feature = "test-support"))]
    pub test_resources: Option<Arc<TestResources>>,
    pub store: MainDurableStore,
    pub system_backup: Option<Arc<crate::system_backup::SystemBackupRuntime>>,
    pub system_maintenance: Arc<control_plane::system_recovery::SystemMaintenance>,
    pub authenticator_registry: Arc<control_plane::auth::AuthenticatorRegistry>,
    pub settings_feature_registry: Arc<access_control::SettingsFeatureRegistry>,
    pub console_operation_registry: Arc<access_control::ConsoleOperationRegistry>,
    pub console_policy_reader: Arc<dyn RoleConsolePolicyReader>,
    pub infrastructure: Arc<HostInfrastructureRegistry>,
    pub event_center: control_plane::event_center::EventCenterRuntime,
    pub event_sources: Arc<EventSourceCatalog>,
    pub event_workflow_changes: Arc<tokio::sync::Notify>,
    pub event_workflow_statuses: crate::workers::event_workflow::EventWorkflowStatuses,
    /// Present on production boot; lightweight test states may omit the production graph.
    pub extension_boot_snapshot: Option<Arc<ExtensionBootSnapshot>>,
    pub console_surface_registry: Arc<ConsoleSurfaceRegistry>,
    pub file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub runtime_engine: Arc<RuntimeEngine>,
    pub provider_runtime: Arc<ApiRuntimeServices>,
    pub runtime_event_stream_projection: Arc<dyn crate::debug_stream_contract::RuntimeEventSsePort>,
    pub native_resource_import: Arc<dyn crate::terminal_host::NativeResourceImportPort>,
    pub terminal_jobs: Arc<crate::local_command::jobs::JobRegistry>,
    pub process_started_at: OffsetDateTime,
    pub runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    pub assistant_conversation_events: Arc<AssistantConversationEventHub>,
    pub notification_service: Arc<ApiNotificationService>,
    pub communication_activity: Arc<crate::communication_runtime::CommunicationActivity>,
    pub notification_events: Arc<crate::routes::notifications::events::NotificationEventHub>,
    /// Process-local owner for detached embedded Assistant executions. Durable run state remains
    /// owned by the runtime repository.
    pub assistant_executions: Arc<Mutex<HashMap<uuid::Uuid, tokio::task::AbortHandle>>>,
    /// Active browser capability lease for each detached Assistant execution.
    pub assistant_client_sessions:
        Arc<Mutex<HashMap<uuid::Uuid, Arc<crate::routes::assistant::AssistantClientToolBridge>>>>,
    pub api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    pub runtime_host_system: Arc<dyn RuntimeHostSystemPort>,
    pub runtime_process_sampler: Arc<runtime_profile::RuntimeProcessSampler>,
    pub official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub official_mcp_bundle_source: Arc<dyn OfficialMcpBundleSourcePort>,
    pub official_extension_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub official_i18n_catalog_update_service:
        Arc<control_plane::i18n_catalog::OfficialI18nCatalogUpdateService<MainDurableStore>>,
    pub official_model_pricing_catalog_index_url: String,
    pub model_billing_require_provider_usage: bool,
    pub api_node_id: String,
    pub provider_install_root: String,
    pub application_template_root: String,
    pub provider_secret_master_key: String,
    pub host_extension_dropin_root: String,
    pub allow_unverified_filesystem_dropins: bool,
    pub allow_uploaded_host_extensions: bool,
    pub session_store: Arc<dyn SessionStore>,
    pub runtime_event_stream: Arc<dyn RuntimeEventStream>,
    pub api_docs: Arc<ApiDocsRegistry>,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub session_ttl_days: i64,
    pub bootstrap_workspace_id: uuid::Uuid,
    pub bootstrap_workspace_name: String,
}

impl ApiState {
    pub fn network_egress_http_clients(
        &self,
    ) -> crate::network_egress_client::NetworkEgressHttpClientResolver {
        crate::network_egress_client::NetworkEgressHttpClientResolver::new(
            self.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new(Arc::clone(&self.provider_runtime)),
            self.provider_secret_master_key.clone(),
            self.api_node_id.clone(),
        )
    }
}

#[cfg(any(test, feature = "test-support"))]
pub use crate::test_support::TestResources;

/// Composition-root alias; protocol routes do not depend on the PostgreSQL adapter.
pub type ApiMcpOAuthService = control_plane::mcp_oauth::McpOAuthService<MainDurableStore>;
