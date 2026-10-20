use super::*;

pub(in super::super) async fn append_compatible_resume_terminal_event(
    terminal_writer: &dyn control_plane::ports::RuntimeEventTerminalWriter,
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
    let _ = terminal_writer
        .append_terminal_if_missing_and_close(payload)
        .await;
}

#[cfg(test)]
pub(super) struct CompatibleTerminalFallback<'a, F> {
    pub(super) state: &'a ApiState,
    pub(super) initial_run: &'a NativeRunResult,
    pub(super) sender: &'a mpsc::Sender<Result<Event, Infallible>>,
    pub(super) mapper: &'a mut F,
    pub(super) stats: &'a mut CompatibleStreamStats,
    pub(super) trigger: &'static str,
    pub(super) warn_if_not_terminal: bool,
    pub(super) ignored_waiting_callback_task_id: Option<uuid::Uuid>,
}

#[cfg(test)]
pub(super) enum CompatibleTerminalFallbackOutcome {
    NotTerminal,
    Sent { event_type: String },
    ClientDisconnected { event_type: Option<String> },
    IgnoredWaitingCallback,
}

#[cfg(test)]
pub(super) async fn emit_compatible_terminal_fallback<F>(
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

    let latest_run = match recover_missing_stream_terminal_winner_with_dependencies(
        &NativeRunTerminalDependencies::new(
            state.store.clone(),
            state.runtime_engine.clone(),
            state.provider_runtime.clone(),
            state.provider_secret_master_key.clone(),
            state.model_billing_require_provider_usage,
            state.infrastructure.provider_transport_store(),
            state.runtime_event_stream.clone(),
        ),
        initial_run,
    )
    .await
    {
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
    if !stats.emitted_content() {
        for answer_event in durable_canonical_partial_runtime_events_from_native_run(&latest_run) {
            let events = mapper(&latest_run, answer_event.clone());
            let emitted_public_event = !events.is_empty();
            if !send_compatible_sse_events(sender, events).await {
                return CompatibleTerminalFallbackOutcome::ClientDisconnected { event_type: None };
            }
            stats.record_sent_runtime_event(&latest_run, &answer_event, emitted_public_event);
        }
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

pub(super) fn is_ignored_waiting_callback(
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

pub(in super::super) fn is_public_terminal_runtime_event(event_type: &str) -> bool {
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

pub(in super::super) fn is_answer_presentation_delta(envelope: &RuntimeEventEnvelope) -> bool {
    matches!(
        envelope.event_type.as_str(),
        "reasoning_delta" | "text_delta"
    ) && debug_stream_events::is_answer_presentation_delta_payload(&envelope.payload)
}
