use super::*;

pub(super) struct SubscribedCompatibleRuntimeEventStream<F> {
    pub(super) state: Arc<ApiState>,
    pub(super) initial_run: NativeRunResult,
    pub(super) sse_projection: &'static str,
    pub(super) from_sequence: Option<i64>,
    pub(super) ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    pub(super) subscription: control_plane::ports::RuntimeEventSubscription,
    pub(super) sender: mpsc::Sender<Result<Event, Infallible>>,
    pub(super) mapper: F,
}

pub(super) async fn send_subscribed_compatible_runtime_event_stream<F>(
    stream: SubscribedCompatibleRuntimeEventStream<F>,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let SubscribedCompatibleRuntimeEventStream {
        state,
        initial_run,
        sse_projection,
        from_sequence,
        ignored_waiting_callback_task_id,
        mut subscription,
        sender,
        mut mapper,
    } = stream;
    let mut stats = CompatibleStreamStats::default();
    let mut last_forwarded_sequence = from_sequence.unwrap_or(0);
    let mut last_forwarded_durable_sequence = durable_sequence_for_ignored_waiting_callback(
        state.as_ref(),
        initial_run.id,
        ignored_waiting_callback_task_id,
    )
    .await
    .unwrap_or(0);
    match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence: &mut last_forwarded_sequence,
        resume_durable_sequence_before_terminal: ignored_waiting_callback_task_id
            .map(|_| &mut last_forwarded_durable_sequence),
        events: subscription.replay,
    })
    .await
    {
        CompatibleForwardOutcome::Terminal { event_type } => {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible public API stream replay reached terminal event"
            );
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "replay",
                false,
            );
            return;
        }
        CompatibleForwardOutcome::ClientDisconnected => {
            debug!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                "compatible public API stream client disconnected during replay"
            );
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "client_disconnected",
                "replay",
                true,
            );
            return;
        }
        CompatibleForwardOutcome::Open => {}
    }

    while let Some(event) = subscription.live_events.recv().await {
        let event_type = event.event_type.clone();
        match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
            state: &state,
            initial_run: &initial_run,
            sender: &sender,
            mapper: &mut mapper,
            stats: &mut stats,
            ignored_waiting_callback_task_id,
            last_forwarded_sequence: &mut last_forwarded_sequence,
            resume_durable_sequence_before_terminal: ignored_waiting_callback_task_id
                .map(|_| &mut last_forwarded_durable_sequence),
            events: vec![event],
        })
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type: _ } => {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    event_type = %event_type,
                    "compatible public API stream reached terminal event"
                );
                log_compatible_sse_closed(
                    sse_projection,
                    &initial_run,
                    &stats,
                    &event_type,
                    "live",
                    false,
                );
                return;
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                debug!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    "compatible public API stream client disconnected"
                );
                log_compatible_sse_closed(
                    sse_projection,
                    &initial_run,
                    &stats,
                    "client_disconnected",
                    "live",
                    true,
                );
                return;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    match *subscription.closure.borrow() {
        Some(closure) => debug!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            close_reason = ?closure.reason,
            final_sequence = closure.final_sequence,
            last_forwarded_sequence,
            "compatible public API runtime event stream closed"
        ),
        None => warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            last_forwarded_sequence,
            "compatible public API runtime event receiver ended without a close signal"
        ),
    }

    match drain_compatible_durable_runtime_events(CompatibleDurableRuntimeEventsForward {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        ignored_waiting_callback_task_id,
        last_forwarded_durable_sequence: &mut last_forwarded_durable_sequence,
    })
    .await
    {
        CompatibleForwardOutcome::Terminal { event_type } => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "stream_closed_durable_reconcile",
                false,
            );
            return;
        }
        CompatibleForwardOutcome::ClientDisconnected => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "client_disconnected",
                "stream_closed_durable_reconcile",
                true,
            );
            return;
        }
        CompatibleForwardOutcome::Open => {}
    }

    match emit_compatible_terminal_fallback(CompatibleTerminalFallback {
        state: &state,
        initial_run: &initial_run,
        sender: &sender,
        mapper: &mut mapper,
        stats: &mut stats,
        trigger: "stream_closed",
        warn_if_not_terminal: true,
        ignored_waiting_callback_task_id,
    })
    .await
    {
        CompatibleTerminalFallbackOutcome::Sent { event_type } => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                &event_type,
                "stream_closed_terminal_fallback",
                false,
            );
        }
        CompatibleTerminalFallbackOutcome::ClientDisconnected { event_type } => {
            let terminal_reason = event_type.as_deref().unwrap_or("client_disconnected");
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                terminal_reason,
                "stream_closed_terminal_fallback",
                true,
            );
        }
        CompatibleTerminalFallbackOutcome::IgnoredWaitingCallback => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "ignored_waiting_callback",
                "stream_closed_terminal_fallback",
                false,
            );
        }
        CompatibleTerminalFallbackOutcome::NotTerminal => {
            log_compatible_sse_closed(
                sse_projection,
                &initial_run,
                &stats,
                "stream_closed_before_terminal",
                "stream_closed",
                false,
            );
        }
    }
}

