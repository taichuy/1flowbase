use super::*;

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
async fn root_1545_ac_2_v1_source_reads_all_category_pages_and_later_page_detail() {
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
