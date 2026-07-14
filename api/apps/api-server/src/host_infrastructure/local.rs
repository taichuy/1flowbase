use std::sync::Arc;

use anyhow::{Result, anyhow};
use control_plane::ports::SessionStore;
use plugin_framework::HostExtensionRegistry;
use storage_ephemeral::{
    MemoryDistributedLock, MemoryEventBus, MemoryTaskQueue, MokaCacheStore, MokaRateLimitStore,
    MokaSessionStore,
};

use super::{
    CacheStore, DistributedLock, EventBus, HostInfrastructureRegistry, LocalRuntimeEventStream,
    RateLimitStore, RuntimeEventStream, SESSION_STORE_NAMESPACE, TaskQueue,
};

const LOCAL_PROVIDER_CODE: &str = "local";
const LOCAL_PROVIDER_SOURCE: &str = "official.local-infra-host";
const CACHE_STORE_NAMESPACE: &str = "flowbase:cache";
const RATE_LIMIT_STORE_NAMESPACE: &str = "flowbase:rate-limit";
const LOCK_NAMESPACE: &str = "flowbase:lock";
const TASK_QUEUE_NAMESPACE: &str = "flowbase:task";
const PROVIDER_REQUEST_LOG_QUEUE: &str = "provider-request-logs";
const PROVIDER_REQUEST_LOG_QUEUE_CAPACITY: usize = 10_000;
const LOCAL_CACHE_MAX_CAPACITY: u64 = 10_000;
const LOCAL_INFRASTRUCTURE_CONTRACTS: &[&str] = &[
    "storage-ephemeral",
    "session-store",
    "cache-store",
    "distributed-lock",
    "event-bus",
    "task-queue",
    "rate-limit-store",
    "runtime-event-stream",
];

pub fn build_local_host_infrastructure() -> HostInfrastructureRegistry {
    let mut registry = HostInfrastructureRegistry::default();
    for contract in LOCAL_INFRASTRUCTURE_CONTRACTS {
        registry
            .register_default_provider(*contract, LOCAL_PROVIDER_CODE, LOCAL_PROVIDER_SOURCE)
            .expect("local provider registration should be unique");
    }

    install_local_infrastructure_services(&mut registry);
    registry
}

pub fn build_local_host_infrastructure_from_host_extensions(
    host_extensions: &HostExtensionRegistry,
) -> Result<HostInfrastructureRegistry> {
    let mut registry = HostInfrastructureRegistry::default();
    for contract in LOCAL_INFRASTRUCTURE_CONTRACTS {
        let provider = host_extensions
            .infrastructure_provider(contract, LOCAL_PROVIDER_CODE)
            .ok_or_else(|| {
                anyhow!(
                    "builtin local infrastructure provider `{}` for `{}` is not registered",
                    LOCAL_PROVIDER_CODE,
                    contract
                )
            })?;
        registry.register_default_provider(
            provider.contract.clone(),
            provider.provider_code.clone(),
            provider.extension_id.clone(),
        )?;
    }

    install_local_infrastructure_services(&mut registry);
    Ok(registry)
}

fn install_local_infrastructure_services(registry: &mut HostInfrastructureRegistry) {
    registry.set_session_store(Arc::new(MokaSessionStore::new(
        SESSION_STORE_NAMESPACE,
        LOCAL_CACHE_MAX_CAPACITY,
    )) as Arc<dyn SessionStore>);
    registry.set_cache_store(Arc::new(MokaCacheStore::new(
        CACHE_STORE_NAMESPACE,
        LOCAL_CACHE_MAX_CAPACITY,
    )) as Arc<dyn CacheStore>);
    registry.set_distributed_lock(
        Arc::new(MemoryDistributedLock::new(LOCK_NAMESPACE)) as Arc<dyn DistributedLock>
    );
    registry.set_event_bus(Arc::new(MemoryEventBus::new()) as Arc<dyn EventBus>);
    registry.set_task_queue(Arc::new(
        MemoryTaskQueue::new(TASK_QUEUE_NAMESPACE).with_queue_capacity(
            PROVIDER_REQUEST_LOG_QUEUE,
            PROVIDER_REQUEST_LOG_QUEUE_CAPACITY,
        ),
    ) as Arc<dyn TaskQueue>);
    registry.set_rate_limit_store(Arc::new(MokaRateLimitStore::new(
        RATE_LIMIT_STORE_NAMESPACE,
        LOCAL_CACHE_MAX_CAPACITY,
    )) as Arc<dyn RateLimitStore>);
    registry.set_runtime_event_stream(
        Arc::new(LocalRuntimeEventStream::new()) as Arc<dyn RuntimeEventStream>
    );
}
