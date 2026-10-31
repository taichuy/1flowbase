use super::*;
use crate::model_pricing_catalog::RemotePricingCatalogReader;
use axum::{extract::OriginalUri, response::IntoResponse};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

struct Fixture {
    url: String,
    hits: Arc<Mutex<Vec<String>>>,
    documents: Arc<Mutex<HashMap<String, Value>>>,
    task: tokio::task::JoinHandle<()>,
    ids: Vec<uuid::Uuid>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn checksum(value: &Value) -> String {
    format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(serde_json::to_vec(value).unwrap()))
    )
}

async fn fixture() -> Fixture {
    fixture_with_pages(3).await
}

async fn fixture_with_pages(page_count: usize) -> Fixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let mut documents = HashMap::new();
    let mut references = Vec::new();
    let mut entries = Vec::new();
    let mut ids = Vec::new();
    let template =
        serde_json::to_value(&crate::model_pricing_catalog::builtin_pricing_rules().unwrap()[0])
            .unwrap();
    for number in 1..=page_count {
        let mut rules = Vec::new();
        for position in 0..2 {
            let id = uuid::Uuid::now_v7();
            ids.push(id);
            let mut rule = template.clone();
            rule["id"] = json!(id);
            rule["source_catalog_id"] = json!(id);
            rule["source_version"] = json!("paged-fixture");
            rule["provider_code"] = json!(if number == 2 {
                "provider-b"
            } else {
                "provider-a"
            });
            rule["upstream_model_id"] = json!(format!("Model-{number}-{position}"));
            rules.push(rule);
        }
        let page = json!({"schema_version":"1flowbase.model-pricing-page/v2", "catalog_version":"paged-fixture", "currency_code":"USD", "page":number, "rules":rules});
        let reference = json!({"page":number,"rule_count":2,"checksum":checksum(&page),"locator":format!("{base}pages/{number}.json")});
        for rule in &rules {
            entries.push(json!({"id":rule["id"],"provider_code":rule["provider_code"],"upstream_model_id":rule["upstream_model_id"].as_str().unwrap().to_lowercase(),"catalog_page":reference}));
        }
        references.push(reference);
        documents.insert(format!("/pages/{number}.json"), page);
    }
    let search = json!({"schema_version":"1flowbase.model-pricing-search/v2", "catalog_version":"paged-fixture","entries":entries});
    let index = json!({"schema_version":"1flowbase.model-pricing-index/v2", "catalog_version":"paged-fixture", "currency_code":"USD", "total_rules":ids.len(),"pages":references,"search_index":{"schema_version":"1flowbase.model-pricing-search/v2","entry_count":ids.len(),"checksum":checksum(&search),"locator":format!("{base}search-index.json")}});
    documents.insert("/index.json".into(), index);
    documents.insert("/search-index.json".into(), search);
    let documents = Arc::new(Mutex::new(documents));
    let hits = Arc::new(Mutex::new(Vec::new()));
    let app = Router::new().fallback({
        let documents = documents.clone();
        let hits = hits.clone();
        move |OriginalUri(uri): OriginalUri| {
            let path = uri.path().to_owned();
            hits.lock().unwrap().push(path.clone());
            let document = documents.lock().unwrap().get(&path).cloned();
            async move {
                match document {
                    Some(value) => Json(value).into_response(),
                    None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                }
            }
        }
    });
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Fixture {
        url: format!("{base}index.json"),
        hits,
        documents,
        task,
        ids,
    }
}

