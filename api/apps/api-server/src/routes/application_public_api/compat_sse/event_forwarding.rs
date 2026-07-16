use super::*;

pub(super) async fn send_compatible_runtime_event_stream<F>(
    state: Arc<ApiState>,
    initial_run: NativeRunResult,
    sse_projection: &'static str,
    from_sequence: Option<i64>,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
    sender: mpsc::Sender<Result<Event, Infallible>>,
    mut mapper: F,
) where
    F: FnMut(&NativeRunResult, RuntimeEventEnvelope) -> Vec<Result<Event, Infallible>>,
{
    let mut stats = CompatibleStreamStats::default();
    let stream = state.runtime_event_stream.clone();
    let Ok(mut subscription) = stream.subscribe(initial_run.id, from_sequence).await else {
        warn!(
            flow_run_id = %initial_run.id,
            application_id = %initial_run.application_id,
            "failed to subscribe compatible public API runtime event stream"
        );
        log_compatible_sse_closed(
            sse_projection,
            &initial_run,
            &stats,
            "subscribe_failed",
            "subscribe",
            false,
        );
        return;
    };

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
        resume_durable_sequence_before_terminal: Some(&mut last_forwarded_durable_sequence),
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
        CompatibleForwardOutcome::Open { .. } => {}
    }

    let mut durable_terminal_check = tokio::time::interval(Duration::from_millis(500));
    durable_terminal_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            maybe_event = subscription.live_events.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                let event_type = event.event_type.clone();
                match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
                    state: &state,
                    initial_run: &initial_run,
                    sender: &sender,
                    mapper: &mut mapper,
                    stats: &mut stats,
                    ignored_waiting_callback_task_id,
                    last_forwarded_sequence: &mut last_forwarded_sequence,
                    resume_durable_sequence_before_terminal: Some(
                        &mut last_forwarded_durable_sequence,
                    ),
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
                    CompatibleForwardOutcome::Open { .. } => {}
                }
            }
            _ = durable_terminal_check.tick() => {
                if let Ok(events) = stream.replay(
                    initial_run.id,
                    Some(last_forwarded_sequence),
                    usize::MAX,
                )
                .await
                {
                    match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
                        state: &state,
                        initial_run: &initial_run,
                        sender: &sender,
                        mapper: &mut mapper,
                        stats: &mut stats,
                        ignored_waiting_callback_task_id,
                        last_forwarded_sequence: &mut last_forwarded_sequence,
                        resume_durable_sequence_before_terminal: Some(
                            &mut last_forwarded_durable_sequence,
                        ),
                        events,
                    })
                    .await
                    {
                        CompatibleForwardOutcome::Terminal { event_type } => {
                            debug!(
                                flow_run_id = %initial_run.id,
                                application_id = %initial_run.application_id,
                                trigger = "durable_poll",
                                "compatible public API stream drained runtime terminal event before durable fallback"
                            );
                            log_compatible_sse_closed(
                                sse_projection,
                                &initial_run,
                                &stats,
                                &event_type,
                                "durable_poll_stream",
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
                                "durable_poll_stream",
                                true,
                            );
                            return;
                        }
                        CompatibleForwardOutcome::Open { saw_event: true } => continue,
                        CompatibleForwardOutcome::Open { saw_event: false } => {}
                    }
                }

                if ignored_waiting_callback_task_id.is_some()
                    && last_forwarded_durable_sequence == 0
                {
                    if let Some(sequence) = durable_sequence_for_ignored_waiting_callback(
                        state.as_ref(),
                        initial_run.id,
                        ignored_waiting_callback_task_id,
                    )
                    .await
                    {
                        last_forwarded_durable_sequence = sequence;
                    } else {
                        continue;
                    }
                }

                if let Ok(records) = state
                    .store
                    .list_runtime_events(initial_run.id, last_forwarded_durable_sequence)
                    .await
                {
                    let saw_durable_record = !records.is_empty();
                    let events = records
                        .into_iter()
                        .map(durable_record_to_runtime_event_envelope)
                        .collect::<Vec<_>>();
                    match forward_compatible_runtime_events(CompatibleRuntimeEventsForward {
                        state: &state,
                        initial_run: &initial_run,
                        sender: &sender,
                        mapper: &mut mapper,
                        stats: &mut stats,
                        ignored_waiting_callback_task_id,
                        last_forwarded_sequence: &mut last_forwarded_durable_sequence,
                        resume_durable_sequence_before_terminal: None,
                        events,
                    })
                    .await
                    {
                        CompatibleForwardOutcome::Terminal { event_type } => {
                            debug!(
                                flow_run_id = %initial_run.id,
                                application_id = %initial_run.application_id,
                                trigger = "durable_poll",
                                "compatible public API stream drained durable terminal event before fallback"
                            );
                            log_compatible_sse_closed(
                                sse_projection,
                                &initial_run,
                                &stats,
                                &event_type,
                                "durable_poll_records",
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
                                "durable_poll_records",
                                true,
                            );
                            return;
                        }
                        CompatibleForwardOutcome::Open { saw_event: true } => continue,
                        CompatibleForwardOutcome::Open { saw_event: false } => {
                            if ignored_waiting_callback_task_id.is_some() && !saw_durable_record {
                                continue;
                            }
                        }
                    }
                }

                match emit_compatible_terminal_fallback(CompatibleTerminalFallback {
                    state: &state,
                    initial_run: &initial_run,
                    sender: &sender,
                    mapper: &mut mapper,
                    stats: &mut stats,
                    trigger: "durable_poll",
                    warn_if_not_terminal: false,
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
                            "durable_terminal_fallback",
                            false,
                        );
                        return;
                    }
                    CompatibleTerminalFallbackOutcome::ClientDisconnected { event_type } => {
                        let terminal_reason =
                            event_type.as_deref().unwrap_or("client_disconnected");
                        log_compatible_sse_closed(
                            sse_projection,
                            &initial_run,
                            &stats,
                            terminal_reason,
                            "durable_terminal_fallback",
                            true,
                        );
                        return;
                    }
                    CompatibleTerminalFallbackOutcome::NotTerminal
                    | CompatibleTerminalFallbackOutcome::IgnoredWaitingCallback => {}
                }
            }
        }
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

