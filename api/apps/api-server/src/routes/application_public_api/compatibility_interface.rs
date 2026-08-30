use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::{
    application_public_api::{
        api_keys::ApplicationApiKeyActor,
        native::{ApplicationNativeRunService, NativeRunRequest, NativeRunResult},
        protocol_translation::TranslationProtocol,
    },
    ports::{ProviderTransportPayload, ProviderTransportSlotId, ProviderTransportStore},
};
use domain::AiNativeOperation;
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    GraphFingerprint, HandlerReference, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceIdentity,
    InterfaceLifecycle, InterfaceOwner, InterfaceProtocol, InterfaceScope, InterfaceTargetFailure,
    InterfaceVersion, InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};

use crate::{
    app_state::ApiState,
    extension_bus::ApplicationApiKeyAuthenticationCredential,
    routes::application_public_api::native::{self, NativeApiError},
};

pub(crate) const OPENAI_CHAT_BINDING_ID: &str = "http.compat.openai.chat-completions.blocking.v1";
pub(crate) const OPENAI_CHAT_ROOT_BINDING_ID: &str =
    "http.compat.openai.chat-completions-root.blocking.v1";
pub(crate) const OPENAI_RESPONSES_BINDING_ID: &str = "http.compat.openai.responses.blocking.v1";
pub(crate) const OPENAI_RESPONSES_ROOT_BINDING_ID: &str =
    "http.compat.openai.responses-root.blocking.v1";
pub(crate) const OPENAI_RESPONSES_COMPACT_BINDING_ID: &str =
    "http.compat.openai.responses-compact.blocking.v1";
pub(crate) const ANTHROPIC_MESSAGES_BINDING_ID: &str = "http.compat.anthropic.messages.blocking.v1";

const OWNER: &str = "api-server.application-public-api";
const OPERATION: &str = "application.native.runs.create";
const TARGET: &str = "control-plane.application-native-run.create";
const AUTHENTICATION_ADAPTER: &str = "api-server.application-api-key";
const AUTHORIZATION_ADAPTER: &str = "api-server.application-compatibility-run";

pub(crate) struct CompatibilityBlockingInput {
    pub(crate) request: NativeRunRequest,
    pub(crate) protocol: TranslationProtocol,
    pub(crate) provider_transport: Option<CompatibilityProviderTransport>,
}

pub(crate) struct CompatibilityProviderTransport {
    pub(crate) operation: AiNativeOperation,
    pub(crate) payload: Option<ProviderTransportPayload>,
}

impl InterfaceContract for CompatibilityBlockingInput {
    const CONTRACT_ID: &'static str = "application-compatibility-blocking-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct CompatibilityBlockingOutput(pub(crate) NativeRunResult);

impl InterfaceContract for CompatibilityBlockingOutput {
    const CONTRACT_ID: &'static str = "application-compatibility-blocking-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct CompatibilityBlockingTargetError(pub(crate) NativeApiError);

impl InterfaceContract for CompatibilityBlockingTargetError {
    const CONTRACT_ID: &'static str = "application-compatibility-blocking-error";
    const CONTRACT_VERSION: &'static str = "1";
}

type CompatibilityBlockingFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<CompatibilityBlockingOutput, CompatibilityBlockingTargetError>>
            + Send
            + 'a,
    >,
>;

trait CompatibilityBlockingPort: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> CompatibilityBlockingFuture<'a>;
}

struct CompatibilityBlockingHandler {
    port: Arc<dyn CompatibilityBlockingPort>,
}

impl
    InterfaceHandler<
        CompatibilityBlockingInput,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
        ApplicationPrincipal,
    > for CompatibilityBlockingHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: CompatibilityBlockingInput,
    ) -> InterfaceHandlerFuture<CompatibilityBlockingOutput, CompatibilityBlockingTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("compatibility_blocking", error))
        })
    }
}

