use super::*;
use control_plane::application_public_api::callback_resume::ApplicationPublishedCallbackAttemptRepository;
use event_forwarding::is_public_terminal_runtime_event;

/// Diagnostic tails and their synthetic start do not establish a round.
pub(super) fn callback_round_replay_established(
    replay: &[RuntimeEventEnvelope],
    round_id: uuid::Uuid,
) -> bool {
    replay.iter().any(|event| {
        (event.event_type == "flow_started"
            && event.payload["response_round_id"] == json!(round_id))
            || event_forwarding::is_public_terminal_runtime_event(&event.event_type)
    })
}

pub(super) async fn attach_live_compatible_callback_round(
    dependencies: &CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    round_id: uuid::Uuid,
    start: i64,
) -> Result<CompatibleTypedTurnStream, NativeApiError> {
    let subscription = dependencies
        .native
        .runtime_event_stream
        .subscribe(initial_run.id, Some(start))
        .await
        .map_err(service_error)?;
    let subscription = follow_callback_subscription(
        dependencies.clone(),
        initial_run.clone(),
        round_id,
        start,
        subscription,
    );
    Ok(attach_compatible_typed_stream_from_subscription(
        native::native_run_terminal_dependencies(&dependencies.native),
        initial_run,
        Some(start),
        Vec::new(),
        RuntimeEventDeliveryScope::Resume {
            callback_task_id: round_id,
        },
        subscription,
    ))
}

/// A callback producer can finish durably after its original transport consumer
/// exits. Keep live delivery, but do not mistake an open ephemeral lane for the
/// owner of the callback's committed outcome.
pub(super) fn follow_callback_subscription(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    round_id: uuid::Uuid,
    from_sequence: i64,
    mut subscription: control_plane::ports::RuntimeEventSubscription,
) -> control_plane::ports::RuntimeEventSubscription {
    let (sender, receiver) = mpsc::channel(32);
    let (diagnostic_sender, diagnostic) = mpsc::channel(1);
    drop(diagnostic_sender);
    let replay = std::mem::take(&mut subscription.replay);
    let terminal_writer = subscription.terminal_writer;
    let closure = subscription.closure;
    let mut cursor = replay.last().map_or(from_sequence, |event| event.sequence);
    let mut started = replay
        .iter()
        .any(|event| event.event_type == "flow_started");
    let mut completed_items = std::collections::HashSet::new();
    for event in &replay {
        remember_completed_output(event, &mut completed_items);
    }
    tokio::spawn(async move {
        let mut durable_cursor = match dependencies
            .native
            .store
            .get_original_callback_waiting_boundary(initial_run.id, round_id)
            .await
        {
            Ok(Some(boundary)) => boundary.original.event_sequence,
            result => {
                warn!(flow_run_id = %initial_run.id, ?result,
                    "callback attachment durable boundary unavailable");
                return;
            }
        };
        let mut terminal_observed = false;
        let mut live_open = true;
        let mut poll = tokio::time::interval(Duration::from_secs(1));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = sender.closed() => return,
                event = subscription.live_events.recv(), if live_open => {
                    let Some(event) = event else {
                        live_open = false;
                        continue;
                    };
                    cursor = cursor.max(event.sequence);
                    started |= event.event_type == "flow_started";
                    remember_completed_output(&event, &mut completed_items);
                    let terminal = is_public_terminal_runtime_event(&event.event_type);
                    if sender.send(event).await.is_err() || terminal {
                        return;
                    }
                }
                _ = poll.tick() => {
                    while !terminal_observed {
                        let page = match dependencies.native.store.list_runtime_event_durable_page(
                            initial_run.id, durable_cursor, None, 64,
                        ).await {
                            Ok(page) => page,
                            Err(error) => {
                                warn!(flow_run_id = %initial_run.id, %error,
                                    "failed to probe durable callback completion");
                                break;
                            }
                        };
                        if let Some(last) = page.last() {
                            if last.sequence <= durable_cursor {
                                warn!(flow_run_id = %initial_run.id,
                                    "durable callback attachment cursor did not advance");
                                return;
                            }
                            durable_cursor = last.sequence;
                        }
                        terminal_observed |= page.iter().any(|event|
                            is_public_terminal_runtime_event(&event.event_type));
                        if page.len() < 64 || terminal_observed {
                            break;
                        }
                    }
                    if !terminal_observed {
                        continue;
                    }
                    let replay = match durable_compatible_round_replay(
                        &dependencies, &initial_run, round_id,
                    ).await {
                        Ok(DurableCompatibleRoundReplay::Ready(replay)) if replay.last()
                            .is_some_and(|event| is_public_terminal_runtime_event(&event.event_type)) => replay,
                        Ok(DurableCompatibleRoundReplay::PendingIdentity) => continue,
                        Ok(DurableCompatibleRoundReplay::Ready(_)) => {
                            // A resolved identity can exclude a foreign terminal.
                            terminal_observed = false;
                            continue;
                        }
                        Err(error) => {
                            warn!(flow_run_id = %initial_run.id, ?error,
                                "failed to follow durable callback completion");
                            continue;
                        }
                    };
                    let live_cursor = cursor;
                    for mut event in replay {
                        let local_sequence = event.payload["sequence_end"].as_i64()
                            .or_else(|| event.payload["stream_sequence"].as_i64());
                        let item_index = completed_output_index(&event);
                        if !is_public_terminal_runtime_event(&event.event_type)
                            && (local_sequence.is_some_and(|sequence| sequence <= live_cursor)
                                || (event.event_type == "flow_started" && started)
                                || item_index.is_some_and(|index| completed_items.contains(&index))) {
                            continue;
                        }
                        // Durable and live cursors belong to different sequence spaces.
                        // This bridge appends the missing suffix to its live lane.
                        cursor = cursor.saturating_add(1);
                        event.sequence = cursor;
                        event.event_id = format!("{}:{}", event.run_id, cursor);
                        if sender.send(event).await.is_err() {
                            return;
                        }
                    }
                    return;
                }
            }
        }
    });
    control_plane::ports::RuntimeEventSubscription {
        terminal_writer,
        replay,
        live_events: control_plane::ports::RuntimeEventReceiver::new(receiver, diagnostic),
        closure,
    }
}

