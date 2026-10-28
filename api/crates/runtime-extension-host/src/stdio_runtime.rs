use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use extension_contracts::{
    PluginDataBinding, PluginDataError, PluginDataErrorKind, PluginDataPort, RuntimeHostFrame,
    RuntimeHostWorkerFrame, PLUGIN_DATA_SERVICE_V1, RUNTIME_HOST_CALL_PROTOCOL_V1,
};

use extension_package_runtime::{
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::{
        ProviderInvocationResult, ProviderRuntimeError, ProviderRuntimeErrorKind,
        ProviderRuntimeLine, ProviderStdioError, ProviderStdioRequest, ProviderStdioResponse,
        ProviderStreamEvent,
    },
    PluginRuntimeLimits,
};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};

mod cancellation;
mod multiplex;
mod streaming;
pub(crate) use cancellation::ProviderCallCancellation;
pub(crate) use multiplex::MultiplexProviderWorker;
pub(crate) use streaming::call_executable_streaming;
use streaming::ProviderStreamOutcome;

pub const DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS: u64 = 300_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderWorkerLifecycleState {
    Activating,
    Active,
    Quiescing,
    Inactive,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderWorkerCleanupReason {
    Drained,
    DeadlineExceeded,
    RuntimeFailure,
    Restarted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderWorkerCleanupReceipt {
    pub generation: u64,
    pub prior_pid: Option<u32>,
    pub kill_sent: bool,
    pub exited: bool,
    pub final_state: ProviderWorkerLifecycleState,
    pub reason: ProviderWorkerCleanupReason,
    pub cleanup_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamingProviderOutput {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
}

#[derive(Clone)]
pub(crate) struct ProviderHostCallContext {
    pub application_logs: Option<(
        runtime_core::runtime_backend::ApplicationLogsBinding,
        Arc<dyn runtime_core::runtime_backend::ApplicationLogsPort>,
    )>,
    pub http_egress: Option<(
        runtime_core::runtime_backend::HttpEgressBinding,
        Arc<dyn runtime_core::runtime_backend::HttpEgressPort>,
    )>,

    pub binding: PluginDataBinding,
    pub plugin_data: Arc<dyn PluginDataPort>,
    pub file_read: Option<super::file_read_host::FileReadCall>,
    pub file_import: Option<(
        runtime_core::runtime_backend::FileImportBinding,
        Arc<dyn runtime_core::runtime_backend::FileImportPort>,
    )>,
    pub plugin_credentials: Option<(
        extension_contracts::PluginCredentialBinding,
        Arc<dyn extension_contracts::PluginCredentialPort>,
    )>,
}

struct HostCallCompletion {
    call_id: String,
    result: Result<extension_contracts::PluginDataResponse, PluginDataError>,
}

#[derive(Default)]
struct ActiveHostCalls(HashMap<String, tokio::task::JoinHandle<()>>);

impl Drop for ActiveHostCalls {
    fn drop(&mut self) {
        for (_, task) in self.0.drain() {
            task.abort();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderTimeoutKind {
    WallClock,
    FirstToken,
    StreamIdle,
}

impl ProviderTimeoutKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WallClock => "wall_clock",
            Self::FirstToken => "first_token",
            Self::StreamIdle => "stream_idle",
        }
    }
}

#[derive(Debug)]
pub struct ProviderWorker {
    executable_path: PathBuf,
    limits: PluginRuntimeLimits,
    process: Option<ProviderWorkerProcess>,
    last_cleanup_receipt: Option<ProviderWorkerCleanupReceipt>,
}

#[derive(Debug)]
struct ProviderWorkerProcess {
    control: ProviderWorkerProcessControl,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

#[derive(Clone)]
pub(crate) struct StreamingCallContext {
    pub(crate) required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    pub(crate) diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    pub(crate) protocol_observation:
        Option<Arc<dyn runtime_core::runtime_backend::RuntimeProtocolObservationSink>>,
    pub(crate) event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    pub(crate) host_calls: Option<ProviderHostCallContext>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderWorkerProcessControl {
    child: Arc<Mutex<Child>>,
    pid: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderWorkerTerminationEvidence {
    prior_pid: Option<u32>,
    kill_sent: bool,
    exited: bool,
    cleanup_error: Option<String>,
}

impl ProviderWorker {
    pub(crate) async fn call_with_host_calls(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        host_calls: Option<ProviderHostCallContext>,
    ) -> FrameworkResult<Value> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process().await?;
        write_worker_request(&executable_path, &mut process.stdin, request, false).await?;
        let mut timeout_state = ProviderStreamTimeoutState::new();
        let (completion_sender, mut completion_receiver) =
            tokio::sync::mpsc::unbounded_channel::<HostCallCompletion>();
        let mut active_host_calls = ActiveHostCalls::default();
        loop {
            let line = tokio::select! {
                completion = completion_receiver.recv(), if !active_host_calls.0.is_empty() => {
                    if let Some(completion) = completion {
                        if active_host_calls.0.remove(&completion.call_id).is_some() {
                            write_host_result(&executable_path, &mut process.stdin, completion).await?;
                        }
                    }
                    continue;
                }
                line = next_provider_stdout_line(
                    &mut process.stdout,
                    &executable_path,
                    &timeout_limits,
                    &mut timeout_state,
                ) => line?,
            };
            let Some(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(frame) = serde_json::from_str::<RuntimeHostWorkerFrame>(trimmed) {
                handle_host_worker_frame(
                    &executable_path,
                    &mut process.stdin,
                    frame,
                    host_calls.as_ref(),
                    &completion_sender,
                    &mut active_host_calls,
                )
                .await?;
                continue;
            }
            return parse_stdio_response_line(&executable_path, trimmed);
        }
        Err(worker_ended_without_output_error())
    }

    pub fn new(executable_path: PathBuf, limits: PluginRuntimeLimits) -> Self {
        Self {
            executable_path,
            limits,
            process: None,
            last_cleanup_receipt: None,
        }
    }

    pub fn activate(&mut self) -> FrameworkResult<Option<u32>> {
        if self.process.is_some() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "provider worker process is already active",
            ));
        }
        self.last_cleanup_receipt = None;
        self.process = Some(spawn_worker_process(&self.executable_path, &self.limits)?);
        Ok(self
            .process
            .as_ref()
            .and_then(|process| process.control.pid))
    }

    pub(crate) fn process_control(&self) -> Option<ProviderWorkerProcessControl> {
        self.process.as_ref().map(|process| process.control.clone())
    }

    pub(crate) fn last_cleanup_receipt(&self) -> Option<&ProviderWorkerCleanupReceipt> {
        self.last_cleanup_receipt.as_ref()
    }

    pub(crate) fn take_last_cleanup_receipt(&mut self) -> Option<ProviderWorkerCleanupReceipt> {
        self.last_cleanup_receipt.take()
    }

    pub async fn call(&mut self, request: &ProviderStdioRequest) -> FrameworkResult<Value> {
        let limits = self.limits.clone();
        self.call_with_limits(request, &limits).await
    }

    pub async fn call_with_limits(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
    ) -> FrameworkResult<Value> {
        let timeout_ms = provider_invocation_timeout_ms(timeout_limits);
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.call_inner(request, timeout_limits),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                let receipt = self
                    .stop_with_reason(
                        0,
                        ProviderWorkerCleanupReason::RuntimeFailure,
                        ProviderWorkerLifecycleState::Failed,
                    )
                    .await;
                self.last_cleanup_receipt = Some(receipt);
                Err(error)
            }
            Err(_) => {
                let receipt = self
                    .stop_with_reason(
                        0,
                        ProviderWorkerCleanupReason::RuntimeFailure,
                        ProviderWorkerLifecycleState::Failed,
                    )
                    .await;
                self.last_cleanup_receipt = Some(receipt);
                Err(provider_timeout_error(
                    ProviderTimeoutKind::WallClock,
                    timeout_ms,
                ))
            }
        }
    }

    #[cfg(test)]
    pub async fn call_streaming(
        &mut self,
        request: &ProviderStdioRequest,
        required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let limits = self.limits.clone();
        self.call_streaming_with_limits(
            request,
            &limits,
            required_live_events,
            diagnostic_live_events,
            event_observer,
        )
        .await
    }

    #[cfg(test)]
    pub async fn call_streaming_with_limits(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        self.call_streaming_with_limits_and_host_calls(
            request,
            timeout_limits,
            StreamingCallContext {
                required_live_events,
                diagnostic_live_events,
                protocol_observation: None,
                event_observer,
                host_calls: None,
            },
        )
        .await
    }

    pub(crate) async fn call_streaming_with_limits_and_host_calls(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: StreamingCallContext,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let timeout_ms = provider_invocation_timeout_ms(timeout_limits);
        // Lives outside the cancellable future: timeout and every secondary `?`
        // must preserve a provider error already observed on the stream.
        let mut outcome = ProviderStreamOutcome::default();
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.call_streaming_inner(request, timeout_limits, context, &mut outcome),
        )
        .await
        {
            Ok(Ok(output)) => outcome.finish(Ok(output)),
            Ok(Err(error)) => {
                let receipt = self
                    .stop_with_reason(
                        0,
                        ProviderWorkerCleanupReason::RuntimeFailure,
                        ProviderWorkerLifecycleState::Failed,
                    )
                    .await;
                self.last_cleanup_receipt = Some(receipt);
                outcome.finish(Err(error))
            }
            Err(_) => {
                let receipt = self
                    .stop_with_reason(
                        0,
                        ProviderWorkerCleanupReason::RuntimeFailure,
                        ProviderWorkerLifecycleState::Failed,
                    )
                    .await;
                self.last_cleanup_receipt = Some(receipt);
                outcome.finish(Err(provider_timeout_error(
                    ProviderTimeoutKind::WallClock,
                    timeout_ms,
                )))
            }
        }
    }

    pub async fn stop(&mut self) -> ProviderWorkerCleanupReceipt {
        self.stop_with_reason(
            0,
            ProviderWorkerCleanupReason::Drained,
            ProviderWorkerLifecycleState::Inactive,
        )
        .await
    }

    pub(crate) async fn stop_with_reason(
        &mut self,
        generation: u64,
        reason: ProviderWorkerCleanupReason,
        final_state: ProviderWorkerLifecycleState,
    ) -> ProviderWorkerCleanupReceipt {
        self.stop_with_evidence(generation, reason, final_state, None)
            .await
    }

    pub(crate) async fn stop_with_evidence(
        &mut self,
        generation: u64,
        reason: ProviderWorkerCleanupReason,
        final_state: ProviderWorkerLifecycleState,
        evidence: Option<ProviderWorkerTerminationEvidence>,
    ) -> ProviderWorkerCleanupReceipt {
        let evidence = match (self.process.take(), evidence) {
            (_, Some(evidence)) => evidence,
            (Some(process), None) => {
                let control = process.control.clone();
                drop(process.stdin);
                control.wait_for_graceful_exit().await
            }
            (None, None) => ProviderWorkerTerminationEvidence {
                prior_pid: None,
                kill_sent: false,
                exited: true,
                cleanup_error: None,
            },
        };
        ProviderWorkerCleanupReceipt {
            generation,
            prior_pid: evidence.prior_pid,
            kill_sent: evidence.kill_sent,
            exited: evidence.exited,
            final_state,
            reason,
            cleanup_error: evidence.cleanup_error,
        }
    }

    async fn call_inner(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
    ) -> FrameworkResult<FrameworkResult<Value>> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process().await?;
        write_worker_request(&executable_path, &mut process.stdin, request, false).await?;

        let mut timeout_state = ProviderStreamTimeoutState::new();
        while let Some(line) = next_provider_stdout_line(
            &mut process.stdout,
            &executable_path,
            &timeout_limits,
            &mut timeout_state,
        )
        .await?
        {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // A complete valid unary envelope restores the request/response boundary,
            // even when its business result is an error. No correlation IDs exist
            // for an incomplete/late response, so those failures retire the process.
            let envelope = parse_stdio_response_envelope(&executable_path, trimmed)?;
            return Ok(stdio_response_result(envelope));
        }

        Err(worker_ended_without_output_error())
    }

    async fn call_streaming_inner(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: StreamingCallContext,
        outcome: &mut ProviderStreamOutcome,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process().await?;
        write_worker_request(
            &executable_path,
            &mut process.stdin,
            request,
            context.protocol_observation.is_some(),
        )
        .await?;

        let mut events = Vec::new();
        let mut result = None;

        let mut timeout_state = ProviderStreamTimeoutState::new();
        let (completion_sender, mut completion_receiver) =
            tokio::sync::mpsc::unbounded_channel::<HostCallCompletion>();
        let mut active_host_calls = ActiveHostCalls::default();
        loop {
            let line = tokio::select! {
                completion = completion_receiver.recv(), if !active_host_calls.0.is_empty() => {
                    if let Some(completion) = completion {
                        if active_host_calls.0.remove(&completion.call_id).is_some() {
                            write_host_result(&executable_path, &mut process.stdin, completion).await?;
                        }
                    }
                    continue;
                }
                line = next_provider_stdout_line(
                    &mut process.stdout,
                    &executable_path,
                    &timeout_limits,
                    &mut timeout_state,
                ) => line?,
            };
            let Some(line) = line else { break };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(frame) = serde_json::from_str::<RuntimeHostWorkerFrame>(trimmed) {
                handle_host_worker_frame(
                    &executable_path,
                    &mut process.stdin,
                    frame,
                    context.host_calls.as_ref(),
                    &completion_sender,
                    &mut active_host_calls,
                )
                .await?;
                continue;
            }

            let runtime_line =
                serde_json::from_str::<ProviderRuntimeLine>(trimmed).map_err(|error| {
                    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
                        "invalid_provider_ndjson",
                        format!("invalid provider ndjson: {error}"),
                        Some(trimmed),
                    ))
                })?;
            match runtime_line {
                ProviderRuntimeLine::Result { result: value } => {
                    result = Some(value);
                    break;
                }
                other => {
                    if let Some(event) = other.into_stream_event() {
                        // Capture traffic is independent of provider progress and idle timers.
                        if matches!(event, ProviderStreamEvent::ProtocolObservation { .. }) {
                            forward_provider_live_event(
                                None,
                                None,
                                context.protocol_observation.as_deref(),
                                event,
                            )
                            .await?;
                            continue;
                        }
                        outcome.observe(&event);
                        timeout_state.record_stream_event(&event);
                        if let Some(event_observer) = &context.event_observer {
                            let _ = event_observer.send(());
                        }
                        forward_provider_live_event(
                            context.required_live_events.as_ref(),
                            context.diagnostic_live_events.as_ref(),
                            context.protocol_observation.as_deref(),
                            event.clone(),
                        )
                        .await?;
                        // Formal observations are persisted by their independent sidecar owner; keeping
                        // raw bodies in invocation output would duplicate them in node debug data.
                        if !matches!(event, ProviderStreamEvent::ProtocolObservation { .. }) {
                            events.push(event);
                        }
                    }
                }
            }
        }

        let result = result.ok_or_else(worker_ended_without_result_error)?;
        Ok(StreamingProviderOutput { events, result })
    }

    async fn ensure_process(&mut self) -> FrameworkResult<&mut ProviderWorkerProcess> {
        if self.process.is_none() {
            self.activate()?;
        }
        let process = self.process.as_mut().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package("provider worker is not active")
        })?;
        let exited = process
            .control
            .child
            .lock()
            .await
            .try_wait()
            .map_err(|error| {
                PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
            })?
            .is_some();
        if exited {
            return Err(worker_exited_error());
        }
        Ok(process)
    }
}

