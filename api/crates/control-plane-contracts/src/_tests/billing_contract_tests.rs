use rust_decimal::Decimal;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{BillingRepository, PluginCreditCommandRequest, PricingRule, CREDIT_UNIT_USD};

fn pricing_rule() -> PricingRule {
    PricingRule {
        id: Uuid::now_v7(),
        provider_code: "openai".to_string(),
        upstream_model_id: "gpt-test".to_string(),
        input_token_unit_size: 1_000_000,
        input_token_unit_price: Decimal::new(125, 2),
        output_token_unit_size: 1_000_000,
        output_token_unit_price: Decimal::new(500, 2),
        cache_hit_token_unit_size: 1_000_000,
        cache_hit_token_unit_price: Decimal::new(25, 2),
        currency_code: CREDIT_UNIT_USD.to_string(),
        effective_from: OffsetDateTime::UNIX_EPOCH,
        effective_to: None,
        timezone: "UTC".to_string(),
        weekday_mask: 0b111_1111,
        local_time_start: None,
        local_time_end: None,
        priority: 0,
        enabled: true,
        rating_policy_enabled: false,
        rating_policy: serde_json::json!({}),
        source_kind: "manual".to_string(),
        source_catalog_id: None,
        source_version: None,
        source_checksum: None,
        extensions: serde_json::json!({}),
        created_by: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn pricing_rule_keeps_value_invariants_and_serde_identity() {
    billing_repository_remains_canonical::<dyn BillingRepository>();

    let rule = pricing_rule();
    rule.validate().expect("valid pricing rule");
    let encoded = serde_json::to_value(&rule).expect("pricing rule must serialize");
    let decoded: PricingRule =
        serde_json::from_value(encoded).expect("pricing rule must deserialize");
    assert_eq!(decoded, rule);

    let mut invalid_range = pricing_rule();
    invalid_range.effective_to = Some(invalid_range.effective_from);
    assert_eq!(
        invalid_range.validate().unwrap_err().to_string(),
        "pricing_effective_range_invalid"
    );

    let mut invalid_policy = pricing_rule();
    invalid_policy.rating_policy_enabled = true;
    invalid_policy.rating_policy = serde_json::json!({ "schema_version": "unknown" });
    assert_eq!(
        invalid_policy.validate().unwrap_err().to_string(),
        "rating_policy_invalid"
    );
}

#[test]
fn plugin_credit_request_keeps_usd_default() {
    let request: PluginCreditCommandRequest = serde_json::from_value(serde_json::json!({
        "command": "grant",
        "user_id": Uuid::nil(),
        "amount": "1.00",
        "reason": "fixture",
        "idempotency_key": "fixture-1"
    }))
    .expect("credit request must deserialize");

    assert_eq!(request.credit_unit, CREDIT_UNIT_USD);
}

fn billing_repository_remains_canonical<Repository>()
where
    Repository: BillingRepository + ?Sized,
{
}
