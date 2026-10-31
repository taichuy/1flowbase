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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let mut documents = HashMap::new();
    let mut references = Vec::new();
    let mut entries = Vec::new();
    let mut ids = Vec::new();
    let template =
        serde_json::to_value(&crate::model_pricing_catalog::builtin_pricing_rules().unwrap()[0])
            .unwrap();
    for number in 1..=3 {
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
    let index = json!({"schema_version":"1flowbase.model-pricing-index/v2", "catalog_version":"paged-fixture", "currency_code":"USD", "total_rules":6,"pages":references,"search_index":{"schema_version":"1flowbase.model-pricing-search/v2","entry_count":6,"checksum":checksum(&search),"locator":format!("{base}search-index.json")}});
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
    let f = fixture().await;
    f.documents.lock().unwrap().remove("/pages/2.json");
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
    assert_eq!(
        *f.hits.lock().unwrap(),
        ["/index.json", "/pages/1.json", "/pages/2.json"]
    );
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
