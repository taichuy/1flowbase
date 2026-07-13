use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    flow::FlowService,
    flow_run_title::display_flow_run_title,
    ports::{
        AppendBillingSessionInput, AppendCostLedgerInput, AppendCreditLedgerInput,
        AppendRunEventInput, AttachCompiledPlanToFlowRunInput, CreateFlowRunShellInput,
        FailQueuedFlowRunShellInput, OrchestrationRuntimeRepository,
    },
};

use super::super::{
    compile_context::ensure_compiled_plan_runnable,
    debug_stream_events, freeze_failover_queue_routes,
    inputs::{build_compiled_plan_input, flow_document_hash, flow_document_schema_version},
    OrchestrationRuntimeService, PrepareFlowDebugRunCommand, StartFlowDebugRunCommand,
};
use super::{append_runtime_event, emit_flow_failed_and_close, fail_flow_run, load_run_detail};

pub(super) async fn start_flow_debug_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: StartFlowDebugRunCommand,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::ApplicationJsDependencySelectionRepository
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
        + Clone,
{
    let actor_user_id = command.actor_user_id;
    let application_id = command.application_id;
    let input_payload = command.input_payload.clone();
    let document_snapshot = command.document_snapshot.clone();
    let debug_session_id = command.debug_session_id.clone().unwrap_or_default();
    let shell = open_flow_debug_run_shell(service, command).await?;

    prepare_flow_debug_run_from_shell(
        service,
        PrepareFlowDebugRunCommand {
            actor_user_id,
            application_id,
            flow_run_id: shell.id,
            input_payload,
            document_snapshot,
            debug_session_id,
        },
    )
    .await
}

fn shell_mismatch_error() -> anyhow::Error {
    anyhow!("flow debug run shell does not match prepare command")
}

fn validate_flow_debug_run_shell(
    flow_run: &domain::FlowRunRecord,
    command: &PrepareFlowDebugRunCommand,
    editor_state: &domain::FlowEditorState,
) -> Result<()> {
    let debug_document = command
        .document_snapshot
        .as_ref()
        .unwrap_or(&editor_state.draft.document);
    let flow_schema_version = flow_document_schema_version(editor_state, debug_document);
    let document_hash = flow_document_hash(debug_document);

    if flow_run.created_by != command.actor_user_id
        || flow_run.run_mode != domain::FlowRunMode::DebugFlowRun
        || flow_run.target_node_id.is_some()
        || flow_run.status != domain::FlowRunStatus::Queued
        || flow_run.compiled_plan_id.is_some()
        || flow_run.debug_session_id != command.debug_session_id
        || flow_run.flow_schema_version != flow_schema_version
        || flow_run.document_hash != document_hash
        || user_input_payload(&flow_run.input_payload) != command.input_payload
        || flow_run.flow_id != editor_state.flow.id
        || flow_run.draft_id != editor_state.draft.id
    {
        return Err(shell_mismatch_error());
    }

    Ok(())
}

fn user_input_payload(input_payload: &Value) -> Value {
    let Some(object) = input_payload.as_object() else {
        return input_payload.clone();
    };
    let mut user_input = object.clone();
    user_input.remove("sys");
    user_input.remove("env");
    user_input.remove("trigger");
    Value::Object(user_input)
}

