use std::{collections::HashMap, sync::Arc};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use control_plane::ports::UiComponentCatalogSource;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

use crate::ui_component_catalog_source::{canonical_json_bytes, ApiUiComponentCatalogSource};

fn checksum(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn component(code: &str) -> Value {
    json!({
        "schema_version": "1flowbase.ui-component-source/v1",
        "component_code": code,
        "name": "Opaque widget",
        "description": "Fixture record",
        "import_code": "import Widget from '@package/that-does-not-exist';",
        "source_code": "<Widget impossible={{ syntax: true }} />",
        "origin": "official",
        "source": "taichuy",
        "group": "missing-package",
        "upstream": { "identity": "@package/that-does-not-exist", "version": "99.0.0" },
        "version": "1.0.0",
        "keywords": ["opaque"],
        "updated_at": "2026-08-23T00:00:00.000Z",
        "source_locator": format!("ui_components/@taichuy/missing-package/{code}.json"),
        "source_checksum": ""
    })
}

fn published_component(code: &str) -> Value {
    let mut value = component(code);
    let source = value.as_object_mut().unwrap();
    source.remove("source_locator");
    source.remove("source_checksum");
    let source_checksum = checksum(&canonical_json_bytes(&value).unwrap());
    let mut published = value.as_object().unwrap().clone();
    published.insert(
        "source_locator".into(),
        Value::String(format!(
            "ui_components/@taichuy/missing-package/{code}.json"
        )),
    );
    published.insert("source_checksum".into(), Value::String(source_checksum));
    Value::Object(published)
}

async fn spawn_catalog(tamper_second_page: bool) -> (String, tokio::task::JoinHandle<()>) {
    let page_one = json!({
        "schema_version": "1flowbase.ui-component-catalog-page/v1",
        "catalog_version": "1.0.0",
        "page": 1,
        "cursor": "start",
        "next_cursor": "after-one",
        "next_page_locator": "PLACEHOLDER/pages/2.json",
        "components": [published_component("taichuy.missing-package.one")]
    });
    let page_two = json!({
        "schema_version": "1flowbase.ui-component-catalog-page/v1",
        "catalog_version": "1.0.0",
        "page": 2,
        "cursor": "after-one",
        "next_cursor": null,
        "next_page_locator": null,
        "components": [published_component("taichuy.missing-package.two")]
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let page_one = serde_json::from_slice::<Value>(
        &canonical_json_bytes(&page_one)
            .unwrap()
            .replace(b"PLACEHOLDER", base.as_bytes()),
    )
    .unwrap();
    let page_one_bytes = canonical_json_bytes(&page_one).unwrap();
    let page_two_bytes = canonical_json_bytes(&page_two).unwrap();
    let index = json!({
        "schema_version": "1flowbase.ui-component-catalog-index/v1",
        "catalog_version": "1.0.0",
        "generated_at": "2026-08-23T00:00:00.000Z",
        "page_size": 1,
        "total_components": 2,
        "source_fingerprint": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "first_page": { "page": 1, "cursor": "start", "locator": format!("{base}/pages/1.json") },
        "search_index": {
            "schema_version": "1flowbase.ui-component-catalog-search/v1",
            "entry_count": 2,
            "checksum": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "locator": format!("{base}/search.json")
        },
        "download": {
            "schema_version": "1flowbase.ui-component-catalog-seed/v1",
            "checksum": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "locator": format!("{base}/seed.json"),
            "release_tag": "ui-component-catalog-v1.0.0",
            "release_locator": format!("{base}/release.json"),
            "release_catalog_locator": format!("{base}/releases.json")
        },
        "update": {
            "strategy": "authoritative_source_group_replace",
            "identity_field": "component_code",
            "source_field": "source",
            "group_field": "group",
            "version_field": "version"
        },
        "pages": [
            { "page": 1, "cursor": "start", "component_count": 1, "checksum": checksum(&page_one_bytes), "locator": format!("{base}/pages/1.json") },
            { "page": 2, "cursor": "after-one", "component_count": 1, "checksum": checksum(&page_two_bytes), "locator": format!("{base}/pages/2.json") }
        ]
    });
    let served_page_two = if tamper_second_page {
        let mut value: Value = serde_json::from_slice(&page_two_bytes).unwrap();
        value["components"][0]["description"] = Value::String("Tampered".into());
        canonical_json_bytes(&value).unwrap()
    } else {
        page_two_bytes
    };
    let documents = Arc::new(HashMap::from([
        (
            "/index.json".to_string(),
            canonical_json_bytes(&index).unwrap(),
        ),
        ("/pages/1.json".to_string(), page_one_bytes),
        ("/pages/2.json".to_string(), served_page_two),
    ]));
    let app = Router::new()
        .fallback(get(
            |State(documents): State<Arc<HashMap<String, Vec<u8>>>>,
             request: axum::extract::Request| async move {
                documents
                    .get(request.uri().path())
                    .cloned()
                    .map(|bytes| (StatusCode::OK, bytes).into_response())
                    .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
            },
        ))
        .with_state(documents);
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("{base}/index.json"), handle)
}

trait ByteReplace {
    fn replace(self, from: &[u8], to: &[u8]) -> Vec<u8>;
}

impl ByteReplace for Vec<u8> {
    fn replace(self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let text = String::from_utf8(self).unwrap();
        text.replace(
            std::str::from_utf8(from).unwrap(),
            std::str::from_utf8(to).unwrap(),
        )
        .into_bytes()
    }
}

#[tokio::test]
async fn wp_d3_catalog_page_uses_index_pagination_and_validates_digest() {
    let (index_locator, server) = spawn_catalog(false).await;
    let source = ApiUiComponentCatalogSource::new(index_locator);

    let page = source.page(2).await.unwrap();

    assert_eq!(page.page, 2);
    assert_eq!(page.cursor, "after-one");
    assert_eq!(
        page.records[0].component_code,
        "taichuy.missing-package.two"
    );
    server.abort();
}

#[tokio::test]
async fn wp_d3_catalog_page_rejects_bytes_that_do_not_match_index_digest() {
    let (index_locator, server) = spawn_catalog(true).await;
    let source = ApiUiComponentCatalogSource::new(index_locator);

    let error = source.page(2).await.unwrap_err();

    assert!(error.to_string().contains("checksum"));
    server.abort();
}
