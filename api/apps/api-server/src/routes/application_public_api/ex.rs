use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use control_plane::{
    application_public_api::{
        mapping::{WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode},
        workflow_extension::{
            CreateWorkflowExtensionRunCommand, WorkflowExtensionRequestParameters,
            WorkflowExtensionRunError, WorkflowExtensionRunService, WorkflowHttpPrincipal,
        },
    },
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    extension_bus::ConsoleAuthenticationCredential,
    routes::application_public_api::native::{api_provider_runtime, service_error, NativeApiError},
    routes::application_public_api::workflow_extension_interface::{
        self, WorkflowExtensionFuture, WorkflowExtensionInput, WorkflowExtensionOutput,
        WorkflowExtensionPort, WorkflowExtensionTargetError,
    },
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};

#[derive(Debug, Serialize)]
struct WorkflowExtensionAcceptedResponse {
    run_id: String,
    status: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new().route("/*slug", axum::routing::any(invoke_workflow_extension))
}

pub async fn invoke_workflow_extension(
    State(state): State<Arc<ApiState>>,
    method: Method,
    headers: HeaderMap,
    Path(slug): Path<String>,
    uri: axum::http::Uri,
    body: Bytes,
) -> Result<Response, NativeApiError> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "interface_registry_unavailable",
            "workflow extension interface is unavailable",
        )
    })?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "workflow extension interface is unavailable",
            )
        })?;
    let binding_id = interface_runtime::BindingId::new(workflow_extension_interface::BINDING_ID)
        .expect("static binding id is valid");
    let activated = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "workflow extension authentication is unavailable",
        )
    })?;
    let principal: interface_runtime::UserPrincipal = boot_snapshot
        .authenticate(
            activated,
            ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state: state.clone(),
                headers: headers.clone(),
            },
        )
        .await
        .map_err(|error| workflow_extension_auth_error(ApiError(error)))?;
    let authentication_activation = activated.activation().clone();
    let method = workflow_extension_method(&method)?;
    let parameters = request_parameters(uri.query(), &headers, &body)?;
    let outcome = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        workflow_extension_interface::WorkflowExtensionAuthorization,
    ))
    .invoke::<WorkflowExtensionInput, WorkflowExtensionOutput, WorkflowExtensionTargetError>(
        snapshot,
        interface_runtime::InvocationEnvelope::with_principal(
            interface_runtime::InvocationLineage::root(interface_runtime::InvocationId::now_v7()),
            binding_id,
            interface_runtime::InterfaceProtocol::Http,
            interface_runtime::AuthenticationAdapterReference::new(
                "api-server.console.require-session",
            )
            .expect("static adapter is valid"),
            authentication_activation,
            principal,
            None,
            WorkflowExtensionInput {
                request_path: slug,
                method,
                parameters,
            },
        ),
    )
    .await
    .map_err(|failure| workflow_extension_invocation_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    project_workflow_extension_output(outcome.into_value())
}

struct WorkflowExtensionAdapter {
    state: std::sync::Weak<ApiState>,
}

impl WorkflowExtensionPort for WorkflowExtensionAdapter {
    fn invoke<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        principal: WorkflowHttpPrincipal,
        input: WorkflowExtensionInput,
    ) -> WorkflowExtensionFuture<'a> {
        let state = self.state.clone();
        let actor = actor.clone();
        Box::pin(async move {
            let state = state.upgrade().ok_or_else(|| {
                WorkflowExtensionTargetError(NativeApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "api_state_unavailable",
                    "API state is unavailable",
                ))
            })?;
            let run = WorkflowExtensionRunService::new(state.store.clone())
                .create_run(CreateWorkflowExtensionRunCommand {
                    actor,
                    principal,
                    request_path: input.request_path,
                    method: input.method,
                    parameters: input.parameters,
                })
                .await
                .map_err(workflow_extension_error)
                .map_err(WorkflowExtensionTargetError)?;
            let _http_activity = state
                .runtime_activity
                .start(run.application_id, ApplicationActivityKind::HttpRequest);
            let execution =
                spawn_workflow_extension_execution(state.clone(), run.application_id, run.id);

            if run.response_mode == WorkflowExtensionResponseMode::Async {
                return Ok(WorkflowExtensionOutput::Accepted {
                    run_id: run.id,
                    status: run.status,
                });
            }

            match tokio::time::timeout(
                std::time::Duration::from_millis(run.sync_timeout_ms),
                execution,
            )
            .await
            {
                Ok(joined) => {
                    let detail = joined
                        .map_err(|error| {
                            WorkflowExtensionTargetError(service_error(anyhow::anyhow!(error)))
                        })?
                        .map_err(|error| WorkflowExtensionTargetError(service_error(error)))?;
                    Ok(WorkflowExtensionOutput::Completed(detail))
                }
                Err(_) => Ok(WorkflowExtensionOutput::Accepted {
                    run_id: run.id,
                    status: domain::FlowRunStatus::Running,
                }),
            }
        })
    }
}

