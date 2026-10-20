use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use control_plane::ports::{
    CreateModelProviderInstanceInput, CreateNetworkEgressPoolInput,
    CreateNetworkEgressPoolMemberInput, CreateNetworkEgressProviderInput,
    CreateNetworkEgressRouteInput, ModelProviderRepository, NetworkEgressPoolRepository,
    NetworkEgressRepository, NetworkEgressRouteRepository, OfficialPluginSourcePort,
    PluginRepository, ReplaceNetworkEgressProjectionInput, UpsertNetworkEgressProviderSecretInput,
    UpsertPluginArtifactInstanceInput, UpsertPluginInstallationInput,
};
use domain::{
    NetworkEgressConsumerSelector, NetworkEgressHealthStatus, NetworkEgressProviderLifecycle,
    PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus, PluginVerificationStatus,
};
use plugin_framework::compute_manifest_fingerprint;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    config::ResolvedOfficialExtensionCatalogSourceConfig,
    network_egress_client::NetworkEgressHttpClientResolver,
    official_extension_catalog::{
        ApiOfficialExtensionCatalogSource, ApiOfficialRuntimeExtensionSource,
        OfficialExtensionArtifactError, OfficialExtensionCatalogSearchQuery,
        OfficialExtensionCatalogSourcePort,
    },
    provider_runtime::ApiProviderRuntime,
};

const CATEGORIES: [&str; 7] = [
    "agent-flow",
    "applications-demo",
    "capability-plugins",
    "host-extensions",
    "i18n",
    "mcp",
    "runtime-extensions",
];

