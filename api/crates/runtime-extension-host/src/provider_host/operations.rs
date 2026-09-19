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

pub(super) fn provider_invocation_limits(
    limits: &PluginRuntimeLimits,
    _input: &ProviderInvocationInput,
) -> PluginRuntimeLimits {
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

const TRANSPORT_BINDING_CAPACITY: usize = 4096;
const TRANSPORT_BINDING_MAX_RETENTION: std::time::Duration =
    std::time::Duration::from_secs(24 * 60 * 60 + 60);

fn transport_binding_error(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        message,
    ))
}

pub(super) fn bind_transport_worker(
    workers: &ProviderWorkerRegistry,
    plugin_id: &str,
    worker: &ProviderWorkerHandle,
    input: &mut ProviderInvocationInput,
) -> FrameworkResult<()> {
    let Some(mut directive) = input
        .transport_session_directive()
        .map_err(PluginFrameworkError::invalid_provider_contract)?
    else {
        return Ok(());
    };
    let incarnation = worker.incarnation()?;
    let identity = ProviderTransportSessionIdentity {
        logical_session_id: directive.logical_session_id.clone(),
        generation: directive.generation,
        worker_incarnation: incarnation,
    };
    let key = (
        plugin_id.to_owned(),
        identity.logical_session_id.clone(),
        identity.generation,
    );
    let now = std::time::Instant::now();
    let mut registry = lock_provider_worker_registry(workers)?;
    registry.transport_bindings.retain(|_, binding| {
        binding.expires_at > now
            || binding
                .worker
                .snapshot()
                .map_or(true, |snapshot| snapshot.in_flight > 0)
    });
    if let Some(binding) = registry.transport_bindings.get(&key) {
        if binding.identity != identity || !Arc::ptr_eq(&binding.worker, worker) {
            return Err(transport_binding_error(
                "transport generation belongs to a previous worker",
            ));
        }
    } else {
        if registry.transport_bindings.len() >= TRANSPORT_BINDING_CAPACITY {
            return Err(transport_binding_error(
                "transport worker binding capacity exhausted",
            ));
        }
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        if directive.physical_deadline_unix_ms <= unix_ms {
            return Err(transport_binding_error(
                "transport binding physical deadline has expired",
            ));
        }
        let remaining_ms = directive.physical_deadline_unix_ms.saturating_sub(unix_ms) as u64;
        let retention = std::time::Duration::from_millis(remaining_ms)
            .saturating_add(std::time::Duration::from_secs(60))
            .min(TRANSPORT_BINDING_MAX_RETENTION);
        registry.transport_bindings.insert(
            key,
            TransportWorkerBinding {
                identity,
                worker: Arc::clone(worker),
                expires_at: now + retention,
            },
        );
    }
    drop(registry);
    // Overwrite input claims using the actual supervisor, before the first wire request.
    directive.worker_incarnation = Some(incarnation);
    input
        .set_transport_session_directive(directive)
        .map_err(PluginFrameworkError::invalid_provider_contract)
}

fn confirmed_exit_receipt(
    binding: &TransportWorkerBinding,
    action: ProviderTransportSessionAction,
) -> FrameworkResult<Option<ProviderTransportSessionReceipt>> {
    let Some(cleanup) = binding.worker.last_cleanup_receipt()? else {
        return Ok(None);
    };
    if !cleanup.exited || cleanup.generation != binding.identity.worker_incarnation {
        return Ok(None);
    }
    Ok(Some(ProviderTransportSessionReceipt {
        generation: binding.identity.generation,
        reused: false,
        physical_state: ProviderPhysicalTransportState::Closed,
        connection_age_ms: 0,
        ttl_remaining_ms: 0,
        close_reason: Some(match action {
            ProviderTransportSessionAction::Drain => {
                ProviderTransportSessionCloseReason::RequestedDrain
            }
            ProviderTransportSessionAction::Close => {
                ProviderTransportSessionCloseReason::RequestedClose
            }
        }),
        close_acknowledged: None,
        closure_evidence: Some(ProviderTransportClosureEvidence {
            identity: binding.identity.clone(),
            source: ProviderTransportClosureSource::ConfirmedWorkerExit,
            local_released: true,
            peer_close_acknowledged: None,
            no_ack_reason: Some(ProviderTransportNoAckReason::Unknown),
        }),
    }))
}

