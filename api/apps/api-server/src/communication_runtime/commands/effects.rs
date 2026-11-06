use super::*;
use crate::routes::assistant::AssistantRunCapabilities;
use control_plane::{
    notifications::{
        apply_communication_model_availability, communication_command_help,
        communication_command_list_reply, decode_builtin_command, BuiltinCommunicationIntent,
        CommunicationCommandIntent,
    },
    orchestration_runtime::CancelFlowRunCommand,
    ports::{
        ClaimedBuiltinCommunicationCommand, CommunicationRepository, OrchestrationRuntimeRepository,
    },
};
use domain::CommunicationCommandView;
use extension_contracts::{
    CommunicationCommandReply, CommunicationMessageBlock as Block,
    CommunicationMessageContent as Content, CommunicationMessageInline as Inline,
};

const NEW_SESSION_REPLY: &str =
    "The previous conversation has stopped generating. A new conversation has started.";

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            return;
        }
        if let Err(error) = process_next(&state, key, stop.clone()).await {
            tracing::warn!(workspace_id=%key.0,instance_id=%key.1,%error,"communication builtin command deferred");
        }
        if !wait(&mut stop).await {
            return;
        }
    }
}

pub(super) async fn process_next(
    state: &ApiState,
    key: (Uuid, Uuid),
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let claim = {
        let _permit = match state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        ) {
            Ok(permit) => permit,
            Err(_) => return Ok(()),
        };
        state
            .notification_service
            .claim_builtin_command(key.0, key.1, LEASE_SECONDS)
            .await?
    };
    let Some(claim) = claim else {
        return Ok(());
    };
    let work = execute(state, &claim);
    tokio::pin!(work);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(u64::from(LEASE_SECONDS / 3)));
    heartbeat.tick().await;
    loop {
        tokio::select! {
            biased;
            _ = stop.changed() => return Ok(()),
            result = &mut work => return result,
            _ = heartbeat.tick() => {
                let _permit = state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)?;
                ensure!(state.notification_service.renew_builtin_command_claim(&claim,LEASE_SECONDS).await?,"command effect claim lost");
            }
        }
    }
}

async fn execute(state: &ApiState, claim: &ClaimedBuiltinCommunicationCommand) -> Result<()> {
    let record = &claim.record;
    let scope = &record.scope;
    let binding = active_binding(state, (scope.workspace_id, scope.instance_id)).await?;
    let actor = actor_for_binding(&state.store, &binding).await?;
    let requested_session = Uuid::parse_str(&record.request.session_id)?;
    let intent = decode_builtin_command(record)?;
    let mut commands = state
        .notification_service
        .event_command_catalog(
            &actor,
            &binding.config.command_settings,
            &binding.config.event_commands,
        )
        .await?;
    let (response_session, text, failure) = match intent {
        CommunicationCommandIntent::Rejected { code, reply } => {
            (requested_session, reply.into(), Some(code))
        }
        CommunicationCommandIntent::Event {
            command_id,
            arguments,
        } => {
            return execute_event(
                state,
                claim,
                &actor,
                &binding,
                &commands,
                &command_id,
                &arguments,
            )
            .await;
        }
        CommunicationCommandIntent::Builtin(builtin) => {
            if let Some((code, reply)) = unavailable(&commands, &record.request.command_id) {
                return complete(state, claim, requested_session, reply.into(), Some(code)).await;
            }
            match builtin {
                BuiltinCommunicationIntent::New => {
                    let rotation = {
                        let _permit = state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)?;
                        state
                            .notification_service
                            .rotate_session(scope, &record.event_id.to_string())
                            .await?
                            .context("command session rotation unavailable")?
                    };
                    // Both owners must be signalled even if one cannot join. A bounded
                    // failure is a durable command result, never a stopped-generation ACK.
                    let mut join_failed = false;
                    for session_id in rotation
                        .invalidated_session_id
                        .into_iter()
                        .chain(std::iter::once(scope.conversation_id))
                    {
                        if let Err(error) = state
                            .communication_activity
                            .cancel_session(session_id)
                            .await
                        {
                            join_failed = true;
                            tracing::warn!(%session_id, %error, "communication_provider_cancel_join_failed");
                        }
                    }
                    for run_id in rotation.cancel_run_ids {
                        let application_id = state
                            .store
                            .communication_run_application_id(scope.workspace_id, run_id)
                            .await?
                            .context("command cancellation run application missing")?;
                        let current = state
                            .store
                            .get_flow_run(application_id, run_id)
                            .await?
                            .context("command cancellation run missing")?;
                        if current.status.is_terminal() {
                            continue;
                        }
                        let _permit = state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)?;
                        let cancellation = super::super::execution::runtime(
                            state,
                            state.store.for_actor(actor.clone()),
                        )
                        .cancel_flow_run(CancelFlowRunCommand {
                            actor_user_id: actor.user_id,
                            application_id,
                            flow_run_id: run_id,
                        })
                        .await;
                        if let Err(error) = cancellation {
                            // A task may have reached a durable terminal while cancellation joined it.
                            if !state
                                .store
                                .get_flow_run(application_id, run_id)
                                .await?
                                .is_some_and(|run| run.status.is_terminal())
                            {
                                return Err(error);
                            }
                        }
                    }
                    if join_failed {
                        return complete(
                            state,
                            claim,
                            rotation.session.id,
                            "The previous conversation could not be confirmed stopped. Replies from it are blocked. A new conversation has been created.".into(),
                            Some("communication_provider_cancel_join_failed".into()),
                        ).await;
                    }
                    (rotation.session.id, NEW_SESSION_REPLY.into(), None)
                }
                BuiltinCommunicationIntent::Help | BuiltinCommunicationIntent::Model { .. } => {
                    let current = state
                        .notification_service
                        .active_session(scope)
                        .await?
                        .context("command session missing")?;
                    if current.id != requested_session {
                        return complete(
                            state,
                            claim,
                            requested_session,
                            "This command belongs to a previous conversation.".into(),
                            Some("command_session_changed".into()),
                        )
                        .await;
                    }
                    let (capabilities, reason) = super::super::command_capabilities::load(
                        &state.store,
                        &actor,
                        Some(binding.config.application_id),
                    )
                    .await
                    .map_err(|error| error.0)?;
                    apply_communication_model_availability(&mut commands, reason);
                    match builtin {
                        BuiltinCommunicationIntent::Help => (
                            requested_session,
                            communication_command_help(&commands)?.into(),
                            None,
                        ),
                        BuiltinCommunicationIntent::Model { model } => {
                            if let Some((code, reply)) =
                                unavailable(&commands, &record.request.command_id)
                            {
                                (requested_session, reply.into(), Some(code))
                            } else {
                                return apply_model(
                                    state,
                                    claim,
                                    &capabilities,
                                    model.as_deref(),
                                    current
                                        .selected_model
                                        .as_deref()
                                        .or(binding.config.model.as_deref()),
                                )
                                .await;
                            }
                        }
                        BuiltinCommunicationIntent::New => unreachable!("handled above"),
                    }
                }
            }
        }
        _ => anyhow::bail!("builtin claim contains non-builtin intent"),
    };
    complete(state, claim, response_session, text, failure).await
}

