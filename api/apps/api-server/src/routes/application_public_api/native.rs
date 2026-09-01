use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use control_plane::{
    application_public_api::{
        api_keys::ApplicationApiKeyService,
        model_catalog::{AgentModelCapabilities, AgentModelDescriptor, AgentModelReasoning},
        native::{
            translate_native_run_request, ApplicationNativeRunService, GetNativeRunCommand,
            NativeRunRequest, NativeRunResult, NativeRunStatus, NativeRunValidationError,
        },
        protocol_translation::{
            TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol,
            TranslationReport, TranslationSafeRepresentation,
        },
        run_service::native_result_from_run_detail,
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::{
        AuthRepository, ProviderProtocolContextSlotId, ProviderProtocolContextValue,
        ProviderTransportStore, RuntimeEventStream,
    },
};
use plugin_framework::provider_contract::ProtocolContextEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, error, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, InvocationEnvelope,
    InvocationId, InvocationLineage,
};

use crate::{
    app_state::ApiState,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::{
        application_public_api::{
            native_interface::{
                self, ApplicationNativeRunFuture, ApplicationNativeRunInput,
                ApplicationNativeRunOutput, ApplicationNativeRunPort,
                ApplicationNativeRunStreamEvent, ApplicationNativeRunStreamFuture,
                ApplicationNativeRunTargetError,
            },
            native_read_interface::{
                self, NativeCancelRunInput, NativeCancelRunOutput, NativeGetRunInput,
                NativeGetRunOutput, NativeModelsInput, NativeModelsOutput, NativeReadTargetError,
                NativeResumeRunInput, NativeResumeRunOutput, NativeRuntimeInvokerFactory,
                NativeUploadFileInput, NativeUploadFileOutput, RuntimeInvokerFuture,
            },
            sse,
            stream_terminal_fallback::{
                recover_missing_stream_terminal_winner_with_dependencies,
                NativeRunTerminalDependencies,
            },
        },
        files::UploadedFileResponse,
        mcp_protocol::virtual_ui,
    },
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

pub(crate) fn api_provider_runtime(state: &ApiState) -> ApiProviderRuntime {
    ApiProviderRuntime::new_with_activity(
        state.provider_runtime.clone(),
        state.runtime_activity.clone(),
    )
}

pub(crate) async fn public_mcp_runtime_invoker(
    state: &Arc<ApiState>,
    bearer_token: &str,
) -> Result<Arc<virtual_ui::ApiMcpRuntimeToolInvoker>, NativeApiError> {
    let api_actor = ApplicationApiKeyService::new(state.store.clone())
        .authenticate_bearer_token(bearer_token)
        .await
        .map_err(service_error)?;
    let actor =
        AuthRepository::load_actor_context_for_user(&state.store, api_actor.creator_user_id)
            .await
            .map_err(service_error)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {bearer_token}").parse().map_err(|_| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "invalid application API key",
            )
        })?,
    );
    Ok(Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new_with_forwarded_authorization(
            crate::runtime_internal_tool_invoker_factory(state, &actor)
                .await
                .map_err(|error| service_error(error.0))?,
            headers,
            actor,
            Vec::new(),
        )
        .await
        .map_err(|error| service_error(error.0))?,
    ))
}

pub(crate) async fn public_mcp_runtime_invoker_for_actor(
    state: &Arc<ApiState>,
    actor: &control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
) -> Result<Arc<virtual_ui::ApiMcpRuntimeToolInvoker>, NativeApiError> {
    Ok(Arc::new(
        virtual_ui::ApiMcpRuntimeToolInvoker::new(
            crate::runtime_internal_tool_invoker_factory(state, &actor.actor)
                .await
                .map_err(|error| service_error(error.0))?,
            HeaderMap::new(),
            actor.actor.clone(),
            Vec::new(),
        )
        .await
        .map_err(|error| service_error(error.0))?,
    ))
}

pub(crate) struct NativeRuntimeInvokerFactoryDependencies {
    pub(crate) runtime_dependencies: virtual_ui::RuntimeInternalToolInvokerDependencies,
    pub(crate) interface_catalog:
        crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies,
    pub(crate) interface_registry:
        Arc<dyn crate::routes::mcp_management::interface_catalog::McpInterfaceRegistrySnapshotPort>,
    pub(crate) interface_dispatch: Arc<dyn virtual_ui::McpInterfaceDispatchPort>,
}

struct NativeRuntimeInvokerFactoryAdapter(NativeRuntimeInvokerFactoryDependencies);

impl NativeRuntimeInvokerFactory for NativeRuntimeInvokerFactoryAdapter {
    fn for_actor<'a>(
        &'a self,
        actor: &'a control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> RuntimeInvokerFuture<'a> {
        let actor = actor.clone();
        Box::pin(async move {
            let catalog_dependencies = self
                .0
                .interface_catalog
                .with_interface_registry_snapshot(self.0.interface_registry.snapshot());
            let interface_catalog =
                crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with(
                    &catalog_dependencies,
                    &actor.actor,
                )
                .await
                .map_err(|error| service_error(error.0))?;
            let invoker = virtual_ui::ApiMcpRuntimeToolInvoker::new(
                virtual_ui::RuntimeInternalToolInvokerFactory::new(
                    self.0.runtime_dependencies.clone(),
                    Arc::new(virtual_ui::McpInterfaceCatalogSnapshot::new(
                        interface_catalog,
                    )),
                    Arc::clone(&self.0.interface_dispatch),
                ),
                HeaderMap::new(),
                actor.actor.clone(),
                Vec::new(),
            )
            .await
            .map_err(|error| service_error(error.0))?;
            Ok(Arc::new(invoker)
                as Arc<
                    dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
                >)
        })
    }
}

