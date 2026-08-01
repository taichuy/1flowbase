use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    config::ResolvedOfficialExtensionCatalogSourceConfig,
    official_extension_catalog::{
        ApiOfficialExtensionCatalogSource, OfficialExtensionCatalogSourcePort,
    },
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
            "runtime-extensions:taichuy/later-runtime",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(located.entry.catalog_page, 2);
    assert_eq!(located.entry.artifact, "later-runtime");
    assert_eq!(located.entry.source.kind, "runtime_extension_manifest");
    assert_eq!(
        located.entry.checksum.as_deref(),
        Some(&artifact_checksum()[..])
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
                        "runtime-extensions:taichuy/later-runtime",
                        "later-runtime",
                    )],
                ),
            ));
        }
        let index = catalog_index(base_url, category, &pages);
        documents.insert(format!("/{category}/catalog/v1/index.json"), index);
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
                if category == "i18n" {
                    b"i18n-artifact".to_vec()
                } else {
                    format!("{category}-artifact").into_bytes()
                },
            );
        }
    }
    (documents, sources)
}

fn catalog_index(base_url: &str, category: &str, pages: &[(u32, &str, Vec<u8>)]) -> Vec<u8> {
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
        "pages": page_references
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
    json!({
        "id": id,
        "name": format!("{artifact} display name"),
        "category": category,
        "organization": "taichuy",
        "artifact": artifact,
        "version": "2.4.1",
        "description": "fixture extension",
        "host_version_requirement": ">=0.3.0",
        "source": {
            "kind": if category == "runtime-extensions" { "runtime_extension_manifest" } else { "fixture_source" },
            "locator": format!("{category}/@taichuy/{artifact}")
        },
        "signature": null,
        "checksum": artifact_checksum(),
        "download_locator": {
            "kind": "repository_file",
            "locator": format!("{base_url}/{category}/artifacts/{artifact}.bin")
        },
        "catalog_page": page
    })
}

fn artifact_checksum() -> String {
    format!("sha256:{}", "a".repeat(64))
}
