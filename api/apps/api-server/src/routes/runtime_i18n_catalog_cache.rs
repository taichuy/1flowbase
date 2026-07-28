use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock, RwLock},
};

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BundleCacheKey {
    workspace_id: Uuid,
    module: String,
    locale: String,
    digest: String,
}

#[derive(Default)]
pub(super) struct RuntimeI18nBundleCache {
    bundles: RwLock<BTreeMap<BundleCacheKey, Arc<[u8]>>>,
}

impl RuntimeI18nBundleCache {
    pub(super) fn get(
        &self,
        workspace_id: Uuid,
        module: &str,
        locale: &str,
        digest: &str,
    ) -> Option<Arc<[u8]>> {
        self.bundles
            .read()
            .ok()?
            .get(&BundleCacheKey {
                workspace_id,
                module: module.to_owned(),
                locale: locale.to_owned(),
                digest: digest.to_owned(),
            })
            .cloned()
    }

    pub(super) fn insert(
        &self,
        workspace_id: Uuid,
        module: &str,
        locale: &str,
        digest: &str,
        body: Arc<[u8]>,
    ) {
        if let Ok(mut bundles) = self.bundles.write() {
            bundles
                .entry(BundleCacheKey {
                    workspace_id,
                    module: module.to_owned(),
                    locale: locale.to_owned(),
                    digest: digest.to_owned(),
                })
                .or_insert(body);
        }
    }
}

pub(super) fn runtime_i18n_bundle_cache() -> &'static RuntimeI18nBundleCache {
    static CACHE: OnceLock<RuntimeI18nBundleCache> = OnceLock::new();
    CACHE.get_or_init(RuntimeI18nBundleCache::default)
}