pub(crate) fn native_runtime_invoker_factory(
    dependencies: NativeRuntimeInvokerFactoryDependencies,
) -> Arc<dyn NativeRuntimeInvokerFactory> {
    Arc::new(NativeRuntimeInvokerFactoryAdapter(dependencies))
}

#[derive(Clone)]
pub(crate) struct ApplicationNativeRunDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) cache_store: Arc<dyn crate::host_infrastructure::CacheStore>,
    pub(crate) runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) provider_secret_master_key: String,
    pub(crate) api_node_id: String,
    pub(crate) provider_install_root: String,
    pub(crate) file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub(crate) task_queue: Arc<dyn crate::host_infrastructure::TaskQueue>,
    pub(crate) provider_transport_store: Arc<dyn ProviderTransportStore>,
    pub(crate) runtime_event_stream: Arc<dyn RuntimeEventStream>,
    pub(crate) runtime_activity: Arc<crate::runtime_activity::ApplicationRuntimeActivityTracker>,
    pub(crate) runtime_invoker_factory: Arc<dyn NativeRuntimeInvokerFactory>,
}

struct ApplicationNativeRunAdapter(ApplicationNativeRunDependencies);

impl ApplicationNativeRunPort for ApplicationNativeRunAdapter {
    fn create<'a>(
        &'a self,
        principal: &'a interface_runtime::ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a> {
        let dependencies = self.0.clone();
        let actor = application_actor_from_principal(principal);
        Box::pin(async move {
            ApplicationNativeRunService::new(dependencies.store.clone())
                .with_last_used_cache(dependencies.cache_store.clone())
                .create_native_run_for_actor(actor, input.request, input.protocol)
                .await
                .map(ApplicationNativeRunOutput)
                .map_err(native_error)
                .map_err(ApplicationNativeRunTargetError)
        })
    }

    fn execute_blocking<'a>(
        &'a self,
        principal: &'a interface_runtime::ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a> {
        let dependencies = self.0.clone();
        let actor = application_actor_from_principal(principal);
        Box::pin(async move {
            let run = ApplicationNativeRunService::new(dependencies.store.clone())
                .with_last_used_cache(dependencies.cache_store.clone())
                .create_native_run_for_actor(actor.clone(), input.request, input.protocol)
                .await
                .map_err(native_error)
                .map_err(ApplicationNativeRunTargetError)?;
            execute_blocking_native_run_for_actor_with_dependencies(dependencies, actor, run, None)
                .await
                .map(ApplicationNativeRunOutput)
                .map_err(ApplicationNativeRunTargetError)
        })
    }

    fn execute_stream<'a>(
        &'a self,
        principal: &'a interface_runtime::ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunStreamFuture<'a> {
        let dependencies = self.0.clone();
        let actor = application_actor_from_principal(principal);
        Box::pin(async move {
            let include_workflow_events =
                include_workflow_events(&input.request).map_err(ApplicationNativeRunTargetError)?;
            let run = ApplicationNativeRunService::new(dependencies.store.clone())
                .with_last_used_cache(dependencies.cache_store.clone())
                .create_native_run_for_actor(actor.clone(), input.request, input.protocol)
                .await
                .map_err(native_error)
                .map_err(ApplicationNativeRunTargetError)?;
            let receiver = start_native_run_event_channel_for_actor_with_dependencies(
                dependencies,
                actor,
                run.clone(),
                include_workflow_events,
            )
            .await
            .map_err(ApplicationNativeRunTargetError)?;
            let (publisher, stream) = interface_runtime::interface_stream_channel(32);
            tokio::spawn(async move {
                let mut receiver = receiver;
                while let Some(event) = receiver.recv().await {
                    if publisher
                        .emit(ApplicationNativeRunStreamEvent(event))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = publisher
                    .finish(interface_runtime::InterfaceStreamTerminal::Completed(
                        ApplicationNativeRunOutput(run),
                    ))
                    .await;
            });
            Ok(stream)
        })
    }
}

pub(crate) fn application_native_run_port(
    dependencies: ApplicationNativeRunDependencies,
) -> Arc<dyn ApplicationNativeRunPort> {
    Arc::new(ApplicationNativeRunAdapter(dependencies))
}

