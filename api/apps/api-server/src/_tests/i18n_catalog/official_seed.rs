use serde_json::Value;

use crate::official_i18n_catalog_seed::{
    decode_catalog_seed, OFFICIAL_SEED_BYTES, OFFICIAL_SEED_SOURCE_BYTES,
};

#[test]
fn ac_002_decodes_digest_verified_build_time_official_seed() {
    decode_catalog_seed(OFFICIAL_SEED_BYTES, OFFICIAL_SEED_SOURCE_BYTES).unwrap();
}

#[test]
fn ac_002_rejects_tampered_seed_content() {
    let mut seed: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    seed["modules"][0]["messages"][0]["translations"]["zh_Hans"] = Value::String("篡改".to_owned());

    let error = decode_catalog_seed(
        &serde_json::to_vec(&seed).unwrap(),
        OFFICIAL_SEED_SOURCE_BYTES,
    )
    .unwrap_err();

    assert!(error.to_string().contains("digest mismatch"));
}

#[test]
fn ac_002_rejects_wrong_schema_and_source_locale() {
    let mut wrong_schema: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    wrong_schema["manifest"]["schema_version"] = Value::String("future/v2".to_owned());
    assert!(decode_catalog_seed(
        &serde_json::to_vec(&wrong_schema).unwrap(),
        OFFICIAL_SEED_SOURCE_BYTES,
    )
    .unwrap_err()
    .to_string()
    .contains("schema"));

    let mut wrong_source: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    wrong_source["manifest"]["source_locale"] = Value::String("zh_Hans".to_owned());
    assert!(decode_catalog_seed(
        &serde_json::to_vec(&wrong_source).unwrap(),
        OFFICIAL_SEED_SOURCE_BYTES,
    )
    .unwrap_err()
    .to_string()
    .contains("source locale"));
}

#[test]
fn ac_002_rejects_seed_digest_that_disagrees_with_source_metadata() {
    let mut source: Value = serde_json::from_slice(OFFICIAL_SEED_SOURCE_BYTES).unwrap();
    source["semantic_sha256"] = Value::String(format!("sha256:{}", "0".repeat(64)));

    let error = decode_catalog_seed(OFFICIAL_SEED_BYTES, &serde_json::to_vec(&source).unwrap())
        .unwrap_err();

    assert!(error.to_string().contains("fixed release descriptor"));
}

#[test]
fn ac_002_rejects_unpinned_official_provenance() {
    let mut source: Value = serde_json::from_slice(OFFICIAL_SEED_SOURCE_BYTES).unwrap();
    source["official_commit"] = Value::String("0".repeat(40));

    let error = decode_catalog_seed(OFFICIAL_SEED_BYTES, &serde_json::to_vec(&source).unwrap())
        .unwrap_err();

    assert!(error.to_string().contains("source commit is not pinned"));
}

#[test]
fn ac_002_rejects_non_plain_text_before_database_binding() {
    let mut seed: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    seed["modules"][0]["messages"][0]["translations"]["zh_Hans"] =
        Value::String("<script>bad</script>".to_owned());

    let error = decode_catalog_seed(
        &serde_json::to_vec(&seed).unwrap(),
        OFFICIAL_SEED_SOURCE_BYTES,
    )
    .unwrap_err();

    assert!(error.to_string().contains("plain text"));
}

#[test]
fn ac_006_official_loader_has_only_embedded_byte_sources() {
    let loader_source = include_str!("../../official_i18n_catalog_seed.rs");

    assert!(loader_source.contains("include_bytes!"));
    assert!(!loader_source.contains("reqwest"));
    assert!(!loader_source.contains("http://"));
    assert!(!loader_source.contains("https://"));
}