async fn durable_sequence_for_ignored_waiting_callback(
    state: &ApiState,
    run_id: uuid::Uuid,
    ignored_waiting_callback_task_id: Option<uuid::Uuid>,
) -> Option<i64> {
    let ignored_task_id = ignored_waiting_callback_task_id?;
    let records = state.store.list_runtime_events(run_id, 0).await.ok()?;
    records
        .into_iter()
        .filter(|record| {
            record
                .payload
                .get("callback_task_id")
                .and_then(Value::as_str)
                .and_then(|value| uuid::Uuid::parse_str(value).ok())
                == Some(ignored_task_id)
        })
        .map(|record| record.sequence)
        .max()
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
    Open { saw_event: bool },
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
    let mut saw_event = false;
    let mut resume_durable_sequence_before_terminal = resume_durable_sequence_before_terminal;

    for event in events {
        if event.sequence <= *last_forwarded_sequence {
            continue;
        }
        *last_forwarded_sequence = event.sequence;
        saw_event = true;

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
                    CompatibleForwardOutcome::Open { .. } => {}
                }
            }
        }
        if let Some(last_forwarded_durable_sequence) =
            resume_durable_sequence_before_terminal.as_deref_mut()
        {
            advance_durable_cursor_for_forwarded_event(
                state,
                initial_run.id,
                &event,
                last_forwarded_durable_sequence,
            )
            .await;
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
            CompatibleForwardOutcome::Open { .. } => {}
        }
    }

    CompatibleForwardOutcome::Open { saw_event }
}

pub(super) async fn advance_durable_cursor_for_forwarded_event(
    state: &ApiState,
    run_id: uuid::Uuid,
    event: &RuntimeEventEnvelope,
    last_forwarded_durable_sequence: &mut i64,
) {
    if !event_can_match_durable_cursor(event) {
        return;
    }
    let Ok(records) = state
        .store
        .list_runtime_events(run_id, *last_forwarded_durable_sequence)
        .await
    else {
        return;
    };
    let Some(record) = records.into_iter().find(|record| {
        record.sequence > *last_forwarded_durable_sequence
            && durable_record_matches_forwarded_event(record, event)
    }) else {
        return;
    };

    *last_forwarded_durable_sequence = record.sequence;
}

fn event_can_match_durable_cursor(event: &RuntimeEventEnvelope) -> bool {
    event.event_type == "flow_started" || is_answer_presentation_delta(event)
}

fn durable_record_matches_forwarded_event(
    record: &domain::RuntimeEventRecord,
    event: &RuntimeEventEnvelope,
) -> bool {
    if record.event_type != event.event_type {
        return false;
    }
    if is_answer_presentation_delta(event) {
        return durable_record_matches_answer_delta(record, event);
    }
    if event.event_type == "flow_started" {
        return record.payload.get("type").and_then(Value::as_str) == Some("flow_started")
            && event.payload.get("type").and_then(Value::as_str) == Some("flow_started");
    }
    false
}