#[derive(Clone)]
struct CatalogHttpFixture {
    documents: Arc<BTreeMap<String, Vec<u8>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
struct MutableCatalogHttpFixture {
    documents: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

pub(super) struct TempNetworkEgressPackage {
    root: PathBuf,
}

impl TempNetworkEgressPackage {
    fn new(proxy_url: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("catalog-network-egress-{nonce}"));
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("provider")).unwrap();
        fs::write(
            root.join("manifest.yaml"),
            r#"manifest_version: 1
plugin_id: fixture_egress@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Egress
description: Fixture Egress
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: stateful_runtime_worker
slot_codes:
  - network_egress_provider
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.network_egress_provider/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_egress
  limits:
    timeout_ms: 2000
node_contributions: []
"#,
        )
        .unwrap();
        fs::write(
            root.join("bin/fixture_egress"),
            format!(
                "#!/usr/bin/env bash\nset -euo pipefail\nif [ \"${{1:-}}\" = \"--network-egress-config-file\" ]; then shift 2; fi\nwhile IFS= read -r request; do\n  case \"${{request}}\" in\n    *'\"operation\":\"sync_egresses\"'*) printf '%s\\n' '{{\"operation\":\"sync_egresses\",\"result\":{{\"egresses\":[{{\"provider_egress_key\":\"fixture-egress\",\"display_name\":\"Fixture Egress\",\"availability\":\"available\"}}]}}}}' ;;\n    *'\"operation\":\"acquire_http_forward_proxy\"'*) printf '%s\\n' '{{\"operation\":\"acquire_http_forward_proxy\",\"result\":{{\"lease_id\":\"fixture-lease\",\"http_proxy_url\":\"{proxy_url}\",\"cleanup_token\":\"host-private\",\"expires_at\":4102444800000}}}}' ;;\n    *'\"operation\":\"release_http_forward_proxy\"'*) printf '%s\\n' '{{\"operation\":\"release_http_forward_proxy\",\"result\":{{\"lease_id\":\"fixture-lease\"}}}}' ;;\n    *) exit 1 ;;\n  esac\ndone\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("provider/egress-provider.yaml"),
            r#"provider_code: fixture_egress
display_name: Fixture Egress
form_schema:
  schema_version: 1flowbase.plugin.form/v1
  fields:
    - key: subscription_url
      label: Subscription URL
      type: string
      control: url
      required: true
      send_mode: secret
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = root.join("bin/fixture_egress");
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempNetworkEgressPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn seed_github_egress_resolver(
    state: &crate::app_state::ApiState,
    proxy_url: &str,
) -> (NetworkEgressHttpClientResolver, TempNetworkEgressPackage) {
    seed_network_egress_resolver(
        state,
        proxy_url,
        NetworkEgressConsumerSelector::GithubOfficialSources,
        ApiProviderRuntime::new(Arc::clone(&state.provider_runtime)),
    )
    .await
}

/// Persists the full Route -> Pool -> Provider -> projection fixture used by all Network Center
/// consumer tests. Callers supply the runtime so the resolver and its consumer share the same
/// Host-owned `NetworkEgressHost`; this deliberately avoids an injected proxy client.
pub(super) async fn seed_network_egress_resolver(
    state: &crate::app_state::ApiState,
    proxy_url: &str,
    selector: NetworkEgressConsumerSelector,
    runtime: ApiProviderRuntime,
) -> (NetworkEgressHttpClientResolver, TempNetworkEgressPackage) {
    let package = TempNetworkEgressPackage::new(proxy_url);
    let root = state
        .store
        .find_user_for_password_login(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID, "root")
        .await
        .unwrap()
        .unwrap();
    let installation_id = uuid::Uuid::now_v7();
    let manifest_fingerprint = compute_manifest_fingerprint(&package.path().join("manifest.yaml"))
        .await
        .unwrap();
    <storage_durable_postgres::MainDurableStore as PluginRepository>::upsert_installation(
        &state.store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "test".into(),
            provider_code: "fixture_egress".into(),
            plugin_id: "fixture_egress@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT.into(),
            protocol: "stdio_json_worker".into(),
            display_name: "Fixture Egress".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"plugin_type": "network_egress_provider"}),
            is_system_reserved: false,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    if let NetworkEgressConsumerSelector::ModelProviderInstance { instance_id } = &selector {
        let model_installation_id = uuid::Uuid::now_v7();
        <storage_durable_postgres::MainDurableStore as PluginRepository>::upsert_installation(
            &state.store,
            &UpsertPluginInstallationInput {
                installation_id: model_installation_id,
                category: domain::ExtensionCategory::RuntimeExtensions,
                organization: "test".into(),
                provider_code: "fixture_provider".into(),
                plugin_id: "fixture_provider@0.1.0".into(),
                plugin_version: "0.1.0".into(),
                contract_version: "1flowbase.provider/v2".into(),
                protocol: "openai_compatible".into(),
                display_name: "Fixture Provider".into(),
                source_kind: "uploaded".into(),
                trust_level: "unverified".into(),
                verification_status: PluginVerificationStatus::Valid,
                desired_state: PluginDesiredState::ActiveRequested,
                expected_checksum: None,
                signature_status: domain::ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                metadata_json: json!({}),
                is_system_reserved: false,
                actor_user_id: root.id,
            },
        )
        .await
        .unwrap();
        <storage_durable_postgres::MainDurableStore as ModelProviderRepository>::create_instance(
            &state.store,
            &CreateModelProviderInstanceInput {
                instance_id: *instance_id,
                workspace_id: state.bootstrap_workspace_id,
                installation_id: model_installation_id,
                provider_code: "fixture_provider".into(),
                protocol: "openai_compatible".into(),
                display_name: "Fixture Model Provider".into(),
                status: domain::ModelProviderInstanceStatus::Ready,
                config_json: json!({"base_url": "https://fixture.invalid"}),
                configured_models: vec![domain::ModelProviderConfiguredModel {
                    model_id: "fixture_chat".into(),
                    enabled: true,
                    context_window_override_tokens: None,
                    supports_multimodal: None,
                    pricing_provider_code: domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE.into(),
                    pricing_model_id: domain::DEFAULT_MODEL_PRICING_MODEL_ID.into(),
                }],
                enabled_model_ids: vec!["fixture_chat".into()],
                included_in_main: Some(false),
                created_by: root.id,
            },
        )
        .await
        .unwrap();
    }
    <storage_durable_postgres::MainDurableStore as PluginRepository>::upsert_artifact_instance(
        &state.store,
        &UpsertPluginArtifactInstanceInput {
            node_id: state.api_node_id.clone(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some(package.path().display().to_string()),
            package_path: None,
            manifest_fingerprint: Some(manifest_fingerprint),
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
        },
    )
    .await
    .unwrap();
    let provider_id = uuid::Uuid::now_v7();
    <storage_durable_postgres::MainDurableStore as NetworkEgressRepository>::create_network_egress_provider(
        &state.store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "test",
                "fixture_egress",
            ),
            provider_code: "fixture_egress".into(),
            display_name: "Fixture Egress".into(),
            description: String::new(),
            secret_ref: "secret://fixture-egress".into(),
            lifecycle: NetworkEgressProviderLifecycle::Active,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    <storage_durable_postgres::MainDurableStore as NetworkEgressRepository>::upsert_network_egress_provider_secret(
        &state.store,
        &UpsertNetworkEgressProviderSecretInput {
            provider_id,
            secret_ref: "secret://fixture-egress".into(),
            plaintext_secret_json: json!({"token": "egress-provider-secret"}),
            master_key: state.provider_secret_master_key.clone(),
            secret_version: 1,
        },
    )
    .await
    .unwrap();
    <storage_durable_postgres::MainDurableStore as NetworkEgressRepository>::replace_network_egress_projection(
        &state.store,
        &ReplaceNetworkEgressProjectionInput {
            provider_id,
            health_status: NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            synchronized_at: time::OffsetDateTime::now_utc(),
            egresses: vec![domain::NetworkEgressProjectionRecord {
                provider_id,
                provider_egress_key: "fixture-egress".into(),
                display_name: "Fixture Egress".into(),
                region: None,
                tags: Vec::new(),
                availability: "available".into(),
                synced_at: time::OffsetDateTime::now_utc(),
            }],
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    let pool_id = uuid::Uuid::now_v7();
    <storage_durable_postgres::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool(
        &state.store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Fixture Pool".into(),
            owner_provider_id: None,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    let member_id = uuid::Uuid::now_v7();
    <storage_durable_postgres::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool_member(
        &state.store,
        &CreateNetworkEgressPoolMemberInput {
            member_id,
            pool_id,
            provider_id,
            provider_egress_key: "fixture-egress".into(),
            enabled: true,
            sequence: 0,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    <storage_durable_postgres::MainDurableStore as NetworkEgressRouteRepository>::create_network_egress_route(
        &state.store,
        &CreateNetworkEgressRouteInput {
            route_id: uuid::Uuid::now_v7(),
            workspace_id: state.bootstrap_workspace_id,
            selector,
            pool_id,
            pool_member_ids: vec![member_id],
            enabled: true,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    (
        NetworkEgressHttpClientResolver::new(
            state.store.clone(),
            runtime,
            state.provider_secret_master_key.clone(),
            state.api_node_id.clone(),
        ),
        package,
    )
}

async fn mutable_catalog_response(
    State(fixture): State<MutableCatalogHttpFixture>,
    request: Request<Body>,
) -> Response {
    fixture
        .requests
        .lock()
        .unwrap()
        .push(request.uri().path().to_string());
    match fixture.documents.lock().unwrap().get(request.uri().path()) {
        Some(bytes) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            bytes.clone(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Clone)]
struct RetryArtifactFixture {
    catalog: CatalogHttpFixture,
    artifact_requests: Arc<std::sync::atomic::AtomicUsize>,
    failures_before_success: usize,
    success_body: Arc<Vec<u8>>,
}

async fn retry_artifact_response(
    State(fixture): State<RetryArtifactFixture>,
    request: Request<Body>,
) -> Response {
    if request.uri().path() == "/i18n/artifacts/i18n-fixture.bin" {
        let request_number = fixture.artifact_requests.fetch_add(1, Ordering::SeqCst) + 1;
        if request_number <= fixture.failures_before_success {
            let stream = futures_util::stream::iter(vec![
                Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"partial")),
                Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "publisher cutover interrupted body",
                )),
            ]);
            return Response::new(Body::from_stream(stream));
        }
        return Body::from(fixture.success_body.as_ref().clone()).into_response();
    }
    catalog_response(State(fixture.catalog), request).await
}

#[tokio::test]
async fn delivery_1560_d5_ac_001_mirror_failure_falls_back_to_official_metadata() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, _) = catalog_documents(&base_url);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let fixture = CatalogHttpFixture {
        documents: Arc::new(documents),
        requests: Arc::clone(&requests),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(catalog_response).with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(BTreeMap::from([(
        "runtime-extensions".to_string(),
        ResolvedOfficialExtensionCatalogSourceConfig {
            source_kind: "configured_mirror".to_string(),
            index_url: format!("{base_url}/unavailable-mirror/index.json"),
            official_index_url: format!("{base_url}/runtime-extensions/catalog/v1/index.json"),
            github_proxy_url: None,
        },
    )]));

    let page = source.list_page("runtime-extensions", None).await.unwrap();
    assert_eq!(page.source_kind, "official_repository");
    assert_eq!(
        page.metadata.freshness,
        crate::official_extension_catalog::OfficialExtensionCatalogFreshness::Fresh
    );
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &[
            "/unavailable-mirror/index.json",
            "/runtime-extensions/catalog/v1/index.json",
            "/runtime-extensions/catalog/v1/pages/1.json"
        ]
    );
    server.abort();
}

#[tokio::test]
async fn delivery_1560_d5_f01b_runtime_plugin_lifecycle_uses_gateway_projection_and_exact_download()
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let fixture = CatalogHttpFixture {
        documents: Arc::new(documents),
        requests: Arc::clone(&requests),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(catalog_response).with_state(fixture),
        )
        .await
        .unwrap();
    });
    let catalog: Arc<dyn OfficialExtensionCatalogSourcePort> =
        Arc::new(ApiOfficialExtensionCatalogSource::new(sources));
    let source = ApiOfficialRuntimeExtensionSource::new(
        catalog,
        "signature_required".to_string(),
        Vec::new(),
    );

    let snapshot = source.list_official_catalog().await.unwrap();
    assert_eq!(snapshot.entries.len(), 2, "all gateway pages are projected");
    assert_eq!(snapshot.entries[0].plugin_id, "1flowbase.openai");
    assert_eq!(snapshot.entries[1].plugin_id, "1flowbase.later-runtime");
    let downloaded = source.download_plugin(&snapshot.entries[1]).await.unwrap();
    assert_eq!(downloaded.package_bytes, b"later-runtime-artifact");

    let mut unprojected = snapshot.entries[1].clone();
    unprojected.latest_version = "9.9.9".to_string();
    assert!(source.download_plugin(&unprojected).await.is_err());
    assert!(requests
        .lock()
        .unwrap()
        .iter()
        .all(|path| !path.contains("official-registry")));
    server.abort();
}

#[tokio::test]
async fn delivery_1560_d5_f01b_duplicate_runtime_plugin_id_fails_closed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (mut documents, sources) = catalog_documents(&base_url);
    let page_path = "/runtime-extensions/catalog/v1/pages/2.json";
    let mut page = serde_json::from_slice::<Value>(&documents[page_path]).unwrap();
    page["entries"][0]["source"]["plugin_id"] = json!("1flowbase.openai");
    let page_bytes = serde_json::to_vec(&page).unwrap();
    documents.insert(page_path.to_string(), page_bytes.clone());
    let index_path = "/runtime-extensions/catalog/v1/index.json";
    let mut index = serde_json::from_slice::<Value>(&documents[index_path]).unwrap();
    index["pages"][1]["checksum"] = json!(format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(&page_bytes))
    ));
    documents.insert(index_path.to_string(), serde_json::to_vec(&index).unwrap());
    let fixture = CatalogHttpFixture {
        documents: Arc::new(documents),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(catalog_response).with_state(fixture),
        )
        .await
        .unwrap();
    });
    let catalog: Arc<dyn OfficialExtensionCatalogSourcePort> =
        Arc::new(ApiOfficialExtensionCatalogSource::new(sources));
    let source = ApiOfficialRuntimeExtensionSource::new(
        catalog,
        "signature_required".to_string(),
        Vec::new(),
    );

    let error = source.list_official_catalog().await.unwrap_err();
    assert!(error.to_string().contains("duplicate plugin_id"));
    server.abort();
}

