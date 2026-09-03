use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

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
            assistant_conversation_native_history_to_values, ApplicationPublishedFlowRunRepository,
            ApplicationPublishedRunService, AssistantPageReference,
            CreateAssistantConversationInput, CreateAssistantRunCommand,
            ASSISTANT_PAGE_REFERENCE_MAX_COUNT, ASSISTANT_PAGE_REFERENCE_MAX_TOTAL_BYTES,
        },
    },
    mcp_management::McpManagementService,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        spawn_runtime_debug_event_persister, wait_for_runtime_debug_event_persister,
        OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{
        CacheStore, OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventStream,
        RuntimeEventStreamPolicy, TaskQueue,
    },
};
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use storage_durable_postgres::MainDurableStore;
use tokio::sync::{mpsc, oneshot};
use utoipa::ToSchema;
use uuid::Uuid;

#[cfg(test)]
mod _tests;
mod client_tools;
pub mod conversation_events;
pub(crate) mod interface;
mod run_activity;
pub(crate) mod websocket;
pub(crate) mod websocket_interface;
pub(crate) mod websocket_ticket_interface;

pub use client_tools::AssistantClientToolBridge;
use client_tools::AssistantRuntimeToolInvoker;
pub use conversation_events::AssistantConversationSummaryResponse;
use conversation_events::{AssistantConversationEventKind, AssistantConversationEventScope};
pub use run_activity::AssistantRunActivityPageResponse;

#[cfg(test)]
use crate::routes::mcp_protocol::virtual_ui::VirtualMcpScope;
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::RequestContext,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
        debug_run_stream,
        mcp_protocol::virtual_ui,
    },
};

const ASSISTANT_META_KEY: &str = "embedded_assistant";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssistantClientToolId {
    GetClientContext,
    RefreshClientView,
    ListPageBlocks,
    InspectBlockRender,
    SearchBlockRender,
    ReadBlockRenderFragment,
    ClickBlockElement,
    RecompileBlock,
}

impl AssistantClientToolId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GetClientContext => "get_client_context",
            Self::RefreshClientView => "refresh_client_view",
            Self::ListPageBlocks => "list_page_blocks",
            Self::InspectBlockRender => "inspect_block_render",
            Self::SearchBlockRender => "search_block_render",
            Self::ReadBlockRenderFragment => "read_block_render_fragment",
            Self::ClickBlockElement => "click_block_element",
            Self::RecompileBlock => "recompile_block",
        }
    }
}

fn default_enabled_client_tools() -> Vec<AssistantClientToolId> {
    vec![
        AssistantClientToolId::GetClientContext,
        AssistantClientToolId::RefreshClientView,
    ]
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AssistantPreferenceBody {
    pub application_id: Option<Uuid>,
    #[serde(default)]
    pub mcp_instance_ids: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default = "default_enabled_client_tools")]
    pub enabled_client_tools: Vec<AssistantClientToolId>,
}