fn unavailable(
    commands: &[CommunicationCommandView],
    command_id: &str,
) -> Option<(String, String)> {
    let matching = commands
        .iter()
        .filter(|entry| entry.command_id == command_id)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Some((
            "command_unavailable".into(),
            "This command is unavailable. Check channel command settings.".into(),
        ));
    }
    let command = matching[0];
    if !command.enabled {
        return Some((
            "command_disabled".into(),
            "This command is disabled for this channel.".into(),
        ));
    }
    if command.available {
        return None;
    }
    let reason = command
        .unavailable_reason
        .as_deref()
        .unwrap_or("command_unavailable");
    let text = match reason {
        "model_selection_unavailable" => "The selected application does not allow model selection.",
        "no_models_available" => "The selected application has no allowed models.",
        "application_unavailable" => {
            "The selected application is unavailable or you do not have access to it."
        }
        _ => "This command is unavailable. Check channel command settings.",
    };
    Some((reason.into(), text.into()))
}

pub(super) async fn apply_model(
    state: &ApiState,
    claim: &ClaimedBuiltinCommunicationCommand,
    capabilities: &AssistantRunCapabilities,
    requested: Option<&str>,
    current: Option<&str>,
) -> Result<()> {
    let session = Uuid::parse_str(&claim.record.request.session_id)?;
    if !capabilities.model_selection_enabled || capabilities.models.is_empty() {
        let reason = if capabilities.model_selection_enabled {
            "no_models_available"
        } else {
            "model_selection_unavailable"
        };
        return complete(
            state,
            claim,
            session,
            "The selected application has no selectable models.".into(),
            Some(reason.into()),
        )
        .await;
    }
    if let Some(requested) = requested {
        let Some(model) = capabilities
            .models
            .iter()
            .find(|model| model.id == requested)
        else {
            return complete(state,claim,session,"This model is not allowed for the selected application. Use /model to see allowed models.".into(),Some("model_not_allowed".into())).await;
        };
        let _permit = state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        )?;
        state
            .notification_service
            .select_command_model(
                claim,
                &model.id,
                CommunicationCommandReply::from_content(Content::Blocks(vec![Block::Paragraph(
                    vec![
                        Inline::Text(format!(
                            "Selected model: {} (",
                            model.name.as_deref().unwrap_or(&model.id)
                        )),
                        Inline::Code(model.id.clone()),
                        Inline::Text(").".into()),
                    ],
                )]))?,
            )
            .await?;
        return Ok(());
    }
    let reply = communication_command_list_reply(
        vec![
            Block::Paragraph(vec![
                Inline::Text("Current model: ".into()),
                Inline::Code(current.unwrap_or("Application default").into()),
            ]),
            Block::Paragraph(vec![Inline::Text("Allowed models:".into())]),
        ],
        capabilities
            .models
            .iter()
            .map(|model| {
                vec![
                    Inline::Text(format!("{} — ", model.name.as_deref().unwrap_or(&model.id))),
                    Inline::Code(format!("/model-user-{}", model.id)),
                ]
            })
            .collect(),
    )?;
    complete(state, claim, session, reply.into(), None).await
}

