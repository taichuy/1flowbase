use std::{collections::BTreeSet, sync::Arc, time::Instant};

use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{KeepAlive, Sse},
    Json,
};
use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    application_public_api::{
        model_catalog::extract_agent_model_catalog_from_start_node,
        native::{NativeExecution, NativeObject, NativeRequestMetadata, NativeRunRequest},
        publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
        run_service::{
            native_result_from_run_detail, ApplicationPublishedRunService,
            CreateAssistantRunCommand,
        },
    },
    mcp_management::McpManagementService,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        spawn_runtime_debug_event_persister, wait_for_runtime_debug_event_persister,
        OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{OrchestrationRuntimeRepository, RuntimeEventPayload, RuntimeEventStreamPolicy},
    profile::{ProfileService, UpdateMeMetaCommand},
};
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use utoipa::ToSchema;
use uuid::Uuid;

pub(crate) mod websocket;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{
        require_csrf::require_csrf,
        require_session::{require_session, RequestContext},
    },
    response::ApiSuccess,
    routes::{
        application_public_api::native::api_provider_runtime,
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
        debug_run_stream,
        mcp_management::upstream_client::{
            map_proxy_arguments, map_proxy_result, McpStreamableHttpClient,
        },
        mcp_management::{
            bindable_mcp_interface, debug_execute, McpDebugExecuteBody, McpDebugResponseMode,
        },
    },
};

const ASSISTANT_META_KEY: &str = "embedded_assistant";

