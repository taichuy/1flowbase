use std::sync::Arc;

use anyhow::{anyhow, Result};
use control_plane::ports::SessionStore;
use plugin_framework::{extension_bus::EffectiveExtensionGraph, HostExtensionRegistry};
use storage_ephemeral::{
    MemoryDistributedLock, MemoryEventBus, MemoryProviderTransportStore, MemoryTaskQueue,
    MokaRateLimitStore, MokaSessionStore,
};
use time::Duration;

use crate::extension_bus::CACHE_STORE_CONTRACT_ID;

use super::{
    cache_store_activation::{
        build_local_cache_store, builtin_cache_store_activation_factories,
        CacheStoreActivationFactoryRegistry, LOCAL_PROVIDER_CODE, LOCAL_PROVIDER_SOURCE,
    },
    DistributedLock, EventBus, HostInfrastructureRegistry, LocalRuntimeEventStream,
    ProviderTransportStore, RateLimitStore, RuntimeEventStream, TaskQueue, SESSION_STORE_NAMESPACE,
};

const RATE_LIMIT_STORE_NAMESPACE: &str = "flowbase:rate-limit";
const LOCK_NAMESPACE: &str = "flowbase:lock";
const TASK_QUEUE_NAMESPACE: &str = "flowbase:task";
const PROVIDER_REQUEST_LOG_QUEUE: &str = "provider-request-logs";
const PROVIDER_REQUEST_LOG_QUEUE_CAPACITY: usize = 10_000;
const LOCAL_CACHE_MAX_CAPACITY: u64 = 10_000;
const PROVIDER_TRANSPORT_RETENTION: Duration = Duration::minutes(15);
const PROVIDER_TRANSPORT_MAX_PAYLOAD_BYTES: usize = crate::RESPONSES_REQUEST_MAX_BYTES;
const LOCAL_INFRASTRUCTURE_CONTRACTS: &[&str] = &[
    "storage-ephemeral",
    "session-store",
    "cache-store",
    "provider-transport-store",
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

    install_compatibility_local_infrastructure_services(&mut registry);
    registry
}

pub fn build_local_host_infrastructure_from_host_extensions(
    host_extensions: &HostExtensionRegistry,
    graph: &Arc<EffectiveExtensionGraph>,
) -> Result<HostInfrastructureRegistry> {
    let factories = builtin_cache_store_activation_factories()?;
    build_local_host_infrastructure_from_host_extensions_with_cache_factories(
        host_extensions,
        graph,
        &factories,
    )
}

pub(crate) fn build_local_host_infrastructure_from_host_extensions_with_cache_factories(
    host_extensions: &HostExtensionRegistry,
    graph: &Arc<EffectiveExtensionGraph>,
    cache_store_factories: &CacheStoreActivationFactoryRegistry,
) -> Result<HostInfrastructureRegistry> {
    let activated_cache_store = cache_store_factories.activate(graph.as_ref(), host_extensions)?;
    let mut registry = HostInfrastructureRegistry::default();
    for contract in LOCAL_INFRASTRUCTURE_CONTRACTS {
        if *contract == CACHE_STORE_CONTRACT_ID {
            continue;
        }
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

    install_legacy_local_infrastructure_services(&mut registry);
    registry.register_default_provider(
        activated_cache_store.contract,
        activated_cache_store.provider_code,
        activated_cache_store.source,
    )?;
    registry.set_cache_store(activated_cache_store.service);
    Ok(registry)
}

fn install_compatibility_local_infrastructure_services(registry: &mut HostInfrastructureRegistry) {
    install_legacy_local_infrastructure_services(registry);
    registry.set_cache_store(build_local_cache_store());
}

fn install_legacy_local_infrastructure_services(registry: &mut HostInfrastructureRegistry) {
    registry.set_session_store(Arc::new(MokaSessionStore::new(
        SESSION_STORE_NAMESPACE,
        LOCAL_CACHE_MAX_CAPACITY,
    )) as Arc<dyn SessionStore>);
    registry.set_provider_transport_store(Arc::new(MemoryProviderTransportStore::new(
        PROVIDER_TRANSPORT_RETENTION,
        PROVIDER_TRANSPORT_MAX_PAYLOAD_BYTES,
    )) as Arc<dyn ProviderTransportStore>);
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
    let mut event_stream = LocalRuntimeEventStream::new();
    if let Ok(value) = std::env::var("API_RUNTIME_REPLAY_HOT_TTL_SECONDS") {
        match value.parse::<u64>() {
            Ok(seconds) => {
                event_stream =
                    event_stream.with_recoverable_retention(std::time::Duration::from_secs(seconds))
            }
            Err(_) => tracing::warn!(
                "invalid API_RUNTIME_REPLAY_HOT_TTL_SECONDS; using default replay retention"
            ),
        }
    }
    if let Ok(value) = std::env::var("API_RUNTIME_REPLAY_RECOVERABLE_MAX_BYTES") {
        match value.parse::<usize>() {
            Ok(bytes) => event_stream = event_stream.with_recoverable_byte_budget(bytes),
            Err(_) => tracing::warn!("invalid API_RUNTIME_REPLAY_RECOVERABLE_MAX_BYTES; leaving recoverable cache budget unset"),
        }
    }
    registry.set_runtime_event_stream(Arc::new(event_stream) as Arc<dyn RuntimeEventStream>);
}
