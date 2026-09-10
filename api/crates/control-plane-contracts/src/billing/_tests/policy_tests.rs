use super::*;
use serde_json::json;

// AC2: reject old documents and malformed conditions/overrides at admission.
#[test]
fn strict_rules_negative_matrix() {
    parse_rules(&json!([])).unwrap();
    for value in [
        json!({}),
        json!({"schema_version":"1flowbase.model-rating-policy/v2"}),
        json!([{"when":{},"overrides":{"input_token_unit_price":"1"}}]),
        json!([{"when":{"timezone":"UTC"},"overrides":{"input_token_unit_price":"1"}}]),
        json!([{"when":{"input_tokens":{"operator":"gt","value":0}},"overrides":{}}]),
        json!([{"when":{"input_tokens":{"operator":"ge","value":0}},"overrides":{"input_token_unit_price":"1"}}]),
        json!([{"when":{"cache_write_ttl_seconds":300},"overrides":{"input_token_unit_price":"1"}}]),
        json!([{"when":{"cache_write_ttl_seconds":0},"overrides":{"cache_write_token_unit_price":"1"}}]),
        json!([{"when":{"effective_from":"yesterday"},"overrides":{"input_token_unit_price":"1"}}]),
        json!([{"when":{"local_time_start":"23:00:00","local_time_end":"01:00:00"},"overrides":{"input_token_unit_price":"1"}}]),
    ] {
        assert!(parse_rules(&value).is_err(), "accepted {value}");
    }
    for (key, value) in [
        ("input_token_unit_price", json!("-1")),
        ("input_token_unit_price", json!("1e3")),
        ("input_token_unit_price", json!(1)),
        ("input_token_unit_price", json!(null)),
        ("input_token_unit_size", json!(0)),
        ("typo", json!("1")),
    ] {
        let mut rule =
            json!([{"when":{"input_tokens":{"operator":"gte","value":0}},"overrides":{}}]);
        rule[0]["overrides"][key] = value;
        assert!(parse_rules(&rule).is_err());
    }
}

#[test]
fn condition_utc_date_and_cross_midnight_boundaries() {
    let rules = parse_rules(&json!([{"when":{"effective_from":"2026-09-10T00:00:00Z","effective_to":"2026-09-12T00:00:00Z","timezone":"UTC","local_time_start":"23:00:00","local_time_end":"01:00:00"},"overrides":{"input_token_unit_price":"1"}}])).unwrap();
    for (at, expected) in [
        ("2026-09-09T23:30:00Z", false),
        ("2026-09-10T00:00:00Z", true),
        ("2026-09-10T01:00:00Z", false),
        ("2026-09-10T23:00:00Z", true),
        ("2026-09-12T00:00:00Z", false),
    ] {
        assert_eq!(
            rules[0].when.matches(0, None, date(at).unwrap()).unwrap(),
            expected
        );
    }
}