struct CompatibilityBlockingAdapter {
    state: std::sync::Weak<ApiState>,
}

impl CompatibilityBlockingPort for CompatibilityBlockingAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> CompatibilityBlockingFuture<'a> {
        let state = self.state.clone();
        let actor = application_actor(principal);
        Box::pin(async move {
            let state = state.upgrade().ok_or_else(|| {
                CompatibilityBlockingTargetError(NativeApiError::new(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "api_state_unavailable",
                    "API state is unavailable",
                ))
            })?;
            let protocol_context = input.request.client_protocol_envelope.clone();
            let run = ApplicationNativeRunService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .create_native_run_for_actor(actor.clone(), input.request, input.protocol)
                .await
                .map_err(native::native_error)
                .map_err(CompatibilityBlockingTargetError)?;
            native::stage_client_protocol_context(
                state.infrastructure.provider_transport_store().as_ref(),
                &run,
                protocol_context,
            )
            .await
            .map_err(CompatibilityBlockingTargetError)?;
            let provider_transport_slot = match input.provider_transport {
                Some(transport) => stage_provider_transport(
                    state.infrastructure.provider_transport_store().as_ref(),
                    run.id,
                    transport.operation,
                    transport.payload,
                )
                .await
                .map_err(CompatibilityBlockingTargetError)?,
                None => None,
            };
            native::execute_blocking_native_run_for_actor_with_provider_transport(
                state,
                actor,
                run,
                provider_transport_slot,
            )
            .await
            .map(CompatibilityBlockingOutput)
            .map_err(CompatibilityBlockingTargetError)
        })
    }
}

pub(crate) struct CompatibilityBlockingAuthorization;

impl InterfaceAuthorizationPort<ApplicationPrincipal> for CompatibilityBlockingAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
            .expect("static compatibility authorization adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    state: std::sync::Weak<ApiState>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new(OWNER).expect("static compatibility owner is valid");
    let operation =
        AuthorizationOperation::new(OPERATION).expect("static compatibility operation is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-compatibility-blocking-v1")
            .expect("static compatibility graph fingerprint is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    let port: Arc<dyn CompatibilityBlockingPort> = Arc::new(CompatibilityBlockingAdapter { state });
    for (interface_id, binding_id, handler, method, path) in bindings() {
        register_binding(
            &mut compiler,
            &owner,
            &operation,
            Arc::clone(&port),
            interface_id,
            binding_id,
            handler,
            method,
            path,
        )?;
    }
    compiler.compile()
}

#[allow(clippy::too_many_arguments)]
fn register_binding(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
    port: Arc<dyn CompatibilityBlockingPort>,
    interface_id: &str,
    binding_id: &str,
    handler: &str,
    method: &str,
    path: &str,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(interface_id).expect("static interface id is valid");
    let contracts = InterfaceContracts::unary(
        contract::<CompatibilityBlockingInput>(),
        contract::<CompatibilityBlockingOutput>(),
        contract::<CompatibilityBlockingTargetError>(),
    );
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let handler = HandlerReference::new(handler).expect("static handler is valid");
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation.clone(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            handler.clone(),
            TargetReference::new(TARGET).expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("built-in authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.application-api-key.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::Application,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new(method, path).expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<CompatibilityBlockingInput, CompatibilityBlockingOutput, CompatibilityBlockingTargetError, ApplicationPrincipal>(
        &interface_id,
        handler,
        Arc::new(CompatibilityBlockingHandler { port }),
    )?;
    Ok(())
}

pub(crate) async fn invoke_blocking(
    state: Arc<ApiState>,
    binding_id: &'static str,
    bearer_token: String,
    input: CompatibilityBlockingInput,
) -> Result<NativeRunResult, NativeApiError> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or_else(|| {
        NativeApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "interface_registry_unavailable",
            "compatibility interface is unavailable",
        )
    })?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "compatibility interface is unavailable",
            )
        })?;
    let binding_id = BindingId::new(binding_id).expect("static binding id is valid");
    let activated = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "compatibility authentication activation is unavailable",
        )
    })?;
    let principal: ApplicationPrincipal = boot_snapshot
        .authenticate(
            activated,
            ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| {
            native::native_error(
                control_plane::application_public_api::native::NativeRunValidationError::NotAuthenticated,
            )
        })?;
    let application_id = principal.application_id();
    let authentication_activation = activated.activation().clone();
    let dispatch_target = native::application_runtime_target(
        state.as_ref(),
        snapshot.as_ref(),
        &binding_id,
        application_id,
    );
    let outcome = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        CompatibilityBlockingAuthorization,
    ))
    .invoke_with_dispatch_target::<
        CompatibilityBlockingInput,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    >(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            authentication_activation,
            principal,
            None,
            input,
        ),
        dispatch_target,
    )
    .await
    .map_err(|failure| invocation_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value().0)
}

