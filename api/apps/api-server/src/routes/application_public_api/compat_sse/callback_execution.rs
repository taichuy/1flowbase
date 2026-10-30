use super::*;
use control_plane::application_public_api::callback_resume::PreparedPublishedCallback;

pub(crate) async fn execute_compatible_resume_for_actor(
    dependencies: CompatibilityExecutionDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    command: ResumePublishedCallbackCommand,
    mut reservation: Option<CompatibleCallbackReservation>,
) -> Result<NativeRunResult, NativeApiError> {
    let round_id = Some(callback_task_id_from_resume_command(&command));
    let runtime_internal_tool_invoker = match dependencies
        .native
        .runtime_invoker_factory
        .for_actor(&actor)
        .await
    {
        Ok(invoker) => invoker,
        Err(error) => {
            if let Some(reservation) = reservation.as_mut() {
                reservation.release().await;
            }
            return Err(error);
        }
    };
    let runtime_service =
        native::native_runtime_service(&dependencies.native, runtime_internal_tool_invoker)
            .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone());
    let service = ApplicationPublishedCallbackResumeService::new(
        dependencies.native.store.clone(),
        runtime_service,
    )
    .with_last_used_cache(dependencies.native.cache_store.clone());
    let prepared = if let Some(receipt) = reservation
        .as_ref()
        .and_then(CompatibleCallbackReservation::receipt)
    {
        let prepared = match service
            .prepare_reserved_native_callback_for_actor(actor.clone(), command.clone(), receipt)
            .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                reservation
                    .as_mut()
                    .expect("prepared receipt has guard")
                    .release()
                    .await;
                return Err(service_error(error));
            }
        };
        // The CP owner now owns both execution and admission cleanup.
        reservation
            .as_mut()
            .expect("prepared receipt has guard")
            .handoff();
        Some(prepared)
    } else {
        None
    };
    tokio::spawn(async move {
        let result = match prepared {
            Some(PreparedPublishedCallback::Execution(owner)) => {
                service.execute_prepared_callback(*owner).await
            }
            Some(PreparedPublishedCallback::NoExecution(result)) => Ok(*result),
            None => service.resume_callback_for_actor(actor, command).await,
        };
        result
            .map(|mut result| {
                if let Some(round_id) = round_id {
                    if result.run.metadata["native_inference_recovery_replay"] != true {
                        result.run.metadata["response_round_id"] = json!(round_id);
                    }
                }
                result.run
            })
            .map_err(service_error)
    })
    .await
    .map_err(|error| service_error(anyhow::anyhow!(error)))?
}