impl Default for AssistantPreferenceBody {
    fn default() -> Self {
        Self {
            application_id: None,
            mcp_instance_ids: Vec::new(),
            model: None,
            reasoning_effort: None,
            enabled_client_tools: default_enabled_client_tools(),
        }
    }
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
    pub page_reference_max_bytes: usize,
    pub page_reference_max_count: usize,
    pub page_reference_max_total_bytes: usize,
    pub run_capabilities: AssistantRunCapabilities,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AssistantPageReferenceBody {
    pub page_url: String,
    pub page_title: String,
    pub outer_html: String,
}

impl AssistantPageReferenceBody {
    fn from_reference(reference: AssistantPageReference) -> Self {
        Self {
            page_url: reference.page_url().to_string(),
            page_title: reference.page_title().to_string(),
            outer_html: reference.outer_html().to_string(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAssistantRunBody {
    pub application_id: Uuid,
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    pub query: String,
    #[serde(default)]
    pub page_references: Vec<AssistantPageReferenceBody>,
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
    pub status: String,
    pub page_references: Vec<AssistantPageReferenceBody>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AssistantRunActivityQuery {
    pub application_id: Uuid,
    pub after_sequence: Option<i64>,
    pub page_size: Option<usize>,
}

pub(super) struct PreparedAssistantExecution {
    application_id: Uuid,
    conversation_id: Uuid,
    actor: domain::ActorContext,
    flow_run_id: Uuid,
    mcp_instance_ids: Vec<String>,
    enabled_client_tools: Vec<AssistantClientToolId>,
    request_headers: HeaderMap,
}

/// Explicit dependencies for embedded Assistant run execution.
///
/// The frozen interface adapter owns these narrow runtime services rather than
/// retaining the API composition state, so the lifecycle can be invoked from
/// HTTP and future non-HTTP Console transports through the same boundary.
#[derive(Clone)]
pub(crate) struct AssistantRunDependencies {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    runtime_activity: Arc<crate::runtime_activity::ApplicationRuntimeActivityTracker>,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    cache_store: Arc<dyn CacheStore>,
    task_queue: Arc<dyn TaskQueue>,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
    conversation_events: Arc<conversation_events::AssistantConversationEventHub>,
    assistant_executions: Arc<Mutex<HashMap<Uuid, tokio::task::AbortHandle>>>,
    assistant_client_sessions: Arc<Mutex<HashMap<Uuid, Arc<AssistantClientToolBridge>>>>,
    runtime_tool_invoker_factory: Arc<dyn AssistantRuntimeToolInvokerFactory>,
}

pub(crate) trait AssistantRuntimeToolInvokerFactory: Send + Sync + 'static {
    fn for_actor<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<virtual_ui::RuntimeInternalToolInvokerFactory, ApiError>>
                + Send
                + 'a,
        >,
    >;
}

struct StateAssistantRuntimeToolInvokerFactory {
    state: Arc<ApiState>,
}

impl AssistantRuntimeToolInvokerFactory for StateAssistantRuntimeToolInvokerFactory {
    fn for_actor<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<virtual_ui::RuntimeInternalToolInvokerFactory, ApiError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(crate::runtime_internal_tool_invoker_factory(
            &self.state,
            actor,
        ))
    }
}

pub(crate) fn run_dependencies(state: Arc<ApiState>) -> AssistantRunDependencies {
    AssistantRunDependencies {
        store: state.store.clone(),
        provider_runtime: state.provider_runtime.clone(),
        runtime_activity: state.runtime_activity.clone(),
        runtime_engine: state.runtime_engine.clone(),
        provider_secret_master_key: state.provider_secret_master_key.clone(),
        api_node_id: state.api_node_id.clone(),
        provider_install_root: state.provider_install_root.clone(),
        file_storage_registry: state.file_storage_registry.clone(),
        cache_store: state.infrastructure.cache_store(),
        task_queue: state.infrastructure.task_queue(),
        runtime_event_stream: state.runtime_event_stream.clone(),
        conversation_events: state.assistant_conversation_events.clone(),
        assistant_executions: state.assistant_executions.clone(),
        assistant_client_sessions: state.assistant_client_sessions.clone(),
        runtime_tool_invoker_factory: Arc::new(StateAssistantRuntimeToolInvokerFactory { state }),
    }
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
            "/assistant/conversations/:conversation_id/messages",
            console_get(get_conversation_messages, Authenticated),
        )
        .route(
            "/assistant/legacy-runs/:flow_run_id/messages",
            console_get(get_legacy_snapshot_messages, Authenticated),
        )
        .route(
            "/assistant/runs/:flow_run_id/activity",
            console_get(get_run_activity, Authenticated),
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
    path = "/api/console/assistant/runs/{flow_run_id}/activity",
    operation_id = "assistant_get_run_activity",
    summary = "Read an embedded assistant run activity timeline",
    description = "Reads durable runtime events in stream order after verifying that the run belongs to the current Cookie session user and selected Agent Flow application.",
    params(
        ("flow_run_id" = Uuid, Path),
        ("application_id" = Uuid, Query),
        ("after_sequence" = Option<i64>, Query),
        ("page_size" = Option<usize>, Query)
    ),
    responses(
        (status = 200, body = AssistantRunActivityPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_run_activity(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(flow_run_id): Path<Uuid>,
    Query(query): Query<AssistantRunActivityQuery>,
) -> Result<Json<ApiSuccess<AssistantRunActivityPageResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.runs.activity.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::AssistantConversationsInput::GetRunActivity { flow_run_id, query },
    )
    .await?;
    let interface::AssistantConversationsOutput::RunActivity(activity) = output else {
        unreachable!("assistant run activity binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(activity)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.conversations.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::AssistantConversationsInput::CreateConversation(body),
    )
    .await?;
    let interface::AssistantConversationsOutput::Conversation(conversation) = output else {
        unreachable!("assistant conversation create binding returned a different output")
    };
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ApiSuccess::new(conversation)),
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.conversations.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::AssistantConversationsInput::ListConversations(query),
    )
    .await?;
    let interface::AssistantConversationsOutput::ConversationPage(page) = output else {
        unreachable!("assistant conversation list binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(page)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.conversations.messages.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::AssistantConversationsInput::GetConversationMessages {
            conversation_id,
            query,
        },
    )
    .await?;
    let interface::AssistantConversationsOutput::ConversationMessages(messages) = output else {
        unreachable!("assistant conversation messages binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(messages)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.legacy-runs.messages.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::AssistantConversationsInput::GetLegacySnapshotMessages { flow_run_id, query },
    )
    .await?;
    let interface::AssistantConversationsOutput::ConversationMessages(messages) = output else {
        unreachable!("assistant legacy snapshot messages binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(messages)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.settings.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::AssistantSettingsInput::Get,
    )
    .await?;
    let interface::AssistantSettingsOutput::Settings(settings) = output;
    Ok(Json(ApiSuccess::new(settings)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.settings.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::AssistantSettingsInput::Update(preference),
    )
    .await?;
    let interface::AssistantSettingsOutput::Settings(settings) = output;
    Ok(Json(ApiSuccess::new(settings)))
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
    let interface::AssistantRunOutput(response) = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.assistant.runs.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::CookieSessionWithCsrf {
            state: Arc::clone(&state),
            headers: headers.clone(),
        },
        interface::AssistantRunInput { body, headers },
    )
    .await?;
    Ok(Json(ApiSuccess::new(response)))
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
    let stream = crate::routes::console_interface::invoke_server_stream::<
        interface::AssistantRunInput,
        interface::AssistantRunStreamEvent,
        interface::AssistantRunStreamOutput,
    >(
        Arc::clone(&state),
        "http.console.assistant.runs.stream.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::CookieSessionWithCsrf {
            state: Arc::clone(&state),
            headers: headers.clone(),
        },
        interface::AssistantRunInput { body, headers },
    )
    .await?;
    let (sender, receiver) = mpsc::channel(32);
    let (mut events, completion) = stream.into_parts();
    tokio::spawn(async move {
        while let Some(interface::AssistantRunStreamEvent(event)) = events.recv().await {
            if sender.send(event).await.is_err() {
                return;
            }
        }
    });
    tokio::spawn(async move {
        let _ = completion.complete().await;
    });

    Ok(Sse::new(debug_run_stream::DebugRunSseStream::new(receiver))
        .keep_alive(KeepAlive::default()))
}

pub(super) async fn launch_assistant_execution(
    dependencies: AssistantRunDependencies,
    execution: PreparedAssistantExecution,
    client_tool_bridge: Option<Arc<AssistantClientToolBridge>>,
) -> Result<Uuid, ApiError> {
    let run_id = execution.flow_run_id;
    let application_id = execution.application_id;
    dependencies
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await?;
    let persister_handle = spawn_runtime_debug_event_persister(
        dependencies.store.clone(),
        dependencies.runtime_event_stream.clone(),
        run_id,
    );
    dependencies
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_accepted(run_id))
        .await?;
    dependencies
        .runtime_event_stream
        .append(run_id, debug_stream_events::heartbeat())
        .await?;

    spawn_assistant_conversation_projection(
        dependencies.store.clone(),
        dependencies.runtime_event_stream.clone(),
        dependencies.conversation_events.clone(),
        AssistantConversationEventScope {
            workspace_id: execution.actor.current_workspace_id,
            application_id,
            actor_user_id: execution.actor.user_id,
        },
        execution.conversation_id,
        run_id,
    );

    if let Some(client) = client_tool_bridge.as_ref() {
        dependencies
            .assistant_client_sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(run_id, client.clone());
    }

    let mcp_client_bridge = client_tool_bridge.clone();
    let mcp_runtime_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    > = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            dependencies
                .runtime_tool_invoker_factory
                .for_actor(&execution.actor)
                .await?,
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?
        .with_assistant_client(mcp_client_bridge),
    );
    let assistant_runtime_tool_invoker = Arc::new(AssistantRuntimeToolInvoker::new(
        mcp_runtime_invoker,
        client_tool_bridge.clone(),
    ));
    let background_dependencies = dependencies.clone();
    let execution_registry = dependencies.assistant_executions.clone();
    let client_session_registry = dependencies.assistant_client_sessions.clone();
    let (start_sender, start_receiver) = oneshot::channel();
    let execution_handle = tokio::spawn(async move {
        if start_receiver.await.is_err() {
            return;
        }
        let runtime = OrchestrationRuntimeService::new(
            background_dependencies.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new_with_activity(
                background_dependencies.provider_runtime.clone(),
                background_dependencies.runtime_activity.clone(),
            ),
            background_dependencies.runtime_engine.clone(),
            background_dependencies.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            background_dependencies.api_node_id.clone(),
            background_dependencies.provider_install_root.clone(),
        )
        .with_file_storage_registry(background_dependencies.file_storage_registry.clone())
        .with_runtime_internal_tool_invoker(assistant_runtime_tool_invoker)
        .with_llm_routing_counter_store(background_dependencies.cache_store.clone())
        .with_provider_request_log_queue(background_dependencies.task_queue.clone())
        .with_runtime_event_stream(background_dependencies.runtime_event_stream.clone());
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
                    background_dependencies.runtime_event_stream.clone(),
                    &detail.flow_run,
                )
                .await;
            }
            Ok(_) => {}
            Err(error) => {
                match background_dependencies
                    .store
                    .get_flow_run(application_id, run_id)
                    .await
                {
                    Ok(Some(winner)) => {
                        project_runtime_event_stream_terminal(
                            background_dependencies.runtime_event_stream.clone(),
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
        execution_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&run_id);
        let client_session = client_session_registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&run_id);
        if let Some(client_session) = client_session {
            client_session.close().await;
        }
    });
    dependencies
        .assistant_executions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(run_id, execution_handle.abort_handle());
    // The task cannot enter the Provider/runtime boundary until its abort handle is visible to
    // every cancellation entry point.
    let _ = start_sender.send(());
    Ok(run_id)
}

pub(crate) async fn execute_assistant_run(
    dependencies: &AssistantRunDependencies,
    principal: &interface_runtime::UserPrincipal,
    body: StartAssistantRunBody,
    headers: HeaderMap,
) -> Result<AssistantRunResponse, ApiError> {
    let execution =
        prepare_assistant_execution(&dependencies.store, &headers, principal.actor(), body).await?;
    let mcp_runtime_invoker = Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            dependencies
                .runtime_tool_invoker_factory
                .for_actor(&execution.actor)
                .await?,
            execution.request_headers.clone(),
            execution.actor.clone(),
            execution.mcp_instance_ids.clone(),
        )
        .await?,
    );
    let runtime = OrchestrationRuntimeService::new(
        dependencies.store.clone(),
        crate::provider_runtime::ApiProviderRuntime::new_with_activity(
            dependencies.provider_runtime.clone(),
            dependencies.runtime_activity.clone(),
        ),
        dependencies.runtime_engine.clone(),
        dependencies.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        dependencies.api_node_id.clone(),
        dependencies.provider_install_root.clone(),
    )
    .with_file_storage_registry(dependencies.file_storage_registry.clone())
    .with_runtime_internal_tool_invoker(mcp_runtime_invoker)
    .with_llm_routing_counter_store(dependencies.cache_store.clone())
    .with_provider_request_log_queue(dependencies.task_queue.clone());
    let detail = runtime
        .start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: execution.application_id,
            flow_run_id: execution.flow_run_id,
            provider_transport_slot: None,
        })
        .await?;
    publish_assistant_conversation_summary(
        &dependencies.store,
        &dependencies.conversation_events,
        AssistantConversationEventScope {
            workspace_id: principal.actor().current_workspace_id,
            application_id: execution.application_id,
            actor_user_id: principal.actor().user_id,
        },
        execution.conversation_id,
        AssistantConversationEventKind::Updated,
    )
    .await;
    let native_result =
        control_plane::application_public_api::run_service::native_result_from_run_detail(
            &detail,
            json!({}),
        );
    Ok(AssistantRunResponse {
        id: detail.flow_run.id,
        application_id: execution.application_id,
        conversation_id: execution.conversation_id,
        status: detail.flow_run.status.as_str().to_string(),
        answer: native_result.answer,
        output_payload: detail.flow_run.output_payload,
        error_payload: detail.flow_run.error_payload,
    })
}