fn native_runtime_service(
    dependencies: &ApplicationNativeRunDependencies,
    runtime_internal_tool_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    >,
) -> OrchestrationRuntimeService<storage_durable_postgres::MainDurableStore, ApiProviderRuntime> {
    OrchestrationRuntimeService::new(
        dependencies.store.clone(),
        ApiProviderRuntime::new_with_activity(
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
    .with_runtime_internal_tool_invoker(runtime_internal_tool_invoker)
    .with_llm_routing_counter_store(dependencies.cache_store.clone())
    .with_provider_request_log_queue(dependencies.task_queue.clone())
    .with_provider_transport_store(dependencies.provider_transport_store.clone())
}

fn native_run_sse_dependencies(
    dependencies: &ApplicationNativeRunDependencies,
) -> sse::NativeRunSseDependencies {
    sse::NativeRunSseDependencies::new(
        dependencies.runtime_event_stream.clone(),
        native_run_terminal_dependencies(dependencies),
    )
}

fn native_run_terminal_dependencies(
    dependencies: &ApplicationNativeRunDependencies,
) -> NativeRunTerminalDependencies {
    NativeRunTerminalDependencies::new(
        dependencies.store.clone(),
        dependencies.runtime_engine.clone(),
        dependencies.provider_runtime.clone(),
        dependencies.provider_secret_master_key.clone(),
        dependencies.runtime_event_stream.clone(),
    )
}

pub(crate) fn application_actor_from_principal(
    principal: &interface_runtime::ApplicationPrincipal,
) -> control_plane::application_public_api::api_keys::ApplicationApiKeyActor {
    control_plane::application_public_api::api_keys::ApplicationApiKeyActor {
        api_key_id: principal.api_key_id(),
        application_id: principal.application_id(),
        creator_user_id: principal.authorized_actor().user_id,
        tenant_id: principal.authorized_actor().tenant_id,
        workspace_id: principal.workspace_id(),
        actor: principal.authorized_actor().clone(),
    }
}

pub(crate) fn compile_native_interface_registry(
    port: Arc<dyn ApplicationNativeRunPort>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    native_interface::compile_registry(port)
}

fn native_interface_error(error: InterfaceInvocationError) -> NativeApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<ApplicationNativeRunTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| {
                NativeApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "interface_target_failed",
                    "native run interface target failed",
                )
            }),
        InterfaceInvocationError::AuthorizationRejected(_)
        | InterfaceInvocationError::AuthorizationContributionRejected(_) => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "native run interface authorization rejected",
        ),
        InterfaceInvocationError::DeadlineElapsed | InterfaceInvocationError::Cancelled => {
            NativeApiError::new(
                StatusCode::REQUEST_TIMEOUT,
                "request_cancelled",
                "native run request was cancelled",
            )
        }
        InterfaceInvocationError::UnknownBinding
        | InterfaceInvocationError::ProtocolBindingMismatch
        | InterfaceInvocationError::AuthenticationAdapterMismatch
        | InterfaceInvocationError::AuthenticationActivationMismatch
        | InterfaceInvocationError::AuthorizationAdapterMismatch
        | InterfaceInvocationError::AdmissionAdapterMismatch
        | InterfaceInvocationError::ContractMismatch
        | InterfaceInvocationError::PrincipalProfileMismatch
        | InterfaceInvocationError::HookPlanFingerprintMismatch
        | InterfaceInvocationError::AdmissionRejected(_)
        | InterfaceInvocationError::AdmissionContributionRejected(_)
        | InterfaceInvocationError::BeforeHookRejected(_) => NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "interface_contract_error",
            "native run interface contract is unavailable",
        ),
    }
}

pub(crate) async fn stage_client_protocol_context(
    store: &dyn ProviderTransportStore,
    run: &NativeRunResult,
    protocol_context: Option<ProtocolContextEnvelope>,
) -> Result<(), NativeApiError> {
    let Some(protocol_context) = protocol_context else {
        return Ok(());
    };
    if matches!(
        run.status,
        NativeRunStatus::Succeeded
            | NativeRunStatus::Incomplete
            | NativeRunStatus::Failed
            | NativeRunStatus::Cancelled
    ) {
        return Ok(());
    }
    let value = ProviderProtocolContextValue::from_envelope(protocol_context).map_err(|_| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "protocol_context_staging_failed",
            "protocol context could not be sealed",
        )
    })?;
    store
        .put_protocol_context(
            ProviderProtocolContextSlotId::for_original_flow_run(run.id),
            value,
        )
        .await
        .map_err(|error| {
            warn!(
                flow_run_id = %run.id,
                error = %error,
                "protocol context ephemeral staging failed"
            );
            NativeApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "protocol_context_staging_failed",
                "protocol context storage is temporarily unavailable",
            )
        })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResumeNativeRunBody {
    pub callback_task_id: Uuid,
    #[serde(default)]
    pub response_payload: Value,
    #[serde(default)]
    pub response_mode: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeRunResponse {
    pub id: Uuid,
    pub application_id: Uuid,
    pub api_key_id: Uuid,
    pub publication_version_id: Uuid,
    pub status: String,
    pub node_input_payload: Value,
    pub metadata: Value,
    pub answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer_segments: Option<Value>,
    pub required_action: Option<Value>,
    pub tool_calls: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_terminal: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelListResponse {
    pub object: &'static str,
    pub data: Vec<NativeModelObject>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelObject {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_compact_token_limit: Option<u64>,
    pub capabilities: NativeModelCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<NativeModelReasoning>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelCapabilities {
    pub reasoning: bool,
    pub tool_call: bool,
    pub multimodal: bool,
    pub structured_output: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeModelReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_effort: Option<String>,
    pub supported_efforts: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NativeErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct NativeApiError {
    pub(crate) status: StatusCode,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) struct NativeRunRequestParseError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    #[cfg(test)]
    pub(crate) report: TranslationReport,
}

impl NativeApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for NativeApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(NativeErrorBody {
                code: self.code.to_string(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Result<String, NativeApiError> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "missing Authorization bearer token",
            )
        })?;
    raw.strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::UNAUTHORIZED,
                "not_authenticated",
                "invalid Authorization bearer token",
            )
        })
}

#[utoipa::path(
    get,
    path = "/api/agent/v1/models",
    operation_id = "list_native_agent_models",
    responses(
        (status = 200, body = NativeModelListResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn list_native_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<NativeModelListResponse>, NativeApiError> {
    let output = invoke_native_read::<NativeModelsInput, NativeModelsOutput>(
        &state,
        bearer_token(&headers)?,
        native_read_interface::MODELS_BINDING_ID,
        NativeModelsInput,
    )
    .await?;
    Ok(Json(output.0))
}

pub(crate) fn native_error(error: NativeRunValidationError) -> NativeApiError {
    match error {
        NativeRunValidationError::NotAuthenticated => NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        ),
        NativeRunValidationError::ApplicationNotPublished => NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "application has no active published public API version",
        ),
        NativeRunValidationError::Forbidden => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "application_run_forbidden",
            "run does not belong to this application API key",
        ),
        NativeRunValidationError::NotFound => NativeApiError::new(
            StatusCode::NOT_FOUND,
            "application_run_not_found",
            "run was not found",
        ),
        NativeRunValidationError::InvalidMapping => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_mapping",
            "application public API mapping is invalid",
        ),
        NativeRunValidationError::InvalidToolResults(message) => {
            NativeApiError::new(StatusCode::BAD_REQUEST, "tool_results", message)
        }
        NativeRunValidationError::InvalidState => NativeApiError::new(
            StatusCode::CONFLICT,
            "invalid_state",
            "run is not in a valid state for this operation",
        ),
        NativeRunValidationError::IdempotencyConflict => NativeApiError::new(
            StatusCode::CONFLICT,
            "idempotency_conflict",
            "idempotency key was already used with a different request",
        ),
    }
}