impl ProviderWorkerProcessControl {
    pub(crate) fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// pidfd readiness reports exit; Child::try_wait remains the sole reaper.
    pub(crate) async fn wait_for_exit_notification(&self) {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::{FromRawFd, OwnedFd};
            if let Some(pid) = self.pid {
                let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
                if fd >= 0 {
                    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
                    // SAFETY: OwnedFd owns the newly opened pidfd exclusively and keeps
                    // the same descriptor open until AsyncFd releases or drops it.
                    if let Ok(fd) = unsafe { tokio::io::unix::AsyncFd::register(fd) } {
                        if fd.readable().await.is_ok() {
                            return;
                        }
                    }
                }
            }
        }
        // Older kernels, unsupported platforms and unavailable pidfds cannot
        // expose exit readiness. Keep their existing bounded observation fallback.
        tracing::debug!(pid = ?self.pid, "pidfd unavailable; using child exit polling fallback");
        loop {
            if self.confirmed_exit().await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub(crate) async fn confirmed_exit(&self) -> bool {
        self.child.lock().await.try_wait().ok().flatten().is_some()
    }

    pub(crate) async fn wait_for_graceful_exit(&self) -> ProviderWorkerTerminationEvidence {
        let exited = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if self.confirmed_exit().await {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .is_ok();
        if exited {
            ProviderWorkerTerminationEvidence {
                prior_pid: self.pid,
                kill_sent: false,
                exited: true,
                cleanup_error: None,
            }
        } else {
            self.terminate().await
        }
    }

    pub(crate) async fn terminate(&self) -> ProviderWorkerTerminationEvidence {
        let mut child = self.child.lock().await;
        let mut kill_sent = false;
        let mut cleanup_error = None;
        let exited = match child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => {
                kill_sent = true;
                match tokio::time::timeout(Duration::from_secs(2), child.kill()).await {
                    Ok(Ok(())) => true,
                    Err(_) => {
                        cleanup_error = Some("provider kill/reap deadline exceeded".to_string());
                        child.try_wait().ok().flatten().is_some()
                    }
                    Ok(Err(error)) => {
                        cleanup_error = Some(error.to_string());
                        child.try_wait().ok().flatten().is_some()
                    }
                }
            }
            Err(error) => {
                cleanup_error = Some(error.to_string());
                false
            }
        };
        ProviderWorkerTerminationEvidence {
            prior_pid: self.pid,
            kill_sent,
            exited,
            cleanup_error,
        }
    }
}

pub async fn call_executable(
    executable_path: &Path,
    request: &ProviderStdioRequest,
    limits: &PluginRuntimeLimits,
) -> FrameworkResult<Value> {
    let mut command = Command::new(executable_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    apply_memory_limit(&mut command, limits.memory_bytes)?;

    let mut child = command
        .spawn()
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        let mut payload = serialize_provider_stdio_request(request, false)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        payload.push(b'\n');
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    }

    let timeout_ms = provider_invocation_timeout_ms(limits);
    let output = tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
        .await
        .map_err(|_| provider_timeout_error(ProviderTimeoutKind::WallClock, timeout_ms))?
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;

    parse_stdio_response(executable_path, &output.stdout, &output.stderr)
}

/// Negotiate additive events outside the strict ProviderInvocationInput schema.
/// Older providers ignore unknown outer request fields; callers cannot supply this field.
fn serialize_provider_stdio_request(
    request: &ProviderStdioRequest,
    capture_protocol: bool,
) -> serde_json::Result<Vec<u8>> {
    let mut wire = serde_json::to_value(request)?;
    if capture_protocol
        && matches!(
            request.method,
            extension_contracts::provider_contract::ProviderStdioMethod::Invoke
        )
        && matches!(
            request.input.get("operation").and_then(Value::as_str),
            None | Some("generate")
        )
    {
        wire[extension_contracts::provider_contract::PROVIDER_HOST_CAPABILITIES_FIELD] =
            serde_json::json!([
                extension_contracts::provider_contract::PROVIDER_PROTOCOL_OBSERVATION_CAPABILITY
            ]);
    }
    serde_json::to_vec(&wire)
}

async fn forward_provider_live_event(
    required: Option<&tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    diagnostic: Option<&tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    protocol_observation: Option<
        &dyn runtime_core::runtime_backend::RuntimeProtocolObservationSink,
    >,
    event: ProviderStreamEvent,
) -> FrameworkResult<()> {
    if matches!(event, ProviderStreamEvent::ProtocolObservation { .. }) {
        if let Some(sink) = protocol_observation {
            sink.observe(event);
        }
        return Ok(());
    }
    if matches!(event, ProviderStreamEvent::NativeEvent { .. }) {
        if let Some(diagnostic) = diagnostic {
            let _ = diagnostic.try_send(event);
        }
        return Ok(());
    }

    if let Some(required) = required {
        required.send(event).await.map_err(|_| {
            PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
                "provider_live_event_lane_closed",
                "required provider live event lane closed",
                None,
            ))
        })?;
    }
    Ok(())
}

