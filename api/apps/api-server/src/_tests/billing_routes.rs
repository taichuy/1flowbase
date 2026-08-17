use crate::_tests::support::{create_member, login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::str::FromStr;
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn billing_routes_validate_pricing_and_manage_workspace_credit_ledger() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &cookie, &csrf, "billing-route-user", "temp-pass").await;

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
    assert_eq!(
        Decimal::from_str(
            rule_payload["data"]["input_token_unit_price"]
                .as_str()
                .unwrap()
        )
        .unwrap(),
        Decimal::from_str("1.25").unwrap()
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
}
