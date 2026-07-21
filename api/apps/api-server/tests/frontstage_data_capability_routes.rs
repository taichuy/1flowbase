use std::sync::Arc;

use api_server::{
    app_state::ApiState,
    app_with_state_and_config,
    config::ApiConfig,
    host_infrastructure::build_local_host_infrastructure,
    official_agent_flow_templates::{
        OfficialAgentFlowTemplateCatalogSnapshot, OfficialAgentFlowTemplateSourcePort,
    },
    official_mcp_bundles::{
        DownloadedOfficialMcpBundle, OfficialMcpBundleCatalogSnapshot,
        OfficialMcpBundleCatalogSource, OfficialMcpBundleSourcePort,
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
    http::{Request, StatusCode},
    Router,
};
use control_plane::{
    bootstrap::{BootstrapConfig, BootstrapService},
    ports::{
        DownloadedOfficialPluginPackage, OfficialPluginCatalogSnapshot, OfficialPluginSourceEntry,
        OfficialPluginSourcePort,
    },
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

const PAGE_TAB_GET_OPERATION_ID: &str = "get_frontstage_page_detail";
const PAGE_TAB_SAVE_OPERATION_ID: &str = "save_frontstage_tab_document";
const GRANT_PREFIX: &str = "frontstage:callable-write-grant:";
const GRANT_LOCK_PREFIX: &str = "frontstage:callable-write-grant-lock:";

#[derive(Clone)]
struct UnreachablePluginRunner;

#[async_trait]
impl PluginRunnerSystemPort for UnreachablePluginRunner {
    async fn fetch_runtime_profile(&self) -> anyhow::Result<runtime_profile::RuntimeProfile> {
        anyhow::bail!("plugin runner unavailable in frontstage callable tests")
    }
}

#[derive(Clone, Default)]
struct NoopPluginSource;

#[async_trait]
impl OfficialPluginSourcePort for NoopPluginSource {
    async fn list_official_catalog(&self) -> anyhow::Result<OfficialPluginCatalogSnapshot> {
        Ok(OfficialPluginCatalogSnapshot {
            source: control_plane::ports::OfficialPluginCatalogSource {
                source_kind: "official_registry".into(),
                source_label: "official".into(),
                registry_url: "https://official.example.com/plugins.json".into(),
            },
            entries: Vec::new(),
        })
    }

    async fn download_plugin(
        &self,
        _entry: &OfficialPluginSourceEntry,
    ) -> anyhow::Result<DownloadedOfficialPluginPackage> {
        anyhow::bail!("official plugin source unavailable in frontstage callable tests")
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        Vec::new()
    }
}

#[derive(Clone, Default)]
struct NoopAgentFlowSource;

#[async_trait]
impl OfficialAgentFlowTemplateSourcePort for NoopAgentFlowSource {
    async fn list_catalog_page(
        &self,
        _cursor: Option<String>,
    ) -> anyhow::Result<OfficialAgentFlowTemplateCatalogSnapshot> {
        anyhow::bail!("agent flow source unavailable in frontstage callable tests")
    }

    async fn download_template(
        &self,
        _workflow_id: &str,
    ) -> anyhow::Result<control_plane::flow::AgentFlowTemplatePackage> {
        anyhow::bail!("agent flow source unavailable in frontstage callable tests")
    }
}

#[derive(Clone, Default)]
struct NoopMcpSource;

#[async_trait]
impl OfficialMcpBundleSourcePort for NoopMcpSource {
    async fn list_catalog(&self) -> anyhow::Result<OfficialMcpBundleCatalogSnapshot> {
        Ok(OfficialMcpBundleCatalogSnapshot {
            source: OfficialMcpBundleCatalogSource {
                source_kind: "official_registry".into(),
                source_label: "official".into(),
                catalog_url: "https://official.example.com/mcp.json".into(),
            },
            entries: Vec::new(),
        })
    }

    async fn download_bundle(
        &self,
        _organization: &str,
        _bundle_id: &str,
    ) -> anyhow::Result<DownloadedOfficialMcpBundle> {
        anyhow::bail!("MCP source unavailable in frontstage callable tests")
    }
}

struct Fixture {
    app: Router,
    state: Arc<ApiState>,
}

fn test_config() -> ApiConfig {
    let database_url = std::env::var("API_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
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
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(base_url)
        .await
        .unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    format!("{base_url}?options=-csearch_path%3D{schema}")
}

async fn fixture() -> Fixture {
    let mut config = test_config();
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
    let registry = runtime_core::runtime_model_registry::RuntimeModelRegistry::default();
    registry.rebuild(store.list_runtime_model_metadata().await.unwrap());
    let runtime_engine = Arc::new(
        runtime_core::runtime_engine::RuntimeEngine::new_with_data_source_backend(
            registry,
            Arc::new(store.clone()),
            Arc::new(ApiDataSourceRuntimeRecordBackend::new(
                store.clone(),
                api_provider_runtime,
                config.provider_secret_master_key.clone(),
            )),
        ),
    );
    let infrastructure = Arc::new(build_local_host_infrastructure());
    let session_store = infrastructure.session_store().unwrap();
    let runtime_event_stream = infrastructure.runtime_event_stream().unwrap();
    let settings_feature_registry =
        api_server::app_state::compile_core_settings_feature_registry().unwrap();
    let console_operation_registry =
        api_server::app_state::compile_core_console_operation_registry(&settings_feature_registry)
            .unwrap();
    let process_started_at = OffsetDateTime::now_utc();
    let state = Arc::new(ApiState {
        store,
        settings_feature_registry,
        console_operation_registry,
        infrastructure,
        console_surface_registry: Arc::new(
            api_server::console_surface_registry::ConsoleSurfaceRegistry::default(),
        ),
        file_storage_registry: Arc::new(storage_object::builtin_driver_registry()),
        runtime_engine,
        provider_runtime,
        process_started_at,
        runtime_activity: Arc::new(
            api_server::runtime_activity::ApplicationRuntimeActivityTracker::default(),
        ),
        api_runtime_profile: Arc::new(
            HostApiRuntimeProfileCollector::new(process_started_at).unwrap(),
        ),
        plugin_runner_system: Arc::new(UnreachablePluginRunner),
        official_plugin_source: Arc::new(NoopPluginSource),
        official_agent_flow_template_source: Arc::new(NoopAgentFlowSource),
        official_mcp_bundle_source: Arc::new(NoopMcpSource),
        api_node_id: config.api_node_id.clone(),
        provider_install_root: config.provider_install_root.clone(),
        provider_secret_master_key: config.provider_secret_master_key.clone(),
        host_extension_dropin_root: config.host_extension_dropin_root.clone(),
        allow_unverified_filesystem_dropins: config.allow_unverified_filesystem_dropins,
        allow_uploaded_host_extensions: config.allow_uploaded_host_extensions,
        session_store,
        runtime_event_stream,
        api_docs: Arc::new(api_server::openapi_docs::build_default_api_docs_registry().unwrap()),
        cookie_name: config.cookie_name.clone(),
        cookie_secure: config.cookie_secure,
        session_ttl_days: config.session_ttl_days,
        bootstrap_workspace_name: config.bootstrap_workspace_name.clone(),
    });
    Fixture {
        app: app_with_state_and_config(state.clone(), &config),
        state,
    }
}

async fn json_request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, payload)
}

async fn get(app: &Router, path: &str, cookie: &str) -> (StatusCode, Value) {
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
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn login(app: &Router, identifier: &str, password: &str) -> (String, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "identifier": identifier, "password": password }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_string();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (
        cookie,
        payload["data"]["csrf_token"].as_str().unwrap().into(),
    )
}

async fn session_identity(app: &Router, cookie: &str) -> (Uuid, Uuid) {
    let (status, payload) = get(app, "/api/console/session", cookie).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    (
        Uuid::parse_str(payload["data"]["actor"]["id"].as_str().unwrap()).unwrap(),
        Uuid::parse_str(
            payload["data"]["session"]["current_workspace_id"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
}

async fn create_page(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
) -> (Uuid, Uuid, String) {
    let (status, payload) = json_request(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages"),
        cookie,
        csrf,
        json!({ "title": "Callable fixture", "rank": "a" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    let page_id = Uuid::parse_str(payload["data"]["id"].as_str().unwrap()).unwrap();
    let tab_id = Uuid::parse_str(payload["data"]["default_tab"]["id"].as_str().unwrap()).unwrap();
    let document_root_uid = payload["data"]["default_tab"]["document_root_uid"]
        .as_str()
        .unwrap()
        .to_owned();
    let (presentation_status, presentation_payload) = json_request(
        app,
        "PATCH",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}"),
        cookie,
        csrf,
        json!({ "content_presentation": "tabs" }),
    )
    .await;
    assert_eq!(
        presentation_status,
        StatusCode::OK,
        "{presentation_payload}"
    );
    (page_id, tab_id, document_root_uid)
}

async fn create_tab(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
    page_id: Uuid,
) -> (Uuid, String) {
    let (status, payload) = json_request(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs"),
        cookie,
        csrf,
        json!({ "title": "Second", "route_segment": "second", "rank": "b" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    (
        Uuid::parse_str(payload["data"]["id"].as_str().unwrap()).unwrap(),
        payload["data"]["document_root_uid"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

async fn catalog_entry(app: &Router, cookie: &str, workspace_id: Uuid, operation: &str) -> Value {
    let (status, payload) = get(
        app,
        &format!("/api/console/frontstage/{workspace_id}/callable-interfaces"),
        cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["operation_id"] == operation)
        .unwrap()
        .clone()
}

fn binding(alias: &str, catalog: &Value) -> Value {
    json!({
        "alias": alias,
        "operation_id": catalog["operation_id"],
        "schema_digest": catalog["schema_digest"],
        "scope": catalog["scope"],
        "risk_level": catalog["risk_level"]
    })
}

fn document(root_uid: &str, blocks: Vec<(&str, Vec<Value>)>) -> Value {
    json!({
        "version": 1,
        "root_uid": root_uid,
        "blocks": blocks.into_iter().map(|(id, interfaces)| json!({
            "id": id,
            "renderer_version": "v1",
            "interfaces": interfaces
        })).collect::<Vec<_>>()
    })
}

async fn save_document(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    payload: &Value,
) {
    let (status, response) = json_request(
        app,
        "PUT",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/document"),
        cookie,
        csrf,
        json!({ "payload": payload }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
}

#[allow(clippy::too_many_arguments)]
async fn dispatch(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    binding_alias: &str,
    schema_digest: &str,
    run_id: &str,
    draft_hash: &str,
    request: Value,
    write_grant: Option<&str>,
    confirmed: Option<bool>,
) -> (StatusCode, Value) {
    let mut body = json!({
        "block_id": block_id,
        "binding_alias": binding_alias,
        "schema_digest": schema_digest,
        "run_id": run_id,
        "draft_hash": draft_hash,
        "request": request,
        "write_grant": write_grant
    });
    if let Some(confirmed) = confirmed {
        body["confirmed"] = json!(confirmed);
    }
    json_request(
        app,
        "POST",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch"
        ),
        cookie,
        csrf,
        body,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn issue_grant(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    binding_alias: &str,
    schema_digest: &str,
    run_id: &str,
    draft_hash: &str,
) -> String {
    let (status, payload) = json_request(
        app,
        "POST",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/callable-interfaces/write-grants"
        ),
        cookie,
        csrf,
        json!({
            "block_id": block_id,
            "binding_alias": binding_alias,
            "schema_digest": schema_digest,
            "run_id": run_id,
            "draft_hash": draft_hash
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"]["grant_token"].as_str().unwrap().into()
}

fn grant_key(token: &str) -> String {
    format!("{GRANT_PREFIX}{:x}", Sha256::digest(token.as_bytes()))
}

fn grant_lock_key(token: &str) -> String {
    format!("{GRANT_LOCK_PREFIX}{:x}", Sha256::digest(token.as_bytes()))
}

#[tokio::test]
async fn callable_catalog_contains_every_console_operation_and_runtime_model_crud() {
    let fixture = fixture().await;
    let (cookie, _) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (status, payload) = get(
        &fixture.app,
        &format!("/api/console/frontstage/{workspace_id}/callable-interfaces"),
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    let entries = payload["data"].as_array().unwrap();
    let console = entries
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("/api/console/"))
        })
        .collect::<Vec<_>>();
    let console_ids = console
        .iter()
        .filter_map(|entry| entry["operation_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(console.len(), 257);
    assert_eq!(console_ids.len(), console.len());
    assert!(console
        .iter()
        .all(|entry| entry["adapter_id"] == "console_openapi"));

    let conversations = entries
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.contains("/application_conversations/"))
        })
        .collect::<Vec<_>>();
    assert_eq!(conversations.len(), 5, "{conversations:?}");
    assert_eq!(
        conversations
            .iter()
            .filter_map(|entry| entry["method"].as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from(["DELETE", "GET", "PATCH", "POST"])
    );
}

#[allow(clippy::too_many_arguments)]
async fn seed_grant(
    state: &ApiState,
    token: &str,
    actor_user_id: Uuid,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    binding_alias: &str,
    run_id: &str,
    draft_hash: &str,
    operation_id: &str,
    expires_at: OffsetDateTime,
) {
    state
        .infrastructure
        .cache_store()
        .set_if_absent_json(
            &grant_key(token),
            json!({
                "actor_user_id": actor_user_id,
                "workspace_id": workspace_id,
                "page_id": page_id,
                "tab_id": tab_id,
                "block_id": block_id,
                "binding_alias": binding_alias,
                "run_id": run_id,
                "draft_hash": draft_hash,
                "operation_id": operation_id,
                "expires_at": serde_json::to_value(expires_at).unwrap()
            }),
            Some(Duration::minutes(5)),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn callable_dispatch_resolves_only_the_current_document_block_binding() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, root_uid) = create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let (second_tab, second_root_uid) =
        create_tab(&fixture.app, &cookie, &csrf, workspace_id, page_id).await;
    let catalog = catalog_entry(
        &fixture.app,
        &cookie,
        workspace_id,
        PAGE_TAB_GET_OPERATION_ID,
    )
    .await;
    let digest = catalog["schema_digest"].as_str().unwrap();
    let bound = document(
        &root_uid,
        vec![
            ("block-a", vec![binding("loadPage", &catalog)]),
            ("block-b", vec![binding("otherLoad", &catalog)]),
        ],
    );
    save_document(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        &bound,
    )
    .await;
    save_document(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        &document(&second_root_uid, vec![("block-a", Vec::new())]),
    )
    .await;

    for (target_tab, block, alias, supplied_digest, expected) in [
        (tab_id, "block-a", "missing", digest, StatusCode::NOT_FOUND),
        (
            tab_id,
            "block-a",
            "otherLoad",
            digest,
            StatusCode::NOT_FOUND,
        ),
        (tab_id, "block-b", "loadPage", digest, StatusCode::NOT_FOUND),
        (
            tab_id,
            "block-a",
            "loadPage",
            "stale-digest",
            StatusCode::BAD_REQUEST,
        ),
        (
            second_tab,
            "block-a",
            "loadPage",
            digest,
            StatusCode::NOT_FOUND,
        ),
    ] {
        let (status, _) = dispatch(
            &fixture.app,
            &cookie,
            &csrf,
            workspace_id,
            page_id,
            target_tab,
            block,
            alias,
            supplied_digest,
            "run-read",
            "draft-read",
            json!({}),
            None,
            None,
        )
        .await;
        assert_eq!(status, expected, "{block}/{alias}/{target_tab}");
    }

    let (status, payload) = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-a",
        "loadPage",
        digest,
        "run-read",
        "draft-read",
        json!({}),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["page"]["id"], page_id.to_string());
}

#[tokio::test]
async fn write_grant_is_consumed_once_and_confirmed_has_no_authority() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, root_uid) = create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let catalog = catalog_entry(
        &fixture.app,
        &cookie,
        workspace_id,
        PAGE_TAB_SAVE_OPERATION_ID,
    )
    .await;
    let digest = catalog["schema_digest"].as_str().unwrap();
    let primary_document = document(
        &root_uid,
        vec![("block-write", vec![binding("savePage", &catalog)])],
    );
    save_document(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        &primary_document,
    )
    .await;

    let (confirmed_status, _) = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-write",
        "savePage",
        digest,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document.clone() } }),
        None,
        Some(true),
    )
    .await;
    assert_eq!(confirmed_status, StatusCode::BAD_REQUEST);

    let token = issue_grant(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-write",
        "savePage",
        digest,
        "run-write",
        "draft-write",
    )
    .await;
    let competing_owner = "competing-draft-run";
    assert!(fixture
        .state
        .infrastructure
        .distributed_lock()
        .acquire(
            &grant_lock_key(&token),
            competing_owner,
            Duration::seconds(10),
        )
        .await
        .unwrap());
    let locked = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-write",
        "savePage",
        digest,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document.clone() } }),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(locked.0, StatusCode::CONFLICT, "{}", locked.1);
    assert!(fixture
        .state
        .infrastructure
        .cache_store()
        .get_json(&grant_key(&token))
        .await
        .unwrap()
        .is_some());
    assert!(fixture
        .state
        .infrastructure
        .distributed_lock()
        .release(&grant_lock_key(&token), competing_owner)
        .await
        .unwrap());
    let first = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-write",
        "savePage",
        digest,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document.clone() } }),
        Some(&token),
        Some(false),
    )
    .await;
    assert_eq!(first.0, StatusCode::OK, "{}", first.1);
    assert!(fixture
        .state
        .infrastructure
        .cache_store()
        .get_json(&grant_key(&token))
        .await
        .unwrap()
        .is_none());

    let replay = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "block-write",
        "savePage",
        digest,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document } }),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(replay.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn write_grant_rejects_expiry_and_every_bound_identity_mismatch() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (actor_id, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, root_uid) = create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let (second_tab, second_root_uid) =
        create_tab(&fixture.app, &cookie, &csrf, workspace_id, page_id).await;
    let catalog = catalog_entry(
        &fixture.app,
        &cookie,
        workspace_id,
        PAGE_TAB_SAVE_OPERATION_ID,
    )
    .await;
    let digest = catalog["schema_digest"].as_str().unwrap();
    let primary_document = document(
        &root_uid,
        vec![
            ("block-write", vec![binding("savePage", &catalog)]),
            ("block-other", vec![binding("saveOther", &catalog)]),
        ],
    );
    save_document(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        &primary_document,
    )
    .await;
    let second_document = document(
        &second_root_uid,
        vec![
            ("block-write", vec![binding("savePage", &catalog)]),
            ("block-other", vec![binding("saveOther", &catalog)]),
        ],
    );
    save_document(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        &second_document,
    )
    .await;

    struct Case {
        name: &'static str,
        actor: Uuid,
        tab: Uuid,
        block: &'static str,
        alias: &'static str,
        draft: &'static str,
        operation: &'static str,
        expires: OffsetDateTime,
        dispatch_tab: Uuid,
    }
    let cases = [
        Case {
            name: "expired",
            actor: actor_id,
            tab: tab_id,
            block: "block-write",
            alias: "savePage",
            draft: "draft",
            operation: PAGE_TAB_SAVE_OPERATION_ID,
            expires: OffsetDateTime::now_utc() - Duration::seconds(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "operation",
            actor: actor_id,
            tab: tab_id,
            block: "block-write",
            alias: "savePage",
            draft: "draft",
            operation: PAGE_TAB_GET_OPERATION_ID,
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "block",
            actor: actor_id,
            tab: tab_id,
            block: "block-other",
            alias: "savePage",
            draft: "draft",
            operation: PAGE_TAB_SAVE_OPERATION_ID,
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "tab",
            actor: actor_id,
            tab: tab_id,
            block: "block-write",
            alias: "savePage",
            draft: "draft",
            operation: PAGE_TAB_SAVE_OPERATION_ID,
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: second_tab,
        },
        Case {
            name: "user",
            actor: Uuid::new_v4(),
            tab: tab_id,
            block: "block-write",
            alias: "savePage",
            draft: "draft",
            operation: PAGE_TAB_SAVE_OPERATION_ID,
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "draft",
            actor: actor_id,
            tab: tab_id,
            block: "block-write",
            alias: "savePage",
            draft: "other-draft",
            operation: PAGE_TAB_SAVE_OPERATION_ID,
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
    ];
    for case in cases {
        let token = format!("grant-{}", case.name);
        seed_grant(
            &fixture.state,
            &token,
            case.actor,
            workspace_id,
            page_id,
            case.tab,
            case.block,
            case.alias,
            "run",
            case.draft,
            case.operation,
            case.expires,
        )
        .await;
        let (status, payload) = dispatch(
            &fixture.app,
            &cookie,
            &csrf,
            workspace_id,
            page_id,
            case.dispatch_tab,
            "block-write",
            "savePage",
            digest,
            "run",
            "draft",
            json!({ "body": { "payload": primary_document.clone() } }),
            Some(&token),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{}: {payload}", case.name);
    }
}