fn spawn_worker_process(
    executable_path: &Path,
    limits: &PluginRuntimeLimits,
) -> FrameworkResult<ProviderWorkerProcess> {
    let mut command = Command::new(executable_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    apply_memory_limit(&mut command, limits.memory_bytes)?;

    let mut child = command
        .spawn()
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    let stdin = child.stdin.take().ok_or_else(|| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_runtime",
            "provider worker stdin was not captured",
            None,
        ))
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_runtime",
            "provider worker stdout was not captured",
            None,
        ))
    })?;

    let pid = child.id();
    Ok(ProviderWorkerProcess {
        control: ProviderWorkerProcessControl {
            child: Arc::new(Mutex::new(child)),
            pid,
        },
        stdin,
        stdout: BufReader::new(stdout).lines(),
    })
}

async fn write_worker_request(
    executable_path: &Path,
    stdin: &mut ChildStdin,
    request: &ProviderStdioRequest,
    capture_protocol: bool,
) -> FrameworkResult<()> {
    let mut payload = serialize_provider_stdio_request(request, capture_protocol)
        .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
    payload.push(b'\n');
    stdin
        .write_all(&payload)
        .await
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))
}

fn parse_stdio_response_line(executable_path: &Path, line: &str) -> FrameworkResult<Value> {
    stdio_response_result(parse_stdio_response_envelope(executable_path, line)?)
}