#[cfg(test)]
pub(super) async fn send_compatible_runtime_event_stream<F>(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    sse_projection: &'static str,
    from_sequence: Option<i64>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
    mapper: F,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let stream = state.runtime_event_stream.clone();
    let Ok(subscription) = stream.subscribe(initial_run.id, from_sequence).await else {
        warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            "failed to subscribe compatible public API runtime event stream"
        );
        return;
    };
    send_subscribed_compatible_runtime_event_stream(SubscribedCompatibleRuntimeEventStream {
        state,
        initial_run,
        sse_projection,
        from_sequence,
        ignored_waiting_callback_task_id,
        subscription,
        sender,
        mapper,
    })
    .await;
}

async fn durable_sequence_for_ignored_waiting_callback(
    state: &ApiState,
    run_id: uuid::Uuid,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
) -> Option<i64> {
    let ignored_task_id = ignored_waiting_callback_task_id?;
    state
        .store
        .get_runtime_event_sequence_for_callback_task(run_id, ignored_task_id)
        .await
        .ok()?
}

fn log_compatible_sse_closed(
    sse_projection: &'static str,
    run: &NativeRunResult,
    stats: &CompatibleStreamStats,
    terminal_reason: &str,
    close_trigger: &str,
    client_disconnected: bool,
) {
    info!(
        flow_run_id = %run.id,
        application_id = %run.application_id,
        sse_projection = %sse_projection,
        emitted_content = stats.emitted_content(),
        content_bytes = stats.emitted_content_bytes,
        terminal_reason = %terminal_reason,
        close_trigger = %close_trigger,
        client_disconnected = client_disconnected,
        "compatible public API SSE stream closed"
    );
}

fn durable_record_to_runtime_event_envelope(
    record: domain::RuntimeEventRecord,
) -> RuntimeEventEnvelope {
    let text = compat_payload_string(&record.payload, "text")
        .or_else(|| compat_payload_string(&record.payload, "delta"));
    let delta_index = compat_payload_i64(&record.payload, "delta_index")
        .or_else(|| compat_payload_i64(&record.payload, "sequence_start"));
    let content_type = compat_payload_string(&record.payload, "content_type");
    RuntimeEventEnvelope {
        run_id: record.flow_run_id,
        node_run_id: record.node_run_id,
        sequence: record.sequence,
        event_id: format!("{}:{}", record.flow_run_id, record.sequence),
        event_type: record.event_type,
        occurred_at: record.created_at,
        delta_index,
        content_type,
        text,
        source: match record.source {
            domain::RuntimeEventSource::ProviderPlugin => {
                control_plane::ports::RuntimeEventSource::Provider
            }
            _ => control_plane::ports::RuntimeEventSource::Runtime,
        },
        durability: match record.durability {
            domain::RuntimeEventDurability::Durable => {
                control_plane::ports::RuntimeEventDurability::DurableRequired
            }
            domain::RuntimeEventDurability::Ephemeral | domain::RuntimeEventDurability::Sampled => {
                control_plane::ports::RuntimeEventDurability::Ephemeral
            }
        },
        persist_required: true,
        trace_visible: true,
        payload: record.payload,
    }
}

fn compat_payload_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn compat_payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

struct CompatibleRuntimeEventsForward<'a, F> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    mapper: &'a mut F,
    stats: &'a mut CompatibleStreamStats,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    last_forwarded_sequence: &'a mut i64,
    resume_durable_sequence_before_terminal: Option<&'a mut i64>,
    events: Vec<RuntimeEventEnvelope>,
}

enum CompatibleForwardOutcome {
    Open,
    Terminal { event_type: String },
    ClientDisconnected,
}

