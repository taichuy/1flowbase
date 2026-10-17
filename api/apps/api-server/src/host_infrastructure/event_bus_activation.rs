//! Activate the declared EventBus provider; never replace a different host's service.
use super::cache_store_activation::{LOCAL_PROVIDER_CODE, LOCAL_PROVIDER_SOURCE};
use super::{EventBus, HostInfrastructureRegistry};
use anyhow::{Result, anyhow, bail};
use plugin_framework::HostExtensionRegistry;
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};

type Factory = fn(PgPool) -> Arc<dyn EventBus>;

#[derive(Default)]
pub(crate) struct EventBusActivationFactoryRegistry {
    factories: BTreeMap<(String, String), Factory>,
}

impl EventBusActivationFactoryRegistry {
    pub(crate) fn register(
        &mut self,
        source: &str,
        provider: &str,
        factory: Factory,
    ) -> Result<()> {
        let key = (source.into(), provider.into());
        if self.factories.contains_key(&key) {
            bail!("duplicate event-bus activation factory");
        }
        self.factories.insert(key, factory);
        Ok(())
    }

    pub(crate) fn activate(
        &self,
        hosts: &HostExtensionRegistry,
        pool: PgPool,
        registry: &mut HostInfrastructureRegistry,
    ) -> Result<()> {
        // HostExtensionRegistry itself rejects multiple defaults for one contract.
        // EventBus has no compiled graph slot today; use its actual declared provider,
        // rather than assuming every active host is official/local.
        let providers = hosts.providers_for_contract("event-bus");
        let [provider] = providers.as_slice() else {
            bail!("event-bus must have exactly one declared active provider");
        };
        let factory = self
            .factories
            .get(&(
                provider.extension_id.clone(),
                provider.provider_code.clone(),
            ))
            .ok_or_else(|| {
                anyhow!(
                    "no event-bus activation factory for {}/{}",
                    provider.extension_id,
                    provider.provider_code
                )
            })?;
        registry.register_default_provider(
            "event-bus",
            &provider.provider_code,
            &provider.extension_id,
        )?;
        registry.set_event_bus(factory(pool));
        Ok(())
    }
}

pub(super) fn builtin_event_bus_factories() -> Result<EventBusActivationFactoryRegistry> {
    let mut factories = EventBusActivationFactoryRegistry::default();
    factories.register(LOCAL_PROVIDER_SOURCE, LOCAL_PROVIDER_CODE, |pool| {
        Arc::new(storage_durable_postgres::PgEventBus::new(pool))
    })?;
    Ok(factories)
}

#[cfg(test)]
#[path = "../_tests/host_infrastructure/event_bus_activation_tests.rs"]
mod tests;