fn parse_stdio_response_envelope(
    executable_path: &Path,
    line: &str,
) -> FrameworkResult<ProviderStdioResponse> {
    let envelope = serde_json::from_str::<ProviderStdioResponse>(line).map_err(|error| {
        PluginFrameworkError::serialization(Some(executable_path), error.to_string())
    })?;
    if envelope.ok == envelope.error.is_some() {
        return Err(PluginFrameworkError::invalid_provider_contract(
            "provider unary response has inconsistent ok/error fields",
        ));
    }
    Ok(envelope)
}

fn stdio_response_result(envelope: ProviderStdioResponse) -> FrameworkResult<Value> {
    match envelope.error {
        None => Ok(envelope.result),
        Some(error) => Err(PluginFrameworkError::runtime(ProviderRuntimeError {
            kind: error.kind,
            message: error.message,
            provider_summary: error.provider_summary,
            provider_details: error.provider_details,
        })),
    }
}

pub(crate) fn multiplex_response_value(envelope: ProviderStdioResponse) -> FrameworkResult<Value> {
    if envelope.ok == envelope.error.is_some() {
        return Err(PluginFrameworkError::invalid_provider_contract(
            "provider multiplex response has inconsistent ok/error fields",
        ));
    }
    stdio_response_result(envelope)
}

