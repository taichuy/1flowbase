use super::*;
use serde_json::json;

fn check_golden(value: &Value, canonical: &str) {
    let prepared = PreparedCanonicalRuntimeJson::new(value).unwrap();
    assert!(std::ptr::eq(prepared.value(), value));
    assert_eq!(prepared.byte_size(), canonical.len() as i64);
    assert_eq!(
        prepared.hash(),
        format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
    );
    let mut bytes = Vec::new();
    write_canonical_runtime_json(prepared.value(), &mut bytes).unwrap();
    assert_eq!(bytes, canonical.as_bytes());
}

#[test]
fn prepared_identity_matches_independent_nested_order_and_unicode_golden() {
    let value: Value = serde_json::from_str(
        r#"{"z":{"b":true,"a":null},"a":["北京",{"z":2,"a":1}]}"#,
    )
    .unwrap();
    check_golden(
        &value,
        r#"{"a":["北京",{"a":1,"z":2}],"z":{"a":null,"b":true}}"#,
    );
    let reordered: Value = serde_json::from_str(
        r#"{"a":["北京",{"a":1,"z":2}],"z":{"a":null,"b":true}}"#,
    )
    .unwrap();
    assert_eq!(
        PreparedCanonicalRuntimeJson::new(&value).unwrap().hash(),
        PreparedCanonicalRuntimeJson::new(&reordered).unwrap().hash()
    );
}

#[test]
fn prepared_identity_preserves_numeric_value_kinds_and_extreme_integers() {
    let value: Value = serde_json::from_str(
        "[0,-7,18446744073709551615,-9223372036854775808,1.5]",
    )
    .unwrap();
    check_golden(
        &value,
        "[0,-7,18446744073709551615,-9223372036854775808,1.5]",
    );
    let integer = json!(1);
    let float = json!(1.0);
    check_golden(&integer, "1");
    check_golden(&float, "1.0");
    assert_ne!(
        PreparedCanonicalRuntimeJson::new(&integer).unwrap().hash(),
        PreparedCanonicalRuntimeJson::new(&float).unwrap().hash()
    );
}

#[test]
fn prepared_identity_distinguishes_original_nul_and_literal_escape() {
    let nul = json!({"key\0": "body\0"});
    let literal = json!({"key\\u0000": "body\\u0000"});
    check_golden(&nul, r#"{"key\u0000":"body\u0000"}"#);
    check_golden(&literal, r#"{"key\\u0000":"body\\u0000"}"#);
    assert_ne!(
        PreparedCanonicalRuntimeJson::new(&nul).unwrap().hash(),
        PreparedCanonicalRuntimeJson::new(&literal).unwrap().hash()
    );
}

#[test]
fn prepared_identity_keeps_array_order_and_one_borrowed_value() {
    let forward = json!(["same", "values"]);
    let reverse = json!(["values", "same"]);
    let prepared = PreparedCanonicalRuntimeJson::new(&forward).unwrap();
    assert!(std::ptr::eq(prepared.value(), &forward));
    assert_ne!(prepared.value(), &reverse);
    assert_ne!(
        prepared.hash(),
        PreparedCanonicalRuntimeJson::new(&reverse).unwrap().hash()
    );
    check_golden(&forward, r#"["same","values"]"#);
}

#[test]
fn prepared_identity_handles_empty_values_without_body_or_history_cache() {
    for (value, canonical) in [
        (Value::Null, "null"),
        (json!({}), "{}"),
        (json!([]), "[]"),
        (json!(""), r#""""#),
    ] {
        check_golden(&value, canonical);
    }
}