#[derive(Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct AssistantPreferenceBody {
    pub application_id: Option<Uuid>,
    #[serde(default)]
    pub mcp_instance_ids: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantPublishedFlowOption {
    pub application_id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantMcpInstanceOption {
    pub instance_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantModelOption {
    pub id: String,
    pub name: Option<String>,
    pub context_window: Option<u64>,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct AssistantRunCapabilities {
    pub model_selection_enabled: bool,
    pub reasoning_effort_enabled: bool,
    pub models: Vec<AssistantModelOption>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantSettingsResponse {
    pub preference: AssistantPreferenceBody,
    pub published_agent_flows: Vec<AssistantPublishedFlowOption>,
    pub enabled_mcp_instances: Vec<AssistantMcpInstanceOption>,
    pub run_capabilities: AssistantRunCapabilities,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAssistantRunBody {
    pub application_id: Uuid,
    pub query: String,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantRunResponse {
    pub id: Uuid,
    pub application_id: Uuid,
    pub status: String,
    pub answer: Option<String>,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
}

struct PreparedAssistantExecution {
    application_id: Uuid,
    actor_user_id: Uuid,
    flow_run_id: Uuid,
    catalog: domain::McpCatalogSnapshot,
    selected_mcp_instance_ids: Vec<String>,
    request_headers: HeaderMap,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/assistant/settings",
            console_get(get_settings, Authenticated).patch(update_settings, Authenticated),
        )
        .route("/assistant/runs", console_post(start_run, Authenticated))
        .route(
            "/assistant/runs/stream",
            console_post(start_run_stream, Authenticated),
        )
        .route(
            "/assistant/runs/websocket-ticket",
            console_post(websocket::create_ticket, Authenticated),
        )
        .route(
            "/assistant/runs/websocket",
            console_get(websocket::upgrade, Authenticated),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/assistant/settings",
    operation_id = "assistant_get_settings",
    responses((status = 200, body = AssistantSettingsResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<AssistantSettingsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    let preference = read_preference(
        &state
            .store
            .find_user_by_id(context.user.id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?
            .meta,
        context.actor.current_workspace_id,
    );
    let (published_agent_flows, enabled_mcp_instances) =
        available_targets(&state, context.user.id).await?;
    let run_capabilities = assistant_run_capabilities(&state, preference.application_id).await?;
    Ok(Json(ApiSuccess::new(AssistantSettingsResponse {
        preference,
        published_agent_flows,
        enabled_mcp_instances,
        run_capabilities,
    })))
}

#[utoipa::path(
    patch,
    path = "/api/console/assistant/settings",
    operation_id = "assistant_update_settings",
    request_body = AssistantPreferenceBody,
    responses((status = 200, body = AssistantSettingsResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn update_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(preference): Json<AssistantPreferenceBody>,
) -> Result<Json<ApiSuccess<AssistantSettingsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    let current_preference = read_preference(
        &state
            .store
            .find_user_by_id(context.user.id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?
            .meta,
        context.actor.current_workspace_id,
    );
    let preference = if current_preference.application_id != preference.application_id {
        AssistantPreferenceBody {
            model: None,
            reasoning_effort: None,
            ..preference
        }
    } else {
        preference
    };
    validate_preference(&state, context.user.id, &preference).await?;
    let workspace_id = context.actor.current_workspace_id;
    let meta_patch = json!({
        ASSISTANT_META_KEY: { "workspaces": { workspace_id.to_string(): preference } }
    });
    ProfileService::new(state.store.clone())
        .update_me_meta(UpdateMeMetaCommand {
            actor_user_id: context.user.id,
            tenant_id: context.actor.tenant_id,
            workspace_id,
            meta_patch,
        })
        .await?;
    let (published_agent_flows, enabled_mcp_instances) =
        available_targets(&state, context.user.id).await?;
    let run_capabilities = assistant_run_capabilities(&state, preference.application_id).await?;
    Ok(Json(ApiSuccess::new(AssistantSettingsResponse {
        preference,
        published_agent_flows,
        enabled_mcp_instances,
        run_capabilities,
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/assistant/runs",
    operation_id = "assistant_start_run",
    request_body = StartAssistantRunBody,
    responses((status = 200, body = AssistantRunResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn start_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<StartAssistantRunBody>,
) -> Result<Json<ApiSuccess<AssistantRunResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    let execution = prepare_assistant_execution(&state, &headers, &context, body).await?;
    let runtime = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let mut detail = runtime
        .start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: execution.application_id,
            flow_run_id: execution.flow_run_id,
            provider_transport_slot: None,
        })
        .await?;
    while detail.flow_run.status == domain::FlowRunStatus::WaitingCallback {
        let callback = detail
            .callback_tasks
            .iter()
            .find(|task| {
                task.status == domain::CallbackTaskStatus::Pending
                    && task.callback_kind == "llm_tool_calls"
            })
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_callback",
            ))?;
        let tool_results = assistant_callback_tool_results(&state, &execution, callback).await?;
        detail = runtime
            .complete_callback_task(
                control_plane::orchestration_runtime::CompleteCallbackTaskCommand {
                    actor_user_id: execution.actor_user_id,
                    application_id: execution.application_id,
                    callback_task_id: callback.id,
                    response_payload: json!({"tool_results": tool_results}),
                },
            )
            .await?;
    }
    let native_result = native_result_from_run_detail(&detail, json!({}));
    Ok(Json(ApiSuccess::new(AssistantRunResponse {
        id: detail.flow_run.id,
        application_id: execution.application_id,
        status: detail.flow_run.status.as_str().to_string(),
        answer: native_result.answer,
        output_payload: detail.flow_run.output_payload,
        error_payload: detail.flow_run.error_payload,
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/assistant/runs/stream",
    operation_id = "assistant_start_run_stream",
    summary = "Start an embedded assistant run stream",
    description = "Creates a session-principal published Agent Flow run and returns its Runtime Event Stream for the embedded Preview console.",
    request_body = StartAssistantRunBody,
    responses(
        (status = 200, body = crate::routes::debug_run_stream::RuntimeDebugSseEventResponse, content_type = "text/event-stream"),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn start_run_stream(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<StartAssistantRunBody>,
) -> Result<Sse<debug_run_stream::DebugRunSseStream>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    let execution = prepare_assistant_execution(&state, &headers, &context, body).await?;
    let run_id = launch_assistant_execution(state.clone(), execution).await?;
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(debug_run_stream::send_runtime_event_stream(
        state.runtime_event_stream.clone(),
        Arc::new(state.store.clone()),
        run_id,
        None,
        sender,
    ));

    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
}

async fn launch_assistant_execution(
    state: Arc<ApiState>,
    execution: PreparedAssistantExecution,
) -> Result<Uuid, ApiError> {
    let run_id = execution.flow_run_id;
    let application_id = execution.application_id;
    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await?;
    let persister_handle = spawn_runtime_debug_event_persister(
        state.store.clone(),
        state.runtime_event_stream.clone(),
        run_id,
    );
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_accepted(run_id))
        .await?;
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::heartbeat())
        .await?;

    let background_state = state;
    tokio::spawn(async move {
        let runtime = OrchestrationRuntimeService::new(
            background_state.store.clone(),
            api_provider_runtime(&background_state),
            background_state.runtime_engine.clone(),
            background_state.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_state.api_node_id.clone(),
            background_state.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_state.file_storage_registry.clone())
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue())
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        let result = async {
            let mut detail = runtime
                .start_published_flow_run(StartPublishedFlowRunCommand {
                    application_id: execution.application_id,
                    flow_run_id: execution.flow_run_id,
                    provider_transport_slot: None,
                })
                .await?;
            while detail.flow_run.status == domain::FlowRunStatus::WaitingCallback {
                let callback = detail
                    .callback_tasks
                    .iter()
                    .find(|task| {
                        task.status == domain::CallbackTaskStatus::Pending
                            && task.callback_kind == "llm_tool_calls"
                    })
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "assistant_callback",
                    ))?;
                let tool_results =
                    assistant_callback_tool_results(&background_state, &execution, callback)
                        .await?;
                detail = runtime
                    .complete_callback_task(
                        control_plane::orchestration_runtime::CompleteCallbackTaskCommand {
                            actor_user_id: execution.actor_user_id,
                            application_id: execution.application_id,
                            callback_task_id: callback.id,
                            response_payload: json!({"tool_results": tool_results}),
                        },
                    )
                    .await?;
            }
            Ok::<domain::ApplicationRunDetail, ApiError>(detail)
        }
        .await;

        match result {
            Ok(detail)
                if matches!(
                    detail.flow_run.status,
                    domain::FlowRunStatus::Succeeded
                        | domain::FlowRunStatus::Incomplete
                        | domain::FlowRunStatus::Failed
                        | domain::FlowRunStatus::Cancelled
                ) =>
            {
                project_runtime_event_stream_terminal(
                    background_state.runtime_event_stream.clone(),
                    &detail.flow_run,
                )
                .await;
            }
            Ok(_) => {}
            Err(error) => {
                match background_state
                    .store
                    .get_flow_run(application_id, run_id)
                    .await
                {
                    Ok(Some(winner)) => {
                        project_runtime_event_stream_terminal(
                            background_state.runtime_event_stream.clone(),
                            &winner,
                        )
                        .await;
                    }
                    Ok(None) => tracing::error!(
                        application_id = %application_id,
                        flow_run_id = %run_id,
                        "assistant stream failure has no durable winner to project"
                    ),
                    Err(load_error) => tracing::error!(
                        application_id = %application_id,
                        flow_run_id = %run_id,
                        error = %load_error,
                        "failed to load assistant stream durable winner"
                    ),
                }
                tracing::error!(
                    application_id = %application_id,
                    flow_run_id = %run_id,
                    error = ?error,
                    "assistant streamed run failed"
                );
            }
        }
        wait_for_runtime_debug_event_persister(persister_handle, application_id, run_id).await;
    });
    Ok(run_id)
}

async fn prepare_assistant_execution(
    state: &Arc<ApiState>,
    headers: &HeaderMap,
    context: &RequestContext,
    body: StartAssistantRunBody,
) -> Result<PreparedAssistantExecution, ApiError> {
    if body.query.trim().is_empty() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("query").into());
    }
    let application_id = body.application_id;
    let preference = assistant_preference_for_target(state, context, application_id).await?;
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(context.user.id)
        .await?;
    let execution = assistant_execution(&preference)?;
    let selected_mcp_instance_ids = preference.mcp_instance_ids;
    let mut inputs = NativeObject::default();
    inputs.insert_value(
        "tools",
        Value::Array(assistant_provider_tools(
            &catalog,
            &selected_mcp_instance_ids,
        )),
    );
    let flow_run = ApplicationPublishedRunService::new(state.store.clone())
        .create_assistant_run(CreateAssistantRunCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            application_id,
            request: NativeRunRequest {
                query: body.query,
                system: Vec::new(),
                model: preference.model,
                history: body.history,
                attachments: Vec::new(),
                conversation: NativeObject::default(),
                expand_id: None,
                response_mode: None,
                stream_options: control_plane::application_public_api::native::NativeStreamOptions {
                    include_workflow_events: control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug,
                },
                execution,
                metadata: NativeRequestMetadata::default(),
                request_context: Default::default(),
                title: body.title,
                inputs,
                client_protocol_envelope: None,
            },
        })
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("assistant_run"))?;

    Ok(PreparedAssistantExecution {
        application_id,
        actor_user_id: context.user.id,
        flow_run_id: flow_run.id,
        catalog,
        selected_mcp_instance_ids,
        request_headers: headers.clone(),
    })
}

async fn assistant_preference_for_target(
    state: &Arc<ApiState>,
    context: &RequestContext,
    application_id: Uuid,
) -> Result<AssistantPreferenceBody, ApiError> {
    let user = state
        .store
        .find_user_by_id(context.user.id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?;
    let preference = read_preference(&user.meta, context.actor.current_workspace_id);
    if preference.application_id != Some(application_id) {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "assistant_application_id",
        )
        .into());
    }
    validate_preference(state, context.user.id, &preference).await?;
    ApplicationService::new(state.store.clone())
        .load_application_for_non_crud_console_operation(
            context.user.id,
            application_id,
            ApplicationNonCrudConsoleOperation::Run,
        )
        .await?;
    Ok(preference)
}

fn assistant_execution(preference: &AssistantPreferenceBody) -> Result<NativeExecution, ApiError> {
    let Some(reasoning_effort) = preference.reasoning_effort.as_deref() else {
        return Ok(NativeExecution::default());
    };
    serde_json::from_value(json!({
        "model_parameters": {
            "reasoning": { "mode": "adaptive", "effort": reasoning_effort }
        }
    }))
    .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("reasoning_effort").into())
}

async fn assistant_callback_tool_results(
    state: &Arc<ApiState>,
    execution: &PreparedAssistantExecution,
    callback: &domain::CallbackTaskRecord,
) -> Result<Vec<Value>, ApiError> {
    let calls = callback
        .request_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "assistant_callback",
        ))?;
    let mut tool_results = Vec::with_capacity(calls.len());
    for call in calls {
        let tool_call_id = call.get("id").and_then(Value::as_str).ok_or(
            control_plane::errors::ControlPlaneError::InvalidInput("assistant_callback"),
        )?;
        let tool_name = call.get("name").and_then(Value::as_str).unwrap_or("Tool");
        let trace_call = assistant_tool_call_for_trace(&execution.catalog, call);
        let started_at = Instant::now();
        append_assistant_tool_call_event(
            state,
            execution,
            debug_stream_events::assistant_tool_call_started(
                execution.flow_run_id,
                callback.node_run_id,
                "assistant",
                trace_call.clone(),
            ),
        )
        .await;

        let tool_result = match assistant_tool_result(
            &execution.catalog,
            &execution.selected_mcp_instance_ids,
            state,
            &execution.request_headers,
            execution.actor_user_id,
            call,
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                append_assistant_tool_call_event(
                    state,
                    execution,
                    debug_stream_events::assistant_tool_call_finished(
                        execution.flow_run_id,
                        callback.node_run_id,
                        "assistant",
                        trace_call.clone(),
                        json!({
                            "tool_call_id": tool_call_id,
                            "name": tool_name,
                            "content": error.0.to_string(),
                            "is_error": true,
                        }),
                        started_at
                            .elapsed()
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX),
                    ),
                )
                .await;
                return Err(error);
            }
        };
        append_assistant_tool_call_event(
            state,
            execution,
            debug_stream_events::assistant_tool_call_finished(
                execution.flow_run_id,
                callback.node_run_id,
                "assistant",
                trace_call,
                tool_result.clone(),
                started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
        )
        .await;
        tool_results.push(tool_result);
    }
    Ok(tool_results)
}

async fn append_assistant_tool_call_event(
    state: &Arc<ApiState>,
    execution: &PreparedAssistantExecution,
    event: RuntimeEventPayload,
) {
    let event_type = event.event_type.clone();
    if let Err(error) = state
        .runtime_event_stream
        .append(execution.flow_run_id, event)
        .await
    {
        tracing::warn!(
            application_id = %execution.application_id,
            flow_run_id = %execution.flow_run_id,
            event_type = %event_type,
            error = %error,
            "assistant tool lifecycle event append failed"
        );
    }
}

fn assistant_provider_tools(
    catalog: &domain::McpCatalogSnapshot,
    selected_instance_ids: &[String],
) -> Vec<Value> {
    let mut names = BTreeSet::new();
    let mut provider_tools = Vec::new();
    for binding in catalog.bindings.iter().filter(|binding| binding.visible) {
        let group_is_enabled = catalog.groups.iter().any(|group| {
            group.instance_record_id == binding.instance_record_id
                && group.path == binding.group_path
                && group.enabled
        });
        if !group_is_enabled {
            continue;
        }
        let Some(instance) = catalog.instances.iter().find(|instance| {
            instance.id == binding.instance_record_id
                && instance.status == McpInstanceStatus::Enabled
                && selected_instance_ids.contains(&instance.instance_id)
        }) else {
            continue;
        };
        let Some(tool) = catalog.tools.iter().find(|tool| {
            tool.id == binding.tool_record_id && tool.status == domain::McpToolStatus::Enabled
        }) else {
            continue;
        };
        let name = assistant_tool_name(instance, tool);
        if !names.insert(name.clone()) {
            continue;
        }
        provider_tools.push(json!({
            "type": "function",
            "function": {
                "name": name,
                "description": tool.full_description,
                "parameters": tool.parameter_schema,
            }
        }));
    }
    provider_tools
}

fn assistant_tool_name(
    instance: &domain::McpInstanceRecord,
    tool: &domain::McpToolRecord,
) -> String {
    format!("mcp_{}_{}", instance.id.simple(), tool.id.simple())
}

fn assistant_tool_call_for_trace(catalog: &domain::McpCatalogSnapshot, call: &Value) -> Value {
    let Some(wire_name) = call.get("name").and_then(Value::as_str) else {
        return call.clone();
    };
    let display_name = catalog.bindings.iter().find_map(|binding| {
        let instance = catalog
            .instances
            .iter()
            .find(|instance| instance.id == binding.instance_record_id)?;
        let tool = catalog
            .tools
            .iter()
            .find(|tool| tool.id == binding.tool_record_id)?;
        (assistant_tool_name(instance, tool) == wire_name).then(|| {
            binding
                .display_alias
                .as_deref()
                .filter(|alias| !alias.trim().is_empty())
                .unwrap_or(tool.name.as_str())
                .to_string()
        })
    });
    let Some(display_name) = display_name else {
        return call.clone();
    };
    let mut trace_call = call.clone();
    if let Some(trace_call) = trace_call.as_object_mut() {
        trace_call.insert("name".to_string(), Value::String(display_name));
        trace_call.insert(
            "wire_name".to_string(),
            Value::String(wire_name.to_string()),
        );
    }
    trace_call
}

async fn assistant_tool_result(
    catalog: &domain::McpCatalogSnapshot,
    selected_instance_ids: &[String],
    state: &Arc<ApiState>,
    headers: &HeaderMap,
    actor_user_id: Uuid,
    call: &Value,
) -> Result<Value, ApiError> {
    let id = call.get("id").and_then(Value::as_str).ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("assistant_callback"),
    )?;
    let name = call.get("name").and_then(Value::as_str).unwrap_or_default();
    let available = catalog
        .bindings
        .iter()
        .filter(|binding| binding.visible)
        .filter(|binding| {
            catalog.groups.iter().any(|group| {
                group.instance_record_id == binding.instance_record_id
                    && group.path == binding.group_path
                    && group.enabled
            })
        })
        .find_map(|binding| {
            let instance = catalog.instances.iter().find(|instance| {
                instance.id == binding.instance_record_id
                    && instance.status == McpInstanceStatus::Enabled
                    && selected_instance_ids.contains(&instance.instance_id)
            })?;
            let tool = catalog.tools.iter().find(|tool| {
                tool.id == binding.tool_record_id && tool.status == domain::McpToolStatus::Enabled
            })?;
            (assistant_tool_name(instance, tool) == name).then_some(tool)
        });
    let Some(tool) = available else {
        return Ok(json!({"tool_call_id": id, "content": "Tool is unavailable", "is_error": true}));
    };
    let arguments = match call.get("arguments") {
        Some(Value::String(value)) => match serde_json::from_str(value) {
            Ok(arguments) => arguments,
            Err(_) => {
                return Ok(
                    json!({"tool_call_id": id, "name": name, "content": "Tool arguments must be valid JSON", "is_error": true}),
                );
            }
        },
        Some(value) => value.clone(),
        None => json!({}),
    };
    let result = match &tool.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let interface =
                bindable_mcp_interface(state.as_ref(), actor_user_id, interface_id).await;
            match interface {
                Ok(interface) => match debug_execute::execute(
                    state.clone(),
                    headers.clone(),
                    interface,
                    McpDebugExecuteBody {
                        interface_id: interface_id.clone(),
                        debug_response_mode: McpDebugResponseMode::ToolResult,
                        mcp_arguments: arguments,
                        input_mapping: tool.input_mapping.clone(),
                        output_mapping: tool.output_mapping.clone(),
                    },
                )
                .await
                {
                    Ok(value) => json!({"content": value}),
                    Err(debug_execute::McpDebugExecuteError::Api(error)) => {
                        json!({"content": format!("MCP interface execution failed: {error}"), "is_error": true})
                    }
                    Err(debug_execute::McpDebugExecuteError::TargetResponse(response)) => {
                        json!({"content": format!("MCP interface returned HTTP {}", response.status()), "is_error": true})
                    }
                },
                Err(_) => json!({"content": "MCP interface is unavailable", "is_error": true}),
            }
        }
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => {
            let service = McpManagementService::new(state.store.clone());
            let upstream = async {
                let availability = service
                    .upstream_proxy_availability(
                        actor_user_id,
                        *upstream_connection_id,
                        remote_tool_name,
                    )
                    .await?;
                if availability != domain::McpToolAvailabilityStatus::Available {
                    anyhow::bail!("upstream tool unavailable: {}", availability.as_str());
                }
                let connection = service
                    .get_upstream_connection(actor_user_id, *upstream_connection_id)
                    .await?;
                let secret = service
                    .upstream_secret_for_execution(
                        actor_user_id,
                        *upstream_connection_id,
                        &state.provider_secret_master_key,
                    )
                    .await?;
                let remote_arguments = map_proxy_arguments(&arguments, &tool.input_mapping)?;
                let client = McpStreamableHttpClient::connect(&connection, secret.as_ref()).await?;
                let result = client.call_tool(remote_tool_name, remote_arguments).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(map_proxy_result(
                    &result,
                    &tool.output_mapping,
                )?)?)
            }
            .await;
            match upstream {
                Ok(value) => json!({"content": value}),
                Err(error) => {
                    json!({"content": format!("Upstream MCP execution failed: {error}"), "is_error": true})
                }
            }
        }
    };
    Ok(
        json!({"tool_call_id": id, "name": name, "content": result["content"], "is_error": result["is_error"].as_bool().unwrap_or(false)}),
    )
}

