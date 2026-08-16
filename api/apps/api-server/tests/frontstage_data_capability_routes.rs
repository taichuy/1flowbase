use std::sync::Arc;

use api_server::{
    app_state::ApiState,
    app_with_state_and_config,
    config::ApiConfig,
    host_infrastructure::build_local_host_infrastructure,
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
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

const PAGE_TAB_SAVE_OPERATION_ID: &str = "save_frontstage_tab_document";
const PAGE_TAB_GET_PATH: &str =
    "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_reference}";
const PAGE_TAB_SAVE_PATH: &str =
    "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/document";
const PAGE_TAB_DELETE_PATH: &str =
    "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}";
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
            freshness: control_plane::ports::OfficialPluginCatalogFreshness::Fresh,
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
    _database: postgres_test_support::PostgresTestSchema,
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

async fn isolated_database(base_url: &str) -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(base_url)
        .await
        .unwrap()
}

async fn fixture() -> Fixture {
    let mut config = test_config();
    let database = isolated_database(&config.database_url).await;
    config.database_url = database.database_url().to_owned();
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

    let provider_runtime = Arc::new(
        ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests(
            Arc::new(RwLock::new(
                plugin_runner::provider_host::ProviderHost::default(),
            )),
            Arc::new(RwLock::new(
                plugin_runner::capability_host::CapabilityHost::default(),
            )),
            Arc::new(RwLock::new(
                plugin_runner::data_source_host::DataSourceHost::default(),
            )),
        ),
    );
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
                config.api_node_id.clone(),
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
    let system_maintenance = Arc::new(control_plane::system_recovery::SystemMaintenance::default());
    let file_storage_registry = Arc::new(storage_object::builtin_driver_registry());
    // This suite exercises frontstage data capabilities, not host backup tooling.
    // Keeping the optional runtime absent avoids requiring pg_dump/pg_restore here.
    let system_backup = None;
    let state = Arc::new(ApiState {
        store: store.clone(),
        system_backup,
        system_maintenance,
        authenticator_registry: Arc::new(control_plane::auth::AuthenticatorRegistry::new()),
        settings_feature_registry,
        console_operation_registry,
        infrastructure,
        console_surface_registry: Arc::new(
            api_server::console_surface_registry::ConsoleSurfaceRegistry::default(),
        ),
        file_storage_registry,
        runtime_engine,
        provider_runtime,
        process_started_at,
        runtime_activity: Arc::new(
            api_server::runtime_activity::ApplicationRuntimeActivityTracker::default(),
        ),
        assistant_conversation_events: Default::default(),
        assistant_executions: Default::default(),
        api_runtime_profile: Arc::new(
            HostApiRuntimeProfileCollector::new(process_started_at).unwrap(),
        ),
        extension_boot_snapshot: None,
        plugin_runner_system: Arc::new(UnreachablePluginRunner),
        official_plugin_source: Arc::new(NoopPluginSource),
        official_mcp_bundle_source: Arc::new(NoopMcpSource),
        official_extension_catalog_source: Arc::new(
            api_server::official_extension_catalog::ApiOfficialExtensionCatalogSource::from_config(
                &config,
            ),
        ),
        official_i18n_catalog_update_service:
            api_server::app_state::build_official_i18n_catalog_update_service(
                store.clone(),
                &config,
            ),
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
        bootstrap_workspace_id: bootstrap.workspace_id,
        bootstrap_workspace_name: config.bootstrap_workspace_name.clone(),
    });
    Fixture {
        app: app_with_state_and_config(state.clone(), &config),
        state,
        _database: database,
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

async fn create_member(app: &Router, cookie: &str, csrf: &str, account: &str, password: &str) {
    let (status, payload) = json_request(
        app,
        "POST",
        "/api/console/settings/members",
        cookie,
        csrf,
        json!({
            "account": account,
            "email": format!("{account}@example.com"),
            "phone": null,
            "password": password,
            "name": account,
            "nickname": account,
            "introduction": "",
            "email_login_enabled": true,
            "phone_login_enabled": false
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
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
    let page_id = Uuid::parse_str(payload["data"]["page"]["id"].as_str().unwrap()).unwrap();
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
        &format!("/api/console/frontstage/{workspace_id}/interface-capabilities/{operation}"),
        cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"].clone()
}

fn document(root_uid: &str) -> Value {
    json!({ "version": 1, "root_uid": root_uid })
}

async fn create_block_node(
    app: &Router,
    cookie: &str,
    csrf: &str,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    title: &str,
) -> String {
    let (status, payload) = json_request(
        app,
        "POST",
        &format!("/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks"),
        cookie,
        csrf,
        json!({
            "tab_id": tab_id,
            "title": title,
            "presentation": "page",
            "parent_block_id": null,
            "before_block_id": null,
            "after_block_id": null,
            "source_code": "export default function Block() { return null; }",
            "dependency_lock": [],
            "runtime_descriptor": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    payload["data"]["block_id"].as_str().unwrap().to_owned()
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
    method: &str,
    path: &str,
    run_id: &str,
    draft_hash: &str,
    request: Value,
    write_grant: Option<&str>,
) -> (StatusCode, Value) {
    let body = json!({
        "block_id": block_id,
        "method": method,
        "path": path,
        "run_id": run_id,
        "draft_hash": draft_hash,
        "request": request,
        "write_grant": write_grant
    });
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
    method: &str,
    path: &str,
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
            "method": method,
            "path": path,
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
async fn capability_catalog_pages_lightweight_path_matches_and_loads_one_detail() {
    let fixture = fixture().await;
    let (cookie, _) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (status, payload) = get(
        &fixture.app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_query=application_conversations&adapter_id=runtime_data_model&method=GET&offset=0&limit=1"
        ),
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    let page = &payload["data"];
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{payload}");
    assert_eq!(page["total"], 2);
    assert_eq!(page["offset"], 0);
    assert_eq!(page["limit"], 1);
    assert_eq!(page["has_more"], true);
    assert_eq!(page["next_offset"], 1);
    assert!(page["adapter_ids"]
        .as_array()
        .unwrap()
        .contains(&json!("runtime_data_model")));
    assert!(page["methods"].as_array().unwrap().contains(&json!("GET")));

    let summary = &page["items"][0];
    assert_eq!(summary["method"], "GET");
    assert_eq!(summary["adapter_id"], "runtime_data_model");
    assert!(summary["path"]
        .as_str()
        .unwrap()
        .contains("/application_conversations/"));
    assert!(summary.get("parameter_schema").is_none());
    assert!(summary.get("result_schema").is_none());
    assert!(summary.get("name").is_none());

    let interface_id = summary["interface_id"].as_str().unwrap();
    let detail = catalog_entry(&fixture.app, &cookie, workspace_id, interface_id).await;
    assert_eq!(detail["interface_id"], interface_id);
    assert!(detail["parameter_schema"].is_object());
    assert!(detail["result_schema"].is_object());
    assert!(detail["schema_digest"].is_string());

    let (id_search_status, id_search_payload) = get(
        &fixture.app,
        &format!(
            "/api/console/frontstage/{workspace_id}/interface-capabilities?path_query={interface_id}"
        ),
        &cookie,
    )
    .await;
    assert_eq!(id_search_status, StatusCode::OK, "{id_search_payload}");
    assert_eq!(id_search_payload["data"]["total"], 0);
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
    method: &str,
    path: &str,
    run_id: &str,
    draft_hash: &str,
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
                "method": method,
                "path": path,
                "run_id": run_id,
                "draft_hash": draft_hash,
                "expires_at": serde_json::to_value(expires_at).unwrap()
            }),
            Some(Duration::minutes(5)),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn callable_dispatch_uses_method_path_and_current_tab_block_node() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, _root_uid) =
        create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let (second_tab, _second_root_uid) =
        create_tab(&fixture.app, &cookie, &csrf, workspace_id, page_id).await;
    let first_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "Block A",
    )
    .await;
    let _first_sibling = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "Block B",
    )
    .await;
    let second_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        "Second tab block",
    )
    .await;

    for (target_tab, block, method, path, expected) in [
        (
            tab_id,
            first_block.as_str(),
            "GET",
            "/api/console/missing",
            StatusCode::NOT_FOUND,
        ),
        (
            tab_id,
            "missing-block",
            "GET",
            PAGE_TAB_GET_PATH,
            StatusCode::NOT_FOUND,
        ),
        (
            tab_id,
            first_block.as_str(),
            "POST",
            PAGE_TAB_GET_PATH,
            StatusCode::NOT_FOUND,
        ),
        (
            second_tab,
            second_block.as_str(),
            "GET",
            PAGE_TAB_GET_PATH,
            StatusCode::OK,
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
            method,
            path,
            "run-read",
            "draft-read",
            json!({}),
            None,
        )
        .await;
        assert_eq!(status, expected, "{block}/{method}/{path}/{target_tab}");
    }

    let (status, payload) = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        &first_block,
        "GET",
        PAGE_TAB_GET_PATH,
        "run-read",
        "draft-read",
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["page"]["id"], page_id.to_string());
}

#[tokio::test]
async fn write_grant_is_consumed_once_and_old_binding_contract_is_rejected() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, root_uid) = create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let primary_document = document(&root_uid);
    let block_id = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "Writable block",
    )
    .await;

    let (legacy_status, _) = json_request(
        &fixture.app,
        "POST",
        &format!(
            "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch"
        ),
        &cookie,
        &csrf,
        json!({
            "block_id": &block_id,
            "interface_id": PAGE_TAB_SAVE_OPERATION_ID,
            "binding_alias": "savePage",
            "schema_digest": "legacy-digest",
            "run_id": "run-write",
            "draft_hash": "draft-write",
            "request": { "body": { "payload": primary_document.clone() } }
        }),
    )
    .await;
    assert_eq!(legacy_status, StatusCode::UNPROCESSABLE_ENTITY);

    let token = issue_grant(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        &block_id,
        "PUT",
        PAGE_TAB_SAVE_PATH,
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
        &block_id,
        "PUT",
        PAGE_TAB_SAVE_PATH,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document.clone() } }),
        Some(&token),
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
        &block_id,
        "PUT",
        PAGE_TAB_SAVE_PATH,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document.clone() } }),
        Some(&token),
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
        &block_id,
        "PUT",
        PAGE_TAB_SAVE_PATH,
        "run-write",
        "draft-write",
        json!({ "body": { "payload": primary_document } }),
        Some(&token),
    )
    .await;
    assert_eq!(replay.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn callable_dispatch_preserves_no_content_and_target_conflict_status() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, first_tab, _first_root_uid) =
        create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let (second_tab, _second_root_uid) =
        create_tab(&fixture.app, &cookie, &csrf, workspace_id, page_id).await;

    let first_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        first_tab,
        "First delete block",
    )
    .await;
    let second_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        "Second delete block",
    )
    .await;

    let first_grant = issue_grant(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        first_tab,
        &first_block,
        "DELETE",
        PAGE_TAB_DELETE_PATH,
        "run-delete-first",
        "draft-delete-first",
    )
    .await;
    let deleted = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        first_tab,
        &first_block,
        "DELETE",
        PAGE_TAB_DELETE_PATH,
        "run-delete-first",
        "draft-delete-first",
        json!({}),
        Some(&first_grant),
    )
    .await;
    assert_eq!(deleted, (StatusCode::NO_CONTENT, Value::Null));

    let second_grant = issue_grant(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        &second_block,
        "DELETE",
        PAGE_TAB_DELETE_PATH,
        "run-delete-last",
        "draft-delete-last",
    )
    .await;
    let conflict = dispatch(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        &second_block,
        "DELETE",
        PAGE_TAB_DELETE_PATH,
        "run-delete-last",
        "draft-delete-last",
        json!({}),
        Some(&second_grant),
    )
    .await;
    assert_eq!(conflict.0, StatusCode::CONFLICT, "{}", conflict.1);
}