struct ProviderStreamTimeoutState {
    started_at: Instant,
    first_token_seen: bool,
    last_stream_event_at: Option<Instant>,
}

impl ProviderStreamTimeoutState {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            first_token_seen: false,
            last_stream_event_at: None,
        }
    }

    fn record_stream_event(&mut self, event: &ProviderStreamEvent) {
        let now = Instant::now();
        if matches!(
            event,
            ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. }
        ) {
            self.first_token_seen = true;
        }
        self.last_stream_event_at = Some(now);
    }

    fn next_read_timeout(
        &self,
        limits: &PluginRuntimeLimits,
    ) -> (Duration, ProviderTimeoutKind, u64) {
        let mut timeout = remaining_timeout(
            self.started_at,
            provider_invocation_timeout_ms(limits),
            ProviderTimeoutKind::WallClock,
        );
        if !self.first_token_seen {
            if let Some(timeout_ms) = limits.first_token_timeout_ms {
                timeout = earlier_timeout(
                    timeout,
                    remaining_timeout(self.started_at, timeout_ms, ProviderTimeoutKind::FirstToken),
                );
            }
        }
        if let (Some(last_stream_event_at), Some(timeout_ms)) =
            (self.last_stream_event_at, limits.stream_idle_timeout_ms)
        {
            timeout = earlier_timeout(
                timeout,
                remaining_timeout(
                    last_stream_event_at,
                    timeout_ms,
                    ProviderTimeoutKind::StreamIdle,
                ),
            );
        }
        timeout
    }
}

