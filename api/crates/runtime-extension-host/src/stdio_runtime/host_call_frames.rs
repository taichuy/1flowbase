use super::*;

pub(super) async fn handle_host_worker_frame(
    executable_path: &Path,
    stdin: &mut ChildStdin,
    frame: RuntimeHostWorkerFrame,
    context: Option<&ProviderHostCallContext>,
    completion_sender: &tokio::sync::mpsc::UnboundedSender<HostCallCompletion>,
    active: &mut ActiveHostCalls,
) -> FrameworkResult<()> {
    match frame {
        RuntimeHostWorkerFrame::HostCall {
            protocol,
            call_id,
            service,
            request,
        } => {
            validate_host_call_identity(&protocol, &call_id, service.as_str())?;
            request.validate().map_err(host_call_contract_error)?;
            if active.0.contains_key(&call_id) {
                return Err(host_protocol_error("duplicate host call id"));
            }
            let Some(context) = context.cloned() else {
                return write_host_result(
                    executable_path,
                    stdin,
                    HostCallCompletion {
                        call_id,
                        result: Err(plugin_data_error(
                            PluginDataErrorKind::PermissionDenied,
                            "runtime_host_call_not_granted",
                            false,
                        )),
                    },
                )
                .await;
            };
            let completion_call_id = call_id.clone();
            let sender = completion_sender.clone();
            let task = tokio::spawn(async move {
                let remaining_ms = context
                    .binding
                    .deadline_unix_ms
                    .saturating_sub(now_unix_ms());
                let result = if remaining_ms <= 0 {
                    Err(plugin_data_error(
                        PluginDataErrorKind::DeadlineExceeded,
                        "plugin_data_deadline",
                        false,
                    ))
                } else {
                    match tokio::time::timeout(
                        Duration::from_millis(u64::try_from(remaining_ms).unwrap_or(u64::MAX)),
                        context.plugin_data.execute(&context.binding, &request),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => Err(plugin_data_error(
                            PluginDataErrorKind::DeadlineExceeded,
                            "plugin_data_deadline",
                            false,
                        )),
                    }
                };
                let _ = sender.send(HostCallCompletion {
                    call_id: completion_call_id,
                    result,
                });
            });
            active.0.insert(call_id, task);
            Ok(())
        }
        RuntimeHostWorkerFrame::HostCancel { protocol, call_id } => {
            validate_host_call_identity(&protocol, &call_id, PLUGIN_DATA_SERVICE_V1)?;
            let Some(task) = active.0.remove(&call_id) else {
                return Err(host_protocol_error("unknown host call id"));
            };
            task.abort();
            write_host_result(
                executable_path,
                stdin,
                HostCallCompletion {
                    call_id,
                    result: Err(plugin_data_error(
                        PluginDataErrorKind::Cancelled,
                        "plugin_data_cancelled",
                        false,
                    )),
                },
            )
            .await
        }
    }
}

pub(super) fn validate_host_call_identity(
    protocol: &str,
    call_id: &str,
    service: &str,
) -> FrameworkResult<()> {
    if protocol != RUNTIME_HOST_CALL_PROTOCOL_V1 || service != PLUGIN_DATA_SERVICE_V1 {
        return Err(host_protocol_error(
            "unsupported host call protocol or service",
        ));
    }
    if call_id.is_empty()
        || call_id.len() > 128
        || !call_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(host_protocol_error("invalid host call id"));
    }
    Ok(())
}

pub(super) async fn write_host_result(
    executable_path: &Path,
    stdin: &mut ChildStdin,
    completion: HostCallCompletion,
) -> FrameworkResult<()> {
    let (result, error) = match completion.result {
        Ok(result) => (Some(result), None),
        Err(error) => (None, Some(error)),
    };
    let frame = RuntimeHostFrame::HostResult {
        protocol: RUNTIME_HOST_CALL_PROTOCOL_V1.to_string(),
        call_id: completion.call_id,
        result,
        error,
    };
    let mut bytes = serde_json::to_vec(&frame)
        .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
    bytes.push(b'\n');
    stdin
        .write_all(&bytes)
        .await
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))
}

pub(super) fn host_call_contract_error(error: PluginDataError) -> PluginFrameworkError {
    host_protocol_error(&format!("invalid plugin data request: {}", error.code))
}

pub(super) fn host_protocol_error(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "runtime_host_call",
        message,
        None,
    ))
}

pub(super) fn plugin_data_error(
    kind: PluginDataErrorKind,
    code: &'static str,
    retryable: bool,
) -> PluginDataError {
    PluginDataError {
        kind,
        code: code.to_string(),
        retryable,
    }
}

pub(super) fn now_unix_ms() -> i64 {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}
