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
    pub binding: PluginDataBinding,
    pub plugin_data: Arc<dyn PluginDataPort>,
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

    pub fn last_cleanup_receipt(&self) -> Option<&ProviderWorkerCleanupReceipt> {
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
            Ok(Ok(output)) => Ok(output),
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
            required_live_events,
            diagnostic_live_events,
            event_observer,
            None,
        )
        .await
    }

    pub(crate) async fn call_streaming_with_limits_and_host_calls(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
        host_calls: Option<ProviderHostCallContext>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let timeout_ms = provider_invocation_timeout_ms(timeout_limits);
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.call_streaming_inner(
                request,
                timeout_limits,
                required_live_events,
                diagnostic_live_events,
                event_observer,
                host_calls,
            ),
        )
        .await
        {
            Ok(Ok(output)) => Ok(output),
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
            (Some(process), None) => process.control.terminate().await,
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
    ) -> FrameworkResult<Value> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process().await?;
        write_worker_request(&executable_path, &mut process.stdin, request).await?;

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
            return parse_stdio_response_line(&executable_path, trimmed);
        }

        Err(worker_ended_without_output_error())
    }

    async fn call_streaming_inner(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
        event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
        host_calls: Option<ProviderHostCallContext>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process().await?;
        write_worker_request(&executable_path, &mut process.stdin, request).await?;

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
                    host_calls.as_ref(),
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
                        timeout_state.record_stream_event(&event);
                        if let Some(event_observer) = &event_observer {
                            let _ = event_observer.send(());
                        }
                        forward_provider_live_event(
                            required_live_events.as_ref(),
                            diagnostic_live_events.as_ref(),
                            event.clone(),
                        )
                        .await?;
                        events.push(event);
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

    pub(crate) async fn terminate(&self) -> ProviderWorkerTerminationEvidence {
        let mut child = self.child.lock().await;
        let mut kill_sent = false;
        let mut cleanup_error = None;
        let exited = match child.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => {
                kill_sent = true;
                match child.kill().await {
                    Ok(()) => true,
                    Err(error) => {
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

async fn handle_host_worker_frame(
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

fn validate_host_call_identity(
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

async fn write_host_result(
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

fn host_call_contract_error(error: PluginDataError) -> PluginFrameworkError {
    host_protocol_error(&format!("invalid plugin data request: {}", error.code))
}

fn host_protocol_error(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
        "runtime_host_call",
        message,
        None,
    ))
}

fn plugin_data_error(
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

fn now_unix_ms() -> i64 {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
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
        let mut payload = serde_json::to_vec(request)
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

pub async fn call_executable_streaming(
    executable_path: &Path,
    request: &ProviderStdioRequest,
    limits: &PluginRuntimeLimits,
    required_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    diagnostic_live_events: Option<tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    event_observer: Option<tokio::sync::mpsc::UnboundedSender<()>>,
) -> FrameworkResult<StreamingProviderOutput> {
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
        let mut payload = serde_json::to_vec(request)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        payload.push(b'\n');
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    }

    let stdout = child.stdout.take().ok_or_else(|| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_runtime",
            "provider runtime stdout was not captured",
            None,
        ))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_runtime",
            "provider runtime stderr was not captured",
            None,
        ))
    })?;

    let stderr_task = tokio::spawn(async move {
        let mut text = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut text).await;
        text
    });

    let mut lines = BufReader::new(stdout).lines();
    let mut events = Vec::new();
    let mut result = None;
    let mut timeout_state = ProviderStreamTimeoutState::new();

    while let Some(line) =
        next_provider_stdout_line(&mut lines, executable_path, limits, &mut timeout_state).await?
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let runtime_line =
            serde_json::from_str::<ProviderRuntimeLine>(trimmed).map_err(|error| {
                // Provider output is upstream diagnostic contract. Preserve the observed
                // line on the runtime error path; do not redact, localize, or replace it here.
                PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
                    "invalid_provider_ndjson",
                    format!("invalid provider ndjson: {error}"),
                    Some(trimmed),
                ))
            })?;
        match runtime_line {
            ProviderRuntimeLine::Result { result: value } => {
                result = Some(value);
            }
            other => {
                if let Some(event) = other.into_stream_event() {
                    timeout_state.record_stream_event(&event);
                    if let Some(event_observer) = &event_observer {
                        let _ = event_observer.send(());
                    }
                    forward_provider_live_event(
                        required_live_events.as_ref(),
                        diagnostic_live_events.as_ref(),
                        event.clone(),
                    )
                    .await?;
                    events.push(event);
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    let stderr = stderr_task.await.unwrap_or_default();
    if !status.success() {
        let summary = stderr.trim();
        // Provider stderr is intentionally surfaced as upstream runtime detail.
        // Host code must not rewrite, redact, or collapse it into a generic message.
        return Err(PluginFrameworkError::runtime(
            ProviderRuntimeError::normalize(
                "provider_runtime",
                if summary.is_empty() {
                    "provider runtime exited with failure"
                } else {
                    summary
                },
                None,
            ),
        ));
    }

    let result = result.ok_or_else(|| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "provider_runtime",
            "provider runtime ended without result line",
            None,
        ))
    })?;

    Ok(StreamingProviderOutput { events, result })
}