#[tokio::test]
async fn callable_dispatch_preserves_target_permission_denial_for_the_page_visitor() {
    let fixture = fixture().await;
    let (root_cookie, root_csrf) = login(&fixture.app, "root", "change-me").await;
    let (_, workspace_id) = session_identity(&fixture.app, &root_cookie).await;
    let (page_id, tab_id, _root_uid) =
        create_page(&fixture.app, &root_cookie, &root_csrf, workspace_id).await;
    let block_id = create_block_node(
        &fixture.app,
        &root_cookie,
        &root_csrf,
        workspace_id,
        page_id,
        tab_id,
        "Members block",
    )
    .await;
    create_member(
        &fixture.app,
        &root_cookie,
        &root_csrf,
        "page-visitor",
        "temp-pass",
    )
    .await;
    let (visibility_status, visibility_payload) = json_request(
        &fixture.app,
        "PUT",
        "/api/console/settings/roles/member/frontstage-routes",
        &root_cookie,
        &root_csrf,
        json!({ "page_ids": [page_id], "tab_ids": [tab_id] }),
    )
    .await;
    assert_eq!(
        visibility_status,
        StatusCode::NO_CONTENT,
        "{visibility_payload}"
    );
    let (visitor_cookie, visitor_csrf) = login(&fixture.app, "page-visitor", "temp-pass").await;

    let denied = dispatch(
        &fixture.app,
        &visitor_cookie,
        &visitor_csrf,
        workspace_id,
        page_id,
        tab_id,
        &block_id,
        "GET",
        "/api/console/settings/members",
        "run-visitor",
        "draft-visitor",
        json!({}),
        None,
    )
    .await;
    assert_eq!(denied.0, StatusCode::FORBIDDEN, "{}", denied.1);
}

