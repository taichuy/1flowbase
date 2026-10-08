use super::*;
use crate::official_extension_catalog::OfficialExtensionCatalogEntry;

fn collector_entry(base: &str) -> Value {
    let mut entry = catalog_entry(
        base,
        "runtime-extensions",
        1,
        "runtime-extensions:taichuy/codex-logs-collector",
        "codex-logs-collector",
    );
    entry["slot_codes"] = json!([]);
    entry["source"] = json!({"kind":"runtime_extension_manifest","locator":"runtime-extensions/@taichuy/codex-logs-collector/collector-manifest.json","distribution_kind":"client_collector","client_collector":{"collector_code":"codex-logs-collector","source_client":"codex","display_name":"Codex","execution_target":"client","protocol_version":"1flowbase.agent-logs/v1"}});
    entry["download_locator"]["kind"] = json!("release_asset");
    entry
}
#[test]
fn collector_catalog_is_typed_and_requires_host_independent_release_asset() {
    let base = "http://127.0.0.1:1";
    let (_, sources) = catalog_documents(base);
    let source = ApiOfficialExtensionCatalogSource::new(sources);
    let value = collector_entry(base);
    let entry: OfficialExtensionCatalogEntry = serde_json::from_value(value.clone()).unwrap();
    let descriptor = entry.source.client_collector().unwrap().unwrap();
    assert_eq!(descriptor.execution_target, "client");
    let artifact = source.resolve_artifact(&entry).unwrap();
    assert_eq!(artifact.locator_kind, "release_asset");
    assert!(artifact.platform.is_none());
    for (field, invalid) in [
        ("distribution_kind", json!("server_plugin")),
        ("client_collector", json!({"execution_target":"server"})),
    ] {
        let mut bad = value.clone();
        bad["source"][field] = invalid;
        let entry: OfficialExtensionCatalogEntry = serde_json::from_value(bad).unwrap();
        assert!(entry.source.client_collector().is_err());
    }
    let mut host_specific = value;
    host_specific["download_locator"]["kind"] = json!("platform_release_assets");
    let entry: OfficialExtensionCatalogEntry = serde_json::from_value(host_specific).unwrap();
    assert!(source.resolve_artifact(&entry).is_err());
}
#[tokio::test]
async fn runtime_catalog_excludes_client_distributions_without_requiring_plugin_metadata() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (mut documents, sources) = catalog_documents(&base);
    let mut first: Value =
        serde_json::from_slice(&documents["/runtime-extensions/catalog/v1/pages/1.json"]).unwrap();
    first["entries"]
        .as_array_mut()
        .unwrap()
        .push(collector_entry(&base));
    let first_bytes = serde_json::to_vec(&first).unwrap();
    let second = documents["/runtime-extensions/catalog/v1/pages/2.json"].clone();
    let pages = vec![(1, "start", first_bytes.clone()), (2, "runtime-2", second)];
    let search = catalog_search_index(&base, "runtime-extensions", &pages);
    documents.insert(
        "/runtime-extensions/catalog/v1/index.json".into(),
        catalog_index(&base, "runtime-extensions", &pages, &search),
    );
    documents.insert(
        "/runtime-extensions/catalog/v1/search-index.json".into(),
        search,
    );
    documents.insert(
        "/runtime-extensions/catalog/v1/pages/1.json".into(),
        first_bytes,
    );
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
    let runtime =
        ApiOfficialRuntimeExtensionSource::new(catalog, "signature_required".into(), Vec::new());
    let snapshot = runtime.list_official_catalog().await.unwrap();
    assert_eq!(snapshot.entries.len(), 2);
    assert!(snapshot
        .entries
        .iter()
        .all(|entry| !entry.plugin_id.contains("collector")));
    server.abort();
}