impl From<AgentModelDescriptor> for NativeModelObject {
    fn from(model: AgentModelDescriptor) -> Self {
        Self {
            id: model.id,
            name: model.name,
            context_window: model.context_window,
            max_context_window: model.max_context_window.or(model.context_window),
            max_output_tokens: model.max_output_tokens,
            auto_compact_token_limit: model.auto_compact_token_limit,
            capabilities: NativeModelCapabilities::from(model.capabilities),
            reasoning: model.reasoning.map(NativeModelReasoning::from),
        }
    }
}

impl From<AgentModelCapabilities> for NativeModelCapabilities {
    fn from(capabilities: AgentModelCapabilities) -> Self {
        Self {
            reasoning: capabilities.reasoning,
            tool_call: capabilities.tool_call,
            multimodal: capabilities.multimodal,
            structured_output: capabilities.structured_output,
        }
    }
}

impl From<AgentModelReasoning> for NativeModelReasoning {
    fn from(reasoning: AgentModelReasoning) -> Self {
        Self {
            default_effort: reasoning.default_effort,
            supported_efforts: reasoning.supported_efforts,
        }
    }
}

pub(crate) fn service_error(error: anyhow::Error) -> NativeApiError {
    if error
        .downcast_ref::<control_plane::errors::ControlPlaneError>()
        .is_some_and(|error| {
            matches!(
                error,
                control_plane::errors::ControlPlaneError::NotAuthenticated
            )
        })
    {
        return NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "invalid application API key",
        );
    }
    if let Some(control_plane::errors::ControlPlaneError::PermissionDenied(reason)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::FORBIDDEN, reason, error.to_string());
    }
    if let Some(control_plane::errors::ControlPlaneError::NotFound(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::NOT_FOUND, name, error.to_string());
    }
    if let Some(control_plane::errors::ControlPlaneError::Conflict(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::CONFLICT, name, error.to_string());
    }
    if let Some(control_plane::errors::ControlPlaneError::InvalidInput(name)) =
        error.downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        return NativeApiError::new(StatusCode::BAD_REQUEST, name, error.to_string());
    }
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return NativeApiError::new(StatusCode::FORBIDDEN, reason, error.to_string());
    }
    let message = error.to_string();
    if is_llm_tool_result_validation_error(&message) {
        return NativeApiError::new(StatusCode::BAD_REQUEST, "tool_results", message);
    }
    NativeApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

fn is_llm_tool_result_validation_error(message: &str) -> bool {
    [
        "llm tool callback response requires tool_results",
        "llm tool callback result is missing tool_call_id",
        "unexpected tool result for ",
        "duplicate tool result for ",
        "missing tool result for ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
}

#[allow(
    clippy::result_large_err,
    reason = "the Native route parser preserves the complete typed compatibility diagnostics"
)]
pub(crate) fn parse_native_run_request(
    bytes: Bytes,
) -> Result<TranslatedNativeRunRequest, NativeRunRequestParseError> {
    let value = serde_json::from_slice::<Value>(&bytes).map_err(|_| {
        let mut report = TranslationReport::new(TranslationProtocol::Native);
        report.record(
            "$.body",
            None,
            TranslationDecisionKind::Rejected,
            Some("invalid JSON body"),
            TranslationSafeRepresentation::Present,
        );
        NativeRunRequestParseError {
            code: "json",
            message: "invalid JSON body".to_string(),
            #[cfg(test)]
            report,
        }
    })?;
    translate_native_run_request(value).map_err(|error| {
        debug!(
            route = "native_runs",
            translation_decision_count = error.report.decisions.len(),
            code = error.code,
            "Native request rejected by protocol adapter"
        );
        NativeRunRequestParseError {
            code: error.code,
            message: error.message,
            #[cfg(test)]
            report: error.report,
        }
    })
}

pub(crate) fn to_native_run_response(run: NativeRunResult) -> NativeRunResponse {
    let exposes_answer = matches!(
        run.status,
        NativeRunStatus::Succeeded | NativeRunStatus::Incomplete
    );
    NativeRunResponse {
        id: run.id,
        application_id: run.application_id,
        api_key_id: run.api_key_id,
        publication_version_id: run.publication_version_id,
        status: serde_json::to_value(run.status)
            .ok()
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_else(|| "unknown".to_string()),
        node_input_payload: run.node_input_payload,
        metadata: run.metadata,
        answer: exposes_answer.then_some(run.answer).flatten(),
        answer_segments: exposes_answer
            .then_some(run.answer_segments)
            .flatten()
            .and_then(|segments| serde_json::to_value(segments).ok()),
        required_action: run
            .required_action
            .and_then(|value| serde_json::to_value(value).ok()),
        tool_calls: run.tool_calls,
        usage: run.usage.and_then(|value| serde_json::to_value(value).ok()),
        error: run.error.and_then(|value| serde_json::to_value(value).ok()),
        operation_terminal: run.operation_terminal.map(|terminal| {
            serde_json::to_value(terminal)
                .expect("typed Native operation terminal must serialize at the protocol boundary")
        }),
        created_at: run.created_at.to_string(),
    }
}

