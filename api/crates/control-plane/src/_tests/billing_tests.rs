use crate::billing::{rate_token_usage, PricingRule, TokenUsage};
use rust_decimal::Decimal;
use std::str::FromStr;
use time::{macros::datetime, Weekday};

fn rule() -> PricingRule {
    PricingRule {
        id: uuid::Uuid::now_v7(),
        provider_code: "openai".to_string(),
        upstream_model_id: "gpt-test".to_string(),
        input_token_unit_size: 1_000_000,
        input_token_unit_price: Decimal::from_str("1.25").unwrap(),
        output_token_unit_size: 1_000_000,
        output_token_unit_price: Decimal::from_str("5.00").unwrap(),
        cache_hit_token_unit_size: 1_000_000,
        cache_hit_token_unit_price: Decimal::from_str("0.25").unwrap(),
        currency_code: "USD".to_string(),
        effective_from: datetime!(2026-01-01 00:00 UTC),
        effective_to: None,
        timezone: "UTC".to_string(),
        weekday_mask: 0b111_1111,
        local_time_start: None,
        local_time_end: None,
        priority: 0,
        enabled: true,
        source_kind: "manual".to_string(),
        source_catalog_id: None,
        source_version: None,
        source_checksum: None,
        extensions: serde_json::json!({}),
        created_by: None,
        created_at: datetime!(2026-01-01 00:00 UTC),
        updated_at: datetime!(2026-01-01 00:00 UTC),
    }
}

#[test]
fn token_rating_uses_mutually_exclusive_input_and_cache_quantities() {
    let cost = rate_token_usage(
        &rule(),
        &TokenUsage {
            input_tokens: 300_000,
            input_cache_hit_tokens: 200_000,
            input_cache_miss_tokens: None,
            output_tokens: 50_000,
        },
    )
    .unwrap();

    assert_eq!(cost.ordinary_input_tokens, 100_000);
    assert_eq!(cost.cache_hit_tokens, 200_000);
    assert_eq!(cost.input_cost.to_string(), "0.125");
    assert_eq!(cost.output_cost.to_string(), "0.25");
    assert_eq!(cost.cache_hit_cost.to_string(), "0.05");
    assert_eq!(cost.total_cost.to_string(), "0.425");
}

#[test]
fn explicit_cache_miss_quantity_wins_over_derived_input_quantity() {
    let cost = rate_token_usage(
        &rule(),
        &TokenUsage {
            input_tokens: 999_999,
            input_cache_hit_tokens: 200_000,
            input_cache_miss_tokens: Some(10),
            output_tokens: 0,
        },
    )
    .unwrap();
    assert_eq!(cost.ordinary_input_tokens, 10);
}

#[test]
fn pricing_rule_rejects_invalid_core_rates_and_currency() {
    let mut invalid = rule();
    invalid.currency_code = "CNY".to_string();
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "billing_currency_not_supported"
    );
    invalid.currency_code = "USD".to_string();
    invalid.input_token_unit_size = 0;
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "pricing_unit_size_invalid"
    );
}

#[test]
fn weekday_mask_uses_monday_as_low_bit() {
    assert_eq!(crate::billing::weekday_bit(Weekday::Monday), 1);
    assert_eq!(crate::billing::weekday_bit(Weekday::Sunday), 64);
}

#[test]
fn pricing_cache_key_is_unambiguous_and_stable() {
    assert_eq!(
        crate::billing::pricing_rules_cache_key("open:ai", "gpt"),
        crate::billing::pricing_rules_cache_key("open:ai", "gpt")
    );
    assert_ne!(
        crate::billing::pricing_rules_cache_key("open:ai", "gpt"),
        crate::billing::pricing_rules_cache_key("open", "ai:gpt")
    );
}
