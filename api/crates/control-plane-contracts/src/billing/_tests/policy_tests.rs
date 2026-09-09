use super::*;
use serde_json::json;

fn document() -> serde_json::Value {
    json!({"schema_version": RATING_POLICY_SCHEMA_V2, "type":"token_pricing", "unit_size":1000000,
        "rates":{"input":"10","output":"50","cache_hit":"0.25","cache_write":{"by_ttl_seconds":{"300":"12.5","3600":"20"}}}})
}

// AC1: admission rejects incomplete, ambiguous, and noncanonical policy documents.
#[test]
fn v2_strict_policy_negative_matrix() {
    TokenPricingPolicy::parse(&document()).unwrap();
    let mutations = [
        ("/extra", json!(true)),
        ("/unit_size", json!(0)),
        ("/unit_size", json!(1.5)),
        ("/unit_size", json!(9007199254740992i64)),
        ("/rates/input", json!(10)),
        ("/rates/input", json!("-1")),
        ("/rates/input", json!("1e3")),
        ("/rates/input", json!("1.0000000000000000001")),
        (
            "/rates/cache_write",
            json!({"unit_price":"1","by_ttl_seconds":{"300":"1"}}),
        ),
        ("/rates/cache_write", json!({})),
        ("/rates/cache_write", json!({"by_ttl_seconds":{"0300":"1"}})),
        (
            "/rates/cache_write",
            json!({"by_ttl_seconds":{"9007199254740992":"1"}}),
        ),
        ("/rates/cache_write", json!({"by_ttl_seconds":{}})),
        ("/input_token_tiers", json!([])),
        ("/input_token_tiers", json!(null)),
    ];
    for (path, replacement) in mutations {
        let mut value = document();
        if let Some(target) = value.pointer_mut(path) {
            *target = replacement;
        } else {
            value[path.trim_start_matches('/')] = replacement;
        }
        assert!(
            TokenPricingPolicy::parse(&value).is_err(),
            "accepted {value}"
        );
    }
    let mut value = document();
    value["rates"].as_object_mut().unwrap().remove("output");
    assert!(TokenPricingPolicy::parse(&value).is_err());
}

#[test]
fn v2_tiers_require_complete_rates_and_strict_threshold_order() {
    let mut value = document();
    let rates = value["rates"].clone();
    value["input_token_tiers"] = json!([
        {"when":{"operator":"gt","value":272000},"rates":rates},
        {"when":{"operator":"gte","value":300000},"rates":rates}
    ]);
    TokenPricingPolicy::parse(&value).unwrap();
    for replacement in [
        json!(272000),
        json!(1),
        json!(-1),
        json!(9007199254740992i64),
    ] {
        let mut bad = value.clone();
        bad["input_token_tiers"][1]["when"]["value"] = replacement;
        assert!(TokenPricingPolicy::parse(&bad).is_err());
    }
    let mut bad = value.clone();
    bad["input_token_tiers"][0]["when"]["operator"] = json!("ge");
    assert!(TokenPricingPolicy::parse(&bad).is_err());
    value["input_token_tiers"][1]["rates"]
        .as_object_mut()
        .unwrap()
        .remove("cache_write");
    assert!(TokenPricingPolicy::parse(&value).is_err());
}