pub(crate) async fn execute_blocking_native_run(
    state: Arc<ApiState>,
    bearer_token: String,
    run: NativeRunResult,
) -> Result<NativeRunResult, NativeApiError> {
    execute_blocking_native_run_with_provider_transport(state, bearer_token, run, None).await
}

pub(crate) async fn execute_blocking_native_run_for_actor(
    state: Arc<ApiState>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
) -> Result<NativeRunResult, NativeApiError> {
    execute_blocking_native_run_for_actor_with_provider_transport(state, actor, run, None).await
}

async fn execute_blocking_native_run_for_actor_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
) -> Result<NativeRunResult, NativeApiError> {
    let _execution_activity = dependencies.runtime_activity.start(
        run.application_id,
        ApplicationActivityKind::ApplicationExecution,
    );
    let runtime_internal_tool_invoker = dependencies
        .runtime_invoker_factory
        .for_actor(&actor)
        .await?;
    let execution_result = scope_application_activity(
        run.application_id,
        native_runtime_service(&dependencies, runtime_internal_tool_invoker)
            .start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: run.application_id,
                flow_run_id: run.id,
                provider_transport_slot,
            }),
    )
    .await;
    match execution_result {
        Ok(detail) => Ok(native_result_from_run_detail(&detail, run.metadata.clone())),
        Err(error) => {
            error!(
                application_id = %run.application_id,
                flow_run_id = %run.id,
                error = %error,
                "blocking native published run reached failed runtime result"
            );
            ApplicationNativeRunService::new(dependencies.store)
                .with_last_used_cache(dependencies.cache_store)
                .get_native_run_for_actor(actor, run.id)
                .await
                .map_err(native_error)
        }
    }
}

pub(crate) async fn execute_blocking_native_run_for_actor_with_provider_transport(
    state: Arc<ApiState>,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
) -> Result<NativeRunResult, NativeApiError> {
    let _execution_activity = state.runtime_activity.start(
        run.application_id,
        ApplicationActivityKind::ApplicationExecution,
    );
    let mcp_runtime_invoker = public_mcp_runtime_invoker_for_actor(&state, &actor).await?;
    let runtime_service = OrchestrationRuntimeService::new(
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
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_provider_transport_store(state.infrastructure.provider_transport_store());
    let execution_result = scope_application_activity(
        run.application_id,
        runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: run.application_id,
            flow_run_id: run.id,
            provider_transport_slot,
        }),
    )
    .await;
    match execution_result {
        Ok(detail) => Ok(native_result_from_run_detail(&detail, run.metadata.clone())),
        Err(error) => {
            error!(
                application_id = %run.application_id,
                flow_run_id = %run.id,
                error = %error,
                "blocking native published run reached failed runtime result"
            );
            ApplicationNativeRunService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .get_native_run_for_actor(actor, run.id)
                .await
                .map_err(native_error)
        }
    }
}

pub(crate) async fn execute_blocking_native_run_with_provider_transport(
    state: Arc<ApiState>,
    bearer_token: String,
    run: NativeRunResult,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
) -> Result<NativeRunResult, NativeApiError> {
    let _execution_activity = state.runtime_activity.start(
        run.application_id,
        ApplicationActivityKind::ApplicationExecution,
    );
    let mcp_runtime_invoker = public_mcp_runtime_invoker(&state, &bearer_token).await?;
    let runtime_service = OrchestrationRuntimeService::new(
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
    .with_provider_request_log_queue(state.infrastructure.task_queue())
    .with_provider_transport_store(state.infrastructure.provider_transport_store());
    let execution_result = scope_application_activity(
        run.application_id,
        runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
            application_id: run.application_id,
            flow_run_id: run.id,
            provider_transport_slot,
        }),
    )
    .await;
    match execution_result {
        Ok(detail) => Ok(native_result_from_run_detail(&detail, run.metadata.clone())),
        Err(error) => {
            error!(
                application_id = %run.application_id,
                flow_run_id = %run.id,
                error = %error,
                "blocking native published run reached failed runtime result"
            );
            ApplicationNativeRunService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .get_native_run(GetNativeRunCommand {
                    bearer_token,
                    run_id: run.id,
                })
                .await
                .map_err(native_error)
        }
    }
}

pub(crate) fn blocking_run_projection_error(run: &NativeRunResult) -> NativeApiError {
    match run.status {
        NativeRunStatus::Failed => {
            let code = match run
                .error
                .as_ref()
                .map(|error| error.code.as_str())
                .unwrap_or("runtime_error")
            {
                "auth_failed" => "auth_failed",
                "endpoint_unreachable" => "endpoint_unreachable",
                "model_not_found" => "model_not_found",
                "provider_affinity_mismatch" => "provider_affinity_mismatch",
                "provider_transport_unavailable" => "provider_transport_unavailable",
                "rate_limited" => "rate_limited",
                "provider_upstream_error" => "provider_upstream_error",
                "provider_invalid_response" => "provider_invalid_response",
                _ => "runtime_error",
            };
            let status = run
                .error
                .as_ref()
                .and_then(|error| error.details.get("status_code"))
                .and_then(Value::as_u64)
                .and_then(|status| u16::try_from(status).ok())
                .and_then(|status| StatusCode::from_u16(status).ok())
                .filter(|status| status.is_client_error() || status.is_server_error())
                .unwrap_or(match code {
                    "rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                    "provider_affinity_mismatch" => StatusCode::CONFLICT,
                    "provider_transport_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
                    "endpoint_unreachable"
                    | "provider_upstream_error"
                    | "provider_invalid_response" => StatusCode::BAD_GATEWAY,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                });
            let message = run
                .error
                .as_ref()
                .map(|error| error.message.as_str())
                .unwrap_or("published run failed");
            NativeApiError::new(status, code, message)
        }
        NativeRunStatus::Cancelled => NativeApiError::new(
            StatusCode::CONFLICT,
            "run_cancelled",
            "published run cancelled",
        ),
        NativeRunStatus::Waiting => NativeApiError::new(
            StatusCode::CONFLICT,
            "required_action_not_supported",
            "waiting states are not supported by compatible endpoints; use the Native API to inspect and resume required_action runs",
        ),
        NativeRunStatus::Created | NativeRunStatus::Queued | NativeRunStatus::Running => {
            NativeApiError::new(
                StatusCode::CONFLICT,
                "run_not_terminal",
                "blocking run did not reach a terminal state",
            )
        }
        NativeRunStatus::Succeeded | NativeRunStatus::Incomplete => NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "projection_error",
            "terminal run cannot be projected as an error",
        ),
    }
}

