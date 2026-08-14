use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, bail, Result};
use plugin_framework::{
    extension_bus::{ContributionId, EffectiveExtensionGraph, ExtensionPointId},
    HostExtensionRegistry,
};
use storage_ephemeral::MokaCacheStore;

use crate::extension_bus::{
    infrastructure_provider_contribution_id, CACHE_STORE_CONTRACT_ID,
    CACHE_STORE_EXTENSION_POINT_ID,
};

use super::CacheStore;

pub(super) const LOCAL_PROVIDER_CODE: &str = "local";
pub(super) const LOCAL_PROVIDER_SOURCE: &str = "official.local-infra-host";

const CACHE_STORE_NAMESPACE: &str = "flowbase:cache";
const LOCAL_CACHE_MAX_CAPACITY: u64 = 10_000;

type CacheStoreFactory = fn() -> Arc<dyn CacheStore>;

pub(crate) struct ActivatedCacheStore {
    pub contract: String,
    pub provider_code: String,
    pub source: String,
    pub service: Arc<dyn CacheStore>,
}

#[derive(Default)]
pub(crate) struct CacheStoreActivationFactoryRegistry {
    factories: BTreeMap<ContributionId, CacheStoreFactory>,
}

impl CacheStoreActivationFactoryRegistry {
    fn register(
        &mut self,
        contribution_id: ContributionId,
        factory: CacheStoreFactory,
    ) -> Result<()> {
        if self
            .factories
            .insert(contribution_id.clone(), factory)
            .is_some()
        {
            bail!(
                "cache-store activation factory is already registered for {:?}",
                contribution_id
            );
        }
        Ok(())
    }

    pub(crate) fn activate(
        &self,
        graph: &EffectiveExtensionGraph,
        host_extensions: &HostExtensionRegistry,
    ) -> Result<ActivatedCacheStore> {
        let point_id = ExtensionPointId::new(CACHE_STORE_EXTENSION_POINT_ID)?;
        let point = graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id == point_id)
            .ok_or_else(|| anyhow!("compiled extension graph has no cache-store slot"))?;
        let [winner] = point.contributions() else {
            bail!(
                "compiled cache-store slot must have exactly one activated winner, found {}",
                point.contributions().len()
            );
        };
        let winner_id = &winner.descriptor().contribution_id;
        let factory = self.factories.get(winner_id).ok_or_else(|| {
            anyhow!(
                "no cache-store activation factory registered for winner {:?}",
                winner_id
            )
        })?;
        let provider = host_extensions
            .providers_for_contract(CACHE_STORE_CONTRACT_ID)
            .into_iter()
            .find(|provider| {
                infrastructure_provider_contribution_id(
                    &provider.extension_id,
                    &provider.contract,
                    &provider.provider_code,
                )
                .is_ok_and(|contribution_id| &contribution_id == winner_id)
            })
            .ok_or_else(|| {
                anyhow!(
                    "cache-store winner {:?} has no registered host provider declaration",
                    winner_id
                )
            })?;

        Ok(ActivatedCacheStore {
            contract: provider.contract.clone(),
            provider_code: provider.provider_code.clone(),
            source: provider.extension_id.clone(),
            service: factory(),
        })
    }
}

pub(crate) fn builtin_cache_store_activation_factories(
) -> Result<CacheStoreActivationFactoryRegistry> {
    let mut factories = CacheStoreActivationFactoryRegistry::default();
    factories.register(
        infrastructure_provider_contribution_id(
            LOCAL_PROVIDER_SOURCE,
            CACHE_STORE_CONTRACT_ID,
            LOCAL_PROVIDER_CODE,
        )?,
        build_local_cache_store,
    )?;
    Ok(factories)
}

pub(super) fn build_local_cache_store() -> Arc<dyn CacheStore> {
    Arc::new(MokaCacheStore::new(
        CACHE_STORE_NAMESPACE,
        LOCAL_CACHE_MAX_CAPACITY,
    ))
}