#[derive(Clone)]
struct SwitchableCatalogFixture {
    documents: Arc<BTreeMap<String, Vec<u8>>>,
    failing: Arc<AtomicBool>,
}

async fn switchable_catalog_response(
    State(fixture): State<SwitchableCatalogFixture>,
    request: Request<Body>,
) -> Response {
    if fixture.failing.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match fixture.documents.get(request.uri().path()) {
        Some(bytes) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            bytes.clone(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn delivery_1560_d5_ac_002_last_success_is_returned_stale_without_touching_local_artifacts() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let failing = Arc::new(AtomicBool::new(false));
    let fixture = SwitchableCatalogFixture {
        documents: Arc::new(documents),
        failing: Arc::clone(&failing),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(switchable_catalog_response)
                .with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    source.list_page("mcp", None).await.unwrap();
    failing.store(true, Ordering::SeqCst);

    let stale = source.list_page("mcp", None).await.unwrap();
    assert_eq!(
        stale.metadata.freshness,
        crate::official_extension_catalog::OfficialExtensionCatalogFreshness::Stale
    );
    assert_eq!(stale.entries[0].id, "mcp:taichuy/mcp-fixture");
    server.abort();
}

async fn never_responds() -> Response {
    std::future::pending::<Response>().await
}

#[tokio::test]
async fn delivery_1560_d5_ac_003_no_stale_timeout_returns_a_bounded_clear_failure() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let index_url = format!("http://{}/index.json", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(never_responds))
            .await
            .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new_with_request_timeouts(
        BTreeMap::from([(
            "mcp".to_string(),
            ResolvedOfficialExtensionCatalogSourceConfig {
                source_kind: "official_repository".to_string(),
                index_url: index_url.clone(),
                official_index_url: index_url,
                github_proxy_url: None,
            },
        )]),
        Duration::from_millis(40),
        Duration::from_millis(500),
    );
    let started = Instant::now();
    let error = source.list_page("mcp", None).await.unwrap_err();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(error
        .to_string()
        .contains("failed to request official extension catalog document"));
    server.abort();
}

async fn delayed_artifact_response() -> Response {
    tokio::time::sleep(Duration::from_millis(60)).await;
    Body::from(artifact_bytes("i18n", "i18n-fixture")).into_response()
}

#[tokio::test]
async fn ac_004_artifact_download_can_outlast_the_catalog_request_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(delayed_artifact_response))
            .await
            .unwrap();
    });
    let entry = serde_json::from_value(catalog_entry(
        &base_url,
        "i18n",
        1,
        "i18n:taichuy/i18n-fixture",
        "i18n-fixture",
    ))
    .unwrap();
    let source = ApiOfficialExtensionCatalogSource::new_with_request_timeouts(
        BTreeMap::from([(
            "i18n".to_string(),
            ResolvedOfficialExtensionCatalogSourceConfig {
                source_kind: "official_repository".to_string(),
                index_url: format!("{base_url}/index.json"),
                official_index_url: format!("{base_url}/index.json"),
                github_proxy_url: None,
            },
        )]),
        Duration::from_millis(20),
        Duration::from_millis(250),
    );

    let downloaded = source.download_artifact(&entry).await.unwrap();

    assert_eq!(downloaded.artifact_bytes, b"i18n-artifact");
    server.abort();
}