#[tokio::test]
async fn write_grant_rejects_expiry_and_every_source_identity_mismatch() {
    let fixture = fixture().await;
    let (cookie, csrf) = login(&fixture.app, "root", "change-me").await;
    let (actor_id, workspace_id) = session_identity(&fixture.app, &cookie).await;
    let (page_id, tab_id, root_uid) = create_page(&fixture.app, &cookie, &csrf, workspace_id).await;
    let (second_tab, _second_root_uid) =
        create_tab(&fixture.app, &cookie, &csrf, workspace_id, page_id).await;
    let primary_document = document(&root_uid);
    let primary_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "Primary write block",
    )
    .await;
    let other_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        tab_id,
        "Other block",
    )
    .await;
    let second_block = create_block_node(
        &fixture.app,
        &cookie,
        &csrf,
        workspace_id,
        page_id,
        second_tab,
        "Second tab block",
    )
    .await;

    struct Case {
        name: &'static str,
        actor: Uuid,
        tab: Uuid,
        block: String,
        dispatch_block: String,
        method: &'static str,
        path: &'static str,
        draft: &'static str,
        expires: OffsetDateTime,
        dispatch_tab: Uuid,
    }
    let cases = [
        Case {
            name: "expired",
            actor: actor_id,
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "PUT",
            path: PAGE_TAB_SAVE_PATH,
            draft: "draft",
            expires: OffsetDateTime::now_utc() - Duration::seconds(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "route-method",
            actor: actor_id,
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "GET",
            path: PAGE_TAB_SAVE_PATH,
            draft: "draft",
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "block",
            actor: actor_id,
            tab: tab_id,
            block: other_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "PUT",
            path: PAGE_TAB_SAVE_PATH,
            draft: "draft",
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "route-path",
            actor: actor_id,
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "PUT",
            path: "/api/console/frontstage/other",
            draft: "draft",
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "tab",
            actor: actor_id,
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: second_block.clone(),
            method: "PUT",
            path: PAGE_TAB_SAVE_PATH,
            draft: "draft",
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: second_tab,
        },
        Case {
            name: "user",
            actor: Uuid::new_v4(),
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "PUT",
            path: PAGE_TAB_SAVE_PATH,
            draft: "draft",
            expires: OffsetDateTime::now_utc() + Duration::minutes(1),
            dispatch_tab: tab_id,
        },
        Case {
            name: "draft",
            actor: actor_id,
            tab: tab_id,
            block: primary_block.clone(),
            dispatch_block: primary_block.clone(),
            method: "PUT",
            path: PAGE_TAB_SAVE_PATH,
            draft: "other-draft",
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
            &case.block,
            case.method,
            case.path,
            "run",
            case.draft,
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
            &case.dispatch_block,
            "PUT",
            PAGE_TAB_SAVE_PATH,
            "run",
            "draft",
            json!({ "body": { "payload": primary_document.clone() } }),
            Some(&token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{}: {payload}", case.name);
    }
}
