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
        model_catalog::{AgentModelCapabilities, AgentModelDescriptor, AgentModelReasoning},
        native::{
            translate_native_run_request, ApplicationNativeRunService, NativeRunRequest,
            NativeRunResult, NativeRunStatus, NativeRunValidationError,
        },
        protocol_translation::{
            TranslatedNativeRunRequest, TranslationDecisionKind, TranslationProtocol,
            TranslationReport, TranslationSafeRepresentation,
        },
        run_service::native_result_from_run_detail,
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    ports::{
        OrchestrationRuntimeRepository, ProviderProtocolContextSlotId,
        ProviderProtocolContextValue, ProviderTransportStore, RuntimeEventStream,
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

use interface_runtime::{InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol};

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

mod response_projection;

pub(crate) use response_projection::{blocking_run_projection_error, native_error, service_error};

#[cfg(test)]
#[path = "native/_tests/catalog_admission.rs"]
mod catalog_admission_tests;

pub(crate) fn api_provider_runtime(state: &ApiState) -> ApiProviderRuntime {
    ApiProviderRuntime::new_with_activity(
        state.provider_runtime.clone(),
        state.runtime_activity.clone(),
    )
    .with_network_egress(Arc::new(state.network_egress_http_clients()))
}

// None means the compiled representation could not be inspected safely. In that
// case the existing catalog path remains authoritative.

#[derive(Clone)]
pub(crate) struct ApplicationNativeRunDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) cache_store: Arc<dyn crate::host_infrastructure::CacheStore>,
    pub(crate) published_plan_cache: Arc<dyn control_plane::ports::PublishedPlanCache>,
    pub(crate) published_publication_cache:
        Arc<dyn control_plane::ports::PublishedPublicationCache>,
    pub(crate) runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) network_egress: Arc<crate::network_egress_client::NetworkEgressHttpClientResolver>,
    pub(crate) provider_secret_master_key: String,
    pub(crate) model_billing_require_provider_usage: bool,
    pub(crate) api_node_id: String,
    pub(crate) provider_install_root: String,
    pub(crate) file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub(crate) notification_sender: Arc<dyn control_plane::notifications::NotificationSenderPort>,
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
                .with_published_plan_cache(dependencies.published_plan_cache.clone())
                .with_published_publication_cache(dependencies.published_publication_cache.clone())
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
                .with_published_plan_cache(dependencies.published_plan_cache.clone())
                .with_published_publication_cache(dependencies.published_publication_cache.clone())
                .create_native_run_for_actor(actor.clone(), input.request, input.protocol)
                .await
                .map_err(native_error)
                .map_err(ApplicationNativeRunTargetError)?;
            execute_blocking_native_run_for_actor_with_dependencies(
                dependencies,
                actor,
                run,
                None,
                None,
                None,
            )
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
                .with_published_plan_cache(dependencies.published_plan_cache.clone())
                .with_published_publication_cache(dependencies.published_publication_cache.clone())
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

pub(crate) fn native_runtime_service(
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
        )
        .with_network_egress(dependencies.network_egress.clone()),
        dependencies.runtime_engine.clone(),
        dependencies.provider_secret_master_key.clone(),
        dependencies.provider_transport_store.clone(),
        dependencies.model_billing_require_provider_usage,
    )
    .with_published_plan_cache(dependencies.published_plan_cache.clone())
    .with_node_artifact_context(
        dependencies.api_node_id.clone(),
        dependencies.provider_install_root.clone(),
    )
    .with_file_storage_registry(dependencies.file_storage_registry.clone())
    .with_notification_sender(dependencies.notification_sender.clone())
    .with_runtime_internal_tool_invoker(runtime_internal_tool_invoker)
    .with_llm_routing_counter_store(dependencies.cache_store.clone())
    .with_provider_request_log_queue(dependencies.task_queue.clone())
}

fn native_run_sse_dependencies(
    dependencies: &ApplicationNativeRunDependencies,
) -> sse::NativeRunSseDependencies {
    sse::NativeRunSseDependencies::new(
        dependencies.runtime_event_stream.clone(),
        native_run_terminal_dependencies(dependencies),
    )
}

pub(crate) fn native_run_terminal_dependencies(
    dependencies: &ApplicationNativeRunDependencies,
) -> NativeRunTerminalDependencies {
    NativeRunTerminalDependencies::new(
        dependencies.store.clone(),
        dependencies.runtime_engine.clone(),
        dependencies.provider_runtime.clone(),
        dependencies.provider_secret_master_key.clone(),
        dependencies.model_billing_require_provider_usage,
        dependencies.provider_transport_store.clone(),
        dependencies.runtime_event_stream.clone(),
    )
    .with_published_plan_cache(dependencies.published_plan_cache.clone())
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
    pub(crate) runtime_error:
        Option<Box<control_plane::application_public_api::native::NativeError>>,
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
            runtime_error: None,
        }
    }
}