fn spawn_assistant_conversation_projection(
    store: MainDurableStore,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
    conversation_events: Arc<conversation_events::AssistantConversationEventHub>,
    scope: AssistantConversationEventScope,
    conversation_id: Uuid,
    run_id: Uuid,
) {
    tokio::spawn(async move {
        let Ok(mut subscription) = runtime_event_stream.subscribe(run_id, Some(0)).await else {
            tracing::warn!(
                application_id = %scope.application_id,
                flow_run_id = %run_id,
                "assistant conversation projection could not subscribe to runtime events"
            );
            return;
        };

        for event in subscription.replay {
            let terminal =
                RuntimeEventCloseReason::from_terminal_event_type(&event.event_type).is_some();
            if should_publish_assistant_conversation_event(&event.event_type) {
                publish_assistant_conversation_summary(
                    &store,
                    &conversation_events,
                    scope,
                    conversation_id,
                    AssistantConversationEventKind::Updated,
                )
                .await;
            }
            if terminal {
                return;
            }
        }

        while let Some(event) = subscription.live_events.recv().await {
            let terminal =
                RuntimeEventCloseReason::from_terminal_event_type(&event.event_type).is_some();
            if should_publish_assistant_conversation_event(&event.event_type) {
                publish_assistant_conversation_summary(
                    &store,
                    &conversation_events,
                    scope,
                    conversation_id,
                    AssistantConversationEventKind::Updated,
                )
                .await;
            }
            if terminal {
                return;
            }
        }
    });
}