fn invocation_error(error: interface_runtime::InterfaceInvocationError) -> NativeApiError {
    match error {
        interface_runtime::InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<CompatibilityBlockingTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| {
                NativeApiError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "compatibility_target_failed",
                    "compatibility target failed",
                )
            }),
        error => NativeApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "compatibility_invocation_failed",
            error.to_string(),
        ),
    }
}

async fn stage_provider_transport(
    store: &dyn ProviderTransportStore,
    flow_run_id: uuid::Uuid,
    operation: AiNativeOperation,
    payload: Option<ProviderTransportPayload>,
) -> Result<Option<ProviderTransportSlotId>, NativeApiError> {
    if matches!(operation, AiNativeOperation::CountTokens) {
        return Ok(None);
    }
    let Some(payload) = payload else {
        return Ok(None);
    };
    let slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    store.put(slot, payload).await.map_err(|_| {
        NativeApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "provider_transport_staging_failed",
            "provider transport is temporarily unavailable",
        )
    })?;
    Ok(Some(slot))
}

fn application_actor(principal: &ApplicationPrincipal) -> ApplicationApiKeyActor {
    let actor = principal.authorized_actor().clone();
    ApplicationApiKeyActor {
        api_key_id: principal.api_key_id(),
        application_id: principal.application_id(),
        creator_user_id: actor.user_id,
        tenant_id: actor.tenant_id,
        workspace_id: principal.workspace_id(),
        actor,
    }
}

type BindingDeclaration = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

fn bindings() -> [BindingDeclaration; 6] {
    [
        (
            "compat.openai.chat-completions.blocking",
            OPENAI_CHAT_BINDING_ID,
            "api-server.compat.openai-chat.blocking",
            "POST",
            "/v1/chat/completions",
        ),
        (
            "compat.openai.chat-completions-root.blocking",
            OPENAI_CHAT_ROOT_BINDING_ID,
            "api-server.compat.openai-chat-root.blocking",
            "POST",
            "/chat/completions",
        ),
        (
            "compat.openai.responses.blocking",
            OPENAI_RESPONSES_BINDING_ID,
            "api-server.compat.openai-responses.blocking",
            "POST",
            "/v1/responses",
        ),
        (
            "compat.openai.responses-root.blocking",
            OPENAI_RESPONSES_ROOT_BINDING_ID,
            "api-server.compat.openai-responses-root.blocking",
            "POST",
            "/responses",
        ),
        (
            "compat.openai.responses-compact.blocking",
            OPENAI_RESPONSES_COMPACT_BINDING_ID,
            "api-server.compat.openai-responses-compact.blocking",
            "POST",
            "/v1/responses/compact",
        ),
        (
            "compat.anthropic.messages.blocking",
            ANTHROPIC_MESSAGES_BINDING_ID,
            "api-server.compat.anthropic-messages.blocking",
            "POST",
            "/v1/messages",
        ),
    ]
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static compatibility contract is valid")
}
