use super::*;

pub(super) fn lock_provider_worker_registry(
    provider_workers: &ProviderWorkerRegistry,
) -> FrameworkResult<std::sync::MutexGuard<'_, ProviderWorkerRegistryState>> {
    provider_workers.lock().map_err(|_| {
        PluginFrameworkError::invalid_provider_package("provider worker registry is unavailable")
    })
}

pub(super) fn provider_worker_handle(
    provider_workers: &ProviderWorkerRegistry,
    plugin_id: String,
    loaded: &LoadedProviderPackage,
) -> FrameworkResult<ProviderWorkerHandle> {
    let mut registry = lock_provider_worker_registry(provider_workers)?;
    if let Some(worker) = registry.workers.get(&plugin_id).cloned() {
        if worker.snapshot()?.state != ProviderWorkerLifecycleState::Failed {
            return Ok(worker);
        }
        if let Some(receipt) = worker.last_cleanup_receipt()? {
            registry.cleanup_receipts.insert(plugin_id.clone(), receipt);
        }
        registry.workers.remove(&plugin_id);
    }
    let generation = *registry
        .next_generation
        .entry(plugin_id.clone())
        .or_insert(1);
    let supervisor = ProviderWorkerSupervisor::activate(
        loaded.runtime_executable.clone(),
        loaded.package.manifest.runtime.limits.clone(),
        generation,
    )?;
    registry
        .next_generation
        .insert(plugin_id.clone(), generation.saturating_add(1));
    registry.workers.insert(plugin_id, Arc::clone(&supervisor));
    Ok(supervisor)
}

pub(super) fn take_provider_worker_for_quiesce(
    provider_workers: &ProviderWorkerRegistry,
    plugin_id: &str,
) -> FrameworkResult<Option<ProviderWorkerHandle>> {
    let mut registry = lock_provider_worker_registry(provider_workers)?;
    let Some(supervisor) = registry.workers.get(plugin_id).cloned() else {
        return Ok(None);
    };
    supervisor.begin_quiesce()?;
    registry.workers.remove(plugin_id);
    Ok(Some(supervisor))
}

pub(super) fn record_provider_worker_cleanup(
    provider_workers: &ProviderWorkerRegistry,
    plugin_id: &str,
    receipt: ProviderWorkerCleanupReceipt,
) -> FrameworkResult<()> {
    lock_provider_worker_registry(provider_workers)?
        .cleanup_receipts
        .insert(plugin_id.to_string(), receipt);
    Ok(())
}

#[cfg(test)]
pub(super) fn provider_worker_supervisor_snapshot(
    provider_workers: &ProviderWorkerRegistry,
    plugin_id: &str,
) -> FrameworkResult<Option<ProviderWorkerSupervisorSnapshot>> {
    let supervisor = lock_provider_worker_registry(provider_workers)?
        .workers
        .get(plugin_id)
        .cloned();
    supervisor
        .map(|supervisor| supervisor.snapshot())
        .transpose()
}

#[cfg(test)]
pub(super) fn provider_worker_cleanup_receipt(
    provider_workers: &ProviderWorkerRegistry,
    plugin_id: &str,
) -> FrameworkResult<Option<ProviderWorkerCleanupReceipt>> {
    let (supervisor, receipt) = {
        let registry = lock_provider_worker_registry(provider_workers)?;
        (
            registry.workers.get(plugin_id).cloned(),
            registry.cleanup_receipts.get(plugin_id).cloned(),
        )
    };
    match supervisor {
        Some(supervisor) => supervisor
            .last_cleanup_receipt()
            .map(|current| current.or(receipt)),
        None => Ok(receipt),
    }
}

pub(super) fn provider_invocation_limits(limits: &PluginRuntimeLimits) -> PluginRuntimeLimits {
    let mut invocation_limits = limits.clone();
    invocation_limits.timeout_ms = limits
        .invoke_timeout_ms
        .or(Some(DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS));
    invocation_limits
}

pub(super) fn provider_pool_key(input: &ProviderInvocationInput) -> String {
    format!(
        "provider_pool:v1:provider_instance={}:provider_code={}:protocol={}:model={}",
        stable_pool_component(&input.provider_instance_id),
        stable_pool_component(&input.provider_code),
        stable_pool_component(&input.protocol),
        stable_pool_component(&input.model),
    )
}

pub(super) fn stable_pool_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
pub(super) fn provider_stream_transport(input: &ProviderInvocationInput) -> String {
    if let Some(transport_mode) = provider_config_transport_mode(&input.provider_config) {
        return normalize_transport_mode_hint(&transport_mode);
    }
    if input.protocol == "openai_responses" || input.provider_code == "openai" {
        return "http_sse".to_string();
    }
    "provider_stream".to_string()
}

#[cfg(test)]
pub(super) fn provider_config_transport_mode(provider_config: &Value) -> Option<String> {
    let value = provider_config.get("transport_mode")?;
    let text = match value {
        Value::String(text) => text.trim().to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
pub(super) fn normalize_transport_mode_hint(transport_mode: &str) -> String {
    match transport_mode.trim().to_ascii_lowercase().as_str() {
        "" => "http_sse".to_string(),
        "sse" | "http" | "http_sse" => "http_sse".to_string(),
        "ws" | "websocket" | "responses_websocket" => "responses_websocket".to_string(),
        "auto" => "auto".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
pub(super) fn elapsed_milliseconds(started_at: OffsetDateTime, now: OffsetDateTime) -> u64 {
    let milliseconds = (now - started_at).whole_milliseconds();
    u64::try_from(milliseconds).unwrap_or(0)
}

#[cfg(test)]
pub(super) fn format_timestamp(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap_or_else(|_| value.to_string())
}

pub(super) fn normalize_models(raw: Value) -> FrameworkResult<Vec<ProviderModelDescriptor>> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

pub(super) fn normalize_balance(raw: Value) -> FrameworkResult<ProviderBalanceResult> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

pub(super) fn normalize_usage_windows(raw: Value) -> FrameworkResult<ProviderUsageWindowsResult> {
    let usage: ProviderUsageWindowsResult = serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))?;
    if usage.queried_at.trim().is_empty() {
        return Err(PluginFrameworkError::invalid_provider_contract(
            "provider usage queried_at must be non-empty",
        ));
    }
    for window in &usage.windows {
        if window.limit_window_seconds == 0 {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider usage limit_window_seconds must be greater than zero",
            ));
        }
        if !window.used_percent.is_finite() || !(0.0..=100.0).contains(&window.used_percent) {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider usage used_percent must be within 0 through 100",
            ));
        }
    }
    Ok(usage)
}

pub(super) fn normalize_reset_credit_result(
    raw: Value,
) -> FrameworkResult<ProviderResetCreditResult> {
    serde_json::from_value(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))
}

pub(super) fn reset_credit_result_matches_operation(
    result: &ProviderResetCreditResult,
    operation: &ProviderResetCreditOperation,
) -> bool {
    matches!(
        (result, operation),
        (
            ProviderResetCreditResult::Count { .. },
            ProviderResetCreditOperation::Count
        ) | (
            ProviderResetCreditResult::Consumed,
            ProviderResetCreditOperation::Consume { .. }
        )
    )
}

pub(super) fn merge_models(
    static_models: &[ProviderModelDescriptor],
    dynamic_models: Vec<ProviderModelDescriptor>,
) -> Vec<ProviderModelDescriptor> {
    let mut merged = BTreeMap::new();
    for model in static_models {
        merged.insert(model.model_id.clone(), model.clone());
    }
    for model in dynamic_models {
        merged.insert(model.model_id.clone(), model);
    }
    merged.into_values().collect()
}
