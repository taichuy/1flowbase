use crate::_tests::support::{
    create_member, login_and_capture_cookie, test_app_with_model_pricing_catalog_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn remote_model_pricing_fixture() -> (String, tokio::task::JoinHandle<()>) {
    let page = json!({
        "schema_version": "1flowbase.model-pricing-page/v1",
        "catalog_version": "2026-08-18.1",
        "currency_code": "USD",
        "page": 1,
        "rules": [{
            "id": "10000000-0000-4000-8000-000000000001",
            "provider_code": "zero",
            "upstream_model_id": "any",
            "input_token_unit_size": 1000000,
            "input_token_unit_price": "0",
            "output_token_unit_size": 1000000,
            "output_token_unit_price": "0",
            "cache_hit_token_unit_size": 1000000,
            "cache_hit_token_unit_price": "0",
            "currency_code": "USD",
            "effective_from": "2026-08-17T00:00:00Z",
            "effective_to": null,
            "timezone": "UTC",
            "weekday_mask": 127,
            "local_time_start": null,
            "local_time_end": null,
            "priority": 0,
            "enabled": true,
            "rating_policy_enabled": false,
            "rating_policy": {},
            "source_kind": "official",
            "source_catalog_id": "10000000-0000-4000-8000-000000000001",
            "source_version": "2026-08-18.1",
            "source_checksum": "sha256:fixture",
            "extensions": {}
        }]
    });
    let page_checksum = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&page).unwrap())
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let page_url = format!("{base_url}/model-pricing/catalog/v1/pages/1.json");
    let index = json!({
        "schema_version": "1flowbase.model-pricing-index/v1",
        "catalog_version": "2026-08-18.1",
        "currency_code": "USD",
        "total_rules": 1,
        "pages": [{
            "page": 1,
            "rule_count": 1,
            "checksum": page_checksum,
            "locator": page_url
        }]
    });
    let app = Router::new()
        .route(
            "/model-pricing/catalog/v1/index.json",
            get({
                let index = index.clone();
                move || {
                    let index = index.clone();
                    async move { Json(index) }
                }
            }),
        )
        .route(
            "/model-pricing/catalog/v1/pages/1.json",
            get(move || {
                let page = page.clone();
                async move { Json(page) }
            }),
        );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (
        format!("{base_url}/model-pricing/catalog/v1/index.json"),
        server,
    )
}