async fn next_provider_stdout_line(
    lines: &mut Lines<BufReader<ChildStdout>>,
    executable_path: &Path,
    limits: &PluginRuntimeLimits,
    timeout_state: &mut ProviderStreamTimeoutState,
) -> FrameworkResult<Option<String>> {
    let (duration, timeout_kind, timeout_ms) = timeout_state.next_read_timeout(limits);
    if duration.is_zero() {
        return Err(provider_timeout_error(timeout_kind, timeout_ms));
    }

    tokio::time::timeout(duration, lines.next_line())
        .await
        .map_err(|_| provider_timeout_error(timeout_kind, timeout_ms))?
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))
}

fn provider_invocation_timeout_ms(limits: &PluginRuntimeLimits) -> u64 {
    limits
        .timeout_ms
        .unwrap_or(DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS)
}

fn remaining_timeout(
    started_at: Instant,
    timeout_ms: u64,
    timeout_kind: ProviderTimeoutKind,
) -> (Duration, ProviderTimeoutKind, u64) {
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let remaining_ms = timeout_ms.saturating_sub(elapsed_ms);
    (
        Duration::from_millis(remaining_ms),
        timeout_kind,
        timeout_ms,
    )
}

fn earlier_timeout(
    left: (Duration, ProviderTimeoutKind, u64),
    right: (Duration, ProviderTimeoutKind, u64),
) -> (Duration, ProviderTimeoutKind, u64) {
    if right.0 < left.0 {
        right
    } else {
        left
    }
}

