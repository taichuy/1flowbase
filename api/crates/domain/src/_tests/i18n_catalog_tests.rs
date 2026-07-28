use std::collections::BTreeMap;

use crate::i18n_catalog::*;

#[test]
fn seed_contract_types_reject_noncanonical_identifiers_and_digests() {
    assert!(CatalogModuleId::new("console/settings").is_err());
    assert!(CatalogModuleId::new("@1flowbase/console/settings").is_ok());
    assert!(CatalogDigest::new(format!("sha256:{}", "a".repeat(64))).is_ok());
    assert!(CatalogDigest::new(format!("sha256:{}", "A".repeat(64))).is_err());
    assert!(CatalogLocale::new("zh_CN").is_ok());
    assert!(CatalogLocale::new("zh-cn").is_err());
}

#[test]
fn official_translations_omit_the_english_source_locale() {
    let identity = CatalogMessageIdentity::new(
        CatalogModuleId::new("@1flowbase/console/settings").unwrap(),
        "Settings",
    )
    .unwrap();
    let mut translations = BTreeMap::new();
    translations.insert(CatalogLocale::source(), "Settings".to_owned());

    assert_eq!(
        OfficialCatalogMessage::new(identity, translations).unwrap_err(),
        I18nCatalogInvariantError::SourceLocaleTranslation
    );
}
