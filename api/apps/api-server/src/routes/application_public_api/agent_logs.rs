//! Application-key admitted observational ingestion, independent of generation.
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{DefaultBodyLimit, State},
    handler::Handler as AxumHandler,
    http::HeaderMap,
    Json,
};
use control_plane::ports::{
    AgentLogsBatch, AgentLogsReceipt, BillingRepository, OrchestrationRuntimeRepository,
};
use interface_runtime::*;
use std::sync::Arc;

pub(crate) const BINDING: &str = "http.application.logs.events.ingest.v1";
const ID: &str = "application.logs.events.ingest";
const OWNER: &str = "api-server.application-agent-logs";
struct Input(AgentLogsBatch);
struct Output(AgentLogsReceipt);
struct TargetError(ApiError);
macro_rules! contract {
    ($ty:ty, $id:literal) => {
        impl InterfaceContract for $ty {
            const CONTRACT_ID: &'static str = $id;
            const CONTRACT_VERSION: &'static str = "1";
            fn managed_projection_schema() -> Option<serde_json::Value> {
                use crate::extension_bus::managed_projection as mp;
                Some(mp::object_schema(&[("kind", mp::tag_schema($id))]))
            }
            fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
                Some(serde_json::json!({"kind": $id}))
            }
        }
    };
}
contract!(Input, "application-agent-logs-input");
contract!(Output, "application-agent-logs-output");
contract!(TargetError, "application-agent-logs-error");

#[cfg(test)]
#[path = "agent_logs/_tests/managed_projection.rs"]
mod managed_projection_tests;

struct Handler<R>(R);
impl<R> InterfaceHandler<Input, Output, TargetError, ApplicationPrincipal> for Handler<R>
where
    R: BillingRepository + OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: Input,
    ) -> InterfaceHandlerFuture<Output, TargetError> {
        let store = self.0.clone();
        Box::pin(async move {
            let principal = context.principal();
            control_plane::agent_logs::AgentLogsService::new(store)
                .ingest(
                    principal.application_id(),
                    principal.workspace_id(),
                    principal.api_key_id(),
                    input.0,
                )
                .await
                .map(Output)
                .map_err(|error| {
                    InterfaceTargetFailure::new("agent_logs", TargetError(ApiError::from(error)))
                })
        })
    }
}
struct Authorization;
impl InterfaceAuthorizationPort<ApplicationPrincipal> for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(OWNER)
            .expect("static agent logs interface identity is valid")
    }
    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
pub(crate) fn compile_registry<R>(
    store: R,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError>
where
    R: BillingRepository + OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
{
    let owner = InterfaceOwner::new(OWNER).expect("static agent logs interface identity is valid");
    let operation =
        AuthorizationOperation::new(ID).expect("static agent logs interface identity is valid");
    let id = InterfaceId::new(ID).expect("static agent logs interface identity is valid");
    let identity = InterfaceIdentity::new(
        id.clone(),
        InterfaceVersion::new("1").expect("static agent logs interface identity is valid"),
    );
    let handler =
        HandlerReference::new(OWNER).expect("static agent logs interface identity is valid");
    let contracts = InterfaceContracts::unary(
        ContractIdentity::new(Input::CONTRACT_ID, "1")
            .expect("static agent logs interface identity is valid"),
        ContractIdentity::new(Output::CONTRACT_ID, "1")
            .expect("static agent logs interface identity is valid"),
        ContractIdentity::new(TargetError::CONTRACT_ID, "1")
            .expect("static agent logs interface identity is valid"),
    );
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-agent-logs-v1")
            .expect("static agent logs interface identity is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            handler.clone(),
            TargetReference::new(OWNER).expect("static agent logs interface identity is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_authentication_adapter(
        &id,
        1,
        InterfaceExtensionRegistration::new(
            PluginIdentity::new("api-server.application-authentication")
                .expect("static agent logs interface identity is valid"),
            InterfaceExtensionTier::BuiltIn,
            InterfaceExtensionPoint::AuthenticationAdapter,
            InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("static agent logs interface identity is valid"),
        ActivatedAuthenticationAdapter::new(
            PluginIdentity::new("api-server.application-authentication")
                .expect("static agent logs interface identity is valid"),
            InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.application-api-key")
                .expect("static agent logs interface identity is valid"),
            AuthenticationActivationIdentity::new("api-server.application-api-key.activation.v1")
                .expect("static agent logs interface identity is valid"),
            PrincipalProfile::Application,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(BINDING).expect("static agent logs interface identity is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("POST", "/api/logs/v1/events")
                    .expect("static agent logs interface identity is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.application-api-key")
                .expect("static agent logs interface identity is valid"),
            AuthorizationAdapterReference::new(OWNER)
                .expect("static agent logs interface identity is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<Input, Output, TargetError, ApplicationPrincipal>(
        &id,
        handler,
        Arc::new(Handler(store)),
    )?;
    compiler.compile()
}

/// Ingest client agent log facts.
/// Atomically persists a versioned event batch with application-scoped replay identity without executing a model or debiting credits.
#[utoipa::path(post,path="/api/logs/v1/events",request_body=serde_json::Value,responses((status=200,body=serde_json::Value),(status=409,description="Source event payload conflicts with an existing receipt")),security(("applicationApiKey"=[])))]
pub async fn ingest_events(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(batch): Json<AgentLogsBatch>,
) -> Result<Json<ApiSuccess<AgentLogsReceipt>>, ApiError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
        .to_owned();
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("interface_registry_unavailable"))?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface_registry_unavailable"))?
        .snapshot();
    let binding = BindingId::new(BINDING).expect("static agent logs interface identity is valid");
    let authenticated = boot
        .authenticate_invocation::<_, ApplicationPrincipal>(
            Arc::clone(&snapshot),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                bearer_token: token,
            },
        )
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let result = InterfaceInvocationKernel::new(Arc::new(Authorization))
        .invoke::<Input, Output, TargetError>(snapshot, authenticated.into_envelope(Input(batch)))
        .await
        .map_err(|failure| match failure.into_error() {
            InterfaceInvocationError::TargetFailed(error) => error
                .into_source::<TargetError>()
                .map(|e| e.0)
                .unwrap_or_else(|| anyhow::anyhow!("agent_logs_target_failed").into()),
            error => anyhow::anyhow!("{error:?}").into(),
        })?;
    Ok(Json(ApiSuccess::new(result.into_value().0)))
}
pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        "/api/logs/v1/events",
        crate::external_route_assembly::post(ingest_events.layer(DefaultBodyLimit::disable())),
    )
}
