use super::*;
use axum::http::HeaderMap;
use control_plane::{
    application_public_api::run_service::{
        ApplicationPublishedRunService, CreateAssistantRunCommand,
    },
    orchestration_runtime::StartPublishedFlowRunCommand,
};

pub(super) async fn create(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationTurn,
    preparer: &dyn CommunicationInputPreparer,
) -> Result<Uuid> {
    let _permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
    let actor = validated_actor(state, input).await?;
    let pending = state
        .notification_service
        .turn_inputs(&input.claim)
        .await?
        .context("communication_turn_fenced")?;
    let prepared = preparer
        .prepare_inputs(&input.claim, pending.inputs)
        .await?;
    let first = prepared
        .iter()
        .position(|input| input.input_sequence == 1)
        .context("communication_anchor_missing")?;
    let request = request::turn_request(input, &prepared[first..first + 1])?;
    let run = ApplicationPublishedRunService::new(state.store.for_actor(actor.clone()))
        .with_published_plan_cache(state.infrastructure.published_plan_cache())
        .with_published_publication_cache(state.infrastructure.published_publication_cache())
        .create_communication_run(CreateAssistantRunCommand {
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            application_id: input.binding.config.application_id,
            assistant_conversation_id: None,
            page_references: vec![],
            request,
        })
        .await
        .map_err(|error| match error {
            control_plane::application_public_api::native::NativeRunError::Service(source) => {
                source
            }
            control_plane::application_public_api::native::NativeRunError::Validation(_) => {
                anyhow::anyhow!("communication_run_invalid")
            }
        })?;
    // Linking checks every admitted attachment. A member arriving while the request was
    // created is imported before retrying the same linkage, never by creating another run.
    while !state
        .notification_service
        .link_turn_run(&input.claim, run.id)
        .await?
    {
        let pending = state
            .notification_service
            .turn_inputs(&input.claim)
            .await?
            .context("communication_turn_fenced")?;
        ensure!(
            pending.turn.run_id.is_none(),
            "communication_run_already_linked"
        );
        preparer
            .prepare_inputs(&input.claim, pending.inputs)
            .await?;
    }
    Ok(run.id)
}

pub(super) async fn execute(
    state: &Arc<ApiState>,
    input: &ClaimedCommunicationTurn,
    run_id: Uuid,
) -> Result<()> {
    let _permit = state.system_maintenance.try_enter_write(
        control_plane::system_recovery::SystemWriteOwner::CommunicationExecution,
    )?;
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
        scope: domain::CommunicationHistoryScope {
            workspace_id: input.claim.scope.workspace_id,
            instance_id: input.claim.scope.instance_id,
            external_identity: input.claim.scope.external_identity.clone(),
        },
    });
    super::super::execution::runtime(state, state.store.for_actor(actor))
        .with_runtime_internal_tool_invoker(invoker)
        .dispatch_communication_run(StartPublishedFlowRunCommand {
            transport_connection_scope: None,
            observation_context: None,
            application_id: input.binding.config.application_id,
            flow_run_id: run_id,
            provider_transport_slot: None,
        })
        .await?;
    Ok(())
}
