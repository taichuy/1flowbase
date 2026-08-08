use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
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
            native_result_from_run_detail, ApplicationPublishedFlowRunRepository,
            ApplicationPublishedRunService, CreateAssistantConversationInput,
            CreateAssistantRunCommand, ListAssistantConversationsInput,
        },
    },
    mcp_management::McpManagementService,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        spawn_runtime_debug_event_persister, wait_for_runtime_debug_event_persister,
        OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{OrchestrationRuntimeRepository, RuntimeEventStreamPolicy},
    profile::{ProfileService, UpdateMeMetaCommand},
};
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use utoipa::ToSchema;
use uuid::Uuid;

pub(crate) mod websocket;

#[cfg(test)]
use crate::routes::mcp_protocol::virtual_ui::VirtualMcpScope;
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
        mcp_protocol::virtual_ui,
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
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
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
    pub conversation_id: Uuid,
    pub status: String,
    pub answer: Option<String>,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAssistantConversationBody {
    pub application_id: Uuid,
    #[serde(default)]
    pub seed_legacy_flow_run_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantConversationResponse {
    pub conversation_id: Uuid,
    pub application_id: Uuid,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ListAssistantConversationsQuery {
    pub application_id: Uuid,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantConversationSummaryResponse {
    pub conversation_id: Option<Uuid>,
    pub legacy_flow_run_id: Option<Uuid>,
    pub latest_flow_run_id: Option<Uuid>,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantConversationPageResponse {
    pub items: Vec<AssistantConversationSummaryResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssistantConversationMessagesQuery {
    pub application_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantConversationMessageResponse {
    pub id: String,
    pub flow_run_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

struct PreparedAssistantExecution {
    application_id: Uuid,
    conversation_id: Uuid,
    actor: domain::ActorContext,
    flow_run_id: Uuid,
    mcp_instance_ids: Vec<String>,
    request_headers: HeaderMap,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/assistant/settings",
            console_get(get_settings, Authenticated).patch(update_settings, Authenticated),
        )
        .route(
            "/assistant/conversations",
            console_get(list_conversations, Authenticated).post(create_conversation, Authenticated),
        )
        .route(
            "/assistant/conversations/{conversation_id}/messages",
            console_get(get_conversation_messages, Authenticated),
        )
        .route(
            "/assistant/legacy-runs/{flow_run_id}/messages",
            console_get(get_legacy_snapshot_messages, Authenticated),
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
    post,
    path = "/api/console/assistant/conversations",
    operation_id = "assistant_create_conversation",
    summary = "Create an embedded assistant conversation",
    description = "Creates a server-owned assistant conversation for the current Cookie session, workspace, and selected Agent Flow application.",
    request_body = CreateAssistantConversationBody,
    responses((status = 201, body = AssistantConversationResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_conversation(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateAssistantConversationBody>,
) -> Result<
    (
        axum::http::StatusCode,
        Json<ApiSuccess<AssistantConversationResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    assistant_preference_for_target(&state, &context, body.application_id).await?;
    if let Some(legacy_flow_run_id) = body.seed_legacy_flow_run_id {
        let seed_messages = state
            .store
            .list_assistant_legacy_snapshot_messages(
                context.actor.current_workspace_id,
                body.application_id,
                context.user.id,
                legacy_flow_run_id,
            )
            .await?;
        if seed_messages.is_empty() {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "assistant_legacy_snapshot",
            )
            .into());
        }
    }
    let conversation = state
        .store
        .create_assistant_conversation(&CreateAssistantConversationInput {
            conversation_id: Uuid::now_v7(),
            workspace_id: context.actor.current_workspace_id,
            application_id: body.application_id,
            actor_user_id: context.user.id,
            seed_legacy_flow_run_id: body.seed_legacy_flow_run_id,
        })
        .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiSuccess::new(assistant_conversation_response(
            conversation,
        ))),
    ))
}

#[utoipa::path(
    get,
    path = "/api/console/assistant/conversations",
    operation_id = "assistant_list_conversations",
    summary = "List embedded assistant conversations",
    description = "Lists the current Cookie session user's embedded assistant conversations and legacy single-run snapshots for one selected Agent Flow application.",
    params(("application_id" = Uuid, Query), ("page" = Option<i64>, Query), ("page_size" = Option<i64>, Query)),
    responses((status = 200, body = AssistantConversationPageResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_conversations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ListAssistantConversationsQuery>,
) -> Result<Json<ApiSuccess<AssistantConversationPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    assistant_preference_for_target(&state, &context, query.application_id).await?;
    let page = state
        .store
        .list_assistant_conversations(&ListAssistantConversationsInput {
            workspace_id: context.actor.current_workspace_id,
            application_id: query.application_id,
            actor_user_id: context.user.id,
            page: query.page.unwrap_or(1),
            page_size: query.page_size.unwrap_or(20),
        })
        .await?;
    Ok(Json(ApiSuccess::new(AssistantConversationPageResponse {
        items: page
            .items
            .into_iter()
            .map(|item| AssistantConversationSummaryResponse {
                conversation_id: item.conversation_id,
                legacy_flow_run_id: item.legacy_flow_run_id,
                latest_flow_run_id: item.latest_flow_run_id,
                title: item.title,
                created_at: rfc3339(item.created_at),
                updated_at: rfc3339(item.updated_at),
            })
            .collect(),
        total: page.total,
        page: page.page,
        page_size: page.page_size,
    })))
}

fn assistant_conversation_response(
    conversation: control_plane::application_public_api::run_service::AssistantConversationRecord,
) -> AssistantConversationResponse {
    AssistantConversationResponse {
        conversation_id: conversation.conversation_id,
        application_id: conversation.application_id,
        created_at: rfc3339(conversation.created_at),
        updated_at: rfc3339(conversation.updated_at),
    }
}

fn assistant_conversation_message_response(
    message: control_plane::application_public_api::run_service::AssistantConversationMessage,
) -> AssistantConversationMessageResponse {
    AssistantConversationMessageResponse {
        id: message.id,
        flow_run_id: message.flow_run_id,
        role: message.role,
        content: message.content,
        created_at: rfc3339(message.created_at),
    }
}

fn rfc3339(value: time::OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .expect("Rfc3339 is a valid fixed formatter")
}

#[utoipa::path(
    get,
    path = "/api/console/assistant/conversations/{conversation_id}/messages",
    operation_id = "assistant_get_conversation_messages",
    summary = "Read embedded assistant conversation messages",
    description = "Reads visible messages in one server-owned embedded assistant conversation after workspace, user, and application filtering.",
    params(("conversation_id" = Uuid, Path), ("application_id" = Uuid, Query)),
    responses((status = 200, body = [AssistantConversationMessageResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_conversation_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<AssistantConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<Vec<AssistantConversationMessageResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    assistant_preference_for_target(&state, &context, query.application_id).await?;
    let messages = state
        .store
        .list_assistant_conversation_messages(
            context.actor.current_workspace_id,
            query.application_id,
            context.user.id,
            conversation_id,
        )
        .await?;
    Ok(Json(ApiSuccess::new(
        messages
            .into_iter()
            .map(assistant_conversation_message_response)
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/assistant/legacy-runs/{flow_run_id}/messages",
    operation_id = "assistant_get_legacy_snapshot_messages",
    summary = "Read embedded assistant legacy snapshot messages",
    description = "Reads the visible messages for one pre-conversation embedded assistant run as an immutable legacy snapshot.",
    params(("flow_run_id" = Uuid, Path), ("application_id" = Uuid, Query)),
    responses((status = 200, body = [AssistantConversationMessageResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_legacy_snapshot_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(flow_run_id): Path<Uuid>,
    Query(query): Query<AssistantConversationMessagesQuery>,
) -> Result<Json<ApiSuccess<Vec<AssistantConversationMessageResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    assistant_preference_for_target(&state, &context, query.application_id).await?;
    let messages = state
        .store
        .list_assistant_legacy_snapshot_messages(
            context.actor.current_workspace_id,
            query.application_id,
            context.user.id,
            flow_run_id,
        )
        .await?;
    Ok(Json(ApiSuccess::new(
        messages
            .into_iter()
            .map(assistant_conversation_message_response)
            .collect(),
    )))
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
        available_targets(&state, &context.actor).await?;
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
    validate_preference(&state, &context.actor, &preference).await?;
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
        available_targets(&state, &context.actor).await?;
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
    let mcp_runtime_invoker = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            state.clone(),
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?,
    );
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
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let detail = runtime
        .start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: execution.application_id,
            flow_run_id: execution.flow_run_id,
            provider_transport_slot: None,
        })
        .await?;
    let native_result = native_result_from_run_detail(&detail, json!({}));
    Ok(Json(ApiSuccess::new(AssistantRunResponse {
        id: detail.flow_run.id,
        application_id: execution.application_id,
        conversation_id: execution.conversation_id,
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

    let mcp_runtime_invoker = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            state.clone(),
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?,
    );
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
        .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
        .with_llm_routing_counter_store(background_state.infrastructure.cache_store())
        .with_provider_request_log_queue(background_state.infrastructure.task_queue())
        .with_runtime_event_stream(background_state.runtime_event_stream.clone());
        let result = async {
            let detail = runtime
                .start_published_flow_run(StartPublishedFlowRunCommand {
                    application_id: execution.application_id,
                    flow_run_id: execution.flow_run_id,
                    provider_transport_slot: None,
                })
                .await?;
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
    let assistant_conversation_id = match body.conversation_id {
        Some(conversation_id) => {
            if state
                .store
                .get_assistant_conversation(
                    context.actor.current_workspace_id,
                    application_id,
                    context.user.id,
                    conversation_id,
                )
                .await?
                .is_none()
            {
                return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                    "assistant_conversation",
                )
                .into());
            }
            conversation_id
        }
        None => {
            state
                .store
                .create_assistant_conversation(&CreateAssistantConversationInput {
                    conversation_id: Uuid::now_v7(),
                    workspace_id: context.actor.current_workspace_id,
                    application_id,
                    actor_user_id: context.user.id,
                    seed_legacy_flow_run_id: None,
                })
                .await?
                .conversation_id
        }
    };
    let execution = assistant_execution(&preference)?;
    let inputs = NativeObject::default();
    let flow_run = ApplicationPublishedRunService::new(state.store.clone())
        .create_assistant_run(CreateAssistantRunCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            application_id,
            assistant_conversation_id: Some(assistant_conversation_id),
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
        conversation_id: assistant_conversation_id,
        actor: context.actor.clone(),
        flow_run_id: flow_run.id,
        mcp_instance_ids: preference.mcp_instance_ids,
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
    validate_preference(state, &context.actor, &preference).await?;
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

async fn available_targets(
    state: &Arc<ApiState>,
    actor: &domain::ActorContext,
) -> Result<
    (
        Vec<AssistantPublishedFlowOption>,
        Vec<AssistantMcpInstanceOption>,
    ),
    ApiError,
> {
    let applications = ApplicationService::new(state.store.clone())
        .list_applications(actor.user_id)
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
        .read_catalog_for_actor(actor)
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
    actor: &domain::ActorContext,
    preference: &AssistantPreferenceBody,
) -> Result<(), ApiError> {
    if let Some(application_id) = preference.application_id {
        let application = ApplicationService::new(state.store.clone())
            .get_application(actor.user_id, application_id)
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
        .read_catalog_for_actor(actor)
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
    fn assistant_mcp_provider_exposes_only_virtual_ui_meta_tools() {
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

        let scope = VirtualMcpScope::selected(&catalog, &["catalog".to_string()]);
        let provider_tools = virtual_ui::provider_tools(&catalog, &scope);

        assert_eq!(provider_tools.len(), 4);
        assert_eq!(
            provider_tools
                .iter()
                .map(|tool| tool["function"]["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "catalog_mcp_list",
                "catalog_mcp_get",
                "catalog_mcp_result",
                "catalog_mcp_call"
            ]
        );
        assert!(provider_tools
            .iter()
            .all(
                |tool| tool["function"]["name"].as_str().is_some_and(|name| name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
            ));
        assert!(provider_tools.iter().all(|tool| {
            !tool["function"]["parameters"]
                .to_string()
                .contains("workspace_id")
        }));
        assert!(provider_tools[0]["function"]["parameters"]["properties"]
            .get("path_regex")
            .is_none());
    }
}
