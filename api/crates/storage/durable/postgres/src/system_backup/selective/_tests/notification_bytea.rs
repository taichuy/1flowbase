use super::bytea;
use serde_json::json;

#[test]
fn postgres_bytea_decodes_all_bytes_and_both_hex_cases() {
    let expected: Vec<u8> = (0..=u8::MAX).collect();
    for encoded in [
        format!("\\x{:x}", base16ct::HexDisplay(&expected)),
        format!("\\x{:X}", base16ct::HexDisplay(&expected)),
    ] {
        assert_eq!(bytea(&json!(encoded)).unwrap(), expected);
    }
    assert!(bytea(&json!("\\x")).unwrap().is_empty());
    assert_eq!(bytea(&json!("\\x00aAfF")).unwrap(), [0, 170, 255]);
}

#[test]
fn postgres_bytea_rejects_invalid_values_without_partial_decoding() {
    for value in [
        json!(null),
        json!(12),
        json!("00"),
        json!("\\x0"),
        json!("\\xgg"),
        json!("\\x00é"),
        json!("\\x00 0"),
    ] {
        assert!(bytea(&value).is_err(), "accepted invalid bytea {value}");
    }
}
