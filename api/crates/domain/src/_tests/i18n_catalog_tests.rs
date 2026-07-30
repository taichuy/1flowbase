use std::collections::BTreeMap;

use crate::i18n_catalog::*;

#[test]
fn seed_contract_types_reject_noncanonical_identifiers_and_digests() {
    assert_eq!(
        CatalogMessageIdentity::new("  ").unwrap_err(),
        I18nCatalogInvariantError::EmptyMessageKey
    );
    for key in [
        "设置",
        "settings.title",
        "custom_key",
        "<b>Settings</b>",
        "${settings}",
        " Settings",
    ] {
        assert_eq!(
            CatalogMessageIdentity::new(key).unwrap_err(),
            I18nCatalogInvariantError::InvalidMessageKey,
            "{key:?} must not become a catalog identity"
        );
    }
    for key in ["Settings", "Save {name}", "API v2.0", "E-mail"] {
        assert!(CatalogMessageIdentity::new(key).is_ok(), "{key:?}");
    }
    assert!(CatalogDigest::new(format!("sha256:{}", "a".repeat(64))).is_ok());
    assert!(CatalogDigest::new(format!("sha256:{}", "A".repeat(64))).is_err());
    assert!(CatalogLocale::new("zh-cn").is_err());
}

#[test]
fn catalog_locale_matches_the_canonical_seed_grammar() {
    for locale in ["en_US", "zh_Hans", "fil_Latn", "en"] {
        assert_eq!(CatalogLocale::new(locale).unwrap().as_str(), locale);
    }
    for locale in [
        "zh_hans",
        "zh_",
        "zh_Hans_CN",
        "zh_H4ns",
        "z_CN",
        "engl_US",
        "zh__Hans",
    ] {
        assert_eq!(
            CatalogLocale::new(locale).unwrap_err(),
            I18nCatalogInvariantError::InvalidLocale,
            "{locale} must be rejected"
        );
    }
}

#[test]
fn official_messages_require_an_explicit_english_translation() {
    let identity = CatalogMessageIdentity::new("Settings title").unwrap();
    assert_eq!(
        OfficialCatalogMessage::new(identity.clone(), BTreeMap::new()).unwrap_err(),
        I18nCatalogInvariantError::MissingSourceTranslation
    );

    let mut translations = BTreeMap::new();
    translations.insert(CatalogLocale::source(), "Settings".to_owned());
    assert!(OfficialCatalogMessage::new(identity, translations).is_ok());
}

#[test]
fn workspace_translation_can_override_the_english_locale() {
    let translation = CatalogTranslation::new(
        CatalogMessageIdentity::new("Settings title").unwrap(),
        CatalogLocale::source(),
        "Workspace settings",
    )
    .unwrap();

    assert!(translation.locale().is_source());
    assert_eq!(translation.translation(), "Workspace settings");
}