#[test]
fn agent_flow_https_download_preserves_integrity_metadata_and_rewrites_github_proxy() {
    let category = "agent-flow";
    let source = ApiOfficialExtensionCatalogSource::new(BTreeMap::from([(
        category.to_string(),
        ResolvedOfficialExtensionCatalogSourceConfig {
            source_kind: "configured_mirror".to_string(),
            index_url: "https://example.test/index.json".to_string(),
            official_index_url: "https://example.test/index.json".to_string(),
            github_proxy_url: Some("https://proxy.example".to_string()),
        },
    )]));
    let mut entry = serde_json::from_value::<
        crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    >(catalog_entry(
        "https://example.test",
        category,
        1,
        "agent-flow:taichuy/fusion",
        "fusion",
    ))
    .unwrap();
    let expected_checksum = format!("sha256:{}", "a".repeat(64));
    let expected_signature = json!({"algorithm": "ed25519", "key_id": "official-key"});
    entry.download_locator = json!({
        "kind": "https",
        "locator": "https://github.com/taichuy/1flowbase-official-plugins/releases/download/agent-flow-taichuy-fusion-v2/template.json"
    });
    entry.checksum = Some(expected_checksum.clone());
    entry.signature = Some(expected_signature.clone());

    let descriptor = source.resolve_artifact(&entry).unwrap();

    assert_eq!(descriptor.locator_kind, "https");
    assert_eq!(
        descriptor.locator,
        "https://proxy.example/https://github.com/taichuy/1flowbase-official-plugins/releases/download/agent-flow-taichuy-fusion-v2/template.json"
    );
    assert_eq!(
        descriptor.expected_checksum.as_deref(),
        Some(expected_checksum.as_str())
    );
    assert_eq!(descriptor.signature.as_ref(), Some(&expected_signature));
    assert!(descriptor.platform.is_none());
}

