mod contracts;
mod local;
mod local_runtime_event_stream;

use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, Result};
use control_plane::ports::SessionStore;

pub use contracts::{
    CacheStore, ClaimedTask, DistributedLock, EventBus, RateLimitDecision, RateLimitStore,
    RuntimeEventStream, TaskQueue,
};
pub use local::{
    build_local_host_infrastructure, build_local_host_infrastructure_from_host_extensions,
};
pub use local_runtime_event_stream::LocalRuntimeEventStream;

pub const SESSION_STORE_NAMESPACE: &str = "flowbase:console:session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredInfrastructureProvider {
    pub contract: String,
    pub provider_code: String,
    pub source: String,
}

#[derive(Clone, Default)]
pub struct HostInfrastructureRegistry {
    providers: BTreeMap<String, RegisteredInfrastructureProvider>,
    session_store: Option<Arc<dyn SessionStore>>,
    cache_store: Option<Arc<dyn CacheStore>>,
    distributed_lock: Option<Arc<dyn DistributedLock>>,
    event_bus: Option<Arc<dyn EventBus>>,
    task_queue: Option<Arc<dyn TaskQueue>>,
    rate_limit_store: Option<Arc<dyn RateLimitStore>>,
    runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
}

impl HostInfrastructureRegistry {
    pub fn register_default_provider(
        &mut self,
        contract: impl Into<String>,
        provider_code: impl Into<String>,
        source: impl Into<String>,
    ) -> Result<()> {
        let contract = contract.into();
        let provider = RegisteredInfrastructureProvider {
            contract: contract.clone(),
            provider_code: provider_code.into(),
            source: source.into(),
        };

        if self.providers.contains_key(&contract) {
            return Err(anyhow!(
                "default provider already registered for infrastructure contract `{contract}`"
            ));
        }

        self.providers.insert(contract, provider);
        Ok(())
    }

    pub fn default_provider(&self, contract: &str) -> Option<&str> {
        self.providers
            .get(contract)
            .map(|provider| provider.provider_code.as_str())
    }

    pub fn default_provider_source(&self, contract: &str) -> Option<&str> {
        self.providers
            .get(contract)
            .map(|provider| provider.source.as_str())
    }

    pub fn set_session_store(&mut self, session_store: Arc<dyn SessionStore>) {
        self.session_store = Some(session_store);
    }

    pub fn session_store(&self) -> Option<Arc<dyn SessionStore>> {
        self.session_store.clone()
    }

    pub fn set_cache_store(&mut self, cache_store: Arc<dyn CacheStore>) {
        self.cache_store = Some(cache_store);
    }

    pub fn registered_cache_store(&self) -> Option<Arc<dyn CacheStore>> {
        self.cache_store.clone()
    }

    pub fn cache_store(&self) -> Arc<dyn CacheStore> {
        self.cache_store
            .clone()
            .expect("cache-store provider must be registered before use")
    }

    pub fn set_distributed_lock(&mut self, distributed_lock: Arc<dyn DistributedLock>) {
        self.distributed_lock = Some(distributed_lock);
    }

    pub fn registered_distributed_lock(&self) -> Option<Arc<dyn DistributedLock>> {
        self.distributed_lock.clone()
    }

    pub fn distributed_lock(&self) -> Arc<dyn DistributedLock> {
        self.distributed_lock
            .clone()
            .expect("distributed-lock provider must be registered before use")
    }

    pub fn set_event_bus(&mut self, event_bus: Arc<dyn EventBus>) {
        self.event_bus = Some(event_bus);
    }

    pub fn registered_event_bus(&self) -> Option<Arc<dyn EventBus>> {
        self.event_bus.clone()
    }

    pub fn event_bus(&self) -> Arc<dyn EventBus> {
        self.event_bus
            .clone()
            .expect("event-bus provider must be registered before use")
    }

    pub fn set_task_queue(&mut self, task_queue: Arc<dyn TaskQueue>) {
        self.task_queue = Some(task_queue);
    }

    pub fn registered_task_queue(&self) -> Option<Arc<dyn TaskQueue>> {
        self.task_queue.clone()
    }

    pub fn task_queue(&self) -> Arc<dyn TaskQueue> {
        self.task_queue
            .clone()
            .expect("task-queue provider must be registered before use")
    }

    pub fn set_rate_limit_store(&mut self, rate_limit_store: Arc<dyn RateLimitStore>) {
        self.rate_limit_store = Some(rate_limit_store);
    }

    pub fn registered_rate_limit_store(&self) -> Option<Arc<dyn RateLimitStore>> {
        self.rate_limit_store.clone()
    }

    pub fn rate_limit_store(&self) -> Arc<dyn RateLimitStore> {
        self.rate_limit_store
            .clone()
            .expect("rate-limit-store provider must be registered before use")
    }

    pub fn set_runtime_event_stream(&mut self, stream: Arc<dyn RuntimeEventStream>) {
        self.runtime_event_stream = Some(stream);
    }

    pub fn runtime_event_stream(&self) -> Option<Arc<dyn RuntimeEventStream>> {
        self.runtime_event_stream.clone()
    }
}
