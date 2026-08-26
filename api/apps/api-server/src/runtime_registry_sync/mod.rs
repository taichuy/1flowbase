use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::RuntimeRegistrySync;
use runtime_core::runtime_model_registry::RuntimeModelRegistry;
use storage_durable_postgres::MainDurableStore;

#[derive(Clone)]
pub struct ApiRuntimeRegistrySync {
    store: MainDurableStore,
    registry: RuntimeModelRegistry,
}

impl ApiRuntimeRegistrySync {
    pub fn new(store: MainDurableStore, registry: RuntimeModelRegistry) -> Self {
        Self { store, registry }
    }
}

#[async_trait]
impl RuntimeRegistrySync for ApiRuntimeRegistrySync {
    async fn rebuild(&self) -> Result<()> {
        let runtime_metadata = self.store.list_runtime_model_metadata().await?;
        self.registry.rebuild(runtime_metadata);
        Ok(())
    }
}