#[test]
fn root_1545_ac_3_platform_download_selects_current_target_and_rewrites_github_proxy() {
    let category = "runtime-extensions";
    let source = ApiOfficialExtensionCatalogSource::new(BTreeMap::from([(
        category.to_string(),
        ResolvedOfficialExtensionCatalogSourceConfig {
            source_kind: "configured_mirror".to_string(),
            index_url: "https://example.test/index.json".to_string(),
            official_index_url: "https://example.test/index.json".to_string(),
            github_proxy_url: Some("https://proxy.example".to_string()),
        },
    )]));
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        value => value,
    };
    let libc = if os == "linux" {
        Some(if cfg!(target_env = "musl") {
            "musl"
        } else {
            "gnu"
        })
    } else if os == "windows" {
        Some("msvc")
    } else {
        None
    };
    let mut entry = serde_json::from_value::<
        crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    >(catalog_entry(
        "https://example.test",
        category,
        1,
        "runtime-extensions:taichuy/runtime-fixture",
        "runtime-fixture",
    ))
    .unwrap();
    entry.download_locator = json!({
        "kind": "platform_release_assets",
        "artifacts": [
            {
                "os": "unsupported",
                "arch": arch,
                "libc": libc,
                "locator": "https://github.com/acme/extensions/releases/download/v1/wrong.pkg",
                "checksum": format!("sha256:{}", "b".repeat(64)),
                "signature": null
            },
            {
                "os": os,
                "arch": arch,
                "libc": libc,
                "locator": "https://github.com/acme/extensions/releases/download/v1/right.pkg",
                "checksum": format!("sha256:{}", "c".repeat(64)),
                "signature": {"algorithm": "ed25519", "key_id": "official-key"}
            }
        ]
    });

    let descriptor = source.resolve_artifact(&entry).unwrap();
    assert_eq!(descriptor.locator_kind, "platform_release_assets");
    assert_eq!(
        descriptor.locator,
        "https://proxy.example/https://github.com/acme/extensions/releases/download/v1/right.pkg"
    );
    assert_eq!(
        descriptor.expected_checksum.as_deref(),
        Some(format!("sha256:{}", "c".repeat(64)).as_str())
    );
    let platform = descriptor.platform.as_ref().unwrap();
    assert_eq!(platform.os, os);
    assert_eq!(platform.arch, arch);
    assert!(!platform.rust_target.is_empty());
}