async fn forward_compatible_runtime_events<F>(
    forward: CompatibleRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence,
        resume_durable_sequence_before_terminal,
        events,
    } = forward;
    let mut resume_durable_sequence_before_terminal = resume_durable_sequence_before_terminal;

    for event in events {
        if event.sequence <= *last_forwarded_sequence {
            continue;
        }
        *last_forwarded_sequence = event.sequence;
        if is_ignored_waiting_callback(&event, ignored_waiting_callback_task_id) {
            continue;
        }

        let is_terminal = is_public_terminal_runtime_event(&event.event_type);
        if is_terminal && ignored_waiting_callback_task_id.is_some() {
            if let Some(last_forwarded_durable_sequence) =
                resume_durable_sequence_before_terminal.as_deref_mut()
            {
                match drain_compatible_durable_runtime_events(
                    CompatibleDurableRuntimeEventsForward {
                        state,
                        initial_run,
                        sender,
                        mapper,
                        stats,
                        ignored_waiting_callback_task_id,
                        last_forwarded_durable_sequence,
                    },
                )
                .await
                {
                    CompatibleForwardOutcome::Terminal { event_type } => {
                        return CompatibleForwardOutcome::Terminal { event_type };
                    }
                    CompatibleForwardOutcome::ClientDisconnected => {
                        return CompatibleForwardOutcome::ClientDisconnected;
                    }
                    CompatibleForwardOutcome::Open => {}
                }
            }
        }
        match forward_single_compatible_runtime_event(
            state,
            initial_run,
            sender,
            mapper,
            stats,
            event,
        )
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type } => {
                return CompatibleForwardOutcome::Terminal { event_type };
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                return CompatibleForwardOutcome::ClientDisconnected;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    CompatibleForwardOutcome::Open
}

async fn forward_compatible_runtime_events_without_resume_durable_prefix<F>(
    forward: CompatibleRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_sequence,
        resume_durable_sequence_before_terminal: _,
        events,
    } = forward;
    for event in events {
        if event.sequence <= *last_forwarded_sequence {
            continue;
        }
        *last_forwarded_sequence = event.sequence;
        if is_ignored_waiting_callback(&event, ignored_waiting_callback_task_id) {
            continue;
        }

        match forward_single_compatible_runtime_event(
            state,
            initial_run,
            sender,
            mapper,
            stats,
            event,
        )
        .await
        {
            CompatibleForwardOutcome::Terminal { event_type } => {
                return CompatibleForwardOutcome::Terminal { event_type };
            }
            CompatibleForwardOutcome::ClientDisconnected => {
                return CompatibleForwardOutcome::ClientDisconnected;
            }
            CompatibleForwardOutcome::Open => {}
        }
    }

    CompatibleForwardOutcome::Open
}

async fn forward_single_compatible_runtime_event<F>(
    state: &ApiState,
    initial_run: &NativeRunResult,
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    mapper: &mut F,
    stats: &mut CompatibleStreamStats,
    event: RuntimeEventEnvelope,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    if !stats.claim_runtime_event(&event) {
        return CompatibleForwardOutcome::Open;
    }
    let is_terminal = is_public_terminal_runtime_event(&event.event_type);
    let terminal_run;
    let run = if is_terminal {
        terminal_run = load_latest_native_run_for_terminal_fallback(state, initial_run).await;
        &terminal_run
    } else {
        initial_run
    };
    let event_type = event.event_type.clone();
    let events = mapper(run, event.clone());
    let emitted_public_event = !events.is_empty();
    if !send_compatible_sse_events(sender, events).await {
        return CompatibleForwardOutcome::ClientDisconnected;
    }
    stats.record_sent_runtime_event(run, &event, emitted_public_event);
    if is_terminal {
        return CompatibleForwardOutcome::Terminal { event_type };
    }
    CompatibleForwardOutcome::Open
}

struct CompatibleDurableRuntimeEventsForward<'a, F> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    mapper: &'a mut F,
    stats: &'a mut CompatibleStreamStats,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    last_forwarded_durable_sequence: &'a mut i64,
}

async fn drain_compatible_durable_runtime_events<F>(
    forward: CompatibleDurableRuntimeEventsForward<'_, F>,
) -> CompatibleForwardOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleDurableRuntimeEventsForward {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        ignored_waiting_callback_task_id,
        last_forwarded_durable_sequence,
    } = forward;

    if ignored_waiting_callback_task_id.is_some() && *last_forwarded_durable_sequence == 0 {
        if let Some(sequence) = durable_sequence_for_ignored_waiting_callback(
            state,
            initial_run.id,
            ignored_waiting_callback_task_id,
        )
        .await
        {
            *last_forwarded_durable_sequence = sequence;
        } else {
            return CompatibleForwardOutcome::Open;
        }
    }

    let records = match state
        .store
        .list_runtime_events(initial_run.id, *last_forwarded_durable_sequence)
        .await
    {
        Ok(records) => records,
        Err(error) => {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                "failed to drain compatible public API durable runtime events"
            );
            return CompatibleForwardOutcome::Open;
        }
    };
    let events = records
        .into_iter()
        .map(durable_record_to_runtime_event_envelope)
        .collect::<Vec<_>>();

    forward_compatible_runtime_events_without_resume_durable_prefix(
        CompatibleRuntimeEventsForward {
            state,
            initial_run,
            sender,
            mapper,
            stats,
            ignored_waiting_callback_task_id,
            last_forwarded_sequence: last_forwarded_durable_sequence,
            resume_durable_sequence_before_terminal: None,
            events,
        },
    )
    .await
}