pub(super) async fn open_flow_debug_run_shell<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: StartFlowDebugRunCommand,
) -> Result<domain::FlowRunRecord>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let actor = crate::ports::ApplicationRepository::load_actor_context_for_user(
        &service.repository,
        command.actor_user_id,
    )
    .await?;
    let editor_state = FlowService::new(service.repository.clone())
        .get_or_create_editor_state(command.actor_user_id, command.application_id)
        .await?;
    let application = service
        .repository
        .get_application(actor.current_workspace_id, command.application_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("application"))?;
    let environment_variables = service
        .repository
        .list_application_environment_variables(application.workspace_id, application.id)
        .await?;
    let debug_document = command
        .document_snapshot
        .as_ref()
        .unwrap_or(&editor_state.draft.document);
    let flow_schema_version = flow_document_schema_version(&editor_state, debug_document);
    let document_hash = flow_document_hash(debug_document);

    service
        .repository
        .create_flow_run_shell(&CreateFlowRunShellInput {
            actor_user_id: command.actor_user_id,
            application_id: command.application_id,
            flow_id: editor_state.flow.id,
            flow_draft_id: editor_state.draft.id,
            debug_session_id: command.debug_session_id.unwrap_or_default(),
            flow_schema_version,
            document_hash,
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: display_flow_run_title("", &command.input_payload),
            status: domain::FlowRunStatus::Queued,
            input_payload: freeze_run_input_environment(
                command.input_payload,
                &environment_variables,
                &application,
            ),
            started_at: OffsetDateTime::now_utc(),
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        })
        .await
}

fn freeze_run_input_environment(
    input_payload: Value,
    variables: &[domain::ApplicationEnvironmentVariable],
    application: &domain::ApplicationRecord,
) -> Value {
    let mut payload = input_payload.as_object().cloned().unwrap_or_default();
    payload.insert(
        "env".to_string(),
        Value::Object(application_environment_variable_payload(variables)),
    );
    if application.application_type == domain::ApplicationType::Workflow {
        if let Some(trigger_type) = application.workflow_trigger_type {
            payload.insert(
                "trigger".to_string(),
                json!({ "type": trigger_type.as_str() }),
            );
        }
    }
    Value::Object(payload)
}

fn application_environment_variable_payload(
    variables: &[domain::ApplicationEnvironmentVariable],
) -> Map<String, Value> {
    variables
        .iter()
        .map(|variable| (variable.name.clone(), variable.value.clone()))
        .collect()
}

pub(super) async fn prepare_flow_debug_run_from_shell<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: PrepareFlowDebugRunCommand,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::ApplicationJsDependencySelectionRepository
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
        + Clone,
{
    let actor = crate::ports::ApplicationRepository::load_actor_context_for_user(
        &service.repository,
        command.actor_user_id,
    )
    .await?;
    let flow_run = service
        .repository
        .get_flow_run(command.application_id, command.flow_run_id)
        .await?
        .ok_or_else(shell_mismatch_error)?;
    let editor_state = FlowService::new(service.repository.clone())
        .get_or_create_editor_state(command.actor_user_id, command.application_id)
        .await?;
    validate_flow_debug_run_shell(&flow_run, &command, &editor_state)?;
    let application = service
        .repository
        .get_application(actor.current_workspace_id, command.application_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("application"))?;
    let debug_document = command
        .document_snapshot
        .as_ref()
        .unwrap_or(&editor_state.draft.document);
    let document_hash = flow_document_hash(debug_document);
    let pre_attach_result = async {
        let compile_context = service
            .build_compile_context(application.workspace_id, application.id)
            .await?;

        let mut compiled_plan = match application.application_type {
            domain::ApplicationType::AgentFlow => {
                orchestration_runtime::compiler::FlowCompiler::compile(
                    editor_state.flow.id,
                    &editor_state.draft.id.to_string(),
                    debug_document,
                    &compile_context,
                )?
            }
            domain::ApplicationType::Workflow => {
                orchestration_runtime::compiler::FlowCompiler::compile_workflow(
                    editor_state.flow.id,
                    &editor_state.draft.id.to_string(),
                    debug_document,
                    &compile_context,
                )?
            }
        };
        freeze_failover_queue_routes(&service.repository, &mut compiled_plan).await?;
        ensure_compiled_plan_runnable(&compiled_plan)?;
        let compiled_record = service
            .repository
            .upsert_compiled_plan(&build_compiled_plan_input(
                command.actor_user_id,
                &editor_state,
                &compiled_plan,
                debug_document,
            )?)
            .await?;

        Result::<_>::Ok((compiled_plan, compiled_record))
    }
    .await;

    let (compiled_plan, compiled_record) = match pre_attach_result {
        Ok(result) => result,
        Err(error) => {
            fail_queued_flow_run_shell(
                service,
                command.application_id,
                command.flow_run_id,
                &error,
            )
            .await?;
            return Err(error);
        }
    };

    let flow_run = match service
        .repository
        .attach_compiled_plan_to_flow_run(&AttachCompiledPlanToFlowRunInput {
            flow_run_id: command.flow_run_id,
            compiled_plan_id: compiled_record.id,
            flow_schema_version: compiled_record.schema_version.clone(),
            document_hash,
            status: domain::FlowRunStatus::Running,
        })
        .await
    {
        Ok(flow_run) => flow_run,
        Err(error) => {
            fail_queued_flow_run_shell(
                service,
                command.application_id,
                command.flow_run_id,
                &error,
            )
            .await?;
            return Err(error);
        }
    };

    let post_attach_result = async {
        service
            .repository
            .append_run_event(&AppendRunEventInput {
                flow_run_id: flow_run.id,
                node_run_id: None,
                event_type: "flow_run_started".to_string(),
                payload: json!({
                    "run_mode": domain::FlowRunMode::DebugFlowRun.as_str(),
                    "input_payload": command.input_payload.clone(),
                }),
            })
            .await?;
        append_runtime_event(
            service,
            flow_run.id,
            debug_stream_events::flow_started(flow_run.id),
        )
        .await;

        record_gateway_billing_audit(
            &service.repository,
            &flow_run,
            command.actor_user_id,
            command.application_id,
            application.workspace_id,
            &compiled_plan,
        )
        .await?;

        load_run_detail(&service.repository, command.application_id, flow_run.id).await
    }
    .await;

    if let Err(error) = post_attach_result {
        fail_flow_run(service, command.application_id, command.flow_run_id, &error).await?;
        return Err(error);
    }

    post_attach_result
}