async fn complete(
    state: &ApiState,
    claim: &ClaimedBuiltinCommunicationCommand,
    session: Uuid,
    content: Content,
    failure: Option<String>,
) -> Result<()> {
    let _permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    state
        .notification_service
        .complete_builtin_command(
            claim,
            session,
            CommunicationCommandReply::from_content(content)?,
            failure,
        )
        .await?;
    Ok(())
}

/// No await occurs between synchronous publish and forming its reply. A reclaimed uncertain
/// Host claim is terminal; it must never replay an event which may already have been admitted.
async fn execute_event(
    state: &ApiState,
    claim: &ClaimedBuiltinCommunicationCommand,
    actor: &domain::ActorContext,
    binding: &domain::CommunicationBindingRecord,
    commands: &[CommunicationCommandView],
    command_id: &str,
    arguments: &str,
) -> Result<()> {
    let session = Uuid::parse_str(&claim.record.request.session_id)?;
    if claim.record.failure_code.as_deref() == Some("command_outcome_unknown") {
        return complete(
            state,
            claim,
            session,
            "The prior event publication outcome is unknown. It will not be retried automatically."
                .into(),
            Some("command_outcome_unknown".into()),
        )
        .await;
    }
    if let Some((code, reply)) = unavailable(commands, command_id) {
        return complete(state, claim, session, reply.into(), Some(code)).await;
    }
    let result = async {
        let current = state.notification_service.active_session(&claim.record.scope).await?.context("command session missing")?;
        ensure!(current.id == session, "command session changed");
        let command = binding.config.event_commands.iter().find(|c| c.command_id == command_id).context("event command removed")?;
        let target = state.notification_service.event_command_definition(actor, command).await?;
        ensure!(target.workspace_id == claim.record.scope.workspace_id && binding.workspace_id == target.workspace_id, "event command workspace mismatch");
        let context = serde_json::json!({"arguments": arguments, "input_id": claim.record.event_id, "conversation_id": claim.record.scope.conversation_id, "instance_id": binding.instance_id});
        let payload = control_plane::notifications::communication_event_payload(command, &context, &target)?;
        let live_binding = active_binding(state, (binding.workspace_id, binding.instance_id)).await?;
        ensure!(live_binding.revision == binding.revision, "event command binding changed");
        let _permit = state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)?;
        // Recheck the live lease after asynchronous configuration and authorization reads.
        ensure!(state.notification_service.renew_builtin_command_claim(claim, LEASE_SECONDS).await?, "command effect claim lost");
        state.event_center.publish_with_ancestors(&target, payload, state.store.workflow_ancestor_ids())
    }.await;
    let (text, failure) = match result {
        Ok(receipt) => event_publication_reply(&receipt),
        Err(error) => {
            tracing::warn!(%error, command_id, "communication event command rejected");
            ("Event publication was rejected. Check the arguments, event definition and channel permissions.".to_string(), Some("event_command_publish_rejected".into()))
        }
    };
    complete(state, claim, session, text.into(), failure).await
}

pub(super) fn event_publication_reply(
    receipt: &control_plane::event_center::EventPublishReceipt,
) -> (String, Option<String>) {
    use control_plane::event_center::EventTargetAdmissionStatus;
    let accepted = receipt
        .targets
        .iter()
        .filter(|t| t.status == EventTargetAdmissionStatus::Accepted)
        .count();
    let rejected = receipt.targets.len() - accepted;
    let text = if receipt.targets.is_empty() {
        "Event published with 0 receiving targets.".into()
    } else {
        format!("Event publication: {accepted} targets accepted, {rejected} targets rejected. Workflow execution has not been confirmed.")
    };
    (
        text,
        (rejected > 0).then(|| {
            if accepted > 0 {
                "event_publication_partial".into()
            } else {
                "event_publication_rejected".into()
            }
        }),
    )
}
