use std::sync::Arc;

use serde_json::{json, Value};

use super::helpers::{
    compiled_plan_start_node_id, inject_application_environment_variables, inject_system_variables,
};
use super::*;
use crate::orchestration_runtime::live_debug_run::PersistedNodeLifecycle;
use crate::orchestration_runtime::persistence::PersistFlowDebugOutcomeInput;

pub(super) async fn continue_flow_debug_run_inner<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: &ContinueFlowDebugRunCommand,
    live_provider_events: Option<LiveProviderStreamEventSender>,
    compact_response_ingress: Option<
        orchestration_runtime::execution_state::CompactResponseIngress,
    >,
    provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone
        + Send
        + Sync,
{
    let flow_run = service
        .repository
        .get_flow_run(command.application_id, command.flow_run_id)
        .await?
        .ok_or_else(|| anyhow!("flow run not found"))?;
    if flow_run.status != domain::FlowRunStatus::Running {
        return load_run_detail(&service.repository, command.application_id, flow_run.id).await;
    }

    let actor = crate::ports::ApplicationRepository::load_actor_context_for_user(
        &service.repository,
        flow_run.created_by,
    )
    .await?;
    let application = service
        .repository
        .get_application(command.workspace_id, command.application_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("application"))?;
    let compiled_plan_id = flow_run
        .compiled_plan_id
        .ok_or_else(|| anyhow!("flow run compiled plan is not attached"))?;
    let compiled_record = service
        .repository
        .get_compiled_plan(compiled_plan_id)
        .await?
        .ok_or_else(|| anyhow!("compiled plan not found"))?;
    let compiled_plan: orchestration_runtime::compiled_plan::CompiledPlan =
        serde_json::from_value(compiled_record.plan)?;
    crate::orchestration_runtime::compile_context::ensure_compiled_plan_runnable(&compiled_plan)?;

    let mut variable_pool = flow_run
        .input_payload
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("input payload must be an object"))?;
    if !variable_pool.contains_key("env") {
        let environment_variables = service
            .repository
            .list_application_environment_variables(application.workspace_id, application.id)
            .await?;
        inject_application_environment_variables(&mut variable_pool, &environment_variables);
    }
    inject_system_variables(
        &mut variable_pool,
        &flow_run,
        application.application_type,
        compiled_plan_start_node_id(&compiled_plan),
    );
    orchestration_runtime::execution_engine::normalize_plan_variable_pool(
        &compiled_plan,
        &mut variable_pool,
    );

    let flow_execution_context = service.runtime_flow_execution_context(
        actor.clone(),
        application.id,
        flow_run.draft_id,
        flow_run.id,
        None,
    );
    let lifecycle =
        PersistedNodeLifecycle::new(service, flow_run.id, flow_execution_context.clone());

    let provider_invocation_capability = provider_transport_payload.as_ref().map(|payload| {
        match payload.protocol() {
            crate::ports::ProviderTransportProtocol::OpenAiResponses => {
                plugin_framework::provider_contract::ProviderInvocationCapability::ResponsesNativePassthrough
            }
        }
    });

    let invoker = match live_provider_events {
        Some(live_provider_events) => service.runtime_invoker_with_live_provider_events(
            application.workspace_id,
            live_provider_events,
        ),
        None => service.runtime_invoker(application.workspace_id),
    }
    .for_flow_run(flow_run.id)
    .with_flow_execution_context(flow_execution_context)
    .with_provider_transport_payload(provider_transport_payload);
    let answer_presentation =
        crate::orchestration_runtime::answer_presentation::AnswerPresentationCursor::from_plan(
            &compiled_plan,
        )
        .map(|cursor| Arc::new(tokio::sync::Mutex::new(cursor)));
    let invoker = match answer_presentation {
        Some(answer_presentation) => invoker.with_answer_presentation(answer_presentation),
        None => invoker,
    };
    let mut runtime_context = service.execution_runtime_context(&compiled_plan, &variable_pool)?;
    if let Some(capability) = provider_invocation_capability {
        runtime_context = runtime_context.with_provider_invocation_capability(capability);
    }
    if let Some(http_file_persister) = service.http_response_file_persister(actor) {
        runtime_context =
            runtime_context.with_http_response_file_persister(Arc::new(http_file_persister));
    }
    if let Some(ingress) = compact_response_ingress {
        runtime_context = runtime_context.with_application_flow_compact_ingress(ingress);
    }

    let outcome = orchestration_runtime::execution_engine::start_flow_debug_run_with_runtime_context_and_lifecycle(
        &compiled_plan,
        &Value::Object(variable_pool),
        runtime_context,
        &invoker,
        &lifecycle,
    )
    .await?;
    let prepared_node_runs = lifecycle.prepared_node_runs()?;

    service
        .persist_flow_debug_outcome(PersistFlowDebugOutcomeInput {
            scope_id: application.workspace_id,
            application_name: &application.name,
            task_queue: service.provider_request_log_queue.as_ref(),
            application_id: command.application_id,
            flow_run: &flow_run,
            compiled_plan: Some(&compiled_plan),
            outcome: &outcome,
            prepared_node_runs: Some(&prepared_node_runs),
            trigger_event_type: "flow_run_execution_started",
            trigger_event_payload: json!({
                "run_mode": flow_run.run_mode.as_str(),
            }),
            base_started_at: flow_run.started_at,
            waiting_node_resume: None,
        })
        .await
}