#[tokio::test]
async fn pricing_pagination_reads_only_intersecting_pages_and_handles_overflow() {
    let f = fixture().await;
    // An unrelated remote page is unavailable: browsing page one must still work.
    f.documents.lock().unwrap().remove("/pages/3.json");
    let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
        .await
        .unwrap();
    let (first, total) = reader.query(None, None, 1, 1).await.unwrap();
    assert_eq!(total, 6);
    assert_eq!(first[0].id, Some(f.ids[0]));
    assert_eq!(*f.hits.lock().unwrap(), ["/index.json", "/pages/1.json"]);
    f.hits.lock().unwrap().clear();
    let (second, _) = reader.query(None, None, 2, 2).await.unwrap();
    assert_eq!(
        second.iter().map(|r| r.id.unwrap()).collect::<Vec<_>>(),
        f.ids[2..4]
    );
    assert_eq!(*f.hits.lock().unwrap(), ["/pages/2.json"]);
    f.hits.lock().unwrap().clear();
    assert!(reader
        .query(None, None, usize::MAX, 100)
        .await
        .unwrap()
        .0
        .is_empty());
    assert!(f.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn pricing_search_counts_matches_before_paging_and_only_reads_matching_pages() {
    let f = fixture().await;
    f.documents.lock().unwrap().remove("/pages/2.json");
    let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
        .await
        .unwrap();
    let (rules, total) = reader
        .query(Some(" PROVIDER-A "), Some("MODEL"), 2, 2)
        .await
        .unwrap();
    assert_eq!(total, 4);
    assert_eq!(
        rules.iter().map(|r| r.id.unwrap()).collect::<Vec<_>>(),
        f.ids[4..6]
    );
    assert_eq!(
        *f.hits.lock().unwrap(),
        ["/index.json", "/search-index.json", "/pages/3.json"]
    );
    f.hits.lock().unwrap().clear();
    let (empty, total) = reader
        .query(Some("provider-b"), Some("no-match"), 1, 20)
        .await
        .unwrap();
    assert!(empty.is_empty());
    assert_eq!(total, 0);
    assert_eq!(*f.hits.lock().unwrap(), ["/search-index.json"]);
}

#[tokio::test]
async fn pricing_import_deduplicates_pages_and_rejects_missing_or_duplicate_ids() {
    let f = fixture().await;
    let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
        .await
        .unwrap();
    let selected = reader
        .select_ids(&[f.ids[1], f.ids[0], f.ids[5]])
        .await
        .unwrap();
    assert_eq!(
        selected.iter().map(|r| r.id.unwrap()).collect::<Vec<_>>(),
        vec![f.ids[0], f.ids[1], f.ids[5]]
    );
    assert_eq!(
        *f.hits.lock().unwrap(),
        [
            "/index.json",
            "/search-index.json",
            "/pages/1.json",
            "/pages/3.json"
        ]
    );
    f.hits.lock().unwrap().clear();
    assert!(reader.select_ids(&[uuid::Uuid::now_v7()]).await.is_err());
    assert_eq!(*f.hits.lock().unwrap(), ["/search-index.json"]);
    f.hits.lock().unwrap().clear();
    assert!(reader.select_ids(&[f.ids[0], f.ids[0]]).await.is_err());
    assert!(f.hits.lock().unwrap().is_empty());
}

#[tokio::test]
async fn pricing_search_rejects_inconsistent_snapshot_and_page_references() {
    for scenario in [
        "checksum",
        "version",
        "locator",
        "missing-id",
        "duplicate-id",
        "count",
    ] {
        let f = fixture().await;
        {
            let mut docs = f.documents.lock().unwrap();
            let search = docs.get_mut("/search-index.json").unwrap();
            match scenario {
                "version" => search["catalog_version"] = json!("other-version"),
                "locator" => {
                    search["entries"][0]["catalog_page"]["locator"] =
                        json!("https://other.example/pages/1.json")
                }
                "missing-id" => search["entries"][0]["id"] = json!(uuid::Uuid::now_v7()),
                "duplicate-id" => search["entries"][1]["id"] = json!(f.ids[0]),
                "count" => {
                    search["entries"].as_array_mut().unwrap().pop();
                }
                _ => {}
            }
            let hash = checksum(search);
            docs.get_mut("/index.json").unwrap()["search_index"]["checksum"] =
                json!(if scenario == "checksum" {
                    "sha256:wrong".to_owned()
                } else {
                    hash
                });
        }
        let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
            .await
            .unwrap();
        assert!(
            reader.query(Some("provider-a"), None, 1, 1).await.is_err(),
            "scenario {scenario}"
        );
    }
}

#[tokio::test]
async fn pricing_sync_later_page_failure_installs_no_earlier_page() {
    // One complete four-page batch stages before the next batch fails remotely.
    let f = fixture_with_pages(5).await;
    f.documents.lock().unwrap().remove("/pages/5.json");
    let app = test_app_with_model_pricing_catalog_url(f.url.clone()).await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/billing/pricing-catalog/sync")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!response.status().is_success());
    {
        let hits = f.hits.lock().unwrap();
        assert_eq!(hits.first().unwrap(), "/index.json");
        assert_eq!(hits.len(), 6);
        for number in 1..=5 {
            assert!(hits
                .iter()
                .any(|path| path == &format!("/pages/{number}.json")));
        }
        assert_eq!(hits.last().unwrap(), "/pages/5.json");
    }
    let listed = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/billing/pricing-rules?provider_code=provider-a")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(response_json(listed).await["data"]["total_count"], 0);
}

#[tokio::test]
async fn pricing_search_rejects_identities_swapped_between_two_selected_pages() {
    let f = fixture().await;
    {
        let mut docs = f.documents.lock().unwrap();
        let search = docs.get_mut("/search-index.json").unwrap();
        for field in ["id", "upstream_model_id"] {
            let first = search["entries"][0][field].clone();
            search["entries"][0][field] = search["entries"][4][field].clone();
            search["entries"][4][field] = first;
        }
        let hash = checksum(search);
        docs.get_mut("/index.json").unwrap()["search_index"]["checksum"] = json!(hash);
    }
    let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
        .await
        .unwrap();
    assert!(reader.query(Some("provider-a"), None, 1, 4).await.is_err());
}

#[tokio::test]
async fn pricing_metadata_returns_counts_even_when_all_rule_pages_are_unavailable() {
    let f = fixture().await;
    f.documents
        .lock()
        .unwrap()
        .retain(|path, _| !path.starts_with("/pages/"));
    let app = test_app_with_model_pricing_catalog_url(f.url.clone()).await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    for (query, expected, hits) in [
        ("metadata_only=true", 6, vec!["/index.json"]),
        (
            "metadata_only=true&provider_code=PROVIDER-A&upstream_model_id=model",
            4,
            vec!["/index.json", "/search-index.json"],
        ),
    ] {
        f.hits.lock().unwrap().clear();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/settings/billing/pricing-catalog?{query}"
                    ))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let data = response_json(response).await;
        assert_eq!(data["data"]["total_count"], expected);
        assert_eq!(data["data"]["items"], json!([]));
        assert_eq!(*f.hits.lock().unwrap(), hits);
    }
}