#[utoipa::path(
    post,
    path = "/api/agent/v1/runs",
    request_body = Value,
    responses(
        (status = 201, body = NativeRunResponse),
        (status = 400, body = NativeErrorBody),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody),
        (status = 422, body = NativeErrorBody)
    )
)]
pub async fn create_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let translated = parse_native_run_request(body)
        .map_err(|error| NativeApiError::new(StatusCode::BAD_REQUEST, error.code, error.message))?;
    debug!(
        route = "native_runs",
        translation_decision_count = translated.report.decisions.len(),
        "Native request translated"
    );
    let request = translated.request;
    let response_mode = request.response_mode.clone();
    let _include_workflow_events = include_workflow_events(&request)?;
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "interface_registry_unavailable",
            "native run interface is unavailable",
        )
    })?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "native run interface is unavailable",
            )
        })?;
    let binding_id = match response_mode.as_deref().unwrap_or("blocking") {
        "streaming" => native_interface::STREAM_BINDING_ID,
        "blocking" => native_interface::BLOCKING_BINDING_ID,
        _ => native_interface::ASYNC_BINDING_ID,
    };
    let binding_id =
        interface_runtime::BindingId::new(binding_id).expect("static binding id is valid");
    let activated_authentication = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "native authentication activation is unavailable",
        )
    })?;
    let principal: interface_runtime::ApplicationPrincipal = boot_snapshot
        .authenticate(
            activated_authentication,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    let application_id = principal.application_id();
    let _http_activity = state
        .runtime_activity
        .start(application_id, ApplicationActivityKind::HttpRequest);
    let authentication_activation = activated_authentication.activation().clone();
    let dispatch_target =
        application_runtime_target(&state, snapshot.as_ref(), &binding_id, application_id);
    let envelope = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        binding_id.clone(),
        InterfaceProtocol::Http,
        interface_runtime::AuthenticationAdapterReference::new("api-server.application-api-key")
            .expect("static adapter is valid"),
        authentication_activation,
        principal,
        None,
        ApplicationNativeRunInput {
            request,
            protocol: TranslationProtocol::Native,
        },
    );
    let kernel = InterfaceInvocationKernel::new(Arc::new(
        native_interface::ApplicationNativeRunAuthorization,
    ));

    if response_mode.as_deref() == Some("streaming") {
        let invocation = kernel
            .invoke_server_stream_with_dispatch_target::<
                ApplicationNativeRunInput,
                ApplicationNativeRunStreamEvent,
                ApplicationNativeRunOutput,
                ApplicationNativeRunTargetError,
            >(
                Arc::clone(&snapshot),
                envelope,
                dispatch_target.clone(),
            )
            .await
            .map_err(|failure| native_interface_error(failure.into_error()))?;
        let (events, completion) = invocation.into_parts();
        tokio::spawn(async move {
            if let Ok(terminal) = completion.complete().await {
                let _receipt = terminal.receipt().clone().projected();
            }
        });
        let sse_activity = state
            .runtime_activity
            .start(application_id, ApplicationActivityKind::SseConnection);
        let stream = tokio_stream::wrappers::ReceiverStream::new(events).map(move |event| {
            let _keep_alive = &sse_activity;
            event.0
        });
        return Ok(Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response());
    }

    if response_mode.as_deref().unwrap_or("blocking") == "blocking" {
        let outcome = kernel
            .invoke_with_dispatch_target::<
                ApplicationNativeRunInput,
                ApplicationNativeRunOutput,
                ApplicationNativeRunTargetError,
            >(
                Arc::clone(&snapshot),
                envelope,
                dispatch_target,
            )
            .await
            .map_err(|failure| native_interface_error(failure.into_error()))?;
        let _receipt = outcome.receipt().clone().projected();
        let run = outcome.into_value().0;
        return Ok((
            StatusCode::CREATED,
            Json(ApiSuccess::new(to_native_run_response(run))),
        )
            .into_response());
    }

    let outcome = kernel
        .invoke::<ApplicationNativeRunInput, ApplicationNativeRunOutput, ApplicationNativeRunTargetError>(
            snapshot,
            envelope,
        )
        .await
        .map_err(|failure| native_interface_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    let run = outcome.into_value().0;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_native_run_response(run))),
    )
        .into_response())
}