async fn catalog_response(
    State(fixture): State<CatalogHttpFixture>,
    request: Request<Body>,
) -> Response {
    let path = request.uri().path().to_string();
    fixture.requests.lock().unwrap().push(path.clone());
    match fixture.documents.get(&path) {
        Some(bytes) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            bytes.clone(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn catalog_documents(
    base_url: &str,
) -> (
    BTreeMap<String, Vec<u8>>,
    BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>,
) {
    let mut documents = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for category in CATEGORIES {
        let page_one_entries = if category == "host-extensions" {
            Vec::new()
        } else {
            let artifact = if category == "runtime-extensions" {
                "openai".to_string()
            } else {
                format!("{category}-fixture")
            };
            vec![catalog_entry(
                base_url,
                category,
                1,
                &format!("{category}:taichuy/{artifact}"),
                &artifact,
            )]
        };
        let next_cursor = (category == "runtime-extensions").then_some("runtime-2");
        let page_one = catalog_page(category, 1, "start", next_cursor, page_one_entries);
        let mut pages = vec![(1, "start", page_one)];
        if category == "runtime-extensions" {
            pages.push((
                2,
                "runtime-2",
                catalog_page(
                    category,
                    2,
                    "runtime-2",
                    None,
                    vec![catalog_entry(
                        base_url,
                        category,
                        2,
                        "runtime-extensions:other/later-runtime",
                        "later-runtime",
                    )],
                ),
            ));
        }
        let search = catalog_search_index(base_url, category, &pages);
        let index = catalog_index(base_url, category, &pages, &search);
        documents.insert(format!("/{category}/catalog/v1/index.json"), index);
        documents.insert(format!("/{category}/catalog/v1/search-index.json"), search);
        for (page, _, bytes) in pages {
            documents.insert(format!("/{category}/catalog/v1/pages/{page}.json"), bytes);
        }
        sources.insert(
            category.to_string(),
            ResolvedOfficialExtensionCatalogSourceConfig {
                source_kind: "official_repository".to_string(),
                index_url: format!("{base_url}/{category}/catalog/v1/index.json"),
                official_index_url: format!("{base_url}/{category}/catalog/v1/index.json"),
                github_proxy_url: None,
            },
        );
        if category != "host-extensions" {
            documents.insert(
                format!("/{category}/artifacts/{category}-fixture.bin"),
                artifact_bytes(category, &format!("{category}-fixture")),
            );
            if category == "runtime-extensions" {
                documents.insert(
                    "/runtime-extensions/artifacts/openai.bin".to_string(),
                    b"openai-artifact".to_vec(),
                );
                documents.insert(
                    "/runtime-extensions/artifacts/later-runtime.bin".to_string(),
                    b"later-runtime-artifact".to_vec(),
                );
            }
        }
    }
    (documents, sources)
}

fn catalog_index(
    base_url: &str,
    category: &str,
    pages: &[(u32, &str, Vec<u8>)],
    search: &[u8],
) -> Vec<u8> {
    let page_references = pages
        .iter()
        .map(|(page, cursor, bytes)| {
            json!({
                "page": page,
                "cursor": cursor,
                "entry_count": serde_json::from_slice::<Value>(bytes).unwrap()["entries"].as_array().unwrap().len(),
                "checksum": format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes))),
                "locator": format!("{base_url}/{category}/catalog/v1/pages/{page}.json")
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "schema_version": "1flowbase.extension-catalog/v1",
        "category": category,
        "generated_at": "2026-08-01T00:00:00Z",
        "page_size": 100,
        "total_entries": page_references.iter().map(|page| page["entry_count"].as_u64().unwrap()).sum::<u64>(),
        "first_page": {
            "page": 1,
            "cursor": "start",
            "locator": format!("{base_url}/{category}/catalog/v1/pages/1.json")
        },
        "pages": page_references,
        "search_index": {
            "schema_version": "1flowbase.extension-catalog-search/v1",
            "entry_count": page_references.iter().map(|page| page["entry_count"].as_u64().unwrap()).sum::<u64>(),
            "checksum": format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(search))),
            "locator": format!("{base_url}/{category}/catalog/v1/search-index.json")
        }
    }))
    .unwrap()
}