impl IntoResponse for NativeApiError {
    fn into_response(self) -> Response {
        if let Some(error) = self.runtime_error {
            return (self.status, Json(error)).into_response();
        }
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

pub(crate) async fn execute_blocking_native_run_for_actor_with_dependencies(
    dependencies: ApplicationNativeRunDependencies,
    actor: control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    run: NativeRunResult,
    provider_transport_slot: Option<control_plane::ports::ProviderTransportSlotId>,
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
) -> Result<NativeRunResult, NativeApiError> {
    let _execution_activity = dependencies.runtime_activity.start(
        run.application_id,
        ApplicationActivityKind::ApplicationExecution,
    );
    let runtime_internal_tool_invoker =
        runtime_internal_tool_invoker_for_run(&dependencies, &actor, &run).await?;
    let execution_result = scope_application_activity(
        run.application_id,
        native_runtime_service(&dependencies, runtime_internal_tool_invoker)
            .start_published_flow_run(StartPublishedFlowRunCommand {
                application_id: run.application_id,
                flow_run_id: run.id,
                provider_transport_slot,
                transport_connection_scope,
                observation_context,
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
    let _activated_authentication = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "native authentication activation is unavailable",
        )
    })?;
    let authenticated = boot_snapshot
        .authenticate_invocation::<_, interface_runtime::ApplicationPrincipal>(
            Arc::clone(&snapshot),
            &binding_id,
            InterfaceProtocol::Http,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    let application_id = authenticated.principal().application_id();
    let _http_activity = state
        .runtime_activity
        .start(application_id, ApplicationActivityKind::HttpRequest);
    let dispatch_target =
        application_runtime_target(&state, snapshot.as_ref(), &binding_id, application_id);
    let envelope = authenticated.into_envelope(ApplicationNativeRunInput {
        request,
        protocol: TranslationProtocol::Native,
    });
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
    let (snapshot, authenticated) =
        authenticate_native_binding(state, bearer_token, binding_id).await?;
    invoke_native_with_principal(snapshot, authenticated, input).await
}

async fn authenticate_native_binding(
    state: &Arc<ApiState>,
    bearer_token: String,
    binding_id: &str,
) -> Result<
    (
        Arc<interface_runtime::CompiledInterfaceRegistry>,
        crate::extension_bus::AuthenticatedInvocation<interface_runtime::ApplicationPrincipal>,
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
    let _activated = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "native read authentication is unavailable",
        )
    })?;
    let authenticated = boot_snapshot
        .authenticate_invocation::<_, interface_runtime::ApplicationPrincipal>(
            Arc::clone(&snapshot),
            &binding_id,
            InterfaceProtocol::Http,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| native_error(NativeRunValidationError::NotAuthenticated))?;
    Ok((snapshot, authenticated))
}

async fn invoke_native_with_principal<I, O>(
    snapshot: Arc<interface_runtime::CompiledInterfaceRegistry>,
    authenticated: crate::extension_bus::AuthenticatedInvocation<
        interface_runtime::ApplicationPrincipal,
    >,
    input: I,
) -> Result<O, NativeApiError>
where
    I: interface_runtime::InterfaceContract,
    O: interface_runtime::InterfaceContract,
{
    let outcome =
        InterfaceInvocationKernel::new(Arc::new(native_read_interface::NativeReadAuthorization))
            .invoke::<I, O, NativeReadTargetError>(snapshot, authenticated.into_envelope(input))
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
    let (snapshot, authenticated) = authenticate_native_binding(
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
        authenticated,
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

#[path = "native/runtime_invoker.rs"]
mod runtime_invoker;

#[path = "native/native_streams.rs"]
mod native_streams;

#[cfg(test)]
use runtime_invoker::compiled_plan_has_mcp_bindings;
pub(crate) use runtime_invoker::native_runtime_invoker_factory;
pub(crate) use runtime_invoker::public_mcp_runtime_invoker_for_actor;
pub(crate) use runtime_invoker::runtime_internal_tool_invoker_for_run;
pub(crate) use runtime_invoker::NativeRuntimeInvokerFactoryDependencies;

pub(crate) use native_streams::application_runtime_target;
pub(crate) use native_streams::include_workflow_event_visibility;
pub(crate) use native_streams::include_workflow_events;
use native_streams::*;