pub(crate) fn application_runtime_target(
    state: &ApiState,
    registry: &interface_runtime::CompiledInterfaceRegistry,
    binding_id: &interface_runtime::BindingId,
    application_id: Uuid,
) -> interface_runtime::ExecutionTargetPin {
    let plan = registry
        .plan(binding_id)
        .expect("published Native binding must own a plan");
    interface_runtime::ExecutionTargetPin::Runtime {
        handler: plan.definition().handler_reference().clone(),
        target: plan.definition().target_reference().clone(),
        plugin: interface_runtime::PluginIdentity::new("builtin.orchestration-runtime")
            .expect("runtime plugin identity is valid"),
        artifact: interface_runtime::ArtifactIdentity::new(format!(
            "api-server:{}",
            env!("CARGO_PKG_VERSION")
        ))
        .expect("runtime artifact identity is valid"),
        runtime: interface_runtime::RuntimeTargetIdentity::new(format!(
            "orchestration-runtime.local.application:{application_id}"
        ))
        .expect("runtime target identity is valid"),
        runtime_generation: interface_runtime::RuntimeGeneration::new(
            state
                .extension_boot_snapshot
                .as_ref()
                .map(|snapshot| snapshot.fingerprint())
                .unwrap_or("graph:unavailable"),
        )
        .expect("runtime generation identity is valid"),
        worker_generation: interface_runtime::WorkerGeneration::new(format!(
            "api-process:{}",
            state.process_started_at.unix_timestamp_nanos()
        ))
        .expect("worker generation identity is valid"),
    }
}

pub(crate) fn include_workflow_events(
    request: &NativeRunRequest,
) -> Result<sse::IncludeWorkflowEvents, NativeApiError> {
    include_workflow_event_visibility(request.stream_options.include_workflow_events)
}

pub(crate) fn include_workflow_event_visibility(
    visibility: control_plane::application_public_api::native::NativeWorkflowEventVisibility,
) -> Result<sse::IncludeWorkflowEvents, NativeApiError> {
    match visibility {
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::None => {
            Ok(sse::IncludeWorkflowEvents::None)
        }
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public => {
            Ok(sse::IncludeWorkflowEvents::Public)
        }
        control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug => {
            Err(NativeApiError::new(
                StatusCode::FORBIDDEN,
                "workflow_event_visibility_forbidden",
                "debug workflow events require a browser session principal",
            ))
        }
    }
}

async fn start_native_run_event_channel_for_actor_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
    include_workflow_events: sse::IncludeWorkflowEvents,
) -> Result<
    mpsc::Receiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    NativeApiError,
> {
    let runtime_internal_tool_invoker = dependencies
        .runtime_invoker_factory
        .for_actor(&actor)
        .await?;
    start_native_run_event_channel_with_dependencies(
        dependencies,
        runtime_internal_tool_invoker,
        run,
        include_workflow_events,
    )
    .await
}

async fn start_native_run_event_channel_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    runtime_internal_tool_invoker: Arc<
        dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
    >,
    run: NativeRunResult,
    include_workflow_events: sse::IncludeWorkflowEvents,
) -> Result<
    mpsc::Receiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    NativeApiError,
> {
    dependencies
        .runtime_event_stream
        .open_run(
            run.id,
            control_plane::ports::RuntimeEventStreamPolicy::debug_default(),
        )
        .await
        .map_err(service_error)?;

    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(sse::send_native_runtime_event_stream_with_dependencies(
        native_run_sse_dependencies(&dependencies),
        run.clone(),
        include_workflow_events,
        None,
        None,
        sender,
    ));

    let background_dependencies = dependencies.clone();
    let background_run = run.clone();
    tokio::spawn(async move {
        let _execution_activity = background_dependencies.runtime_activity.start(
            background_run.application_id,
            ApplicationActivityKind::ApplicationExecution,
        );
        let runtime_service =
            native_runtime_service(&background_dependencies, runtime_internal_tool_invoker)
                .with_runtime_event_stream(background_dependencies.runtime_event_stream.clone());
        if let Err(runtime_error) = scope_application_activity(
            background_run.application_id,
            runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: background_run.application_id,
                flow_run_id: background_run.id,
                provider_transport_slot: None,
            }),
        )
        .await
        {
            if let Err(recovery_error) = recover_missing_stream_terminal_winner_with_dependencies(
                &native_run_terminal_dependencies(&background_dependencies),
                &background_run,
            )
            .await
            {
                error!(
                    application_id = %background_run.application_id,
                    flow_run_id = %background_run.id,
                    error = %recovery_error,
                    "failed to recover the durable winner after native streaming execution ended"
                );
            }
            error!(
                application_id = %background_run.application_id,
                flow_run_id = %background_run.id,
                error = %runtime_error,
                "failed to execute native streaming published run"
            );
        }
    });

    Ok(receiver)
}

#[utoipa::path(
    get,
    path = "/api/agent/v1/runs/{run_id}",
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody)
    )
)]
pub async fn get_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<NativeRunResponse>>, NativeApiError> {
    let output = invoke_native_read::<NativeGetRunInput, NativeGetRunOutput>(
        &state,
        bearer_token(&headers)?,
        native_read_interface::GET_RUN_BINDING_ID,
        NativeGetRunInput(run_id),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

async fn invoke_native_read<I, O>(
    state: &Arc<ApiState>,
    bearer_token: String,
    binding_id: &str,
    input: I,
) -> Result<O, NativeApiError>
where
    I: interface_runtime::InterfaceContract,
    O: interface_runtime::InterfaceContract,
{
    let (snapshot, binding_id, authentication_activation, principal) =
        authenticate_native_binding(state, bearer_token, binding_id).await?;
    invoke_native_with_principal(
        snapshot,
        binding_id,
        authentication_activation,
        principal,
        input,
    )
    .await
}

async fn authenticate_native_binding(
    state: &Arc<ApiState>,
    bearer_token: String,
    binding_id: &str,
) -> Result<
    (
        Arc<interface_runtime::CompiledInterfaceRegistry>,
        interface_runtime::BindingId,
        interface_runtime::AuthenticationActivationIdentity,
        interface_runtime::ApplicationPrincipal,
    ),
    NativeApiError,
> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "interface_registry_unavailable",
            "native read interface is unavailable",
        )
    })?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "native read interface is unavailable",
            )
        })?;
    let binding_id = interface_runtime::BindingId::new(binding_id)
        .expect("static native read binding id is valid");
    let activated = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "native read authentication is unavailable",
        )
    })?;
    let principal: interface_runtime::ApplicationPrincipal = boot_snapshot
        .authenticate(
            activated,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    let authentication_activation = activated.activation().clone();
    Ok((snapshot, binding_id, authentication_activation, principal))
}