async fn available_targets(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
) -> Result<
    (
        Vec<AssistantPublishedFlowOption>,
        Vec<AssistantMcpInstanceOption>,
    ),
    ApiError,
> {
    let applications = ApplicationService::new(state.store.clone())
        .list_applications(actor_user_id)
        .await?;
    let mut published_agent_flows = Vec::new();
    for application in applications
        .into_iter()
        .filter(|application| application.application_type == domain::ApplicationType::AgentFlow)
    {
        if ApplicationPublicationService::new(state.store.clone())
            .load_active_publication(LoadActiveApplicationPublicationCommand {
                application_id: application.id,
            })
            .await
            .is_ok()
        {
            published_agent_flows.push(AssistantPublishedFlowOption {
                application_id: application.id,
                name: application.name,
            });
        }
    }
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(actor_user_id)
        .await?;
    let enabled_mcp_instances = catalog
        .instances
        .into_iter()
        .filter(|instance| instance.status == McpInstanceStatus::Enabled)
        .map(|instance| AssistantMcpInstanceOption {
            instance_id: instance.instance_id,
            name: instance.name,
        })
        .collect();
    Ok((published_agent_flows, enabled_mcp_instances))
}

async fn assistant_run_capabilities(
    state: &Arc<ApiState>,
    application_id: Option<Uuid>,
) -> Result<AssistantRunCapabilities, ApiError> {
    let Some(application_id) = application_id else {
        return Ok(AssistantRunCapabilities::default());
    };
    let publication = ApplicationPublicationService::new(state.store.clone())
        .load_active_publication(LoadActiveApplicationPublicationCommand { application_id })
        .await?;
    let model_selection_enabled = publication.mapping_snapshot.input.model_target.is_some();
    let reasoning_effort_enabled = publication
        .document_snapshot
        .get("graph")
        .and_then(|graph| graph.get("nodes"))
        .and_then(Value::as_array)
        .is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node.get("type").and_then(Value::as_str) == Some("llm")
                    && node
                        .get("config")
                        .and_then(|config| config.get("external_reasoning_policy"))
                        .and_then(|policy| policy.get("follow_external_reasoning"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
        });
    let models = model_selection_enabled
        .then(|| extract_agent_model_catalog_from_start_node(&publication.document_snapshot))
        .unwrap_or_default()
        .into_iter()
        .map(|model| AssistantModelOption {
            id: model.id,
            name: model.name,
            context_window: model.context_window,
            reasoning_efforts: model
                .reasoning
                .as_ref()
                .map(|reasoning| reasoning.supported_efforts.clone())
                .unwrap_or_default(),
            default_reasoning_effort: model
                .reasoning
                .and_then(|reasoning| reasoning.default_effort),
        })
        .collect();
    Ok(AssistantRunCapabilities {
        model_selection_enabled,
        reasoning_effort_enabled,
        models,
    })
}