pub(super) async fn call_bound_transport_session(
    workers: &ProviderWorkerRegistry,
    plugin_id: &str,
    mut command: ProviderTransportSessionCommand,
    limits: &PluginRuntimeLimits,
) -> FrameworkResult<ProviderTransportSessionReceipt> {
    let binding = {
        let mut registry = lock_provider_worker_registry(workers)?;
        let now = std::time::Instant::now();
        registry.transport_bindings.retain(|_, binding| {
            binding.expires_at > now
                || binding
                    .worker
                    .snapshot()
                    .map_or(true, |snapshot| snapshot.in_flight > 0)
        });
        registry
            .transport_bindings
            .get(&(
                plugin_id.to_owned(),
                command.logical_session_id.clone(),
                command.generation,
            ))
            .cloned()
            .ok_or_else(|| {
                transport_binding_error("transport worker binding evidence is unavailable")
            })?
    };
    if command
        .worker_incarnation
        .is_some_and(|value| value != binding.identity.worker_incarnation)
    {
        return Err(transport_binding_error(
            "transport control worker incarnation mismatch",
        ));
    }
    command.worker_incarnation = Some(binding.identity.worker_incarnation);
    if let Some(receipt) = confirmed_exit_receipt(&binding, command.action)? {
        return Ok(receipt);
    }
    let request = ProviderStdioRequest {
        method: ProviderStdioMethod::TransportSession,
        input: serde_json::to_value(&command)
            .map_err(|error| PluginFrameworkError::invalid_provider_contract(error.to_string()))?,
    };
    // Use the original supervisor even if a newer worker is now registered.
    let result = binding
        .worker
        .call_with_deadline(&request, limits, command.deadline_unix_ms)
        .await;
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(session_id = %binding.identity.logical_session_id,
                generation = binding.identity.generation, worker_incarnation = binding.identity.worker_incarnation,
                control_error_kind = ?error.kind(), control_failure_code = safe_control_failure_code(&error), attempt_deadline_ms = command.deadline_unix_ms,
                "provider control failed; checking independently confirmed worker exit");
            return match confirmed_exit_receipt(&binding, command.action) {
                Ok(Some(receipt)) => Ok(receipt),
                Ok(None) | Err(_) => Err(error),
            };
        }
    };
    let receipt: ProviderTransportSessionReceipt =
        serde_json::from_value(output).map_err(|error| {
            PluginFrameworkError::invalid_provider_contract(format!(
                "provider transport receipt is malformed: {error}"
            ))
        })?;
    receipt
        .validate()
        .map_err(PluginFrameworkError::invalid_provider_contract)?;
    if let Some(evidence) = &receipt.closure_evidence {
        if evidence.source != ProviderTransportClosureSource::ProviderLocalRelease
            || evidence.identity != binding.identity
        {
            return Err(transport_binding_error(
                "provider transport closure evidence identity or source rejected",
            ));
        }
    }
    Ok(receipt)
}

fn safe_control_failure_code(error: &PluginFrameworkError) -> &'static str {
    match error {
        PluginFrameworkError::RuntimeContract { error } => match error.message.as_str() {
            "provider worker ended without response line" | "provider worker process exited" => {
                "control_worker_eof"
            }
            "provider transport control deadline exceeded" => "control_queue_deadline",
            message if message.starts_with("provider runtime timed out:") => {
                "control_stdio_timeout"
            }
            _ => "control_provider_rejection",
        },
        PluginFrameworkError::Serialization { .. } => "control_invalid_envelope",
        PluginFrameworkError::Io { .. } => "control_stdio_io_error",
        _ => "control_contract_error",
    }
}
