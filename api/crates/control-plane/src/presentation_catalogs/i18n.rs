use std::collections::BTreeMap;

use plugin_framework::provider_package::ProviderI18nCatalog;
use serde_json::Value;

pub type I18nCatalog = BTreeMap<String, BTreeMap<String, Value>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedLocales {
    pub resolved_locale: String,
    pub fallback_locale: String,
}

impl RequestedLocales {
    pub fn new(resolved_locale: impl Into<String>, fallback_locale: impl Into<String>) -> Self {
        Self {
            resolved_locale: resolved_locale.into(),
            fallback_locale: fallback_locale.into(),
        }
    }

    pub fn ordered(&self) -> Vec<&str> {
        let mut locales = vec![self.resolved_locale.as_str()];
        if self.fallback_locale != self.resolved_locale {
            locales.push(self.fallback_locale.as_str());
        }
        locales
    }
}

pub fn plugin_namespace(provider_code: &str) -> String {
    format!("plugin.{provider_code}")
}

pub fn trim_provider_bundles(
    namespace: &str,
    catalog: &ProviderI18nCatalog,
    locales: &RequestedLocales,
) -> I18nCatalog {
    trim_json_bundles(namespace, &catalog.bundles, locales)
}

pub fn trim_json_bundles(
    namespace: &str,
    bundles: &BTreeMap<String, Value>,
    locales: &RequestedLocales,
) -> I18nCatalog {
    let mut trimmed = BTreeMap::new();
    for locale in locales.ordered() {
        if let Some(bundle) = bundles.get(locale) {
            trimmed.insert(locale.to_string(), bundle.clone());
        }
    }

    BTreeMap::from([(namespace.to_string(), trimmed)])
}

pub fn merge_i18n_catalog(target: &mut I18nCatalog, next: I18nCatalog) {
    for (namespace, bundles) in next {
        target.entry(namespace).or_default().extend(bundles);
    }
}