fn should_publish_assistant_conversation_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_accepted"
            | "flow_started"
            | "flow_finished"
            | "flow_incomplete"
            | "flow_failed"
            | "flow_cancelled"
            | "waiting_human"
            | "waiting_callback"
    )
}

async fn publish_assistant_conversation_summary(
    store: &MainDurableStore,
    conversation_events: &conversation_events::AssistantConversationEventHub,
    scope: AssistantConversationEventScope,
    conversation_id: Uuid,
    kind: AssistantConversationEventKind,
) {
    match store
        .get_assistant_conversation_summary(
            scope.workspace_id,
            scope.application_id,
            scope.actor_user_id,
            conversation_id,
        )
        .await
    {
        Ok(Some(summary)) => conversation_events.publish(
            scope,
            kind,
            AssistantConversationSummaryResponse::from(summary),
        ),
        Ok(None) => tracing::warn!(
            application_id = %scope.application_id,
            conversation_id = %conversation_id,
            "assistant conversation projection could not find its durable summary"
        ),
        Err(error) => tracing::warn!(
            application_id = %scope.application_id,
            conversation_id = %conversation_id,
            error = %error,
            "assistant conversation projection could not load its durable summary"
        ),
    }
}

fn abort_registered_assistant_execution(
    executions: &std::sync::Mutex<std::collections::HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> Option<tokio::task::AbortHandle> {
    executions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&run_id)
}