async fn validate_preference(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    preference: &AssistantPreferenceBody,
) -> Result<(), ApiError> {
    if let Some(application_id) = preference.application_id {
        let application = ApplicationService::new(state.store.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        if application.application_type != domain::ApplicationType::AgentFlow {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_application_id",
            )
            .into());
        }
        let capabilities = assistant_run_capabilities(state, Some(application_id)).await?;
        if let Some(model) = preference.model.as_deref() {
            let selected = capabilities
                .models
                .iter()
                .find(|candidate| candidate.id == model)
                .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                    "assistant_model",
                ))?;
            if let Some(reasoning_effort) = preference.reasoning_effort.as_deref() {
                if !capabilities.reasoning_effort_enabled
                    || !selected
                        .reasoning_efforts
                        .iter()
                        .any(|supported| supported == reasoning_effort)
                {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "reasoning_effort",
                    )
                    .into());
                }
            }
        } else if preference.reasoning_effort.is_some() {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("reasoning_effort").into(),
            );
        }
    } else if preference.model.is_some() || preference.reasoning_effort.is_some() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "assistant_application_id",
        )
        .into());
    }
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(actor_user_id)
        .await?;
    for instance_id in &preference.mcp_instance_ids {
        if !catalog.instances.iter().any(|instance| {
            instance.instance_id == *instance_id && instance.status == McpInstanceStatus::Enabled
        }) {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("mcp_instance_ids").into(),
            );
        }
    }
    Ok(())
}