pub(crate) fn compile_workflow_extension_registry(
    state: std::sync::Weak<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    workflow_extension_interface::compile_registry(Arc::new(WorkflowExtensionAdapter { state }))
}

fn spawn_workflow_extension_execution(
    state: Arc<ApiState>,
    application_id: uuid::Uuid,
    flow_run_id: uuid::Uuid,
) -> tokio::task::JoinHandle<anyhow::Result<domain::ApplicationRunDetail>> {
    tokio::spawn(async move {
        let _execution_activity = state.runtime_activity.start(
            application_id,
            ApplicationActivityKind::ApplicationExecution,
        );
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
        .with_llm_routing_counter_store(state.infrastructure.cache_store())
        .with_provider_request_log_queue(state.infrastructure.task_queue())
        .with_runtime_event_stream(state.runtime_event_stream.clone());
        scope_application_activity(
            application_id,
            runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                application_id,
                flow_run_id,
                provider_transport_slot: None,
            }),
        )
        .await
    })
}

fn workflow_extension_method(
    method: &Method,
) -> Result<WorkflowExtensionHttpMethod, NativeApiError> {
    if method == Method::GET {
        Ok(WorkflowExtensionHttpMethod::Get)
    } else if method == Method::POST {
        Ok(WorkflowExtensionHttpMethod::Post)
    } else if method == Method::PUT {
        Ok(WorkflowExtensionHttpMethod::Put)
    } else if method == Method::PATCH {
        Ok(WorkflowExtensionHttpMethod::Patch)
    } else if method == Method::DELETE {
        Ok(WorkflowExtensionHttpMethod::Delete)
    } else if method == Method::HEAD {
        Ok(WorkflowExtensionHttpMethod::Head)
    } else if method == Method::OPTIONS {
        Ok(WorkflowExtensionHttpMethod::Options)
    } else {
        Err(NativeApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "HTTP method is not supported for workflow extension APIs",
        ))
    }
}

fn request_parameters(
    raw_query: Option<&str>,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<WorkflowExtensionRequestParameters, NativeApiError> {
    let path = BTreeMap::new();
    let query = parse_urlencoded_object(raw_query.unwrap_or_default());
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let (form, body) = if content_type.split(';').next().is_some_and(|value| {
        value
            .trim()
            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    }) {
        (
            parse_urlencoded_object(std::str::from_utf8(body).map_err(|_| {
                NativeApiError::new(StatusCode::BAD_REQUEST, "form", "invalid form body")
            })?),
            json!({}),
        )
    } else if body.is_empty() {
        (Map::new(), json!({}))
    } else {
        let body = serde_json::from_slice(body).map_err(|_| {
            NativeApiError::new(StatusCode::BAD_REQUEST, "json", "invalid JSON body")
        })?;
        (Map::new(), body)
    };

    Ok(WorkflowExtensionRequestParameters {
        path,
        query,
        form,
        body,
    })
}

fn parse_urlencoded_object(raw: &str) -> Map<String, Value> {
    form_urlencoded::parse(raw.as_bytes())
        .map(|(key, value)| (key.into_owned(), Value::String(value.into_owned())))
        .collect()
}

fn workflow_extension_error(error: WorkflowExtensionRunError) -> NativeApiError {
    match error {
        WorkflowExtensionRunError::ExtensionNotFound => NativeApiError::new(
            StatusCode::NOT_FOUND,
            "workflow_extension_not_found",
            "workflow extension API was not found",
        ),
        WorkflowExtensionRunError::ApplicationNotPublished => NativeApiError::new(
            StatusCode::CONFLICT,
            "application_not_published",
            "workflow extension API is not enabled",
        ),
        WorkflowExtensionRunError::Forbidden => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "workflow_extension_forbidden",
            "the current user cannot invoke the workflow extension API",
        ),
        WorkflowExtensionRunError::MethodNotAllowed => NativeApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "HTTP method does not match the workflow extension API configuration",
        ),
        WorkflowExtensionRunError::TriggerTypeMismatch => NativeApiError::new(
            StatusCode::CONFLICT,
            "workflow_trigger_type_mismatch",
            "workflow trigger type does not allow extension API invocation",
        ),
        WorkflowExtensionRunError::InvalidMapping => NativeApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_mapping",
            "workflow extension API mapping is invalid",
        ),
        WorkflowExtensionRunError::RouteConflict => NativeApiError::new(
            StatusCode::CONFLICT,
            "workflow_route_conflict",
            "workflow extension route configuration is ambiguous",
        ),
        WorkflowExtensionRunError::Internal(error) => {
            tracing::error!(error = ?error, "workflow extension service failed");
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "workflow extension service failed",
            )
        }
    }
}