fn completed_output_index(event: &RuntimeEventEnvelope) -> Option<u64> {
    (event.event_type == "provider_output_item_done")
        .then(|| event.payload["output_index"].as_u64())
        .flatten()
}

fn remember_completed_output(
    event: &RuntimeEventEnvelope,
    completed_items: &mut std::collections::HashSet<u64>,
) {
    if let Some(index) = completed_output_index(event) {
        completed_items.insert(index);
    }
}

pub(super) async fn follow_missing_compatible_round_events(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    mut command: ResumePublishedCallbackCommand,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    sender: mpsc::Sender<CompatibleProjectionInput>,
) -> Result<(), NativeApiError> {
    let round_id = callback_task_id_from_resume_command(&command);
    let mut cursor = dependencies
        .native
        .store
        .get_original_callback_waiting_boundary(initial_run.id, round_id)
        .await
        .map_err(service_error)?
        .ok_or_else(|| {
            service_error(
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_event_delivery_scope_unresolved",
                )
                .into(),
            )
        })?
        .original
        .event_sequence;
    let mut terminal_observed = false;
    loop {
        if sender.is_closed() {
            return Ok(());
        }
        let attached = if let Some(start) =
            live_compatible_round_start(&dependencies, &initial_run, round_id).await
        {
            Some(
                attach_live_compatible_callback_round(
                    &dependencies,
                    initial_run.clone(),
                    round_id,
                    start,
                )
                .await?,
            )
        } else {
            None
        };
        if let Some(mut attached) = attached {
            while let Some(event) = attached.events.recv().await {
                if sender.send(event).await.is_err() {
                    return Ok(());
                }
            }
            return Ok(());
        }
        // The cursor detects durable completion without retaining the round's
        // history in memory. Replay is loaded once only when the round settles.
        let records = dependencies
            .native
            .store
            .list_runtime_event_durable_page(initial_run.id, cursor, None, 64)
            .await
            .map_err(service_error)?;
        if let Some(last) = records.last() {
            if last.sequence <= cursor {
                return Err(service_error(anyhow::anyhow!(
                    "durable callback follower cursor did not advance"
                )));
            }
            cursor = last.sequence;
        }
        terminal_observed |= records
            .iter()
            .any(|record| event_forwarding::is_public_terminal_runtime_event(&record.event_type));
        if terminal_observed {
            let durable_round_replay =
                durable_compatible_round_replay(&dependencies, &initial_run, round_id).await?;
            if let DurableCompatibleRoundReplay::Ready(durable_round_replay) = durable_round_replay
            {
                if durable_round_replay.last().is_some_and(|event| {
                    event_forwarding::is_public_terminal_runtime_event(&event.event_type)
                }) {
                    let mut attached = attach_terminal_compatible_round(
                        &dependencies,
                        initial_run.clone(),
                        round_id,
                        durable_round_replay,
                    )
                    .await?;
                    while let Some(event) = attached.events.recv().await {
                        if sender.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                // Identity has resolved and excluded the observed foreign
                // terminal. PendingIdentity keeps the observation latched so
                // an identity-only later page can still complete this round.
                terminal_observed = false;
            }
        }
        // A full page may precede the requested terminal. Drain it without
        // retaining previous pages or delaying once per page.
        if records.len() == 64 {
            continue;
        }
        let attempt = dependencies
            .native
            .store
            .get_published_callback_resume_attempt(round_id)
            .await
            .map_err(service_error)?;
        if let Some(attempt) = &attempt {
            if attempt.response_payload != command.response_payload
                || attempt.source != command.source.as_str()
            {
                return Err(service_error(anyhow::anyhow!(
                    "callback resume attempt changed while following"
                )));
            }
        }
        if let Some(attempt) = attempt.filter(|attempt| {
            attempt.status == domain::FlowRunCallbackResumeAttemptStatus::Processing
                && attempt.updated_at + time::Duration::minutes(5)
                    <= time::OffsetDateTime::now_utc()
        }) {
            let invoker = native::runtime_internal_tool_invoker_for_run(
                &dependencies.native,
                &actor,
                &initial_run,
            )
            .await?;
            let runtime_service =
                native::native_runtime_service(&dependencies.native, invoker.clone())
                    .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone());
            let service = Arc::new(
                ApplicationPublishedCallbackResumeService::new(
                    dependencies.native.store.clone(),
                    runtime_service,
                )
                .with_last_used_cache(dependencies.native.cache_store.clone()),
            );
            match service
                .reserve_native_callback_reservation_for_actor(actor.clone(), &command)
                .await
            {
                Ok(acquired) if acquired.attempt_id() == attempt.id => {
                    let mut reservation =
                        CompatibleCallbackReservation::new(acquired, Arc::clone(&service));
                    reservation
                        .capture_previous_generation(
                            dependencies.native.runtime_event_stream.as_ref(),
                            initial_run.id,
                        )
                        .await;
                    let reclaimed_id = reservation.attempt_id();
                    if !reservation.is_acquired() {
                        let mut settled_run = initial_run;
                        settled_run.metadata["active_callback_attach"] = json!(false);
                        let mut settled = start_compatible_typed_resume_stream_for_actor(
                            dependencies,
                            settled_run,
                            command,
                            actor,
                            Some(reservation),
                        )
                        .await?;
                        while let Some(event) = settled.events.recv().await {
                            if sender.send(event).await.is_err() {
                                return Ok(());
                            }
                        }
                        return Ok(());
                    }
                    let current = dependencies
                        .native
                        .store
                        .get_published_callback_resume_attempt(round_id)
                        .await
                        .map_err(service_error)?;
                    if !current.is_some_and(|current| {
                        current.id == reclaimed_id
                            && current.status
                                == domain::FlowRunCallbackResumeAttemptStatus::Processing
                    }) {
                        reservation.release().await;
                        continue;
                    }
                    command.reserved_attempt_id = Some(reclaimed_id);
                    let mut recovered_run = initial_run.clone();
                    recovered_run.metadata["active_callback_attach"] = json!(false);
                    let opened = open_compatible_turn_with_invoker(
                        dependencies.clone(),
                        recovered_run,
                        CompatibleTurnAction::ResumeForActor {
                            command,
                            actor,
                            reservation: Some(reservation),
                        },
                        None,
                        invoker,
                    )
                    .await?;
                    let mut recovered = opened_compatible_typed_stream(dependencies, opened)?;
                    while let Some(event) = recovered.events.recv().await {
                        if sender.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                Ok(_) => {
                    return Err(service_error(anyhow::anyhow!(
                        "callback resume attempt changed while reclaiming"
                    )));
                }
                Err(error)
                    if error
                        .downcast_ref::<control_plane::errors::ControlPlaneError>()
                        .is_some_and(|error| {
                            matches!(
                                error,
                                control_plane::errors::ControlPlaneError::Conflict(
                                    "callback_resume_already_admitted"
                                )
                            )
                        }) => {}
                Err(error) => return Err(service_error(error)),
            }
        }
        tokio::select! {
            _ = sender.closed() => return Ok(()),
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
}
