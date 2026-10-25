use super::*;
use axum::http::HeaderMap;
use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    application_public_api::run_service::{
        ApplicationPublishedRunService, CreateAssistantRunCommand,
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::{
        ClaimedCommunicationInput, CommunicationClaim, CommunicationProgress,
        ListApplicationRunConversationMessageItemsPageInput, OrchestrationRuntimeRepository,
    },
};
use domain::{CommunicationHistoryScope, CommunicationProcessingStatus, NotificationSource};
use serde_json::json;
use std::sync::atomic::{AtomicU8, Ordering};

pub(super) async fn run(state: Arc<ApiState>, key: (Uuid, Uuid), mut stop: watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            return;
        }
        let _ = process_next(&state, key, stop.clone()).await;
        if *stop.borrow() {
            return;
        }
        tokio::select! { _ = stop.changed() => return, _ = tokio::time::sleep(RETRY_DELAY) => {} }
    }
}

async fn process_next(
    state: &Arc<ApiState>,
    key: (Uuid, Uuid),
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let _write_permit = match state
        .system_maintenance
        .try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)
    {
        Ok(permit) => permit,
        Err(_) => return Ok(()),
    };
    let Some(input) = state
        .notification_service
        .claim_pending_input(key.0, key.1, LEASE_SECONDS)
        .await?
    else {
        return Ok(());
    };
    let claim = CommunicationClaim {
        workspace_id: key.0,
        instance_id: key.1,
        message_id: input.message.id,
        claim_id: input.claim_id,
    };
    let mut heartbeat = tokio::time::interval(Duration::from_secs(u64::from(LEASE_SECONDS / 3)));
    heartbeat.tick().await;
    let stage = AtomicU8::new(
        if input.message.processing_status == CommunicationProcessingStatus::Succeeded {
            2
        } else {
            0
        },
    );
    let work = process(state, &input, &claim, &stage);
    tokio::pin!(work);
    loop {
        tokio::select! {
            _ = stop.changed() => {
                let _ = state.notification_service.record_progress(&claim,interrupted(&stage,"host_stopped")).await;
                return Ok(());
            }
            result = &mut work => {
                if result.is_err() {
                    let _ = state.notification_service.record_progress(&claim,interrupted(&stage,"communication_execution_failed")).await;
                }
                return result;
            }
            _ = heartbeat.tick() => {
                if !state.notification_service.renew_claim(&claim,LEASE_SECONDS).await? {
                    // Preserve terminal evidence if disable raced work already in flight.
                    let _ = state.notification_service.record_progress(&claim,interrupted(&stage,"claim_lost")).await;
                    return Ok(());
                }
            }
        }
    }
}
async fn validated_actor(
    state: &ApiState,
    input: &ClaimedCommunicationInput,
) -> Result<ActorContext> {
    let current = active_binding(
        state,
        (input.binding.workspace_id, input.binding.instance_id),
    )
    .await?;
    ensure!(
        current.revision == input.binding.revision,
        "binding_changed"
    );
    let actor = actor(state, &current).await?;
    crate::routes::assistant::validate_preference(
        &state.store,
        &actor,
        &crate::routes::assistant::AssistantPreferenceBody {
            application_id: Some(current.config.application_id),
            mcp_instance_ids: current.config.mcp_instance_ids.clone(),
            model: current.config.model.clone(),
            reasoning_effort: current.config.reasoning_effort.clone(),
            enabled_client_tools: Vec::new(),
        },
    )
    .await
    .map_err(|_| anyhow::anyhow!("assistant_capability_denied"))?;
    ApplicationService::new(state.store.for_actor(actor.clone()))
        .load_application_for_non_crud_console_operation(
            actor.user_id,
            current.config.application_id,
            ApplicationNonCrudConsoleOperation::Run,
        )
        .await?;
    attachments::validate_target(state, &actor, &current).await?;
    Ok(actor)
}
async fn progress(
    state: &ApiState,
    claim: &CommunicationClaim,
    value: CommunicationProgress,
) -> Result<()> {
    ensure!(
        state
            .notification_service
            .record_progress(claim, value)
            .await?,
        "claim_lost"
    );
    Ok(())
}
async fn process(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationInput,
    claim: &CommunicationClaim,
    stage: &AtomicU8,
) -> Result<()> {
    validated_actor(state, input).await?;
    let reply = if input.message.processing_status == CommunicationProcessingStatus::Succeeded {
        input.message.final_reply.clone().context("reply_missing")?
    } else {
        // A crash with a reserved run is handled as interrupted by the durable owner. Never
        // blindly restart an existing run (tools may already have performed external work).
        ensure!(input.message.run_id.is_none(), "run_already_reserved");
        let files = attachments::attach(state, input, claim).await?;
        let actor = validated_actor(state, input).await?;
        let inner = crate::runtime_internal_tool_invoker_factory(state, &actor)
            .await
            .map_err(|_| anyhow::anyhow!("mcp_access_denied"))?
            .create_server_delegated(
                HeaderMap::new(),
                actor.clone(),
                input.binding.config.mcp_instance_ids.clone(),
            )
            .await
            .map_err(|_| anyhow::anyhow!("mcp_access_denied"))?;
        let invoker = Arc::new(scoped_tools::ScopedTools {
            inner: Arc::new(inner),
            scope: CommunicationHistoryScope {
                workspace_id: input.binding.workspace_id,
                instance_id: input.binding.instance_id,
                external_identity: input.conversation.external_identity.clone(),
            },
        });
        let run = ApplicationPublishedRunService::new(state.store.clone())
            .with_published_plan_cache(state.infrastructure.published_plan_cache())
            .with_published_publication_cache(state.infrastructure.published_publication_cache())
            .create_communication_run(CreateAssistantRunCommand {
                actor_user_id: actor.user_id,
                workspace_id: actor.current_workspace_id,
                application_id: input.binding.config.application_id,
                assistant_conversation_id: None,
                page_references: Vec::new(),
                request: request::request(input, files)?,
            })
            .await
            .map_err(|_| anyhow::anyhow!("communication_run_invalid"))?;
        progress(
            state,
            claim,
            CommunicationProgress::RunStarted { run_id: run.id },
        )
        .await?;
        let runtime = OrchestrationRuntimeService::new(
            state.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                state.provider_runtime.clone(),
                state.runtime_activity.clone(),
            ),
            state.runtime_engine.clone(),
            state.provider_secret_master_key.clone(),
            state.infrastructure.provider_transport_store(),
            state.model_billing_require_provider_usage,
        )
        .with_published_plan_cache(state.infrastructure.published_plan_cache())
        .with_node_artifact_context(
            state.api_node_id.clone(),
            state.provider_install_root.clone(),
        )
        .with_file_storage_registry(state.file_storage_registry.clone())
        .with_notification_sender(state.notification_service.clone())
        .with_runtime_internal_tool_invoker(invoker)
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .with_runtime_event_stream(state.runtime_event_stream.clone());
        let detail = runtime
            .start_published_flow_run(StartPublishedFlowRunCommand {
                transport_connection_scope: None,
                observation_context: None,
                application_id: input.binding.config.application_id,
                flow_run_id: run.id,
                provider_transport_slot: None,
            })
            .await?;
        if detail.flow_run.status != domain::FlowRunStatus::Succeeded {
            let code = if detail.flow_run.status == domain::FlowRunStatus::WaitingCallback {
                "run_waiting_callback"
            } else {
                "run_not_succeeded"
            };
            progress(
                state,
                claim,
                CommunicationProgress::RunFailed {
                    failure_code: code.into(),
                },
            )
            .await?;
            return Ok(());
        }
        let page = state
            .store
            .list_application_run_conversation_message_items_page(
                input.binding.config.application_id,
                run.id,
                ListApplicationRunConversationMessageItemsPageInput {
                    before_sequence: None,
                    after_sequence: None,
                    limit: 1,
                },
            )
            .await?;
        let reply = page
            .items
            .iter()
            .find_map(request::final_reply)
            .context("final_answer_missing")?;
        progress(
            state,
            claim,
            CommunicationProgress::RunSucceeded {
                final_reply: reply.clone(),
            },
        )
        .await?;
        stage.store(2, Ordering::SeqCst);
        reply
    };
    // Revalidate live account, membership, application/model/MCP/file selection and channel
    // state once more immediately before reserving the provider send.
    let actor = validated_actor(state, input).await?;
    progress(state, claim, CommunicationProgress::SendStarted).await?;
    stage.store(3, Ordering::SeqCst);
    let result = state
        .notification_service
        .send_to(
            &actor,
            NotificationSource::Workflow,
            input.binding.instance_id,
            json!({"text":reply}),
            extension_contracts::NotificationSendTarget {
                sender_id: input.conversation.external_identity.clone(),
                conversation_id: input.conversation.external_conversation_key.clone(),
                provider_message_id: input.message.external_message_key.clone(),
            },
        )
        .await;
    let terminal = match result {
        Ok(_) => CommunicationProgress::SendSucceeded,
        Err(error) => send_failure(&error),
    };
    progress(state, claim, terminal).await?;
    stage.store(4, Ordering::SeqCst);
    Ok(())
}
fn interrupted(stage: &AtomicU8, code: &str) -> CommunicationProgress {
    match stage.load(Ordering::SeqCst) {
        2 => CommunicationProgress::SendFailed {
            failure_code: code.into(),
        },
        3.. => CommunicationProgress::SendUnknown {
            failure_code: "provider_result_unknown".into(),
        },
        _ => CommunicationProgress::RunFailed {
            failure_code: code.into(),
        },
    }
}
fn send_failure(error: &anyhow::Error) -> CommunicationProgress {
    use control_plane::errors::ControlPlaneError as Error;
    let known = match error.downcast_ref::<Error>() {
        Some(
            Error::Conflict(code)
            | Error::InvalidInput(code)
            | Error::PermissionDenied(code)
            | Error::NotFound(code),
        ) => Some(*code),
        Some(Error::UpstreamUnavailable("provider_rejected")) => Some("provider_rejected"),
        Some(Error::UpstreamUnavailable("subscription_unavailable")) => {
            Some("subscription_unavailable")
        }
        _ => None,
    };
    match known {
        Some(code) => CommunicationProgress::SendFailed {
            failure_code: code.to_owned(),
        },
        None => CommunicationProgress::SendUnknown {
            failure_code: "provider_result_unknown".into(),
        },
    }
}

#[cfg(test)]
#[path = "_tests/execution.rs"]
mod tests;