fn durable_record_matches_answer_delta(
    record: &domain::RuntimeEventRecord,
    event: &RuntimeEventEnvelope,
) -> bool {
    record.event_type == event.event_type
        && debug_stream_events::is_answer_presentation_delta_payload(&record.payload)
        && answer_delta_payload_field(&record.payload, "text")
            == answer_delta_payload_field(&event.payload, "text")
        && answer_delta_presentation_field(&record.payload, "answer_node_id")
            == answer_delta_presentation_field(&event.payload, "answer_node_id")
        && answer_delta_presentation_field(&record.payload, "segment_index")
            == answer_delta_presentation_field(&event.payload, "segment_index")
        && answer_delta_presentation_field(&record.payload, "source_node_id")
            == answer_delta_presentation_field(&event.payload, "source_node_id")
        && answer_delta_presentation_field(&record.payload, "source_output_key")
            == answer_delta_presentation_field(&event.payload, "source_output_key")
}

fn answer_delta_payload_field(payload: &Value, key: &str) -> Option<Value> {
    payload.get(key).cloned()
}

fn answer_delta_presentation_field(payload: &Value, key: &str) -> Option<Value> {
    payload
        .get("presentation")
        .and_then(Value::as_object)
        .and_then(|presentation| presentation.get(key))
        .cloned()
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
    let mut saw_event = false;

    for event in events {
        if event.sequence <= *last_forwarded_sequence {
            continue;
        }
        *last_forwarded_sequence = event.sequence;
        saw_event = true;

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
            CompatibleForwardOutcome::Open { .. } => {}
        }
    }

    CompatibleForwardOutcome::Open { saw_event }
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
        return CompatibleForwardOutcome::Open { saw_event: true };
    }
    let is_terminal = is_public_terminal_runtime_event(&event.event_type);
    let terminal_run;
    let run = if is_terminal {
        terminal_run = load_latest_native_run_for_terminal_fallback(state, initial_run).await;
        &terminal_run
    } else {
        initial_run
    };
    let event = if is_terminal {
        enrich_terminal_runtime_event_with_durable_answer(state, run, event).await
    } else {
        event
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
    CompatibleForwardOutcome::Open { saw_event: true }
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
            return CompatibleForwardOutcome::Open { saw_event: false };
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
            return CompatibleForwardOutcome::Open { saw_event: false };
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
    detail: &domain::ApplicationRunDetail,
) {
    let run = native_result_from_run_detail(detail, resume_metadata_from_detail(detail));
    let Some(event) = terminal_runtime_event_from_native_run(&run) else {
        return;
    };
    let close_reason = match run.status {
        NativeRunStatus::Succeeded => control_plane::ports::RuntimeEventCloseReason::Finished,
        NativeRunStatus::Failed => control_plane::ports::RuntimeEventCloseReason::Failed,
        NativeRunStatus::Cancelled => control_plane::ports::RuntimeEventCloseReason::Cancelled,
        NativeRunStatus::Waiting => control_plane::ports::RuntimeEventCloseReason::WaitingCallback,
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => return,
    };
    let _ = state
        .runtime_event_stream
        .append(run.id, runtime_event_payload_from_envelope(event))
        .await;
    let _ = state
        .runtime_event_stream
        .close_run(run.id, close_reason)
        .await;
}

fn runtime_event_payload_from_envelope(envelope: RuntimeEventEnvelope) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: envelope.event_type,
        source: envelope.source,
        durability: envelope.durability,
        persist_required: envelope.persist_required,
        trace_visible: envelope.trace_visible,
        payload: envelope.payload,
    }
}

fn resume_metadata_from_detail(detail: &domain::ApplicationRunDetail) -> Value {
    json!({
        "external_user": detail.flow_run.external_user,
        "external_conversation_id": detail.flow_run.external_conversation_id,
        "external_trace_id": detail.flow_run.external_trace_id,
        "compatibility_mode": detail.flow_run.compatibility_mode,
        "idempotency_key": detail.flow_run.idempotency_key,
        "request": {
            "conversation": {
                "id": detail.flow_run.external_conversation_id,
                "user": detail.flow_run.external_user,
            }
        }
    })
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

    let latest_run = load_latest_native_run_for_terminal_fallback(state, initial_run).await;
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
    let terminal_event =
        enrich_terminal_runtime_event_with_durable_answer(state, &latest_run, terminal_event).await;
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
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

pub(super) fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    matches!(
        envelope.event_type.as_str(),
        "reasoning_delta" | "text_delta"
    ) && debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}