pub(crate) fn abort_assistant_execution_in(
    executions: &Mutex<HashMap<Uuid, tokio::task::AbortHandle>>,
    run_id: Uuid,
) -> bool {
    let handle = abort_registered_assistant_execution(executions, run_id);
    if let Some(handle) = handle {
        handle.abort();
        true
    } else {
        false
    }
}

pub(super) async fn prepare_assistant_execution(
    store: &MainDurableStore,
    headers: &HeaderMap,
    actor: &domain::ActorContext,
    body: StartAssistantRunBody,
) -> Result<PreparedAssistantExecution, ApiError> {
    if body.query.trim().is_empty() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("query").into());
    }
    let page_reference_total_bytes = body
        .page_references
        .iter()
        .try_fold(0_usize, |total, reference| {
            total.checked_add(reference.outer_html.len())
        })
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "page_references",
        ))?;
    if body.page_references.len() > ASSISTANT_PAGE_REFERENCE_MAX_COUNT
        || page_reference_total_bytes > ASSISTANT_PAGE_REFERENCE_MAX_TOTAL_BYTES
    {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("page_references").into(),
        );
    }
    let page_references = body
        .page_references
        .into_iter()
        .map(|reference| {
            AssistantPageReference::try_new(
                reference.page_url,
                reference.page_title,
                reference.outer_html,
            )
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "page_references",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let application_id = body.application_id;
    let preference = assistant_preference_for_actor(store, actor, application_id).await?;
    let assistant_conversation_id = match body.conversation_id {
        Some(conversation_id) => {
            if store
                .get_assistant_conversation(
                    actor.current_workspace_id,
                    application_id,
                    actor.user_id,
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
            if store
                .has_active_assistant_conversation_run(conversation_id)
                .await?
            {
                return Err(control_plane::errors::ControlPlaneError::Conflict(
                    "assistant_conversation_active",
                )
                .into());
            }
            conversation_id
        }
        None => {
            store
                .create_assistant_conversation(&CreateAssistantConversationInput {
                    conversation_id: Uuid::now_v7(),
                    workspace_id: actor.current_workspace_id,
                    application_id,
                    actor_user_id: actor.user_id,
                    seed_legacy_flow_run_id: None,
                })
                .await?
                .conversation_id
        }
    };
    let history = assistant_conversation_native_history_to_values(
        store
            .list_assistant_conversation_native_history(
                actor.current_workspace_id,
                application_id,
                actor.user_id,
                assistant_conversation_id,
            )
            .await?,
    );
    let execution = assistant_execution(&preference)?;
    let inputs = NativeObject::default();
    let flow_run = ApplicationPublishedRunService::new(store.clone())
        .create_assistant_run(CreateAssistantRunCommand {
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            application_id,
            assistant_conversation_id: Some(assistant_conversation_id),
            page_references,
            request: NativeRunRequest {
                query: body.query,
                system: Vec::new(),
                model: preference.model,
                history,
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
        actor: actor.clone(),
        flow_run_id: flow_run.id,
        mcp_instance_ids: preference.mcp_instance_ids,
        enabled_client_tools: preference.enabled_client_tools,
        request_headers: headers.clone(),
    })
}

async fn assistant_preference_for_target(
    state: &Arc<ApiState>,
    context: &RequestContext,
    application_id: Uuid,
) -> Result<AssistantPreferenceBody, ApiError> {
    assistant_preference_for_actor(&state.store, &context.actor, application_id).await
}

async fn assistant_preference_for_actor(
    store: &MainDurableStore,
    actor: &domain::ActorContext,
    application_id: Uuid,
) -> Result<AssistantPreferenceBody, ApiError> {
    let user = store
        .find_user_by_id(actor.user_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?;
    let preference = read_preference(&user.meta, actor.current_workspace_id);
    if preference.application_id != Some(application_id) {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "assistant_application_id",
        )
        .into());
    }
    validate_preference(store, actor, &preference).await?;
    ApplicationService::new(store.for_actor(actor.clone()))
        .load_application_for_non_crud_console_operation(
            actor.user_id,
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
    store: &MainDurableStore,
    actor: &domain::ActorContext,
) -> Result<
    (
        Vec<AssistantPublishedFlowOption>,
        Vec<AssistantMcpInstanceOption>,
    ),
    ApiError,
> {
    let applications = ApplicationService::new(store.for_actor(actor.clone()))
        .list_applications(actor.user_id)
        .await?;
    let mut published_agent_flows = Vec::new();
    for application in applications
        .into_iter()
        .filter(|application| application.application_type == domain::ApplicationType::AgentFlow)
    {
        if ApplicationPublicationService::new(store.for_actor(actor.clone()))
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
    let catalog = McpManagementService::new(store.clone())
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
    store: &MainDurableStore,
    actor: &domain::ActorContext,
    application_id: Option<Uuid>,
) -> Result<AssistantRunCapabilities, ApiError> {
    let Some(application_id) = application_id else {
        return Ok(AssistantRunCapabilities::default());
    };
    let publication = ApplicationPublicationService::new(store.for_actor(actor.clone()))
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
    let models = if model_selection_enabled {
        extract_agent_model_catalog_from_start_node(&publication.document_snapshot)
    } else {
        Vec::new()
    }
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
    store: &MainDurableStore,
    actor: &domain::ActorContext,
    preference: &AssistantPreferenceBody,
) -> Result<(), ApiError> {
    if let Some(application_id) = preference.application_id {
        let application = ApplicationService::new(store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await?;
        if application.application_type != domain::ApplicationType::AgentFlow {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_application_id",
            )
            .into());
        }
        let capabilities = assistant_run_capabilities(store, actor, Some(application_id)).await?;
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
    let catalog = McpManagementService::new(store.clone())
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
        assert_eq!(
            read_preference(&meta, current).enabled_client_tools,
            vec![
                AssistantClientToolId::GetClientContext,
                AssistantClientToolId::RefreshClientView,
            ]
        );
    }

    #[test]
    fn ac_001_assistant_client_tools_default_enabled_and_preserve_explicit_disable() {
        let workspace_id = Uuid::from_u128(20);
        assert_eq!(
            read_preference(&json!({}), workspace_id).enabled_client_tools,
            vec![
                AssistantClientToolId::GetClientContext,
                AssistantClientToolId::RefreshClientView,
            ]
        );

        let meta = json!({ ASSISTANT_META_KEY: { "workspaces": {
            workspace_id.to_string(): {
                "application_id": null,
                "mcp_instance_ids": [],
                "enabled_client_tools": ["get_client_context"]
            }
        }}});
        assert_eq!(
            read_preference(&meta, workspace_id).enabled_client_tools,
            vec![AssistantClientToolId::GetClientContext]
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
            webmcp_exposure: domain::WebMcpExposure::Disabled,
            managed_by: None,
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
            managed_by: None,
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