#[tokio::test]
async fn billing_routes_validate_pricing_and_manage_workspace_credit_ledger() {
    let (catalog_index_url, catalog_server) = remote_model_pricing_fixture().await;
    let app = test_app_with_model_pricing_catalog_url(catalog_index_url).await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &cookie, &csrf, "billing-route-user", "temp-pass").await;

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/billing/pricing-catalog?provider_code=zer&upstream_model_id=an&page=1&page_size=1")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_payload = response_json(catalog).await;
    assert_eq!(
        catalog_payload["data"]["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(catalog_payload["data"]["total_count"], 1);
    assert_eq!(catalog_payload["data"]["page"], 1);
    assert_eq!(catalog_payload["data"]["page_size"], 1);
    assert_eq!(catalog_payload["data"]["items"][0]["provider_code"], "zero");
    assert_eq!(
        catalog_payload["data"]["items"][0]["upstream_model_id"],
        "any"
    );

    let install_catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/billing/pricing-catalog/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "catalog_ids": ["10000000-0000-4000-8000-000000000001"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install_catalog.status(), StatusCode::OK);
    let install_payload = response_json(install_catalog).await;
    assert_eq!(install_payload["data"]["inserted"], 0);
    assert_eq!(install_payload["data"]["skipped"], 1);
    assert_eq!(install_payload["data"]["updated"], 0);
    assert_eq!(install_payload["data"]["deleted"], 0);

    let create_rule = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/billing/pricing-rules")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider_code":"fixture-provider",
                        "upstream_model_id":"fixture-model",
                        "input_token_unit_size":1000000,
                        "input_token_unit_price":"1.25",
                        "output_token_unit_size":1000000,
                        "output_token_unit_price":"5",
                        "cache_hit_token_unit_size":1000000,
                        "cache_hit_token_unit_price":"0.25",
                        "currency_code":"USD",
                        "effective_from":"2026-01-01T00:00:00Z",
                        "effective_to":null,
                        "timezone":"UTC",
                        "weekday_mask":127,
                        "local_time_start":null,
                        "local_time_end":null,
                        "priority":0,
                        "enabled":true,
                        "rating_policy_enabled":true,
                        "rating_policy":{
                            "schema_version":"1flowbase.model-rating-policy/v1",
                            "type":"input_token_tiers",
                            "tiers":[{
                                "when":{"operator":"gte","value":200000},
                                "rates":{
                                    "input":{"unit_size":1000000,"unit_price":"2.5"},
                                    "output":{"unit_size":1000000,"unit_price":"10"},
                                    "cache_hit":{"unit_size":1000000,"unit_price":"0.5"}
                                }
                            }]
                        },
                        "source_kind":"manual",
                        "extensions":{}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_rule.status(), StatusCode::OK);
    let rule_payload = response_json(create_rule).await;
    assert_eq!(rule_payload["data"]["currency_code"], "USD");
    assert_eq!(rule_payload["data"]["rating_policy_enabled"], true);
    assert_eq!(
        rule_payload["data"]["rating_policy"]["type"],
        "input_token_tiers"
    );
    assert_eq!(
        Decimal::from_str(
            rule_payload["data"]["input_token_unit_price"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        Decimal::from_str("1.25").unwrap()
    );

    // AC-005: pricing rules expose the same server-owned page contract used by
    // the shared settings data table instead of returning an uncounted array.
    let rules = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(
                    "/api/console/settings/billing/pricing-rules?provider_code=fixture-provider&page=1&page_size=20&enabled=true&source_kind=manual",
                )
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rules.status(), StatusCode::OK);
    let rules_payload = response_json(rules).await;
    assert_eq!(rules_payload["data"]["total_count"], 1);
    assert_eq!(rules_payload["data"]["page"], 1);
    assert_eq!(rules_payload["data"]["page_size"], 20);
    assert_eq!(
        rules_payload["data"]["items"][0]["upstream_model_id"],
        "fixture-model"
    );

    let accounts = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/billing/credit-accounts")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accounts.status(), StatusCode::OK);
    let accounts_payload = response_json(accounts).await;
    let member_account = accounts_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["user_id"] == member_id)
        .unwrap();
    assert_eq!(member_account["charge_enabled"], true);
    assert_eq!(member_account["available_balance"], "0.000000000000000000");

    let grant = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/billing/credits/{member_id}/grant"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "amount":"5.50",
                        "reason":"route_fixture",
                        "source_type":"test",
                        "source_id":"billing-route",
                        "idempotency_key":"billing-route:grant"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);
    let grant_payload = response_json(grant).await;
    assert_eq!(
        grant_payload["data"]["balance_after"],
        "5.500000000000000000"
    );

    let ledger = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/billing/credit-ledger?user_id={member_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ledger.status(), StatusCode::OK);
    let ledger_payload = response_json(ledger).await;
    assert_eq!(ledger_payload["data"][0]["transaction_type"], "grant");
    catalog_server.abort();
}

#[test]
fn model_pricing_builtin_source_contains_only_the_global_zero_fallback() {
    let rules = crate::model_pricing_catalog::builtin_pricing_rules().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].provider_code, "zero");
    assert_eq!(rules[0].upstream_model_id, "any");
}

#[test]
fn model_pricing_bootstrap_loader_reads_directory_sources_and_catalog_version() {
    let fixture_root = std::env::temp_dir().join(format!(
        "1flowbase-model-pricing-bootstrap-{}",
        uuid::Uuid::now_v7()
    ));
    let fixture_model = fixture_root.join("@zero/any");
    std::fs::create_dir_all(&fixture_model).unwrap();
    std::fs::write(
        fixture_root.join("catalog-source.json"),
        r#"{"catalog_version":"fixture-v1"}"#,
    )
    .unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources/model-pricing/@zero/any/pricing.json"),
        fixture_model.join("pricing.json"),
    )
    .unwrap();

    let rules = crate::model_pricing_catalog::load_bootstrap_pricing_rules(&fixture_root).unwrap();

    std::fs::remove_dir_all(&fixture_root).unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].provider_code, "zero");
    assert_eq!(rules[0].upstream_model_id, "any");
    assert_eq!(rules[0].source_version.as_deref(), Some("fixture-v1"));
}
