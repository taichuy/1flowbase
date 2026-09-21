use super::*;

#[test]
fn arbitrary_json_keeps_exact_original_without_user_markers() {
    let original = json!({"a\0b": ["\0", "\\u0000", "\\", null], "a\\u0000b": "distinct"});
    let packet = lossless_json_parameter(&original).unwrap();
    let restored: Value = serde_json::from_str(packet[1].as_str().unwrap()).unwrap();
    assert_eq!(restored, original);
    let mut projected_again = packet[0].clone();
    assert!(!project(&mut projected_again));
    assert_eq!(projected_again, packet[0]);
    assert_eq!(packet[0].as_object().unwrap().len(), 2);
    assert!(packet[0].get("a\\u0000b").is_some());
    assert!(packet[0].get("a\\\\u0000b").is_some());
}

#[test]
fn ordinary_values_and_sql_null_have_distinct_envelopes() {
    assert_eq!(lossless_json_parameter(&None::<Value>), None);
    assert_eq!(
        lossless_json_parameter(&Some(Value::Null)),
        Some(json!([null, null]))
    );
    let ordinary = json!({"raw_json_payloads": {"input_payload": "forged"}, "text": "\\u0000"});
    assert_eq!(
        lossless_json_parameter(&ordinary),
        Some(json!([ordinary, null]))
    );
}
