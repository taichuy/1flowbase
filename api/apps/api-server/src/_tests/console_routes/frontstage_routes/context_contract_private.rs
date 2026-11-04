use serde_json::{json, Value};

#[test]
fn ac_001_embedded_contract_rejects_invalid_versions_and_duplicate_names() {
    let source = include_bytes!("../../../../resources/ctx/block-context.v1.json");
    let mut invalid_version: Value = serde_json::from_slice(source).unwrap();
    invalid_version["contract_version"] = json!("2.0.0");
    assert!(
        crate::routes::frontstage::block_context_contract::decode_block_context_contract(
            &serde_json::to_vec(&invalid_version).unwrap()
        )
        .is_err()
    );

    let mut duplicate: Value = serde_json::from_slice(source).unwrap();
    duplicate["entries"][1]["key"] = json!("root");
    assert!(
        crate::routes::frontstage::block_context_contract::decode_block_context_contract(
            &serde_json::to_vec(&duplicate).unwrap()
        )
        .is_err()
    );

    let mut missing_i18n: Value = serde_json::from_slice(source).unwrap();
    missing_i18n["entries"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| entry["key"] != "i18n");
    assert!(
        crate::routes::frontstage::block_context_contract::decode_block_context_contract(
            &serde_json::to_vec(&missing_i18n).unwrap()
        )
        .is_err()
    );
}
