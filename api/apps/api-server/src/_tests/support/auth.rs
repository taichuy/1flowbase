use super::applications::{
    create_member, create_role, replace_member_roles, replace_role_permissions,
    sample_runtime_profile, set_user_preferred_locale,
};
use super::plugins::{
    InMemoryOfficialAgentFlowTemplateSource, InMemoryOfficialMcpBundleSource,
    InMemoryOfficialPluginSource,
};
use super::*;
use axum::response::Response;
use control_plane::ports::{FileManagementRepository, SessionStore};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

#[derive(Clone)]
struct StaticApiRuntimeProfileCollector {
    profile: RuntimeProfile,
}

#[async_trait]
impl ApiRuntimeProfilePort for StaticApiRuntimeProfileCollector {
    async fn collect_runtime_profile(&self) -> anyhow::Result<RuntimeProfile> {
        Ok(self.profile.clone())
    }
}

#[derive(Clone)]
struct StubPluginRunnerSystemClient {
    result: Result<RuntimeProfile, String>,
}

#[async_trait]
impl PluginRunnerSystemPort for StubPluginRunnerSystemClient {
    async fn fetch_runtime_profile(&self) -> anyhow::Result<RuntimeProfile> {
        self.result
            .clone()
            .map_err(|message| anyhow::anyhow!(message))
    }
}