async fn fail_queued_flow_run_shell<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    application_id: Uuid,
    flow_run_id: Uuid,
    error: &anyhow::Error,
) -> Result<Option<domain::ApplicationRunDetail>>
where
    R: crate::ports::ApplicationRepository
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
        + Clone,
{
    let error_payload = json!({ "message": error.to_string() });
    let Some(flow_run) = service
        .repository
        .fail_queued_flow_run_shell(&FailQueuedFlowRunShellInput {
            flow_run_id,
            output_payload: json!({}),
            error_payload: error_payload.clone(),
            finished_at: OffsetDateTime::now_utc(),
        })
        .await?
    else {
        return Ok(None);
    };
    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
    service
        .repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: None,
            event_type: "flow_run_failed".to_string(),
            payload: error_payload.clone(),
        })
        .await?;

    load_run_detail(&service.repository, application_id, flow_run.id)
        .await
        .map(Some)
}

async fn record_gateway_billing_audit<R>(
    repository: &R,
    flow_run: &domain::FlowRunRecord,
    actor_user_id: Uuid,
    application_id: Uuid,
    workspace_id: Uuid,
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let route_id = Uuid::now_v7();
    let llm_runtime = compiled_plan
        .nodes
        .values()
        .find_map(|node| node.llm_runtime.as_ref());
    let provider_instance_id =
        llm_runtime.and_then(|runtime| Uuid::parse_str(&runtime.provider_instance_id).ok());
    let routing_mode = llm_runtime
        .and_then(|runtime| runtime.routing.as_ref())
        .map(|routing| match routing.routing_mode {
            orchestration_runtime::compiled_plan::LlmRoutingMode::FixedModel => "fixed_model",
            orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue => "failover_queue",
        })
        .unwrap_or("debug_run");
    let logical_model_id = llm_runtime
        .map(|runtime| runtime.model.as_str())
        .unwrap_or("debug_run");
    let upstream_model_id = llm_runtime.map(|runtime| runtime.model.clone());
    let route_trace = json!({
        "logical_model_id": logical_model_id,
        "route_id": route_id,
        "provider_instance_id": provider_instance_id,
        "provider_account_id": null,
        "upstream_model_id": upstream_model_id.clone(),
        "routing_mode": routing_mode,
        "trust_level": domain::RuntimeTrustLevel::HostFact.as_str(),
    });
    let idempotency_key = format!("gateway:{}:reserve", flow_run.id);
    let cost = repository
        .append_cost_ledger(&AppendCostLedgerInput {
            flow_run_id: Some(flow_run.id),
            span_id: None,
            usage_ledger_id: None,
            workspace_id,
            provider_instance_id,
            provider_account_id: None,
            gateway_route_id: Some(route_id),
            model_id: Some(logical_model_id.to_string()),
            upstream_model_id,
            price_snapshot: json!({
                "route_trace": route_trace.clone(),
                "source": "gateway_debug_run_start"
            }),
            raw_cost: Some("0".to_string()),
            normalized_cost: Some("0".to_string()),
            settlement_currency: Some("credit".to_string()),
            cost_source: "gateway_reservation".to_string(),
            cost_status: "pending_usage".to_string(),
        })
        .await?;
    let credit = repository
        .append_credit_ledger(&AppendCreditLedgerInput {
            workspace_id,
            user_id: Some(actor_user_id),
            application_id: Some(application_id),
            agent_id: None,
            flow_run_id: Some(flow_run.id),
            span_id: None,
            cost_ledger_id: Some(cost.id),
            transaction_type: "reserve".to_string(),
            amount: "0".to_string(),
            balance_after: None,
            credit_unit: "credit".to_string(),
            reason: "gateway_billing_session_reserved".to_string(),
            idempotency_key: idempotency_key.clone(),
            status: "posted".to_string(),
        })
        .await?;
    let billing_session = repository
        .append_billing_session(&AppendBillingSessionInput {
            workspace_id,
            flow_run_id: Some(flow_run.id),
            client_request_id: None,
            idempotency_key,
            route_id: Some(route_id),
            provider_account_id: None,
            status: domain::BillingSessionStatus::Reserved,
            reserved_credit_ledger_id: Some(credit.id),
            settled_credit_ledger_id: None,
            refund_credit_ledger_id: None,
            metadata: json!({
                "route_trace": route_trace.clone(),
                "fail_safe": "continue"
            }),
        })
        .await?;
    let cost_hash = repository
        .append_audit_hash(
            flow_run.id,
            "runtime_cost_ledger",
            cost.id,
            json!({
                "cost_source": cost.cost_source.clone(),
                "cost_status": cost.cost_status.clone(),
                "route_trace": route_trace.clone(),
            }),
        )
        .await?;
    let credit_hash = repository
        .append_audit_hash(
            flow_run.id,
            "runtime_credit_ledger",
            credit.id,
            json!({
                "transaction_type": credit.transaction_type.clone(),
                "amount": credit.amount.clone(),
                "reason": credit.reason.clone(),
                "idempotency_key": credit.idempotency_key.clone(),
            }),
        )
        .await?;
    let billing_hash = repository
        .append_audit_hash(
            flow_run.id,
            "billing_sessions",
            billing_session.id,
            json!({
                "status": billing_session.status.as_str(),
                "idempotency_key": billing_session.idempotency_key.clone(),
                "route_trace": route_trace.clone(),
            }),
        )
        .await?;

    repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: None,
            event_type: "gateway_billing_session_reserved".to_string(),
            payload: json!({
                "billing_session": {
                    "id": billing_session.id,
                    "status": billing_session.status.as_str(),
                    "idempotency_key": billing_session.idempotency_key,
                },
                "cost_ledger": {
                    "id": cost.id,
                    "cost_status": cost.cost_status,
                },
                "credit_ledger": {
                    "id": credit.id,
                    "transaction_type": credit.transaction_type,
                },
                "audit_hashes": [
                    cost_hash.id,
                    credit_hash.id,
                    billing_hash.id
                ],
                "route_trace": route_trace,
            }),
        })
        .await?;
    Ok(())
}