fn read_preference(meta: &Value, workspace_id: Uuid) -> AssistantPreferenceBody {
    meta.get(ASSISTANT_META_KEY)
        .and_then(|value| value.get("workspaces"))
        .and_then(|value| value.get(workspace_id.to_string()))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn preference_is_scoped_to_current_workspace() {
        let current = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let application = Uuid::from_u128(3);
        let meta = json!({ ASSISTANT_META_KEY: { "workspaces": {
            current.to_string(): { "application_id": application, "mcp_instance_ids": ["alpha"] },
            other.to_string(): { "application_id": Uuid::from_u128(4), "mcp_instance_ids": ["beta"] }
        }}});
        assert_eq!(
            read_preference(&meta, current).application_id,
            Some(application)
        );
        assert_eq!(
            read_preference(&meta, current).mcp_instance_ids,
            vec!["alpha"]
        );
    }

    #[test]
    fn assistant_routes_are_authenticated_console_routes() {
        let assembly = route_assembly();
        let bindings = assembly.bindings();
        assert!(bindings.iter().any(|binding| {
            binding.route.method == "GET"
                && binding.route.path == "/api/console/assistant/settings"
                && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
        }));
        assert!(bindings.iter().any(|binding| {
            binding.route.method == "POST"
                && binding.route.path == "/api/console/assistant/runs"
                && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
        }));
        assert!(bindings.iter().any(|binding| {
            binding.route.method == "POST"
                && binding.route.path == "/api/console/assistant/runs/stream"
                && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
        }));
    }

    #[test]
    fn assistant_console_routes_have_static_openapi_identities() {
        let document = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();

        for (method, path, operation_id) in [
            (
                "get",
                "/api/console/assistant/settings",
                "assistant_get_settings",
            ),
            (
                "patch",
                "/api/console/assistant/settings",
                "assistant_update_settings",
            ),
            ("post", "/api/console/assistant/runs", "assistant_start_run"),
            (
                "post",
                "/api/console/assistant/runs/stream",
                "assistant_start_run_stream",
            ),
        ] {
            assert_eq!(document["paths"][path][method]["operationId"], operation_id);
            assert!(
                document["paths"][path][method]["responses"]
                    .get("403")
                    .is_some(),
                "{method} {path} documents the API-key principal rejection"
            );
        }
    }

    #[test]
    fn mounted_tool_bound_to_multiple_groups_is_emitted_once() {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let workspace_id = Uuid::from_u128(10);
        let instance_id = Uuid::from_u128(11);
        let tool_id = Uuid::from_u128(12);
        let instance = domain::McpInstanceRecord {
            id: instance_id,
            workspace_id,
            instance_id: "catalog".to_string(),
            name: "Catalog".to_string(),
            description_short: None,
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".to_string(),
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        };
        let tool = domain::McpToolRecord {
            id: tool_id,
            workspace_id,
            tool_id: "lookup".to_string(),
            name: "Lookup".to_string(),
            short_description: "Lookup".to_string(),
            full_description: "Lookup a catalog item".to_string(),
            execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "lookup".to_string(),
            },
            parameter_schema: json!({"type":"object"}),
            result_schema: json!({}),
            input_mapping: json!({"mappings": []}),
            output_mapping: json!({"mappings": []}),
            permission_code: None,
            risk_level: domain::McpRiskLevel::Low,
            des_id: "revision".to_string(),
            des_id_required: false,
            status: domain::McpToolStatus::Enabled,
            revision: 1,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        };
        let group = |id, path: &str| domain::McpGroupRecord {
            id,
            instance_record_id: instance_id,
            path: path.to_string(),
            display_name: path.to_string(),
            description_short: None,
            enabled: true,
            sort_order: 0,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        };
        let binding = |id, path: &str| domain::McpToolBindingRecord {
            id,
            instance_record_id: instance_id,
            tool_record_id: tool_id,
            group_path: path.to_string(),
            tool_id: "lookup".to_string(),
            display_alias: None,
            visible: true,
            sort_order: 0,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        };
        let catalog = domain::McpCatalogSnapshot {
            instances: vec![instance],
            groups: vec![
                group(Uuid::from_u128(13), "/one"),
                group(Uuid::from_u128(14), "/two"),
            ],
            tools: vec![tool],
            bindings: vec![
                binding(Uuid::from_u128(15), "/one"),
                binding(Uuid::from_u128(16), "/two"),
            ],
            discovery_policies: Vec::new(),
        };

        let provider_tools = assistant_provider_tools(&catalog, &["catalog".to_string()]);

        assert_eq!(provider_tools.len(), 1);

        let wire_name = assistant_tool_name(&catalog.instances[0], &catalog.tools[0]);
        let trace_call = assistant_tool_call_for_trace(
            &catalog,
            &json!({
                "id": "call_lookup",
                "name": wire_name,
                "arguments": {}
            }),
        );
        assert_eq!(trace_call["name"], "Lookup");
        assert_eq!(trace_call["wire_name"], wire_name);
    }
}
