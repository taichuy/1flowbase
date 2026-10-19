//! Exercise the real shared HTTP source: a fallback-only application-template fixture misses
//! category/identity validation in both the search index and verified page decoder.
use super::*;

async fn source_fixture(
    invalid_identity: bool,
) -> (
    ApiOfficialExtensionCatalogSource,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let category = "applications-demo";
    let (mut documents, sources) = catalog_documents(&base_url);
    let entry = |page, artifact: &str| {
        let mut value = catalog_entry(
            &base_url,
            category,
            page,
            &format!("{category}:taichuy/{artifact}"),
            artifact,
        );
        value["version"] = json!("2");
        value["source"]["kind"] = json!("application_template_release");
        value["download_locator"] = json!({"kind":"release_asset","locator":format!("https://github.com/taichuy/1flowbase-official-plugins/releases/download/application-template-taichuy-{artifact}-v2/taichuy-{artifact}-v2.zip")});
        if invalid_identity {
            value["id"] = json!(format!("agent-flow:taichuy/{artifact}"));
        }
        value
    };
    let pages = vec![
        (
            1,
            "start",
            catalog_page(
                category,
                1,
                "start",
                Some("application-template-2"),
                vec![entry(1, "gateway-demo")],
            ),
        ),
        (
            2,
            "application-template-2",
            catalog_page(
                category,
                2,
                "application-template-2",
                None,
                vec![entry(2, "second-demo")],
            ),
        ),
    ];
    let search = catalog_search_index(&base_url, category, &pages);
    let index = catalog_index(&base_url, category, &pages, &search);
    documents.insert(format!("/{category}/catalog/v1/index.json"), index);
    documents.insert(format!("/{category}/catalog/v1/search-index.json"), search);
    for (page, _, bytes) in pages {
        documents.insert(format!("/{category}/catalog/v1/pages/{page}.json"), bytes);
    }
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
    (ApiOfficialExtensionCatalogSource::new(sources), server)
}

#[tokio::test]
async fn application_template_shared_source_validates_search_pages_and_identity() {
    let (source, server) = source_fixture(false).await;
    let result = source
        .search(
            "applications-demo",
            OfficialExtensionCatalogSearchQuery {
                slot_code: None,
                q: Some("demo".into()),
                limit: 1,
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.total_entries, 2);
    assert_eq!(result.entries.len(), 1);
    assert_eq!(
        result.entries[0].id,
        "applications-demo:taichuy/gateway-demo"
    );
    assert_eq!(
        result.entries[0].source.kind,
        "application_template_release"
    );
    let second = source
        .search(
            "applications-demo",
            OfficialExtensionCatalogSearchQuery {
                slot_code: None,
                q: Some("demo".into()),
                limit: 1,
                cursor: result.next_cursor,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.entries.len(), 1);
    assert_eq!(
        second.entries[0].id,
        "applications-demo:taichuy/second-demo"
    );
    assert!(second.next_cursor.is_none());
    let first_page = source.list_page("applications-demo", None).await.unwrap();
    assert_eq!(first_page.metadata.total_entries, 2);
    assert_eq!(
        first_page.metadata.next_cursor.as_deref(),
        Some("application-template-2")
    );
    let later_page = source
        .list_page(
            "applications-demo",
            first_page.metadata.next_cursor.as_deref(),
        )
        .await
        .unwrap();
    assert_eq!(
        later_page.entries[0].id,
        "applications-demo:taichuy/second-demo"
    );
    let located = source
        .find_entry("applications-demo", "applications-demo:taichuy/second-demo")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(located.entry.version, "2");
    let descriptor = source.resolve_artifact(&located.entry).unwrap();
    assert_eq!(descriptor.locator_kind, "release_asset");
    assert!(descriptor.locator.ends_with("taichuy-second-demo-v2.zip"));
    server.abort();
}

#[tokio::test]
async fn application_template_shared_source_rejects_cross_category_identity() {
    let (source, server) = source_fixture(true).await;
    assert!(source
        .search(
            "applications-demo",
            OfficialExtensionCatalogSearchQuery {
                slot_code: None,
                q: None,
                limit: 100,
                cursor: None
            }
        )
        .await
        .is_err());
    assert!(source.list_page("applications-demo", None).await.is_err());
    server.abort();
}
