use super::*;

/// First typed provider error owns the invocation outcome. The caller still
/// consumes the result delimiter (or retires a broken process) before returning.
#[derive(Default)]
pub(super) struct ProviderStreamOutcome {
    first_error: Option<ProviderRuntimeError>,
}

impl ProviderStreamOutcome {
    pub(super) fn observe(&mut self, event: &ProviderStreamEvent) {
        if let ProviderStreamEvent::Error { error } = event {
            if self.first_error.is_none() {
                self.first_error = Some(error.clone());
            }
        }
    }

    pub(super) fn finish(
        self,
        output: FrameworkResult<StreamingProviderOutput>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        match self.first_error {
            Some(error) => Err(PluginFrameworkError::runtime(error)),
            None => output,
        }
    }
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
        let mut payload = serialize_provider_stdio_request(request)
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

    let mut stderr_task = tokio::spawn(async move {
        let mut text = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut text).await;
        text
    });

    let mut lines = BufReader::new(stdout).lines();
    let mut events = Vec::new();
    let mut result = None;
    let mut timeout_state = ProviderStreamTimeoutState::new();

    let mut outcome = ProviderStreamOutcome::default();
    let output = async {
        while let Some(line) =
            next_provider_stdout_line(&mut lines, executable_path, limits, &mut timeout_state)
                .await?
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
                        outcome.observe(&event);
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
        let stderr = (&mut stderr_task).await.unwrap_or_default();
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
    .await;
    if output.is_err() {
        // A damaged stream is not reusable. Reap the one-shot process without
        // promoting kill/wait failures over the invocation's primary error.
        let _ = child.kill().await;
        let _ = child.wait().await;
        stderr_task.abort();
    }
    outcome.finish(output)
}