pub(super) async fn open_compatible_turn_with_invoker(
    dependencies: CompatibilityExecutionDependencies,
    initial_run: NativeRunResult,
    mut action: CompatibleTurnAction,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    mcp_runtime_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    >,
) -> Result<OpenedCompatibleTurn, NativeApiError> {
    let turn_action = action.name();
    // Retain the invoker's generation through recovery/terminal writes after business return.
    let execution_invoker = mcp_runtime_invoker.clone();
    let ignored_waiting_callback_task_id = action.resumed_callback_task_id();
    let fresh_acquisition = matches!(&action,
        CompatibleTurnAction::ResumeForActor { reservation: Some(reservation), .. } if reservation.is_acquired());
    let runtime_service =
        native::native_runtime_service(&dependencies.native, mcp_runtime_invoker.clone())
            .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone());
    let callback_service = ApplicationPublishedCallbackResumeService::new(
        dependencies.native.store.clone(),
        native::native_runtime_service(&dependencies.native, mcp_runtime_invoker)
            .with_runtime_event_stream(dependencies.native.runtime_event_stream.clone()),
    )
    .with_last_used_cache(dependencies.native.cache_store.clone());
    let mut prepared_owner = None;
    let mut previous_generation = None;
    if let CompatibleTurnAction::ResumeForActor {
        actor,
        command,
        reservation: Some(reservation),
    } = &mut action
    {
        if let Some(receipt) = reservation.receipt() {
            let prepared = match callback_service
                .prepare_reserved_native_callback_for_actor(actor.clone(), command.clone(), receipt)
                .await
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    reservation.release().await;
                    return Err(service_error(error));
                }
            };
            previous_generation = reservation.take_previous_generation();
            reservation.handoff();
            match prepared {
                PreparedPublishedCallback::Execution(owner) => prepared_owner = Some(owner),
                PreparedPublishedCallback::NoExecution(mut result) => {
                    let round_id =
                        ignored_waiting_callback_task_id.expect("reserved callback has round");
                    let waiting = result.run.status == NativeRunStatus::Waiting;
                    let replay = if waiting {
                        // Partial tool results have a canonical typed waiting
                        // receipt, but no execution generation. Project that
                        // receipt read-only; do not persist a producer marker.
                        result.run.metadata["response_round_id"] = json!(round_id);
                        let mut started = debug_stream_events::flow_started(result.run.id);
                        started.payload["response_round_id"] = json!(round_id);
                        let mut terminal = terminal_runtime_event_from_native_run(&result.run)
                            .ok_or_else(|| {
                                service_error(anyhow::anyhow!(
                                    "partial callback waiting receipt missing"
                                ))
                            })?;
                        terminal.sequence = 1;
                        terminal.payload["response_round_id"] = json!(round_id);
                        vec![
                            RuntimeEventEnvelope::new(result.run.id, 0, started),
                            terminal,
                        ]
                    } else {
                        match durable_compatible_round_replay(&dependencies, &initial_run, round_id)
                            .await?
                        {
                            DurableCompatibleRoundReplay::Ready(replay)
                                if replay.last().is_some_and(|event| {
                                    event_forwarding::is_public_terminal_runtime_event(
                                        &event.event_type,
                                    )
                                }) =>
                            {
                                replay
                            }
                            _ => {
                                return Err(service_error(anyhow::anyhow!(
                                    "settled callback round terminal identity missing"
                                )))
                            }
                        }
                    };
                    let subscription = terminal_compatible_round_subscription(&replay);
                    return Ok(OpenedCompatibleTurn {
                        initial_run: result.run,
                        from_sequence: Some(i64::MAX),
                        durable_round_replay: replay,
                        ignored_waiting_callback_task_id: if waiting {
                            None
                        } else {
                            ignored_waiting_callback_task_id
                        },
                        subscription,
                        execution: tokio::spawn(async {}),
                    });
                }
            }
        }
    }
    let preparation = async {
        let (live_round_start, mut durable_round_replay) = if fresh_acquisition {
            (None, Vec::new())
        } else if let Some(round_id) = ignored_waiting_callback_task_id {
            compatible_round_replay(&dependencies, &initial_run, round_id).await?
        } else {
            (None, Vec::new())
        };
        let durable_round_established = ignored_waiting_callback_task_id.is_some_and(|round_id| {
            callback_round_replay_established(&durable_round_replay, round_id)
        });
        if !durable_round_established {
            durable_round_replay.clear();
        }
        if let Some(previous_generation) = previous_generation {
            previous_generation
                .close_generation(control_plane::ports::RuntimeEventCloseReason::Expired)
                .await
                .map_err(service_error)?;
        }
        if let Err(error) = dependencies
            .native
            .runtime_event_stream
            .open_run(
                initial_run.id,
                control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
            )
            .await
        {
            warn!(
                flow_run_id = %initial_run.id,
                application_id = %initial_run.application_id,
                error = %error,
                "failed to open compatible public API runtime event stream"
            );
            return Err(service_error(error));
        }

        let from_sequence = if let Some(sequence) = live_round_start {
            Some(sequence)
        } else if let Some(round_id) = ignored_waiting_callback_task_id {
            if !durable_round_established {
                let mut started = debug_stream_events::flow_started(initial_run.id);
                started.payload["response_round_id"] = json!(round_id);
                let started = dependencies
                    .native
                    .runtime_event_stream
                    .append(initial_run.id, started)
                    .await
                    .map_err(service_error)?;
                Some(started.sequence.saturating_sub(1))
            } else {
                None
            }
        } else {
            None
        };
        let subscription = dependencies
            .native
            .runtime_event_stream
            .subscribe(initial_run.id, from_sequence)
            .await
            .map_err(|error| {
                warn!(
                    flow_run_id = %initial_run.id,
                    application_id = %initial_run.application_id,
                    turn_action,
                    error = %error,
                    "failed to subscribe compatible public API runtime event stream"
                );
                service_error(error)
            })?;

        let persister = start_runtime_debug_event_persister(
            dependencies.native.store.clone(),
            dependencies.native.runtime_event_stream.clone(),
            initial_run.id,
        )
        .await;
        Ok::<_, NativeApiError>((from_sequence, durable_round_replay, subscription, persister))
    };
    let preparation = if let Some(owner) = prepared_owner.as_ref() {
        match owner.while_owned(preparation).await {
            Ok(result) => result,
            Err(error) => Err(service_error(error)),
        }
    } else {
        preparation.await
    };
    let (from_sequence, durable_round_replay, subscription, persister) = match preparation {
        Ok(prepared) => prepared,
        Err(error) => {
            if let Some(owner) = prepared_owner.take() {
                if let Err(cleanup_error) = owner.abort().await {
                    warn!(%cleanup_error, "failed to abort compatible callback preparation");
                }
            }
            if let CompatibleTurnAction::ResumeForActor {
                reservation: Some(reservation),
                ..
            } = &mut action
            {
                reservation.release().await;
            }
            return Err(error);
        }
    };
    let terminal_writer = subscription.terminal_writer.clone();
    let background_dependencies = dependencies.clone();
    let background_run = initial_run.clone();
    let execution = tokio::spawn(async move {
        let _execution_invoker = execution_invoker;
        match action {
            CompatibleTurnAction::Start {
                transport_connection_scope,
                observation_context,
            } => {
                if let Err(runtime_error) = runtime_service
                    .start_published_flow_run(StartPublishedFlowRunCommand {
                        application_id: background_run.application_id,
                        flow_run_id: background_run.id,
                        provider_transport_slot,
                        transport_connection_scope,
                        observation_context,
                    })
                    .await
                {
                    warn!(
                        flow_run_id = %background_run.id,
                        application_id = %background_run.application_id,
                        error = %runtime_error,
                        "compatible public API streamed run failed"
                    );
                    if let Err(recovery_error) =
                        recover_missing_stream_terminal_winner_with_dependencies(
                            &native::native_run_terminal_dependencies(
                                &background_dependencies.native,
                            ),
                            &background_run,
                        )
                        .await
                    {
                        warn!(
                            flow_run_id = %background_run.id,
                            application_id = %background_run.application_id,
                            error = %recovery_error,
                            "failed to recover the durable winner after compatible streaming execution ended"
                        );
                    }
                }
            }
            CompatibleTurnAction::ResumeForActor { command, actor, .. } => {
                let result = match prepared_owner {
                    Some(owner) => callback_service.execute_prepared_callback(*owner).await,
                    None => {
                        callback_service
                            .resume_callback_for_actor(actor, command)
                            .await
                    }
                };
                match result {
                    Ok(result) => {
                        append_compatible_resume_terminal_event(
                            terminal_writer.as_ref(),
                            &result.run,
                        )
                        .await
                    }
                    Err(error) => {
                        warn!(
                            flow_run_id = %background_run.id,
                            error = %error,
                            "compatible callback resume failed"
                        );
                        let _ = terminal_writer
                            .append_terminal_if_missing_and_close(debug_stream_events::flow_failed(
                                background_run.id,
                                json!({ "message": error.to_string() }),
                            ))
                            .await;
                    }
                }
            }
        }
        wait_for_runtime_debug_event_persister(
            persister,
            background_run.application_id,
            background_run.id,
        )
        .await;
    });

    Ok(OpenedCompatibleTurn {
        initial_run,
        from_sequence,
        durable_round_replay,
        ignored_waiting_callback_task_id,
        subscription,
        execution,
    })
}