async fn invoke_native_with_principal<I, O>(
    snapshot: Arc<interface_runtime::CompiledInterfaceRegistry>,
    binding_id: interface_runtime::BindingId,
    authentication_activation: interface_runtime::AuthenticationActivationIdentity,
    principal: interface_runtime::ApplicationPrincipal,
    input: I,
) -> Result<O, NativeApiError>
where
    I: interface_runtime::InterfaceContract,
    O: interface_runtime::InterfaceContract,
{
    let outcome =
        InterfaceInvocationKernel::new(Arc::new(native_read_interface::NativeReadAuthorization))
            .invoke::<I, O, NativeReadTargetError>(
                snapshot,
                InvocationEnvelope::with_principal(
                    InvocationLineage::root(InvocationId::now_v7()),
                    binding_id,
                    InterfaceProtocol::Http,
                    interface_runtime::AuthenticationAdapterReference::new(
                        "api-server.application-api-key",
                    )
                    .unwrap(),
                    authentication_activation,
                    principal,
                    None,
                    input,
                ),
            )
            .await
            .map_err(|failure| match failure.into_error() {
                InterfaceInvocationError::TargetFailed(error) => error
                    .into_source::<NativeReadTargetError>()
                    .map(|error| error.0)
                    .unwrap_or_else(|| {
                        NativeApiError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "native_read_failed",
                            "native read interface target failed",
                        )
                    }),
                error => native_interface_error(error),
            })?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value())
}

#[utoipa::path(
    post,
    path = "/api/agent/v1/runs/{run_id}/cancel",
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn cancel_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<NativeRunResponse>>, NativeApiError> {
    let output = invoke_native_read::<NativeCancelRunInput, NativeCancelRunOutput>(
        &state,
        bearer_token(&headers)?,
        native_read_interface::CANCEL_RUN_BINDING_ID,
        NativeCancelRunInput(run_id),
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)))
}

#[utoipa::path(
    post,
    path = "/api/agent/v1/runs/{run_id}/resume",
    request_body = ResumeNativeRunBody,
    params(("run_id" = String, Path, description = "Published run id")),
    responses(
        (status = 200, body = NativeRunResponse),
        (status = 401, body = NativeErrorBody),
        (status = 403, body = NativeErrorBody),
        (status = 404, body = NativeErrorBody),
        (status = 409, body = NativeErrorBody)
    )
)]
pub async fn resume_native_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    Json(body): Json<ResumeNativeRunBody>,
) -> Result<Response, NativeApiError> {
    let output = invoke_native_read::<NativeResumeRunInput, NativeResumeRunOutput>(
        &state,
        bearer_token(&headers)?,
        native_read_interface::RESUME_RUN_BINDING_ID,
        NativeResumeRunInput {
            run_id,
            callback_task_id: body.callback_task_id,
            response_payload: body.response_payload,
            response_mode: body.response_mode,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.0)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/agent/v1/files",
    responses(
        (status = 201, body = crate::routes::files::UploadedFileResponse),
        (status = 401, body = NativeErrorBody)
    )
)]
pub async fn upload_native_file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<UploadedFileResponse>>), NativeApiError> {
    let bearer_token = bearer_token(&headers)?;
    let (snapshot, binding_id, authentication_activation, principal) = authenticate_native_binding(
        &state,
        bearer_token,
        native_read_interface::UPLOAD_FILE_BINDING_ID,
    )
    .await?;

    let mut file_table_id = None;
    let mut filename = None;
    let mut content_type = None;
    let mut bytes = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart",
            format!("invalid multipart payload: {error}"),
        )
    })? {
        match field.name() {
            Some("file_table_id") => {
                file_table_id = Some(field.text().await.map_err(|error| {
                    NativeApiError::new(
                        StatusCode::BAD_REQUEST,
                        "file_table_id",
                        format!("invalid file_table_id field: {error}"),
                    )
                })?)
            }
            Some("file") => {
                filename = field.file_name().map(str::to_string);
                content_type = field.content_type().map(str::to_string);
                bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|error| {
                            NativeApiError::new(
                                StatusCode::BAD_REQUEST,
                                "file",
                                format!("invalid file field: {error}"),
                            )
                        })?
                        .to_vec(),
                );
            }
            _ => {}
        }
    }

    let file_table_id = file_table_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::BAD_REQUEST,
                "file_table_id",
                "file_table_id is required",
            )
        })?;
    let bytes = bytes.ok_or_else(|| {
        NativeApiError::new(StatusCode::BAD_REQUEST, "file", "file field is required")
    })?;
    let uploaded = invoke_native_with_principal::<NativeUploadFileInput, NativeUploadFileOutput>(
        snapshot,
        binding_id,
        authentication_activation,
        principal,
        NativeUploadFileInput {
            file_table_id,
            original_filename: filename.unwrap_or_else(|| "upload.bin".into()),
            content_type,
            bytes,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(uploaded.0))))
}
