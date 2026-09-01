use super::*;

#[derive(Clone)]
pub(crate) struct MemoryInspectionDependencies {
    pub(crate) session_store: Option<Arc<dyn SessionStore>>,
    pub(crate) cache_store: Option<Arc<dyn CacheStore>>,
    pub(crate) rate_limit_store: Option<Arc<dyn RateLimitStore>>,
    pub(crate) distributed_lock: Option<Arc<dyn DistributedLock>>,
    pub(crate) task_queue: Option<Arc<dyn TaskQueue>>,
    pub(crate) event_bus: Option<Arc<dyn EventBus>>,
    pub(crate) runtime_event_stream: Option<Arc<dyn RuntimeEventStream>>,
    pub(crate) provider_codes: std::collections::BTreeMap<String, String>,
}

pub(super) enum MemoryInspectionTarget {
    Session(Arc<dyn SessionStore>),
    Cache(Arc<dyn CacheStore>),
    RateLimit(Arc<dyn RateLimitStore>),
    Lock(Arc<dyn DistributedLock>),
    TaskQueue(Arc<dyn TaskQueue>),
    EventBus(Arc<dyn EventBus>),
    RuntimeEvents(Arc<dyn RuntimeEventStream>),
    Unsupported,
}

impl MemoryInspectionTarget {
    pub(super) fn capabilities(&self) -> EphemeralInspectionCapabilities {
        match self {
            Self::Session(store) => store.ephemeral_inspection_capabilities(),
            Self::Cache(store) => store.ephemeral_inspection_capabilities(),
            Self::RateLimit(store) => store.ephemeral_inspection_capabilities(),
            Self::Lock(store) => store.ephemeral_inspection_capabilities(),
            Self::TaskQueue(store) => store.ephemeral_inspection_capabilities(),
            Self::EventBus(store) => store.ephemeral_inspection_capabilities(),
            Self::RuntimeEvents(stream) => stream.ephemeral_inspection_capabilities(),
            Self::Unsupported => EphemeralInspectionCapabilities::unsupported(),
        }
    }

    pub(super) async fn summarize_entries_at_path(
        &self,
        inspection_path: &[String],
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        let entries = match self {
            Self::Session(store) => store.list_ephemeral_entries().await?,
            Self::Cache(store) => store.list_ephemeral_entries().await?,
            Self::RateLimit(store) => store.list_ephemeral_entries().await?,
            Self::Lock(store) => store.list_ephemeral_entries().await?,
            Self::TaskQueue(store) => store.list_ephemeral_entries().await?,
            Self::EventBus(store) => store.list_ephemeral_entries().await?,
            Self::RuntimeEvents(stream) => stream.list_ephemeral_entries().await?,
            Self::Unsupported => Vec::new(),
        };

        Ok(summarize_memory_entries_at_path(entries, inspection_path))
    }

