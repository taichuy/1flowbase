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
    CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
    CreateNetworkEgressProviderInput, CreateNetworkEgressRouteInput, NetworkEgressPoolRepository,
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

const CATEGORIES: [&str; 6] = [
    "agent-flow",
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

struct TempNetworkEgressPackage {
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
    let package = TempNetworkEgressPackage::new(proxy_url);
    let root = state
        .store
        .find_user_for_password_login(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID, "root")
        .await
        .unwrap()
        .unwrap();
    let installation_id = uuid::Uuid::now_v7();
    let manifest_fingerprint = compute_manifest_fingerprint(&package.path().join("manifest.yaml"))
        .await
        .unwrap();
    <storage_durable::MainDurableStore as PluginRepository>::upsert_installation(
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
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    <storage_durable::MainDurableStore as PluginRepository>::upsert_artifact_instance(
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
            is_current: false,
        },
    )
    .await
    .unwrap();
    let provider_id = uuid::Uuid::now_v7();
    <storage_durable::MainDurableStore as NetworkEgressRepository>::create_network_egress_provider(
        &state.store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            installation_id,
            provider_code: "fixture_egress".into(),
            display_name: "Fixture Egress".into(),
            secret_ref: "secret://fixture-egress".into(),
            lifecycle: NetworkEgressProviderLifecycle::Active,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    <storage_durable::MainDurableStore as NetworkEgressRepository>::upsert_network_egress_provider_secret(
        &state.store,
        &UpsertNetworkEgressProviderSecretInput {
            provider_id,
            secret_ref: "secret://fixture-egress".into(),
            plaintext_secret_json: json!({"fixture": true}),
            master_key: state.provider_secret_master_key.clone(),
            secret_version: 1,
        },
    )
    .await
    .unwrap();
    <storage_durable::MainDurableStore as NetworkEgressRepository>::replace_network_egress_projection(
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
    <storage_durable::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool(
        &state.store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Fixture Pool".into(),
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    <storage_durable::MainDurableStore as NetworkEgressPoolRepository>::create_network_egress_pool_member(
        &state.store,
        &CreateNetworkEgressPoolMemberInput {
            member_id: uuid::Uuid::now_v7(),
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
    <storage_durable::MainDurableStore as NetworkEgressRouteRepository>::create_network_egress_route(
        &state.store,
        &CreateNetworkEgressRouteInput {
            route_id: uuid::Uuid::now_v7(),
            workspace_id: state.bootstrap_workspace_id,
            selector: NetworkEgressConsumerSelector::GithubOfficialSources,
            pool_id,
            enabled: true,
            actor_user_id: root.id,
        },
    )
    .await
    .unwrap();
    (
        NetworkEgressHttpClientResolver::new(
            state.store.clone(),
            ApiProviderRuntime::new(Arc::clone(&state.provider_runtime)),
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
async fn publisher_cutover_artifact_body_interruption_retries_once_with_fresh_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let artifact_requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fixture = RetryArtifactFixture {
        catalog: CatalogHttpFixture {
            documents: Arc::new(documents),
            requests: Arc::new(Mutex::new(Vec::new())),
        },
        artifact_requests: Arc::clone(&artifact_requests),
        failures_before_success: 1,
        success_body: Arc::new(b"i18n-artifact".to_vec()),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(retry_artifact_response)
                .with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let page = source.list_page("i18n", None).await.unwrap();

    let downloaded = source.download_artifact(&page.entries[0]).await.unwrap();

    assert_eq!(downloaded.artifact_bytes, b"i18n-artifact");
    assert_eq!(artifact_requests.load(Ordering::SeqCst), 2);
    server.abort();
}

#[tokio::test]
async fn publisher_cutover_artifact_two_body_failures_stop_after_two_requests() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let artifact_requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fixture = RetryArtifactFixture {
        catalog: CatalogHttpFixture {
            documents: Arc::new(documents),
            requests: Arc::new(Mutex::new(Vec::new())),
        },
        artifact_requests: Arc::clone(&artifact_requests),
        failures_before_success: 2,
        success_body: Arc::new(b"i18n-artifact".to_vec()),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(retry_artifact_response)
                .with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let page = source.list_page("i18n", None).await.unwrap();

    assert!(source.download_artifact(&page.entries[0]).await.is_err());
    assert_eq!(artifact_requests.load(Ordering::SeqCst), 2);
    server.abort();
}

/// Root #1805 AC-009: a matching GitHub route must acquire a Host-owned provider lease and use
/// its derived client. The catalog origin is non-routable, so the request can only arrive at the
/// fake proxy after the real resolver selected the persisted route/pool/provider projection.
#[tokio::test]
async fn root_1805_github_consumer_routes_through_host_owned_fake_proxy() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let origin = "http://catalog-origin.invalid";
    let (documents, sources) = catalog_documents(origin);
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
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let (resolver, _package) = seed_github_egress_resolver(&state, &proxy_url).await;
    let source = ApiOfficialExtensionCatalogSource::new(sources).with_network_egress(resolver);

    let page = source
        .list_page_for_workspace(state.bootstrap_workspace_id, "runtime-extensions", None)
        .await
        .expect("the real GitHub consumer must reach the fake proxy through Network Center");

    assert_eq!(page.entries[0].id, "runtime-extensions:taichuy/openai");
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &[
            "/runtime-extensions/catalog/v1/index.json",
            "/runtime-extensions/catalog/v1/pages/1.json"
        ]
    );
    server.abort();
}

#[tokio::test]
async fn publisher_cutover_checksum_mismatch_does_not_retry_artifact_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let artifact_requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fixture = RetryArtifactFixture {
        catalog: CatalogHttpFixture {
            documents: Arc::new(documents),
            requests: Arc::new(Mutex::new(Vec::new())),
        },
        artifact_requests: Arc::clone(&artifact_requests),
        failures_before_success: 0,
        success_body: Arc::new(b"checksum-mismatch".to_vec()),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(retry_artifact_response)
                .with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let page = source.list_page("i18n", None).await.unwrap();

    let error = source
        .download_artifact(&page.entries[0])
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<OfficialExtensionArtifactError>(),
        Some(OfficialExtensionArtifactError::ChecksumMismatch)
    ));
    assert_eq!(artifact_requests.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn missing_release_asset_preserves_404_host_evidence_without_retrying() {
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
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let page = source.list_page("i18n", None).await.unwrap();
    let mut missing = page.entries[0].clone();
    missing.download_locator = json!({
        "kind": "https",
        "locator": format!("{base_url}/missing-release.json")
    });

    let error = source.download_artifact(&missing).await.unwrap_err();
    match error.downcast_ref::<OfficialExtensionArtifactError>() {
        Some(OfficialExtensionArtifactError::NotFound { host, status }) => {
            assert_eq!(host, "127.0.0.1");
            assert_eq!(*status, 404);
        }
        other => panic!("expected typed 404 artifact error, got {other:?}"),
    }
    assert_eq!(
        requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/missing-release.json")
            .count(),
        1
    );
    server.abort();
}

#[tokio::test]
async fn api_01_search_filters_before_pagination_binds_cursor_and_reuses_verified_pages() {
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
    let source = ApiOfficialExtensionCatalogSource::new(sources);

    let first = source
        .search(
            "runtime-extensions",
            OfficialExtensionCatalogSearchQuery {
                slot_code: Some("model_provider".to_string()),
                q: None,
                limit: 1,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let cursor = first.next_cursor.unwrap();

    requests.lock().unwrap().clear();
    let cached = source
        .cached_verified_entries("runtime-extensions", Some("model_provider"))
        .expect("verified search snapshot should be readable without remote I/O");
    assert_eq!(cached.entries.len(), 1);
    assert_eq!(cached.entries[0].artifact, "openai");
    assert!(requests.lock().unwrap().is_empty());
    let runtime_source = ApiOfficialRuntimeExtensionSource::new(
        Arc::new(source.clone()),
        "allow_unsigned".to_string(),
        Vec::new(),
    );
    let runtime_snapshot = runtime_source
        .cached_official_catalog()
        .await
        .expect("runtime projection should reuse the verified generic snapshot");
    assert_eq!(runtime_snapshot.entries.len(), 1);
    assert_eq!(runtime_snapshot.entries[0].provider_code, "openai");
    assert!(requests.lock().unwrap().is_empty());

    let cached_snapshot_entry = source
        .find_entry(
            "runtime-extensions",
            "runtime-extensions:other/later-runtime",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cached_snapshot_entry.entry.catalog_page, 2);
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &["/runtime-extensions/catalog/v1/pages/2.json"]
    );

    requests.lock().unwrap().clear();
    let filtered = source
        .search(
            "runtime-extensions",
            OfficialExtensionCatalogSearchQuery {
                slot_code: Some("model_provider".to_string()),
                q: Some("later-runtime".to_string()),
                limit: 1,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.total_entries, 1);
    assert_eq!(filtered.entries[0].artifact, "later-runtime");
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &[
            "/runtime-extensions/catalog/v1/index.json",
            "/runtime-extensions/catalog/v1/search-index.json"
        ],
        "search refreshes publication metadata while reusing verified page bytes"
    );

    let stale = source
        .search(
            "runtime-extensions",
            OfficialExtensionCatalogSearchQuery {
                slot_code: Some("model_provider".to_string()),
                q: Some("later-runtime".to_string()),
                limit: 1,
                cursor: Some(cursor),
            },
        )
        .await;
    assert!(stale
        .unwrap_err()
        .to_string()
        .contains("snapshot and query"));

    server.abort();
}

#[tokio::test]
async fn ac_001_002_search_refreshes_an_updated_snapshot_and_uses_the_last_complete_snapshot_during_cutover(
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (documents, sources) = catalog_documents(&base_url);
    let documents = Arc::new(Mutex::new(documents));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let fixture = MutableCatalogHttpFixture {
        documents: Arc::clone(&documents),
        requests: Arc::clone(&requests),
    };
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .fallback(mutable_catalog_response)
                .with_state(fixture),
        )
        .await
        .unwrap();
    });
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let query = OfficialExtensionCatalogSearchQuery {
        slot_code: Some("model_provider".to_string()),
        q: Some("openai".to_string()),
        limit: 20,
        cursor: None,
    };

    let initial = source
        .search("runtime-extensions", query.clone())
        .await
        .unwrap();
    assert_eq!(initial.entries[0].version, "2.4.1");

    let page_path = "/runtime-extensions/catalog/v1/pages/1.json";
    let page_two_path = "/runtime-extensions/catalog/v1/pages/2.json";
    let mut next_page =
        serde_json::from_slice::<Value>(&documents.lock().unwrap()[page_path]).unwrap();
    next_page["entries"][0]["version"] = json!("2.4.2");
    let next_page = serde_json::to_vec(&next_page).unwrap();
    let page_two = documents.lock().unwrap()[page_two_path].clone();
    let next_pages = vec![(1, "start", next_page.clone()), (2, "runtime-2", page_two)];
    let next_search = catalog_search_index(&base_url, "runtime-extensions", &next_pages);
    let next_index = catalog_index(&base_url, "runtime-extensions", &next_pages, &next_search);
    {
        let mut live = documents.lock().unwrap();
        live.insert(
            "/runtime-extensions/catalog/v1/index.json".to_string(),
            next_index,
        );
        live.insert(
            "/runtime-extensions/catalog/v1/search-index.json".to_string(),
            next_search,
        );
    }
    requests.lock().unwrap().clear();

    let during_cutover = source
        .search("runtime-extensions", query.clone())
        .await
        .expect("an inconsistent candidate must not replace the last complete snapshot");
    assert_eq!(during_cutover.entries[0].version, "2.4.1");
    assert_eq!(
        during_cutover.freshness,
        crate::official_extension_catalog::OfficialExtensionCatalogFreshness::Stale
    );
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &[
            "/runtime-extensions/catalog/v1/index.json",
            "/runtime-extensions/catalog/v1/search-index.json",
            "/runtime-extensions/catalog/v1/pages/1.json"
        ],
        "a cached snapshot must still probe and validate the current publication"
    );

    documents
        .lock()
        .unwrap()
        .insert(page_path.to_string(), next_page);
    requests.lock().unwrap().clear();
    let refreshed = source
        .search("runtime-extensions", query)
        .await
        .expect("the coherent candidate snapshot must replace the old cache");
    assert_eq!(refreshed.entries[0].version, "2.4.2");
    assert_eq!(
        refreshed.freshness,
        crate::official_extension_catalog::OfficialExtensionCatalogFreshness::Fresh
    );
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &[
            "/runtime-extensions/catalog/v1/index.json",
            "/runtime-extensions/catalog/v1/search-index.json",
            "/runtime-extensions/catalog/v1/pages/1.json"
        ]
    );
    server.abort();
}

#[tokio::test]
async fn root_1545_ac_2_v1_source_reads_six_category_pages_and_later_page_detail() {
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
    let source = ApiOfficialExtensionCatalogSource::new(sources);

    for category in CATEGORIES {
        let page = source.list_page(category, None).await.unwrap();
        assert_eq!(page.category, category);
        assert_eq!(page.metadata.cursor, "start");
        assert!(page.metadata.checksum.starts_with("sha256:"));
        if category == "host-extensions" {
            assert!(page.entries.is_empty(), "empty categories stay explicit");
        } else {
            assert_eq!(page.entries[0].category, category);
            assert_eq!(page.entries[0].download_locator["kind"], "repository_file");
        }
    }

    requests.lock().unwrap().clear();
    let runtime_page = source.list_page("runtime-extensions", None).await.unwrap();
    assert_eq!(
        runtime_page.entries[0].id,
        "runtime-extensions:taichuy/openai"
    );
    assert_eq!(
        runtime_page.metadata.next_cursor.as_deref(),
        Some("runtime-2")
    );
    let requested = requests.lock().unwrap().clone();
    assert_eq!(requested.len(), 2, "list fetches one index and one page");
    assert!(requested
        .iter()
        .all(|path| !path.contains("official-registry")));
    assert!(requested
        .iter()
        .all(|path| !path.ends_with("/pages/2.json")));

    requests.lock().unwrap().clear();
    let located = source
        .find_entry(
            "runtime-extensions",
            "runtime-extensions:other/later-runtime",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(located.entry.catalog_page, 2);
    assert_eq!(located.entry.artifact, "later-runtime");
    assert_eq!(located.entry.source.kind, "runtime_extension_manifest");
    assert_eq!(
        located.entry.checksum.as_deref(),
        Some(&artifact_checksum("runtime-extensions", "later-runtime")[..])
    );
    let requested = requests.lock().unwrap().clone();
    assert_eq!(requested.len(), 3, "detail walks only until the match");
    assert!(requested.iter().any(|path| path.ends_with("/pages/2.json")));

    let i18n = source.list_page("i18n", None).await.unwrap();
    assert_eq!(i18n.entries[0].id, "i18n:taichuy/i18n-fixture");
    requests.lock().unwrap().clear();
    let downloaded = source.download_artifact(&i18n.entries[0]).await.unwrap();
    assert_eq!(downloaded.artifact_bytes, b"i18n-artifact");
    assert_eq!(
        requests.lock().unwrap().as_slice(),
        &["/i18n/artifacts/i18n-fixture.bin"]
    );

    server.abort();
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
    index["pages"][1]["checksum"] = json!(format!("sha256:{:x}", Sha256::digest(&page_bytes)));
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
    let source = ApiOfficialExtensionCatalogSource::new_with_request_timeout(
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
    );
    let started = Instant::now();
    let error = source.list_page("mcp", None).await.unwrap_err();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert!(error
        .to_string()
        .contains("failed to request official extension catalog document"));
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
                "checksum": format!("sha256:{:x}", Sha256::digest(bytes)),
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
            "checksum": format!("sha256:{:x}", Sha256::digest(search)),
            "locator": format!("{base_url}/{category}/catalog/v1/search-index.json")
        }
    }))
    .unwrap()
}

fn catalog_search_index(base_url: &str, category: &str, pages: &[(u32, &str, Vec<u8>)]) -> Vec<u8> {
    let mut entries = Vec::new();
    for (page, cursor, bytes) in pages {
        let page_document = serde_json::from_slice::<Value>(bytes).unwrap();
        let page_checksum = format!("sha256:{:x}", Sha256::digest(bytes));
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
        Sha256::digest(artifact_bytes(category, artifact))
    )
}
