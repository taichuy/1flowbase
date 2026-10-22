use super::*;
use crate::official_extension_catalog::rewrite_github_raw_url;

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
    let proxy_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", proxy_listener.local_addr().unwrap());
    let origin = "http://catalog-origin.invalid";
    let (documents, sources) = catalog_documents(origin);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let fixture = CatalogHttpFixture {
        documents: Arc::new(documents),
        requests: Arc::clone(&requests),
    };
    let server = tokio::spawn(async move {
        let (verification_stream, _) = proxy_listener
            .accept()
            .await
            .expect("fake proxy must accept lease verification");
        drop(verification_stream);
        axum::serve(
            proxy_listener,
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

#[test]
fn rewrite_github_raw_url_adds_proxy_prefix_for_raw_githubusercontent() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg",
            Some("https://gh-proxy.com/")
        ),
        "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg"
    );
}

#[test]
fn rewrite_github_raw_url_normalizes_proxy_prefix_without_trailing_slash() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg",
            Some("https://gh-proxy.com")
        ),
        "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg"
    );
}

#[test]
fn rewrite_github_raw_url_keeps_non_raw_urls_unchanged() {
    assert_eq!(
        rewrite_github_raw_url(
            "https://example.com/openai-compatible.1flowbasepkg",
            Some("https://gh-proxy.com/")
        ),
        "https://example.com/openai-compatible.1flowbasepkg"
    );
}

#[test]
fn rewrite_github_raw_url_does_not_duplicate_existing_proxy_prefix() {
    let proxied = "https://gh-proxy.com/https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/file.svg";

    assert_eq!(
        rewrite_github_raw_url(proxied, Some("https://gh-proxy.com/")),
        proxied
    );
}