async fn forward_provider_live_event(
    required: Option<&tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    diagnostic: Option<&tokio::sync::mpsc::Sender<ProviderStreamEvent>>,
    event: ProviderStreamEvent,
) -> FrameworkResult<()> {
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
) -> FrameworkResult<()> {
    let mut payload = serde_json::to_vec(request)
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
    let envelope = serde_json::from_str::<ProviderStdioResponse>(line).map_err(|error| {
        PluginFrameworkError::serialization(Some(executable_path), error.to_string())
    })?;

    if envelope.ok {
        return Ok(envelope.result);
    }

    let error = envelope.error.unwrap_or_else(|| ProviderStdioError {
        kind: ProviderRuntimeErrorKind::ProviderInvalidResponse,
        message: "provider runtime execution failed".to_string(),
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn reasoning_signature_uses_required_live_lane_and_preserves_order_at_capacity_one() {
        use extension_package_runtime::provider_contract::{ProviderFinishReason, ProviderUsage};

        let (required, mut required_receiver) = tokio::sync::mpsc::channel(1);
        let (diagnostic, _diagnostic_receiver) = tokio::sync::mpsc::channel(1);
        let expected = vec![
            ProviderStreamEvent::TextDelta {
                delta: "same".to_string(),
            },
            ProviderStreamEvent::TextDelta {
                delta: "same".to_string(),
            },
            ProviderStreamEvent::ReasoningSignatureDelta {
                signature: "opaque-signature-fixture".to_string(),
            },
            ProviderStreamEvent::ToolCallDelta {
                call_id: "call-1".to_string(),
                delta: serde_json::json!({"arguments":"{}"}),
            },
            ProviderStreamEvent::UsageDelta {
                usage: ProviderUsage::default(),
            },
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            },
        ];
        let produced = expected.clone();
        let producer = tokio::spawn(async move {
            for event in produced {
                forward_provider_live_event(Some(&required), Some(&diagnostic), event)
                    .await
                    .unwrap();
            }
        });

        let mut received = Vec::new();
        while received.len() < expected.len() {
            tokio::task::yield_now().await;
            received.push(required_receiver.recv().await.unwrap());
        }
        producer.await.unwrap();

        assert_eq!(received, expected);
    }

    #[tokio::test]
    async fn saturated_diagnostic_lane_does_not_block_required_live_events() {
        let (required, mut required_receiver) = tokio::sync::mpsc::channel(1);
        let (diagnostic, _diagnostic_receiver) = tokio::sync::mpsc::channel(1);
        let native = ProviderStreamEvent::NativeEvent {
            protocol: "fixture".to_string(),
            event: serde_json::json!({"progress":1}),
        };
        forward_provider_live_event(Some(&required), Some(&diagnostic), native.clone())
            .await
            .unwrap();
        forward_provider_live_event(Some(&required), Some(&diagnostic), native)
            .await
            .unwrap();

        let required_event = ProviderStreamEvent::ReasoningDelta {
            delta: "truth".to_string(),
        };
        forward_provider_live_event(Some(&required), Some(&diagnostic), required_event.clone())
            .await
            .unwrap();

        assert_eq!(required_receiver.recv().await, Some(required_event));
    }

    fn assert_expired_timeout_contract(
        timeout_state: ProviderStreamTimeoutState,
        limits: PluginRuntimeLimits,
        expected_kind: ProviderTimeoutKind,
        expected_timeout_ms: u64,
    ) {
        let (duration, timeout_kind, timeout_ms) = timeout_state.next_read_timeout(&limits);
        assert!(duration.is_zero());
        assert_eq!(timeout_kind, expected_kind);
        assert_eq!(timeout_ms, expected_timeout_ms);

        let error = provider_timeout_error(timeout_kind, timeout_ms);
        let PluginFrameworkError::RuntimeContract { error } = error else {
            panic!("expected provider runtime contract error, got {error:?}");
        };
        assert!(error.message.contains("provider runtime timed out"));
        let expected_summary = format!(
            "timeout_kind={};timeout_ms={timeout_ms}",
            timeout_kind.as_str()
        );
        assert_eq!(
            error.provider_summary.as_deref(),
            Some(expected_summary.as_str())
        );
    }

    #[test]
    fn expired_wall_clock_budget_produces_wall_clock_contract_error() {
        assert_expired_timeout_contract(
            ProviderStreamTimeoutState {
                started_at: Instant::now() - Duration::from_millis(200),
                first_token_seen: false,
                last_stream_event_at: None,
            },
            PluginRuntimeLimits {
                timeout_ms: Some(100),
                ..Default::default()
            },
            ProviderTimeoutKind::WallClock,
            100,
        );
    }

    #[test]
    fn expired_first_token_budget_produces_first_token_contract_error() {
        assert_expired_timeout_contract(
            ProviderStreamTimeoutState {
                started_at: Instant::now() - Duration::from_millis(200),
                first_token_seen: false,
                last_stream_event_at: None,
            },
            PluginRuntimeLimits {
                timeout_ms: Some(2_000),
                first_token_timeout_ms: Some(100),
                ..Default::default()
            },
            ProviderTimeoutKind::FirstToken,
            100,
        );
    }

    #[test]
    fn expired_stream_idle_budget_produces_stream_idle_contract_error() {
        assert_expired_timeout_contract(
            ProviderStreamTimeoutState {
                started_at: Instant::now() - Duration::from_millis(200),
                first_token_seen: true,
                last_stream_event_at: Some(Instant::now() - Duration::from_millis(200)),
            },
            PluginRuntimeLimits {
                timeout_ms: Some(2_000),
                stream_idle_timeout_ms: Some(100),
                ..Default::default()
            },
            ProviderTimeoutKind::StreamIdle,
            100,
        );
    }
}