fn catalog_search_index(base_url: &str, category: &str, pages: &[(u32, &str, Vec<u8>)]) -> Vec<u8> {
    let mut entries = Vec::new();
    for (page, cursor, bytes) in pages {
        let page_document = serde_json::from_slice::<Value>(bytes).unwrap();
        let page_checksum = format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)));
        for entry in page_document["entries"].as_array().unwrap() {
            let mut search_entry = entry.clone();
            search_entry
                .as_object_mut()
                .unwrap()
                .remove("download_locator");
            search_entry["slot_codes"] = if category == "runtime-extensions" {
                json!(["model_provider"])
            } else {
                json!([])
            };
            search_entry["keywords"] = json!([entry["artifact"].as_str().unwrap()]);
            search_entry["catalog_page"] = json!({
                "page": page,
                "cursor": cursor,
                "checksum": page_checksum,
                "locator": format!("{base_url}/{category}/catalog/v1/pages/{page}.json")
            });
            entries.push(search_entry);
        }
    }
    serde_json::to_vec(&json!({
        "schema_version": "1flowbase.extension-catalog-search/v1",
        "category": category,
        "generated_at": "2026-08-01T00:00:00Z",
        "source_fingerprint": format!("sha256:fixture-{category}"),
        "entries": entries
    }))
    .unwrap()
}