    pub(super) async fn list_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        match self {
            Self::Session(store) => store.list_ephemeral_tree(request).await,
            Self::Cache(store) => store.list_ephemeral_tree(request).await,
            Self::RateLimit(store) => store.list_ephemeral_tree(request).await,
            Self::Lock(store) => store.list_ephemeral_tree(request).await,
            Self::TaskQueue(store) => store.list_ephemeral_tree(request).await,
            Self::EventBus(store) => store.list_ephemeral_tree(request).await,
            Self::RuntimeEvents(stream) => stream.list_ephemeral_tree(request).await,
            Self::Unsupported => Ok(empty_memory_tree_page(request)),
        }
    }

    pub(super) async fn list_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        match self {
            Self::Session(store) => store.list_ephemeral_entry_page(request).await,
            Self::Cache(store) => store.list_ephemeral_entry_page(request).await,
            Self::RateLimit(store) => store.list_ephemeral_entry_page(request).await,
            Self::Lock(store) => store.list_ephemeral_entry_page(request).await,
            Self::TaskQueue(store) => store.list_ephemeral_entry_page(request).await,
            Self::EventBus(store) => store.list_ephemeral_entry_page(request).await,
            Self::RuntimeEvents(stream) => stream.list_ephemeral_entry_page(request).await,
            Self::Unsupported => Ok(empty_memory_entry_page(request)),
        }
    }

    pub(super) async fn search_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        match self {
            Self::Session(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::Cache(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::RateLimit(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::Lock(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::TaskQueue(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::EventBus(store) => store.search_ephemeral_entry_page(query, request).await,
            Self::RuntimeEvents(stream) => stream.search_ephemeral_entry_page(query, request).await,
            Self::Unsupported => Ok(empty_memory_entry_page(request)),
        }
    }

    pub(super) async fn reveal_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        match self {
            Self::Session(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::Cache(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::RateLimit(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::Lock(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::TaskQueue(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::EventBus(store) => store.reveal_ephemeral_entry(entry_ref, reveal_mode).await,
            Self::RuntimeEvents(stream) => {
                stream.reveal_ephemeral_entry(entry_ref, reveal_mode).await
            }
            Self::Unsupported => Ok(None),
        }
    }
}

const MEMORY_CONTRACTS: &[(&str, &str)] = &[
    ("session-store", "Sessions"),
    ("cache-store", "Cache"),
    ("rate-limit-store", "Rate Limits"),
    ("distributed-lock", "Locks"),
    ("task-queue", "Task Queue"),
    ("event-bus", "Event Bus"),
    ("runtime-event-stream", "Runtime Events"),
];

pub(super) fn memory_contract_definitions() -> &'static [(&'static str, &'static str)] {
    MEMORY_CONTRACTS
}

pub(super) fn memory_contract_label(contract_code: &str) -> Result<&'static str, ApiError> {
    MEMORY_CONTRACTS
        .iter()
        .find_map(|(candidate, label)| (*candidate == contract_code).then_some(*label))
        .ok_or(ControlPlaneError::NotFound("memory_contract").into())
}

pub(super) fn memory_inspection_target(
    dependencies: &MemoryInspectionDependencies,
    contract_code: &str,
) -> Result<MemoryInspectionTarget, ApiError> {
    match contract_code {
        "session-store" => Ok(dependencies
            .session_store
            .clone()
            .map(MemoryInspectionTarget::Session)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "cache-store" => Ok(dependencies
            .cache_store
            .clone()
            .map(MemoryInspectionTarget::Cache)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "rate-limit-store" => Ok(dependencies
            .rate_limit_store
            .clone()
            .map(MemoryInspectionTarget::RateLimit)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "distributed-lock" => Ok(dependencies
            .distributed_lock
            .clone()
            .map(MemoryInspectionTarget::Lock)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "task-queue" => Ok(dependencies
            .task_queue
            .clone()
            .map(MemoryInspectionTarget::TaskQueue)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "event-bus" => Ok(dependencies
            .event_bus
            .clone()
            .map(MemoryInspectionTarget::EventBus)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        "runtime-event-stream" => Ok(dependencies
            .runtime_event_stream
            .clone()
            .map(MemoryInspectionTarget::RuntimeEvents)
            .unwrap_or(MemoryInspectionTarget::Unsupported)),
        _ => Err(ControlPlaneError::NotFound("memory_contract").into()),
    }
}

pub(super) fn memory_contract_supported(capabilities: &EphemeralInspectionCapabilities) -> bool {
    capabilities.list_entries
        || capabilities.list_tree
        || capabilities.search_entries
        || capabilities.reveal_value
}

pub(super) fn memory_page_request(
    path: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
    byte_limit: Option<usize>,
) -> EphemeralInspectionPageRequest {
    EphemeralInspectionPageRequest::new(
        memory_query_path(path),
        cursor.filter(|value| !value.is_empty()),
        limit,
        byte_limit,
    )
}

pub(super) fn memory_query_path(path: Option<String>) -> Vec<String> {
    path.unwrap_or_default()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(super) fn memory_path_has_prefix(path: &[String], prefix: &[String]) -> bool {
    path.len() >= prefix.len()
        && path
            .iter()
            .zip(prefix.iter())
            .all(|(left, right)| left == right)
}

pub(super) fn summarize_memory_entries_at_path(
    entries: Vec<EphemeralEntrySnapshot>,
    inspection_path: &[String],
) -> EphemeralInspectionSummarySnapshot {
    let mut summary = EphemeralInspectionSummarySnapshot::empty();

    for entry in entries
        .into_iter()
        .filter(|entry| memory_path_has_prefix(&entry.inspection_path, inspection_path))
    {
        summary.entry_count += 1;
        summary.sensitive_entry_count += u64::from(entry.sensitive);
        summary.total_value_size_bytes += entry.value_size_bytes;
    }

    summary
}

pub(super) fn empty_memory_entry_page(
    request: EphemeralInspectionPageRequest,
) -> EphemeralInspectionEntryPage {
    EphemeralInspectionEntryPage {
        inspection_path: request.inspection_path,
        entries: Vec::new(),
        next_cursor: None,
        limit: request.limit as u64,
        byte_limit: request.byte_limit as u64,
        emitted_bytes: 0,
        truncated_by_byte_limit: false,
    }
}

pub(super) fn empty_memory_tree_page(
    request: EphemeralInspectionPageRequest,
) -> EphemeralInspectionTreePage {
    EphemeralInspectionTreePage {
        inspection_path: request.inspection_path,
        nodes: Vec::new(),
        next_cursor: None,
        limit: request.limit as u64,
        byte_limit: request.byte_limit as u64,
        emitted_bytes: 0,
        truncated_by_byte_limit: false,
    }
}

pub(super) fn parse_memory_reveal_mode(
    reveal_mode: Option<&str>,
) -> Result<EphemeralValueRevealMode, ApiError> {
    match reveal_mode.unwrap_or("preview") {
        "metadata" => Ok(EphemeralValueRevealMode::Metadata),
        "preview" => Ok(EphemeralValueRevealMode::Preview),
        "full" => Ok(EphemeralValueRevealMode::Full),
        _ => Err(ControlPlaneError::InvalidInput("memory_reveal_mode").into()),
    }
}

pub(super) fn format_memory_reveal_mode(reveal_mode: EphemeralValueRevealMode) -> String {
    match reveal_mode {
        EphemeralValueRevealMode::Metadata => "metadata",
        EphemeralValueRevealMode::Preview => "preview",
        EphemeralValueRevealMode::Full => "full",
    }
    .to_string()
}

pub(super) fn format_memory_value_state(
    value_state: control_plane::ports::EphemeralEntryValueState,
) -> String {
    match value_state {
        control_plane::ports::EphemeralEntryValueState::Hidden => "hidden",
        control_plane::ports::EphemeralEntryValueState::Available => "available",
        control_plane::ports::EphemeralEntryValueState::Preview => "preview",
        control_plane::ports::EphemeralEntryValueState::ValueTooLarge => "value_too_large",
    }
    .to_string()
}

pub(super) async fn memory_contract_summary(
    dependencies: &MemoryInspectionDependencies,
    contract_code: &str,
    label: &str,
) -> Result<MemoryContractSummaryResponse, ApiError> {
    let target = memory_inspection_target(dependencies, contract_code)?;
    let capabilities = target.capabilities();
    let supported = memory_contract_supported(&capabilities);

    Ok(MemoryContractSummaryResponse {
        contract_code: contract_code.to_string(),
        label: label.to_string(),
        provider_code: dependencies.provider_codes.get(contract_code).cloned(),
        capabilities: capabilities.into(),
        supported,
    })
}

pub(super) async fn memory_contract_stats_response(
    dependencies: &MemoryInspectionDependencies,
    contract_code: &str,
    label: &str,
    inspection_path: &[String],
) -> Result<MemoryStatsResponse, ApiError> {
    let target = memory_inspection_target(dependencies, contract_code)?;
    let capabilities = target.capabilities();
    let supported = memory_contract_supported(&capabilities);
    let summary = if supported {
        target.summarize_entries_at_path(inspection_path).await?
    } else {
        EphemeralInspectionSummarySnapshot::empty()
    };
    Ok(MemoryStatsResponse {
        contract_code: contract_code.to_string(),
        label: label.to_string(),
        provider_code: dependencies.provider_codes.get(contract_code).cloned(),
        capabilities: capabilities.into(),
        supported,
        inspection_path: inspection_path.to_vec(),
        entry_count: summary.entry_count,
        sensitive_entry_count: summary.sensitive_entry_count,
        total_value_size_bytes: summary.total_value_size_bytes,
    })
}