fn provider_timeout_error(
    timeout_kind: ProviderTimeoutKind,
    timeout_ms: u64,
) -> PluginFrameworkError {
    let provider_summary = format!(
        "timeout_kind={};timeout_ms={timeout_ms}",
        timeout_kind.as_str()
    );
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "invoke",
        format!(
            "provider runtime timed out: timeout_kind={} timeout_ms={timeout_ms}",
            timeout_kind.as_str()
        ),
        Some(&provider_summary),
    ))
}

fn worker_ended_without_output_error() -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "provider_runtime",
        "provider worker ended without response line",
        None,
    ))
}

fn worker_exited_error() -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "provider_runtime",
        "provider worker process exited",
        None,
    ))
}

fn worker_ended_without_result_error() -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "provider_runtime",
        "provider worker ended without result line",
        None,
    ))
}

fn parse_stdio_response(
    executable_path: &Path,
    stdout: &[u8],
    stderr: &[u8],
) -> FrameworkResult<Value> {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if stdout.is_empty() {
        return Err(PluginFrameworkError::runtime(
            ProviderRuntimeError::normalize(
                "provider_runtime",
                if stderr.is_empty() {
                    "provider runtime returned empty output"
                } else {
                    stderr.as_str()
                },
                None,
            ),
        ));
    }

    let envelope = serde_json::from_str::<ProviderStdioResponse>(&stdout).map_err(|error| {
        PluginFrameworkError::serialization(Some(executable_path), error.to_string())
    })?;

    if envelope.ok {
        return Ok(envelope.result);
    }

    let error = envelope.error.unwrap_or_else(|| ProviderStdioError {
        kind: ProviderRuntimeErrorKind::ProviderInvalidResponse,
        message: if stderr.is_empty() {
            "provider runtime execution failed".to_string()
        } else {
            stderr.clone()
        },
        provider_summary: None,
        provider_details: None,
    });
    Err(PluginFrameworkError::runtime(ProviderRuntimeError {
        kind: error.kind,
        message: error.message,
        provider_summary: error.provider_summary,
        provider_details: error.provider_details,
    }))
}

fn apply_memory_limit(command: &mut Command, memory_bytes: Option<u64>) -> FrameworkResult<()> {
    #[cfg(unix)]
    {
        if let Some(limit) = memory_bytes {
            unsafe {
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: limit as libc::rlim_t,
                        rlim_max: limit as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (command, memory_bytes);
    }

    Ok(())
}

#[cfg(test)]
#[path = "_tests/stdio_runtime/tests.rs"]
mod tests;

#[path = "stdio_runtime/host_call_frames.rs"]
mod host_call_frames;

use host_call_frames::*;
