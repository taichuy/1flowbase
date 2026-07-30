use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use plugin_framework::{
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::{
        ProviderInvocationResult, ProviderRuntimeError, ProviderRuntimeErrorKind,
        ProviderRuntimeLine, ProviderStdioError, ProviderStdioRequest, ProviderStdioResponse,
        ProviderStreamEvent,
    },
    PluginRuntimeLimits,
};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};

pub const DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS: u64 = 300_000;

#[derive(Debug, Clone, PartialEq)]
pub struct StreamingProviderOutput {
    pub events: Vec<ProviderStreamEvent>,
    pub result: ProviderInvocationResult,
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
}

#[derive(Debug)]
struct ProviderWorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl ProviderWorker {
    pub fn new(executable_path: PathBuf, limits: PluginRuntimeLimits) -> Self {
        Self {
            executable_path,
            limits,
            process: None,
        }
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
                self.stop().await;
                Err(error)
            }
            Err(_) => {
                self.stop().await;
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
        let timeout_ms = provider_invocation_timeout_ms(timeout_limits);
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.call_streaming_inner(
                request,
                timeout_limits,
                required_live_events,
                diagnostic_live_events,
                event_observer,
            ),
        )
        .await
        {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(error)) => {
                self.stop().await;
                Err(error)
            }
            Err(_) => {
                self.stop().await;
                Err(provider_timeout_error(
                    ProviderTimeoutKind::WallClock,
                    timeout_ms,
                ))
            }
        }
    }

    pub async fn stop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.child.kill().await;
        }
    }

    async fn call_inner(
        &mut self,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
    ) -> FrameworkResult<Value> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process()?;
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
    ) -> FrameworkResult<StreamingProviderOutput> {
        let executable_path = self.executable_path.clone();
        let timeout_limits = timeout_limits.clone();
        let process = self.ensure_process()?;
        write_worker_request(&executable_path, &mut process.stdin, request).await?;

        let mut events = Vec::new();
        let mut result = None;

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

    fn ensure_process(&mut self) -> FrameworkResult<&mut ProviderWorkerProcess> {
        let should_start = match self.process.as_mut() {
            Some(process) => process
                .child
                .try_wait()
                .map_err(|error| {
                    PluginFrameworkError::io(Some(&self.executable_path), error.to_string())
                })?
                .is_some(),
            None => true,
        };
        if should_start {
            self.process = Some(spawn_worker_process(&self.executable_path, &self.limits)?);
        }
        Ok(self
            .process
            .as_mut()
            .expect("worker process is initialized"))
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

    Ok(ProviderWorkerProcess {
        child,
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
    async fn required_live_lane_preserves_duplicate_text_tool_usage_and_terminal_at_capacity_one() {
        use plugin_framework::provider_contract::{ProviderFinishReason, ProviderUsage};

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
