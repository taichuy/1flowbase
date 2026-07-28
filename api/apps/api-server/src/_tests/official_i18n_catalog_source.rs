use domain::{CatalogDigest, CatalogVersion};
use serde_json::Value;

use crate::{
    config::ResolvedOfficialI18nCatalogSourceConfig,
    official_i18n_catalog_seed::{
        decode_downloaded_catalog_seed, inspect_catalog_seed, CatalogSeedInspection,
        OFFICIAL_SEED_BYTES,
    },
    official_i18n_catalog_source::{validate_sidecar, ApiOfficialI18nCatalogSource},
};

fn source(
    latest_url: &str,
    release_base_url: &str,
    github_proxy_url: Option<&str>,
) -> ApiOfficialI18nCatalogSource {
    ApiOfficialI18nCatalogSource::new(ResolvedOfficialI18nCatalogSourceConfig {
        latest_url: latest_url.to_owned(),
        release_base_url: release_base_url.to_owned(),
        github_proxy_url: github_proxy_url.map(str::to_owned),
    })
}

#[test]
fn ac_005_official_mirror_and_github_proxy_urls_follow_existing_rewrite_semantics() {
    let official = source(
        "https://raw.githubusercontent.com/taichuy/official/main/i18n/dist/catalog-seed.json",
        "https://github.com/taichuy/official/releases/download",
        Some("https://github-proxy.example"),
    );
    assert_eq!(
        official.latest_url(),
        "https://github-proxy.example/https://raw.githubusercontent.com/taichuy/official/main/i18n/dist/catalog-seed.json"
    );
    assert_eq!(
        official.fixed_release_urls("1.2.3"),
        (
            "https://github.com/taichuy/official/releases/download/i18n-catalog-v1.2.3/i18n-catalog-seed-v1.2.3.json".to_owned(),
            "https://github.com/taichuy/official/releases/download/i18n-catalog-v1.2.3/i18n-catalog-seed-v1.2.3.json.sha256".to_owned(),
        )
    );

    let mirror = source(
        "https://mirror.example/i18n/latest.json",
        "https://raw.githubusercontent.com/company/mirror/releases",
        Some("https://github-proxy.example/"),
    );
    assert_eq!(
        mirror.latest_url(),
        "https://mirror.example/i18n/latest.json"
    );
    assert_eq!(
        mirror.fixed_release_urls("2.0.0").0,
        "https://github-proxy.example/https://raw.githubusercontent.com/company/mirror/releases/i18n-catalog-v2.0.0/i18n-catalog-seed-v2.0.0.json"
    );
}

#[test]
fn ac_005_sha_sidecar_is_bound_to_exact_versioned_asset() {
    let inspected = inspect_catalog_seed(OFFICIAL_SEED_BYTES).unwrap();
    let hex = inspected
        .seed_sha256
        .as_str()
        .strip_prefix("sha256:")
        .unwrap();
    let sidecar = format!("{hex}  i18n-catalog-seed-v1.1.0.json\n");
    validate_sidecar(sidecar.as_bytes(), inspected.seed_sha256.as_str(), "1.1.0").unwrap();
    assert!(validate_sidecar(sidecar.as_bytes(), inspected.seed_sha256.as_str(), "1.1.1").is_err());
    assert!(validate_sidecar(
        b"bad  i18n-catalog-seed-v1.1.0.json\n",
        inspected.seed_sha256.as_str(),
        "1.1.0"
    )
    .is_err());
}

#[test]
fn ac_005_downloaded_seed_reuses_canonical_schema_placeholder_and_digest_validator() {
    let valid = inspect_catalog_seed(OFFICIAL_SEED_BYTES).unwrap();
    decode_downloaded_catalog_seed(OFFICIAL_SEED_BYTES, &valid).unwrap();

    let bad_sha = CatalogSeedInspection {
        seed_sha256: CatalogDigest::new(format!("sha256:{}", "0".repeat(64))).unwrap(),
        ..valid.clone()
    };
    assert!(decode_downloaded_catalog_seed(OFFICIAL_SEED_BYTES, &bad_sha).is_err());

    let mut bad_schema: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    bad_schema["manifest"]["schema_version"] = Value::String("unsupported/v9".into());
    assert!(inspect_catalog_seed(&serde_json::to_vec(&bad_schema).unwrap()).is_err());

    let mut bad_placeholder: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    bad_placeholder["modules"][0]["messages"][1]["translations"]["zh_Hans"] =
        Value::String("保存 {other}".into());
    assert!(inspect_catalog_seed(&serde_json::to_vec(&bad_placeholder).unwrap()).is_err());

    let mut tampered_semantic: Value = serde_json::from_slice(OFFICIAL_SEED_BYTES).unwrap();
    tampered_semantic["manifest"]["semantic_sha256"] =
        Value::String(format!("sha256:{}", "1".repeat(64)));
    assert!(inspect_catalog_seed(&serde_json::to_vec(&tampered_semantic).unwrap()).is_err());

    let wrong_fixed_descriptor = CatalogSeedInspection {
        catalog_version: CatalogVersion::new("9.0.0").unwrap(),
        ..valid
    };
    assert!(decode_downloaded_catalog_seed(OFFICIAL_SEED_BYTES, &wrong_fixed_descriptor).is_err());
}