fn default_test_config() -> ApiConfig {
    let database_url = std::env::var("API_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let root_account = std::env::var("BOOTSTRAP_ROOT_ACCOUNT").unwrap_or_else(|_| "root".into());
    let root_email =
        std::env::var("BOOTSTRAP_ROOT_EMAIL").unwrap_or_else(|_| "root@example.com".into());
    let root_password =
        std::env::var("BOOTSTRAP_ROOT_PASSWORD").unwrap_or_else(|_| "change-me".into());
    let workspace_name =
        std::env::var("BOOTSTRAP_WORKSPACE_NAME").unwrap_or_else(|_| "1flowbase".into());
    let entries = [
        ("API_DATABASE_URL".to_string(), database_url),
        (
            "API_DATABASE_POOL_MAX_CONNECTIONS".to_string(),
            "1".to_string(),
        ),
        (
            "API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS".to_string(),
            "true".to_string(),
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT".to_string(), root_account),
        ("BOOTSTRAP_ROOT_EMAIL".to_string(), root_email),
        ("BOOTSTRAP_ROOT_PASSWORD".to_string(), root_password),
        ("BOOTSTRAP_WORKSPACE_NAME".to_string(), workspace_name),
    ];

    let refs = entries
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    ApiConfig::from_env_map(&refs).unwrap()
}

pub(crate) fn test_config() -> ApiConfig {
    default_test_config()
}

async fn isolated_database_url(base_url: &str) -> String {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(30))
        .connect(base_url)
        .await
        .unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;

    format!("{base_url}?options=-csearch_path%3D{schema}")
}

async fn test_state_with_runtime_profile_state(
    process_started_at: OffsetDateTime,
    api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
) -> (Arc<ApiState>, String) {
    let mut config = default_test_config();
    config.database_url = isolated_database_url(&config.database_url).await;
    config.business_file_local_root = std::env::temp_dir()
        .join(format!("api-business-files-{}", Uuid::now_v7()))
        .display()
        .to_string();
    config.provider_install_root = std::env::temp_dir()
        .join(format!("api-provider-plugins-{}", Uuid::now_v7()))
        .display()
        .to_string();
    config.host_extension_dropin_root = std::path::PathBuf::from(&config.provider_install_root)
        .join("host-extension")
        .join("dropins")
        .display()
        .to_string();
    let mut pool_settings =
        storage_durable::PgPoolSettings::with_max_connections(config.database_pool_max_connections);
    pool_settings.acquire_timeout = Duration::from_secs(30);
    pool_settings.idle_timeout = Some(Duration::from_millis(250));
    pool_settings.max_lifetime = Some(Duration::from_secs(1));
    let durable = storage_durable::build_main_durable_postgres_with_pool_settings(
        &config.database_url,
        pool_settings,
    )
    .await
    .unwrap();
    let store = durable.store.clone();
    let file_storage_registry = Arc::new(storage_object::builtin_driver_registry());
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let root_password_hash = Argon2::default()
        .hash_password(config.bootstrap_root_password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    let bootstrap = BootstrapService::new(store.clone())
        .run(&BootstrapConfig {
            workspace_name: config.bootstrap_workspace_name.clone(),
            root_account: config.bootstrap_root_account.clone(),
            root_email: config.bootstrap_root_email.clone(),
            root_password_hash,
            root_name: config.bootstrap_root_name.clone(),
            root_nickname: config.bootstrap_root_nickname.clone(),
        })
        .await
        .unwrap();
    let default_storage = if let Some(existing) =
        <storage_durable::MainDurableStore as FileManagementRepository>::get_default_file_storage(
            &store,
        )
        .await
        .unwrap()
    {
        existing
    } else {
        control_plane::file_management::FileStorageService::new(store.clone())
            .create_storage(control_plane::file_management::CreateFileStorageCommand {
                actor_user_id: bootstrap.root_user_id,
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
            .await
            .unwrap()
    };
    control_plane::file_management::FileManagementBootstrapService::new(store.clone())
        .ensure_builtin_attachments(bootstrap.root_user_id, default_storage.id, "attachments")
        .await
        .unwrap();
    control_plane::system_metadata::SystemMetadataBootstrapService::new(store.clone())
        .ensure_builtin_user_and_role_models(bootstrap.root_user_id)
        .await
        .unwrap();
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
    runtime_registry.rebuild(store.list_runtime_model_metadata().await.unwrap());
    let runtime_engine = std::sync::Arc::new(
        runtime_core::runtime_engine::RuntimeEngine::new_with_data_source_backend(
            runtime_registry,
            std::sync::Arc::new(store.clone()),
            std::sync::Arc::new(
                crate::provider_runtime::ApiDataSourceRuntimeRecordBackend::new(
                    store.clone(),
                    api_provider_runtime,
                    config.provider_secret_master_key.clone(),
                ),
            ),
        ),
    );
    let api_docs = std::sync::Arc::new(
        crate::openapi_docs::build_default_api_docs_registry_with_cookie_name(&config.cookie_name)
            .unwrap(),
    );
    let infrastructure = Arc::new(build_local_host_infrastructure());
    let session_store = infrastructure
        .session_store()
        .expect("local test infrastructure must provide session store");
    let runtime_event_stream = infrastructure
        .runtime_event_stream()
        .expect("local test infrastructure must provide runtime event stream");
    let settings_feature_registry = crate::app_state::compile_core_settings_feature_registry()
        .expect("core settings feature registry should compile");
    let console_operation_registry =
        crate::app_state::compile_core_console_operation_registry(&settings_feature_registry)
            .expect("core console operation registry should compile");

    (
        Arc::new(ApiState {
            store,
            settings_feature_registry,
            console_operation_registry,
            infrastructure,
            console_surface_registry: Arc::new(
                crate::console_surface_registry::ConsoleSurfaceRegistry::default(),
            ),
            file_storage_registry,
            runtime_engine,
            provider_runtime,
            process_started_at,
            runtime_activity: Arc::new(
                crate::runtime_activity::ApplicationRuntimeActivityTracker::default(),
            ),
            api_runtime_profile,
            plugin_runner_system,
            official_plugin_source: Arc::new(InMemoryOfficialPluginSource),
            official_agent_flow_template_source: Arc::new(InMemoryOfficialAgentFlowTemplateSource),
            official_mcp_bundle_source: Arc::new(InMemoryOfficialMcpBundleSource),
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
            bootstrap_workspace_name: config.bootstrap_workspace_name.clone(),
        }),
        config.database_url,
    )
}

async fn test_app_with_runtime_profile_state(
    process_started_at: OffsetDateTime,
    api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
) -> (Router, String) {
    let (state, database_url) = test_state_with_runtime_profile_state(
        process_started_at,
        api_runtime_profile,
        plugin_runner_system,
    )
    .await;
    let config = default_test_config();
    let app = crate::app_with_state_and_config(state, &config);

    (app, database_url)
}

pub async fn test_app_with_database_url() -> (Router, String) {
    test_app_with_runtime_profile_state(
        OffsetDateTime::now_utc(),
        Arc::new(HostApiRuntimeProfileCollector::new(OffsetDateTime::now_utc()).unwrap()),
        Arc::new(StubPluginRunnerSystemClient {
            result: Err("plugin runner unavailable".to_string()),
        }),
    )
    .await
}

pub async fn test_app() -> Router {
    test_app_with_database_url().await.0
}

pub(crate) async fn test_api_state_with_database_url() -> (Arc<ApiState>, String) {
    test_state_with_runtime_profile_state(
        OffsetDateTime::now_utc(),
        Arc::new(HostApiRuntimeProfileCollector::new(OffsetDateTime::now_utc()).unwrap()),
        Arc::new(StubPluginRunnerSystemClient {
            result: Err("plugin runner unavailable".to_string()),
        }),
    )
    .await
}

pub(crate) async fn seed_session(state: &ApiState, session: domain::SessionRecord) {
    state.session_store.put(session).await.unwrap();
}

pub(crate) async fn read_first_sse_frame(response: Response) -> String {
    let mut stream = response.into_body().into_data_stream();
    let mut buffer = Vec::new();

    while let Some(chunk) = tokio_stream::StreamExt::next(&mut stream).await {
        buffer.extend_from_slice(&chunk.unwrap());
        if buffer.windows(2).any(|window| window == b"\n\n") {
            break;
        }
    }

    String::from_utf8(buffer).unwrap()
}

pub async fn login_and_capture_cookie(
    app: &Router,
    identifier: &str,
    password: &str,
) -> (String, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": identifier,
                        "password": password
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    (
        cookie,
        payload["data"]["csrf_token"].as_str().unwrap().to_string(),
    )
}

pub async fn get_json(app: &Router, path: &str, cookie: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

pub fn sample_api_profile(host_fingerprint: &str) -> RuntimeProfile {
    sample_runtime_profile("api-server", host_fingerprint)
}

pub fn sample_runner_profile(host_fingerprint: &str) -> RuntimeProfile {
    sample_runtime_profile("plugin-runner", host_fingerprint)
}

pub async fn test_app_with_runtime_profiles(
    api_profile: RuntimeProfile,
    runner_profile: Option<RuntimeProfile>,
    permissions: &[&str],
    preferred_locale: Option<&str>,
) -> (Router, String) {
    let process_started_at = api_profile.started_at;
    let (app, database_url) = test_app_with_runtime_profile_state(
        process_started_at,
        Arc::new(StaticApiRuntimeProfileCollector {
            profile: api_profile,
        }),
        Arc::new(StubPluginRunnerSystemClient {
            result: runner_profile.ok_or_else(|| "plugin runner unavailable".to_string()),
        }),
    )
    .await;

    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    if permissions.is_empty() {
        if let Some(locale) = preferred_locale {
            set_user_preferred_locale(&database_url, "root", Some(locale)).await;
        }
        return (app, root_cookie);
    }

    let suffix = Uuid::now_v7().to_string().replace('-', "");
    let account = format!("runtime_viewer_{}", &suffix[..8]);
    let role_code = format!("runtime_viewer_{}", &suffix[8..16]);
    let member_id = create_member(&app, &root_cookie, &root_csrf, &account, "temp-pass").await;
    create_role(&app, &root_cookie, &root_csrf, &role_code).await;
    replace_role_permissions(&app, &root_cookie, &root_csrf, &role_code, permissions).await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &[&role_code]).await;

    if let Some(locale) = preferred_locale {
        set_user_preferred_locale(&database_url, &account, Some(locale)).await;
    }

    let (cookie, _) = login_and_capture_cookie(&app, &account, "temp-pass").await;
    (app, cookie)
}

pub async fn test_app_with_runtime_profile_error(permissions: &[&str]) -> (Router, String) {
    test_app_with_runtime_profiles(
        sample_api_profile("host_api_server"),
        None,
        permissions,
        None,
    )
    .await
}
