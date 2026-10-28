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
        CommunicationRepository, ListApplicationRunConversationMessageItemsPageInput,
        OrchestrationRuntimeRepository,
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
    let write_permit = match state
        .system_maintenance
        .try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution)
    {
        Ok(permit) => permit,
        Err(_) => return Ok(()),
    };
    let Some(input) = state
        .notification_service
        .claim_pending_legacy_input(key.0, key.1, LEASE_SECONDS)
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
    let mut write_permit = Some(write_permit);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(u64::from(LEASE_SECONDS / 3)));
    heartbeat.tick().await;
    let stage = AtomicU8::new(
        if input.message.processing_status == CommunicationProcessingStatus::Succeeded {
            2
        } else if input.message.run_id.is_some() {
            1
        } else {
            0
        },
    );
    let work = process(state, &input, &claim, &stage);
    tokio::pin!(work);
    loop {
        // Observation is read-only and must not block backup/recovery while a human or
        // external callback is pending. Actual execution owns its own admission permit.
        if stage.load(Ordering::SeqCst) == 1 {
            write_permit.take();
        }
        tokio::select! {
            _ = stop.changed() => {
                record_interruption(state,&claim,&stage,"host_stopped").await;
                return Ok(());
            }
            result = &mut work => {
                if result.is_err() {
                    record_interruption(state,&claim,&stage,"communication_execution_failed").await;
                }
                return result;
            }
            _ = heartbeat.tick() => {
                if stage.load(Ordering::SeqCst)==1 { write_permit.take(); }
                let renewal_permit=if write_permit.is_none() {
                    match state.system_maintenance.try_enter_write(control_plane::system_recovery::SystemWriteOwner::CommunicationExecution) {
                        Ok(permit)=>Some(permit),Err(_)=>return Ok(()),
                    }
                } else {None};
                if !state.notification_service.renew_claim(&claim,LEASE_SECONDS).await? {
                    // Preserve terminal evidence if disable raced work already in flight.
                    record_interruption(state,&claim,&stage,"claim_lost").await;
                    return Ok(());
                }
                drop(renewal_permit);
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
    validate_binding(&state.store, &current).await
}

pub(crate) async fn validate_binding(
    store: &storage_durable_postgres::MainDurableStore,
    current: &CommunicationBindingRecord,
) -> Result<ActorContext> {
    let instance = store
        .get_notification_instance(current.workspace_id, current.instance_id)
        .await?
        .context("channel_missing")?;
    ensure!(
        instance.status == domain::NotificationInstanceStatus::Enabled,
        "channel_disabled"
    );
    let actor = actor_for_binding(store, current).await?;
    crate::routes::assistant::validate_preference(
        store,
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
    ApplicationService::new(store.for_actor(actor.clone()))
        .load_application_for_non_crud_console_operation(
            actor.user_id,
            current.config.application_id,
            ApplicationNonCrudConsoleOperation::Run,
        )
        .await?;
    attachments::validate_target(store, &actor, current).await?;
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
    let mut linked_run_id = input.message.run_id;
    let reply = if input.message.processing_status == CommunicationProcessingStatus::Succeeded {
        input.message.final_reply.clone().context("reply_missing")?
    } else {
        let run_id = linked_or_start(input.message.run_id, || {
            start_new(state, input, claim, stage)
        })
        .await?;
        linked_run_id = Some(run_id);
        let Some(reply) = observe(state, input, claim, run_id).await? else {
            return Ok(());
        };
        stage.store(2, Ordering::SeqCst);
        reply
    };
    // Revalidate live account, membership, application/model/MCP/file selection and channel
    // state once more immediately before reserving the provider send.
    let original_application_id = state
        .store
        .communication_run_application_id(
            input.binding.workspace_id,
            linked_run_id.context("linked_run_missing")?,
        )
        .await?
        .context("linked_run_missing")?;
    ensure!(
        original_application_id == input.binding.config.application_id,
        "binding_application_changed"
    );
    let _send_permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
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
async fn linked_or_start<F, Fut>(existing: Option<Uuid>, start: F) -> Result<Uuid>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Uuid>>,
{
    match existing {
        Some(id) => Ok(id),
        None => start().await,
    }
}

async fn start_new(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationInput,
    claim: &CommunicationClaim,
    stage: &AtomicU8,
) -> Result<Uuid> {
    // Creation/linkage is durable before dispatch. The observer can dispatch this queued
    // reservation after any process interruption; creation itself is never replayed.
    validated_actor(state, input).await?;
    let _creation_permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    let files = attachments::attach(state, input, claim).await?;
    let actor = validated_actor(state, input).await?;
    let run = ApplicationPublishedRunService::new(state.store.for_actor(actor.clone()))
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
    stage.store(1, Ordering::SeqCst);
    progress(
        state,
        claim,
        CommunicationProgress::RunStarted { run_id: run.id },
    )
    .await?;
    Ok(run.id)
}

pub(crate) fn runtime(
    state: &ApiState,
    store: storage_durable_postgres::MainDurableStore,
) -> OrchestrationRuntimeService<
    storage_durable_postgres::MainDurableStore,
    crate::provider_runtime::ApiProviderRuntime,
> {
    OrchestrationRuntimeService::new(
        store,
        crate::provider_runtime::ApiProviderRuntime::new_with_activity(
            state.provider_runtime.clone(),
            state.runtime_activity.clone(),
        )
        .with_communication_activity(state.communication_activity.clone()),
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
    .with_communication_execution_resolver(state.communication_activity.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_runtime_event_stream(state.runtime_event_stream.clone())
}

async fn spawn_dispatch(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationInput,
    application_id: Uuid,
    run_id: Uuid,
) -> Result<tokio::task::JoinHandle<()>> {
    let execution_permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    ensure!(
        input.binding.config.application_id == application_id,
        "binding_application_changed"
    );
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

    let runtime =
        runtime(state, state.store.for_actor(actor)).with_runtime_internal_tool_invoker(invoker);
    let session = state
        .communication_activity
        .session(input.conversation.id, 0)?;
    let lease = session.enter()?;
    Ok(tokio::spawn(async move {
        let _permit = execution_permit;
        let _lease = lease;
        let work = async {
            if runtime
                .dispatch_communication_run(StartPublishedFlowRunCommand {
                    transport_connection_scope: None,
                    observation_context: None,
                    application_id,
                    flow_run_id: run_id,
                    provider_transport_slot: None,
                })
                .await
                .is_err()
            {
                tracing::warn!(%run_id,"communication_dispatch_failed");
            }
        };
        tokio::select! { biased; _ = session.cancelled() => {}, _ = work => {} }
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunDisposition {
    Observe,
    FinalAnswer,
    Failed(&'static str),
}
fn run_disposition(status: domain::FlowRunStatus) -> RunDisposition {
    use domain::FlowRunStatus as Status;
    match status {
        Status::Succeeded => RunDisposition::FinalAnswer,
        Status::Failed => RunDisposition::Failed("run_failed"),
        Status::Cancelled => RunDisposition::Failed("run_cancelled"),
        Status::Incomplete => RunDisposition::Failed("run_incomplete"),
        Status::Queued
        | Status::Running
        | Status::WaitingCallback
        | Status::WaitingHuman
        | Status::Paused => RunDisposition::Observe,
    }
}
async fn observe(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationInput,
    claim: &CommunicationClaim,
    run_id: Uuid,
) -> Result<Option<String>> {
    let application_id = state
        .store
        .communication_run_application_id(input.binding.workspace_id, run_id)
        .await?
        .context("linked_run_missing")?;
    // Keep legacy callback/human resumes under the same epoch-zero owner while observing.
    let session = state
        .communication_activity
        .session(input.conversation.id, 0)?;
    let _provider_registration = state
        .communication_activity
        .register_provider_run(run_id, session)?;
    let mut dispatch: Option<tokio::task::JoinHandle<()>> = None;
    loop {
        let metadata = state
            .store
            .get_flow_run_metadata(application_id, run_id)
            .await?
            .context("linked_run_missing")?;
        ensure!(
            metadata.created_by == input.binding.actor_user_id,
            "linked_run_actor_mismatch"
        );
        if matches!(
            metadata.status,
            domain::FlowRunStatus::Queued | domain::FlowRunStatus::Running
        ) && !state.store.flow_run_has_resume_claims(run_id).await?
            && dispatch.as_ref().is_none_or(|task| task.is_finished())
        {
            match spawn_dispatch(state, input, application_id, run_id).await {
                Ok(task) => dispatch = Some(task),
                Err(_) => {
                    let _permit = state.system_maintenance.try_enter_write(
                        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
                    )?;
                    runtime(state, state.store.clone())
                        .reject_communication_dispatch(application_id, run_id)
                        .await?;
                }
            }
        }
        match run_disposition(metadata.status) {
            RunDisposition::Observe => {}
            RunDisposition::Failed(code) => {
                let _permit = state.system_maintenance.try_enter_write(
                    control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
                )?;
                progress(
                    state,
                    claim,
                    CommunicationProgress::RunFailed {
                        failure_code: code.into(),
                    },
                )
                .await?;
                return Ok(None);
            }
            RunDisposition::FinalAnswer => {
                let page = state
                    .store
                    .list_application_run_conversation_message_items_page(
                        application_id,
                        run_id,
                        ListApplicationRunConversationMessageItemsPageInput {
                            before_sequence: None,
                            after_sequence: None,
                            limit: 1,
                        },
                    )
                    .await?;
                if let Some(reply) = page.items.iter().find_map(request::final_reply) {
                    let _permit = state.system_maintenance.try_enter_write(
                        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
                    )?;
                    progress(
                        state,
                        claim,
                        CommunicationProgress::RunSucceeded {
                            final_reply: reply.clone(),
                        },
                    )
                    .await?;
                    return Ok(Some(reply));
                }
                // Projection may finish after the run does. Keep observing until the canonical
                // business-turn owner publishes its final answer; never send provisional output.
            }
        }
        tokio::time::sleep(RETRY_DELAY).await;
    }
}
async fn record_interruption(
    state: &ApiState,
    claim: &CommunicationClaim,
    stage: &AtomicU8,
    code: &str,
) {
    if let Some(progress) = interrupted(stage, code) {
        let Ok(_permit) = state.system_maintenance.try_enter_write(
            control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
        ) else {
            return;
        };
        let _ = state
            .notification_service
            .record_progress(claim, progress)
            .await;
    }
}
fn interrupted(stage: &AtomicU8, code: &str) -> Option<CommunicationProgress> {
    Some(match stage.load(Ordering::SeqCst) {
        // Linked nonterminal work stays processing; expiry releases only its observation claim.
        1 | 4 => return None,
        2 => CommunicationProgress::SendFailed {
            failure_code: code.into(),
        },
        3.. => CommunicationProgress::SendUnknown {
            failure_code: "provider_result_unknown".into(),
        },
        _ => CommunicationProgress::RunFailed {
            failure_code: code.into(),
        },
    })
}
pub(super) fn send_failure(error: &anyhow::Error) -> CommunicationProgress {
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