async fn send_compatible_sse_events(
    sender: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<Result<Event, Infallible>>,
) -> bool {
    for sse in events {
        if sender.send(sse).await.is_err() {
            return false;
        }
    }
    true
}

pub(super) async fn append_compatible_resume_terminal_event(
    state: &ApiState,
    run: &NativeRunResult,
) {
    let Some(event) = terminal_runtime_event_from_native_run(run) else {
        return;
    };
    match run.status {
        NativeRunStatus::Succeeded
        | NativeRunStatus::Incomplete
        | NativeRunStatus::Failed
        | NativeRunStatus::Cancelled
        | NativeRunStatus::Waiting => {}
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => return,
    }
    let payload = RuntimeEventPayload {
        event_type: event.event_type,
        source: event.source,
        durability: event.durability,
        persist_required: event.persist_required,
        trace_visible: event.trace_visible,
        payload: event.payload,
    };
    let _ = state
        .runtime_event_stream
        .append_terminal_if_missing_and_close(run.id, payload)
        .await;
}

struct CompatibleTerminalFallback<'a, F> {
    state: &'a ApiState,
    initial_run: &'a NativeRunResult,
    sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    mapper: &'a mut F,
    stats: &'a mut CompatibleStreamStats,
    trigger: &'static str,
    warn_if_not_terminal: bool,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
}

enum CompatibleTerminalFallbackOutcome {
    NotTerminal,
    Sent { event_type: String },
    ClientDisconnected { event_type: Option<String> },
    IgnoredWaitingCallback,
}

async fn emit_compatible_terminal_fallback<F>(
    fallback: CompatibleTerminalFallback<'_, F>,
) -> CompatibleTerminalFallbackOutcome
where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let CompatibleTerminalFallback {
        state,
        initial_run,
        sender,
        mapper,
        stats,
        trigger,
        warn_if_not_terminal,
        ignored_waiting_callback_task_id,
    } = fallback;

    let latest_run = match recover_missing_stream_terminal_winner(state, initial_run).await {
        Ok(run) => run,
        Err(error) => {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                trigger = %trigger,
                "failed to recover the durable winner after compatible stream EOF"
            );
            return CompatibleTerminalFallbackOutcome::NotTerminal;
        }
    };
    let Some(terminal_event) = terminal_runtime_event_from_native_run(&latest_run) else {
        if warn_if_not_terminal {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                status = ?latest_run.status,
                trigger = %trigger,
                "compatible public API stream ended before durable run reached a terminal state"
            );
        }
        return CompatibleTerminalFallbackOutcome::NotTerminal;
    };

    warn!(
        flow_run_id = %initial_run.id,
        application_id = %initial_run.application_id,
        status = ?latest_run.status,
        trigger = %trigger,
        "compatible public API stream missing runtime terminal event; emitting durable terminal fallback"
    );
    if is_ignored_waiting_callback(&terminal_event, ignored_waiting_callback_task_id) {
        debug!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            trigger = %trigger,
            "compatible public API resume stream ignored stale waiting callback terminal fallback"
        );
        return CompatibleTerminalFallbackOutcome::IgnoredWaitingCallback;
    }

    if !stats.emitted_public_event {
        let started_event = RuntimeEventEnvelope::new(
            latest_run.id,
            0,
            debug_stream_events::flow_started(latest_run.id),
        );
        let events = mapper(&latest_run, started_event.clone());
        let emitted_public_event = !events.is_empty();
        if !send_compatible_sse_events(sender, events).await {
            return CompatibleTerminalFallbackOutcome::ClientDisconnected { event_type: None };
        }
        stats.record_sent_runtime_event(&latest_run, &started_event, emitted_public_event);
    }
    let event_type = terminal_event.event_type.clone();
    let events = mapper(&latest_run, terminal_event.clone());
    let emitted_public_event = !events.is_empty();
    if !send_compatible_sse_events(sender, events).await {
        return CompatibleTerminalFallbackOutcome::ClientDisconnected {
            event_type: Some(event_type),
        };
    }
    stats.record_sent_runtime_event(&latest_run, &terminal_event, emitted_public_event);
    CompatibleTerminalFallbackOutcome::Sent { event_type }
}

fn is_ignored_waiting_callback(
    event: &RuntimeEventEnvelope,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
) -> bool {
    if event.event_type != "waiting_callback" {
        return false;
    }
    let Some(ignored_task_id) = ignored_waiting_callback_task_id else {
        return false;
    };
    event
        .payload
        .get("callback_task_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        == Some(ignored_task_id)
}

pub(super) fn is_public_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished"
            | "flow_incomplete"
            | "flow_failed"
            | "flow_cancelled"
            | "waiting_human"
            | "waiting_callback"
    )
}

pub(super) fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    matches!(
        envelope.event_type.as_str(),
        "reasoning_delta" | "text_delta"
    ) && debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}