fn workflow_extension_auth_error(error: ApiError) -> NativeApiError {
    match error
        .0
        .downcast_ref::<control_plane::errors::ControlPlaneError>()
    {
        Some(control_plane::errors::ControlPlaneError::NotAuthenticated) => NativeApiError::new(
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
            "a current login session or user API key is required",
        ),
        Some(control_plane::errors::ControlPlaneError::PermissionDenied(_)) => NativeApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "current login session validation failed",
        ),
        _ => {
            tracing::error!(error = ?error.0, "workflow extension authentication failed");
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "workflow extension authentication failed",
            )
        }
    }
}

fn workflow_extension_invocation_error(
    error: interface_runtime::InterfaceInvocationError,
) -> NativeApiError {
    match error {
        interface_runtime::InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<WorkflowExtensionTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| {
                NativeApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "interface_target_failed",
                    "workflow extension interface target failed",
                )
            }),
        interface_runtime::InterfaceInvocationError::AuthorizationRejected(_)
        | interface_runtime::InterfaceInvocationError::AuthorizationContributionRejected(_) => {
            NativeApiError::new(
                StatusCode::FORBIDDEN,
                "workflow_extension_forbidden",
                "the current user cannot invoke the workflow extension API",
            )
        }
        interface_runtime::InterfaceInvocationError::DeadlineElapsed
        | interface_runtime::InterfaceInvocationError::Cancelled => NativeApiError::new(
            StatusCode::REQUEST_TIMEOUT,
            "request_cancelled",
            "workflow extension request was cancelled",
        ),
        interface_runtime::InterfaceInvocationError::UnknownBinding
        | interface_runtime::InterfaceInvocationError::ProtocolBindingMismatch
        | interface_runtime::InterfaceInvocationError::AuthenticationAdapterMismatch
        | interface_runtime::InterfaceInvocationError::AuthenticationActivationMismatch
        | interface_runtime::InterfaceInvocationError::AuthorizationAdapterMismatch
        | interface_runtime::InterfaceInvocationError::AdmissionAdapterMismatch
        | interface_runtime::InterfaceInvocationError::ContractMismatch
        | interface_runtime::InterfaceInvocationError::PrincipalProfileMismatch
        | interface_runtime::InterfaceInvocationError::HookPlanFingerprintMismatch
        | interface_runtime::InterfaceInvocationError::AdmissionRejected(_)
        | interface_runtime::InterfaceInvocationError::AdmissionContributionRejected(_)
        | interface_runtime::InterfaceInvocationError::BeforeHookRejected(_) => {
            NativeApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "interface_contract_error",
                "workflow extension interface contract is unavailable",
            )
        }
    }
}

fn project_workflow_extension_output(
    output: WorkflowExtensionOutput,
) -> Result<Response, NativeApiError> {
    match output {
        WorkflowExtensionOutput::Accepted { run_id, status } => {
            Ok(accepted_response(run_id, status))
        }
        WorkflowExtensionOutput::Completed(detail) => match detail.flow_run.status {
            domain::FlowRunStatus::Succeeded => {
                Ok((StatusCode::OK, Json(detail.flow_run.output_payload)).into_response())
            }
            domain::FlowRunStatus::Incomplete => Err(NativeApiError::new(
                StatusCode::CONFLICT,
                "workflow_incomplete",
                "workflow run reached its output limit",
            )),
            domain::FlowRunStatus::Queued
            | domain::FlowRunStatus::Running
            | domain::FlowRunStatus::WaitingCallback
            | domain::FlowRunStatus::WaitingHuman
            | domain::FlowRunStatus::Paused => Ok(accepted_response(
                detail.flow_run.id,
                detail.flow_run.status,
            )),
            domain::FlowRunStatus::Failed => Err(NativeApiError::new(
                StatusCode::CONFLICT,
                "workflow_failed",
                detail
                    .flow_run
                    .error_payload
                    .as_ref()
                    .and_then(|value| value.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("workflow run failed"),
            )),
            domain::FlowRunStatus::Cancelled => Err(NativeApiError::new(
                StatusCode::CONFLICT,
                "workflow_cancelled",
                "workflow run was cancelled",
            )),
        },
    }
}

fn accepted_response(run_id: uuid::Uuid, status: domain::FlowRunStatus) -> Response {
    (
        StatusCode::ACCEPTED,
        Json(WorkflowExtensionAcceptedResponse {
            run_id: run_id.to_string(),
            status: status.as_str().to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod error_mapping_tests {
    use super::*;

    #[test]
    fn authorization_errors_use_credential_neutral_messages() {
        let forbidden = workflow_extension_error(WorkflowExtensionRunError::Forbidden);
        assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
        assert_eq!(
            forbidden.message,
            "the current user cannot invoke the workflow extension API"
        );
    }

    #[test]
    fn internal_errors_are_stable_and_do_not_leak_repository_details() {
        let response = workflow_extension_error(WorkflowExtensionRunError::Internal(
            anyhow::anyhow!("database password was rejected"),
        ));

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.code, "internal_error");
        assert_eq!(response.message, "workflow extension service failed");
    }
}
