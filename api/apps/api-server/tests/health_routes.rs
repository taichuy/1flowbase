use api_server::{
    app,
    app_state::ApiState,
    app_with_state_and_config,
    config::{ApiConfig, ApiEnvironment},
    host_infrastructure::build_local_host_infrastructure,
    official_agent_flow_templates::{
        OfficialAgentFlowTemplateCatalogSnapshot, OfficialAgentFlowTemplateSourcePort,
    },
    provider_runtime::{ApiDataSourceRuntimeRecordBackend, ApiProviderRuntime, ApiRuntimeServices},
    runtime_profile_client::{HostApiRuntimeProfileCollector, PluginRunnerSystemPort},
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{HeaderValue, Request, StatusCode},
    Router,
};
use control_plane::bootstrap::{BootstrapConfig, BootstrapService};
use control_plane::ports::{
    DownloadedOfficialPluginPackage, OfficialPluginCatalogSnapshot, OfficialPluginSourceEntry,
    OfficialPluginSourcePort,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct UnreachablePluginRunnerSystemClient;

#[async_trait]
impl PluginRunnerSystemPort for UnreachablePluginRunnerSystemClient {
    async fn fetch_runtime_profile(&self) -> anyhow::Result<runtime_profile::RuntimeProfile> {
        anyhow::bail!("plugin runner unavailable in health route tests")
    }
}

#[derive(Clone, Default)]
struct NoopOfficialPluginSource;

#[derive(Clone, Default)]
struct NoopOfficialAgentFlowTemplateSource;

#[async_trait]
impl OfficialPluginSourcePort for NoopOfficialPluginSource {
    async fn list_official_catalog(&self) -> anyhow::Result<OfficialPluginCatalogSnapshot> {
        Ok(OfficialPluginCatalogSnapshot {
            source: control_plane::ports::OfficialPluginCatalogSource {
                source_kind: "official_registry".to_string(),
                source_label: "官方源".to_string(),
                registry_url: "https://official.example.com/official-registry.json".to_string(),
            },
            entries: Vec::new(),
        })
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage> {
        anyhow::bail!("official plugin source not configured for health route tests")
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        Vec::new()
    }
}

#[async_trait]
impl OfficialAgentFlowTemplateSourcePort for NoopOfficialAgentFlowTemplateSource {
    async fn list_catalog_page(
        &self,
        _cursor: Option<String>,
    ) -> anyhow::Result<OfficialAgentFlowTemplateCatalogSnapshot> {
        anyhow::bail!("official AgentFlow template source not configured for health route tests")
    }

    async fn download_template(
        &self,
        _workflow_id: &str,
    ) -> anyhow::Result<control_plane::flow::AgentFlowTemplatePackage> {
        anyhow::bail!("official AgentFlow template source not configured for health route tests")
    }
}

fn default_test_config() -> ApiConfig {
    let database_url = std::env::var("API_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".to_string());

    ApiConfig::from_env_map(&[
        ("API_DATABASE_URL", &database_url),
        ("API_DATABASE_POOL_MAX_CONNECTIONS", "1"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "change-me"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap()
}

async fn isolated_database_url(base_url: &str) -> String {
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
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

async fn test_app() -> Router {
    test_app_with_config(default_test_config()).await
}

async fn test_app_with_config(mut config: ApiConfig) -> Router {
    config.database_url = isolated_database_url(&config.database_url).await;
    let durable = storage_durable::build_main_durable_postgres_with_max_connections(
        &config.database_url,
        config.database_pool_max_connections,
    )
    .await
    .unwrap();
    let store = durable.store.clone();
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let root_password_hash = Argon2::default()
        .hash_password(config.bootstrap_root_password.as_bytes(), &salt)
        .unwrap()
        .to_string();

    BootstrapService::new(store.clone())
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

    let provider_runtime = std::sync::Arc::new(ApiRuntimeServices::new(
        std::sync::Arc::new(RwLock::new(
            plugin_runner::provider_host::ProviderHost::default(),
        )),
        std::sync::Arc::new(RwLock::new(
            plugin_runner::capability_host::CapabilityHost::default(),
        )),
        std::sync::Arc::new(RwLock::new(
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
            std::sync::Arc::new(ApiDataSourceRuntimeRecordBackend::new(
                store.clone(),
                api_provider_runtime,
            )),
        ),
    );
    let api_docs =
        std::sync::Arc::new(api_server::openapi_docs::build_default_api_docs_registry().unwrap());
    let infrastructure = std::sync::Arc::new(build_local_host_infrastructure());
    let session_store = infrastructure
        .session_store()
        .expect("local health test infrastructure must provide session store");
    let runtime_event_stream = infrastructure
        .runtime_event_stream()
        .expect("local health test infrastructure must provide runtime event stream");

    app_with_state_and_config(
        std::sync::Arc::new(ApiState {
            store,
            infrastructure,
            file_storage_registry: std::sync::Arc::new(storage_object::builtin_driver_registry()),
            runtime_engine,
            provider_runtime,
            process_started_at: OffsetDateTime::now_utc(),
            runtime_activity: std::sync::Arc::new(
                api_server::runtime_activity::ApplicationRuntimeActivityTracker::default(),
            ),
            api_runtime_profile: std::sync::Arc::new(HostApiRuntimeProfileCollector),
            plugin_runner_system: std::sync::Arc::new(UnreachablePluginRunnerSystemClient),
            official_plugin_source: std::sync::Arc::new(NoopOfficialPluginSource),
            official_agent_flow_template_source: std::sync::Arc::new(
                NoopOfficialAgentFlowTemplateSource,
            ),
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
        &config,
    )
}

async fn login_and_capture_cookie(
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
                    serde_json::json!({
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
    let payload: Value = serde_json::from_slice(&body).unwrap();

    (
        cookie,
        payload["data"]["csrf_token"].as_str().unwrap().to_string(),
    )
}

async fn create_member(app: &Router, cookie: &str, csrf: &str, account: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": "temp-pass",
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    payload["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn health_route_returns_ok_payload() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["service"], "api-server");
    assert_eq!(payload["status"], "ok");
}

#[tokio::test]
async fn app_from_config_uses_local_host_infrastructure_session_store() {
    let mut config = default_test_config();
    config.database_url = isolated_database_url(&config.database_url).await;

    let app = api_server::app_from_config(&config).await.unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "identifier": "root",
                        "password": "change-me"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("set-cookie").is_some());
}

#[tokio::test]
async fn openapi_route_exposes_api_title() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["info"]["title"], "1flowbase API");
}

#[tokio::test]
async fn member_action_routes_remove_legacy_aliases() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let action_member_id = create_member(&app, &cookie, &csrf, "action-route-member").await;
    let legacy_member_id = create_member(&app, &cookie, &csrf, "legacy-route-member").await;

    let action_reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/members/{action_member_id}/actions/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "new_password": "next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action_reset_response.status(), StatusCode::NO_CONTENT);

    let action_disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/members/{action_member_id}/actions/disable"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(action_disable_response.status(), StatusCode::NO_CONTENT);

    let legacy_reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/members/{legacy_member_id}/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "new_password": "legacy-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_reset_response.status(), StatusCode::NOT_FOUND);

    let legacy_disable_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/members/{legacy_member_id}/disable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_disable_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn production_router_hides_legacy_openapi_endpoints() {
    let mut config = default_test_config();
    config.env = ApiEnvironment::Production;
    config.cors_allowed_origins = Some(vec![HeaderValue::from_static("http://127.0.0.1:3100")]);
    let app = test_app_with_config(config).await;

    let docs_response = app
        .clone()
        .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(docs_response.status(), StatusCode::NOT_FOUND);

    let openapi_response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(openapi_response.status(), StatusCode::NOT_FOUND);
}