#[tokio::test]
async fn pricing_reader_retries_a_transient_unavailable_index_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/index.json", listener.local_addr().unwrap());
    let app = Router::new().fallback({
        let hits = hits.clone();
        move || {
            let attempt = hits.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt == 0 {
                    StatusCode::SERVICE_UNAVAILABLE.into_response()
                } else {
                    Json(json!({"schema_version":"1flowbase.model-pricing-index/v2",
                        "catalog_version":"retry-fixture","currency_code":"USD",
                        "total_rules":0,"pages":[]}))
                    .into_response()
                }
            }
        }
    });
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let result = RemotePricingCatalogReader::open(&reqwest::Client::new(), &url).await;
    task.abort();
    assert_eq!(result.unwrap().count(None, None).await.unwrap(), 0);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn pricing_reader_retries_an_interrupted_body_and_bounds_persistent_failures() {
    use futures_util::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    for recover in [true, false] {
        let hits = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/index.json", listener.local_addr().unwrap());
        let app = Router::new().fallback({
            let hits = hits.clone();
            move || {
                let attempt = hits.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 || !recover {
                        let chunks = futures_util::stream::iter([
                            Ok(axum::body::Bytes::from_static(b"{")),
                            Err(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "interrupted fixture",
                            )),
                        ])
                        .then(|chunk| async move {
                            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                            chunk
                        });
                        axum::response::Response::new(Body::from_stream(chunks))
                    } else {
                        Json(json!({"schema_version":"1flowbase.model-pricing-index/v2",
                            "catalog_version":"retry-fixture","currency_code":"USD",
                            "total_rules":0,"pages":[]}))
                        .into_response()
                    }
                }
            }
        });
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let result = RemotePricingCatalogReader::open(&reqwest::Client::new(), &url).await;
        task.abort();
        assert_eq!(result.is_ok(), recover);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test]
async fn pricing_sharded_search_skips_unrelated_metadata_and_keeps_exact_counts() {
    let f = fixture().await;
    {
        let mut docs = f.documents.lock().unwrap();
        let search = docs.remove("/search-index.json").unwrap();
        let mut shards = Vec::new();
        for (number, (start, end, pages, providers)) in [
            (0, 4, vec![1, 2], vec!["provider-a", "provider-b"]),
            (4, 6, vec![3], vec!["provider-a"]),
        ]
        .into_iter()
        .enumerate()
        {
            let mut document = search.clone();
            document["entries"] = json!(&search["entries"].as_array().unwrap()[start..end]);
            let name = format!("/search/{}.json", number + 1);
            shards.push(json!({"schema_version":"1flowbase.model-pricing-search/v2",
                "entry_count": end - start, "checksum":checksum(&document),
                "locator":format!("{}search/{}.json", f.url.strip_suffix("index.json").unwrap(), number + 1),
                "pages":pages,"provider_codes":providers}));
            docs.insert(name, document);
        }
        docs.get_mut("/index.json").unwrap()["search_shards"] = json!(shards);
        // The other provider's metadata endpoint is unavailable.
        docs.remove("/search/2.json");
    }
    let reader = RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
        .await
        .unwrap();
    assert_eq!(
        reader
            .count(Some(" PROVIDER-B "), Some("MODEL"))
            .await
            .unwrap(),
        2
    );
    assert_eq!(*f.hits.lock().unwrap(), ["/index.json", "/search/1.json"]);
    f.hits.lock().unwrap().clear();
    let (rules, count) = reader
        .query(Some("provider-b"), Some("MODEL"), 2, 1)
        .await
        .unwrap();
    assert_eq!(count, 2);
    assert_eq!(rules[0].id, Some(f.ids[3]));
    assert_eq!(*f.hits.lock().unwrap(), ["/search/1.json", "/pages/2.json"]);
    // A broad query cannot claim a complete count when a required shard is missing.
    assert!(reader.count(None, Some("Model")).await.is_err());
    // Malformed partition ownership fails before requesting metadata.
    f.documents.lock().unwrap().get_mut("/index.json").unwrap()["search_shards"][1]["pages"] =
        json!([2]);
    assert!(
        RemotePricingCatalogReader::open(&reqwest::Client::new(), &f.url)
            .await
            .is_err()
    );
}
