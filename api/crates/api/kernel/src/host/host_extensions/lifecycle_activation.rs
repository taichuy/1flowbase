use std::{collections::BTreeMap, sync::Arc};

use anyhow::{bail, Result};
use plugin_framework::{
    extension_bus::LifecycleHandlerBinding, HostExtensionContributionManifest, PluginManifestV1,
};

type LifecycleHandlerFactory = Arc<dyn Fn() -> Result<Vec<LifecycleHandlerBinding>> + Send + Sync>;

#[derive(Default)]
pub(crate) struct HostExtensionLifecycleFactoryCatalog {
    factories: BTreeMap<(String, String), LifecycleHandlerFactory>,
}

impl HostExtensionLifecycleFactoryCatalog {
    pub(crate) fn register(
        &mut self,
        library: impl Into<String>,
        entry_symbol: impl Into<String>,
        factory: LifecycleHandlerFactory,
    ) -> Result<()> {
        let key = (library.into(), entry_symbol.into());
        if self.factories.insert(key.clone(), factory).is_some() {
            bail!(
                "duplicate HostExtension lifecycle factory {}::{}",
                key.0,
                key.1
            );
        }
        Ok(())
    }

    pub(crate) fn activate(
        &self,
        active_extensions: &[(PluginManifestV1, HostExtensionContributionManifest)],
    ) -> Result<Vec<LifecycleHandlerBinding>> {
        let mut bindings = Vec::new();
        for (_, contribution) in active_extensions {
            if contribution.lifecycle_subscriptions.is_empty() {
                continue;
            }
            let key = (
                contribution.native.library.clone(),
                contribution.native.entry_symbol.clone(),
            );
            let factory = self.factories.get(&key).ok_or_else(|| {
                anyhow::anyhow!(
                    "active HostExtension {} has lifecycle subscriptions but no activation factory for {}::{}",
                    contribution.extension_id,
                    key.0,
                    key.1
                )
            })?;
            bindings.extend(factory()?);
        }
        Ok(bindings)
    }
}