fn catalog_page(
    category: &str,
    page: u32,
    cursor: &str,
    next_cursor: Option<&str>,
    entries: Vec<Value>,
) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": "1flowbase.extension-catalog/v1",
        "category": category,
        "page": page,
        "cursor": cursor,
        "next_cursor": next_cursor,
        "next_page_locator": next_cursor.map(|_| "fixture-next-page"),
        "entries": entries
    }))
    .unwrap()
}

fn catalog_entry(base_url: &str, category: &str, page: u32, id: &str, artifact: &str) -> Value {
    let organization = id
        .split_once(':')
        .and_then(|(_, identity)| identity.split_once('/'))
        .map(|(organization, _)| organization)
        .unwrap();
    let mut entry = json!({
        "id": id,
        "name": format!("{artifact} display name"),
        "category": category,
        "organization": organization,
        "artifact": artifact,
        "version": "2.4.1",
        "description": "fixture extension",
        "host_version_requirement": ">=0.3.0",
        "slot_codes": if category == "runtime-extensions" { json!(["model_provider"]) } else { json!([]) },
        "keywords": [artifact],
        "source": {
            "kind": if category == "runtime-extensions" { "runtime_extension_manifest" } else { "fixture_source" },
            "locator": format!("{category}/@{organization}/{artifact}")
        },
        "signature": null,
        "checksum": artifact_checksum(category, artifact),
        "download_locator": {
            "kind": "repository_file",
            "locator": format!("{base_url}/{category}/artifacts/{artifact}.bin")
        },
        "catalog_page": page
    });
    if category == "runtime-extensions" {
        entry["source"]["plugin_id"] = json!(format!("1flowbase.{artifact}"));
        entry["source"]["plugin_type"] = json!("model_provider");
        entry["source"]["provider_code"] = json!(artifact);
        entry["source"]["protocol"] = json!("openai_compatible");
        entry["source"]["model_discovery_mode"] = json!("dynamic");
    }
    entry
}

fn artifact_bytes(category: &str, artifact: &str) -> Vec<u8> {
    if category == "i18n" {
        b"i18n-artifact".to_vec()
    } else if category == "runtime-extensions" {
        format!("{artifact}-artifact").into_bytes()
    } else {
        format!("{category}-artifact").into_bytes()
    }
}

fn artifact_checksum(category: &str, artifact: &str) -> String {
    format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(artifact_bytes(category, artifact)))
    )
}

#[path = "official_extension_catalog_source/application_templates.rs"]
mod application_templates;

#[path = "official_extension_catalog_source/client_collectors.rs"]
mod client_collectors;

#[path = "official_extension_catalog_source/artifact_transport.rs"]
mod artifact_transport;
mod catalog_search;
