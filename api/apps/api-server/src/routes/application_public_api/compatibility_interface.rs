use std::{future::Future, pin::Pin, sync::Arc};

use axum::response::{
    sse::{KeepAlive, Sse},
    IntoResponse, Response,
};
use control_plane::{
    application_public_api::{
        api_keys::ApplicationApiKeyActor,
        native::{ApplicationNativeRunService, NativeRunRequest, NativeRunResult},
        protocol_translation::TranslationProtocol,
    },
    ports::{
        ProviderTransportPayload, ProviderTransportSlotId, ProviderTransportStore,
        RuntimeEventEnvelope,
    },
};
use domain::AiNativeOperation;
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    GraphFingerprint, HandlerReference, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceEventStream, InterfaceExecution, InterfaceExecutionMode,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId,
    InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceProtocol, InterfaceScope,
    InterfaceStreamHandler, InterfaceStreamHandlerFuture, InterfaceTargetFailure, InterfaceVersion,
    InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage, ProtocolBinding,
    ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
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
pub(crate) const OPENAI_CHAT_STREAM_BINDING_ID: &str =
    "http.compat.openai.chat-completions.stream.v1";
pub(crate) const OPENAI_CHAT_ROOT_STREAM_BINDING_ID: &str =
    "http.compat.openai.chat-completions-root.stream.v1";
pub(crate) const OPENAI_RESPONSES_STREAM_BINDING_ID: &str =
    "http.compat.openai.responses.stream.v1";
pub(crate) const OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID: &str =
    "http.compat.openai.responses-root.stream.v1";
pub(crate) const ANTHROPIC_MESSAGES_STREAM_BINDING_ID: &str =
    "http.compat.anthropic.messages.stream.v1";
pub(crate) const NATIVE_WEBSOCKET_STREAM_BINDING_ID: &str =
    "http.application.native.runs.websocket.stream.v1";
pub(crate) const OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID: &str =
    "http.compat.openai.responses.websocket.stream.v1";

const OWNER: &str = "api-server.application-public-api";
const OPERATION: &str = "application.native.runs.create";
const TARGET: &str = "control-plane.application-native-run.create";
const AUTHENTICATION_ADAPTER: &str = "api-server.application-api-key";
const AUTHORIZATION_ADAPTER: &str = "api-server.application-compatibility-run";

pub(crate) struct CompatibilityBlockingInput {
    pub(crate) command: CompatibilityInvocationCommand,
}

pub(crate) enum CompatibilityInvocationCommand {
    Start {
        request: NativeRunRequest,
        protocol: TranslationProtocol,
        provider_transport: Option<CompatibilityProviderTransport>,
    },
    Resume {
        initial_run: NativeRunResult,
        command:
            control_plane::application_public_api::callback_resume::ResumePublishedCallbackCommand,
    },
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

pub(crate) struct CompatibilityStreamEvent {
    run: NativeRunResult,
    envelope: RuntimeEventEnvelope,
}

impl InterfaceContract for CompatibilityStreamEvent {
    const CONTRACT_ID: &'static str = "application-compatibility-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct CompatibilityTypedStreamInvocation {
    events: tokio::sync::mpsc::Receiver<CompatibilityStreamEvent>,
    completion: interface_runtime::InterfaceStreamCompletion<
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    >,
}

impl CompatibilityTypedStreamInvocation {
    pub(crate) fn into_parts(
        self,
    ) -> (
        tokio::sync::mpsc::Receiver<CompatibilityStreamEvent>,
        interface_runtime::InterfaceStreamCompletion<
            CompatibilityBlockingOutput,
            CompatibilityBlockingTargetError,
        >,
    ) {
        (self.events, self.completion)
    }
}

impl CompatibilityStreamEvent {
    pub(crate) fn into_parts(self) -> (NativeRunResult, RuntimeEventEnvelope) {
        (self.run, self.envelope)
    }
}

type CompatibilityBlockingFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<CompatibilityBlockingOutput, CompatibilityBlockingTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait CompatibilityBlockingPort: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> CompatibilityBlockingFuture<'a>;

    fn execute_stream<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        InterfaceEventStream<
                            CompatibilityStreamEvent,
                            CompatibilityBlockingOutput,
                            CompatibilityBlockingTargetError,
                        >,
                        CompatibilityBlockingTargetError,
                    >,
                > + Send
                + 'a,
        >,
    >;
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

struct CompatibilityStreamHandler {
    port: Arc<dyn CompatibilityBlockingPort>,
}

impl
    InterfaceStreamHandler<
        CompatibilityBlockingInput,
        CompatibilityStreamEvent,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
        ApplicationPrincipal,
    > for CompatibilityStreamHandler
{
    fn invoke_stream(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: CompatibilityBlockingInput,
    ) -> InterfaceStreamHandlerFuture<
        CompatibilityStreamEvent,
        CompatibilityBlockingOutput,
        CompatibilityBlockingTargetError,
    > {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute_stream(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("compatibility_stream", error))
        })
    }
}

impl CompatibilityBlockingPort for ApiState {
    fn execute<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> CompatibilityBlockingFuture<'a> {
        let state = Arc::new(self.clone());
        let actor = application_actor(principal);
        Box::pin(async move {
            let (request, protocol, provider_transport) = match input.command {
                CompatibilityInvocationCommand::Start {
                    request,
                    protocol,
                    provider_transport,
                } => (request, protocol, provider_transport),
                CompatibilityInvocationCommand::Resume { command, .. } => {
                    return crate::routes::application_public_api::compat_sse::execute_compatible_resume_for_actor(
                        state,
                        actor,
                        command,
                    )
                    .await
                    .map(CompatibilityBlockingOutput)
                    .map_err(CompatibilityBlockingTargetError);
                }
            };
            let protocol_context = request.client_protocol_envelope.clone();
            let run = ApplicationNativeRunService::new(state.store.clone())
                .with_last_used_cache(state.infrastructure.cache_store())
                .create_native_run_for_actor(actor.clone(), request, protocol)
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
            let provider_transport_slot = match provider_transport {
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

    fn execute_stream<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityBlockingInput,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        InterfaceEventStream<
                            CompatibilityStreamEvent,
                            CompatibilityBlockingOutput,
                            CompatibilityBlockingTargetError,
                        >,
                        CompatibilityBlockingTargetError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        let state = Arc::new(self.clone());
        let actor = application_actor(principal);
        Box::pin(async move {
            let typed = match input.command {
                CompatibilityInvocationCommand::Start {
                    request,
                    protocol,
                    provider_transport,
                } => {
                    let protocol_context = request.client_protocol_envelope.clone();
                    let run = ApplicationNativeRunService::new(state.store.clone())
                        .with_last_used_cache(state.infrastructure.cache_store())
                        .create_native_run_for_actor(actor.clone(), request, protocol)
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
                    let provider_transport_slot = match provider_transport {
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
                    crate::routes::application_public_api::compat_sse::start_compatible_typed_start_stream_for_actor(
                        state,
                        run,
                        provider_transport_slot,
                        actor,
                    )
                    .await
                    .map_err(CompatibilityBlockingTargetError)?
                }
                CompatibilityInvocationCommand::Resume {
                    initial_run,
                    command,
                } => crate::routes::application_public_api::compat_sse::start_compatible_typed_resume_stream_for_actor(
                    state,
                    initial_run,
                    command,
                    actor,
                )
                .await
                .map_err(CompatibilityBlockingTargetError)?,
            };
            let (initial_run, mut events) = typed.into_parts();
            let (publisher, stream) = interface_runtime::interface_stream_channel(32);
            tokio::spawn(async move {
                let mut terminal_run = initial_run;
                while let Some(event) = events.recv().await {
                    let (run, envelope) = event.into_parts();
                    terminal_run = run.clone();
                    if publisher
                        .emit(CompatibilityStreamEvent { run, envelope })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = publisher
                    .finish(interface_runtime::InterfaceStreamTerminal::Completed(
                        CompatibilityBlockingOutput(terminal_run),
                    ))
                    .await;
            });
            Ok(stream)
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
    port: Arc<dyn CompatibilityBlockingPort>,
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
    for (interface_id, binding_id, handler, method, path) in stream_bindings() {
        register_stream_binding(
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

#[cfg(test)]
pub(crate) fn compile_registry_for_test(
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    compile_registry(Arc::new(UnavailableCompatibilityBlockingPort))
}

#[cfg(test)]
struct UnavailableCompatibilityBlockingPort;

#[cfg(test)]
impl CompatibilityBlockingPort for UnavailableCompatibilityBlockingPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a ApplicationPrincipal,
        _input: CompatibilityBlockingInput,
    ) -> CompatibilityBlockingFuture<'a> {
        Box::pin(async {
            Err(CompatibilityBlockingTargetError(NativeApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "test_port_unavailable",
                "test compatibility port is unavailable",
            )))
        })
    }

    fn execute_stream<'a>(
        &'a self,
        _principal: &'a ApplicationPrincipal,
        _input: CompatibilityBlockingInput,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        InterfaceEventStream<
                            CompatibilityStreamEvent,
                            CompatibilityBlockingOutput,
                            CompatibilityBlockingTargetError,
                        >,
                        CompatibilityBlockingTargetError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async {
            Err(CompatibilityBlockingTargetError(NativeApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "test_port_unavailable",
                "test compatibility port is unavailable",
            )))
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn register_stream_binding(
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
    let contracts = InterfaceContracts::server_stream(
        contract::<CompatibilityBlockingInput>(),
        contract::<CompatibilityStreamEvent>(),
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
            InterfaceExecutionMode::ServerStream,
            handler.clone(),
            TargetReference::new(TARGET).expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    register_authentication(compiler, &interface_id)?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http_variant(
                RouteIdentity::new(method, path).expect("static route is valid"),
                "streaming",
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
    compiler.bind_stream_handler::<CompatibilityBlockingInput, CompatibilityStreamEvent, CompatibilityBlockingOutput, CompatibilityBlockingTargetError, ApplicationPrincipal>(
        &interface_id,
        handler,
        Arc::new(CompatibilityStreamHandler { port }),
    )?;
    Ok(())
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
    register_authentication(compiler, &interface_id)?;
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

fn register_authentication(
    compiler: &mut RegistryCompiler,
    interface_id: &InterfaceId,
) -> Result<(), interface_runtime::RegistryCompilationError> {
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
    )
}

pub(crate) async fn authenticate_application_principal(
    state: Arc<ApiState>,
    binding_id: &'static str,
    bearer_token: String,
) -> Result<ApplicationPrincipal, NativeApiError> {
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
    boot_snapshot
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
        })
}

pub(crate) async fn invoke_typed_stream_with_principal(
    state: Arc<ApiState>,
    binding_id: &'static str,
    principal: ApplicationPrincipal,
    input: CompatibilityBlockingInput,
) -> Result<CompatibilityTypedStreamInvocation, NativeApiError> {
    let snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|boot| boot.interface_registry())
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "compatibility interface is unavailable",
            )
        })?;
    let binding_id = BindingId::new(binding_id).expect("static binding id is valid");
    let authentication_activation = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_activation_unavailable",
                "compatibility authentication activation is unavailable",
            )
        })?
        .activation()
        .clone();
    let application_id = principal.application_id();
    let dispatch_target = native::application_runtime_target(
        state.as_ref(),
        snapshot.as_ref(),
        &binding_id,
        application_id,
    );
    let invocation = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        CompatibilityBlockingAuthorization,
    ))
    .invoke_server_stream_with_dispatch_target::<
        CompatibilityBlockingInput,
        CompatibilityStreamEvent,
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
    let (events, completion) = invocation.into_parts();
    Ok(CompatibilityTypedStreamInvocation { events, completion })
}

pub(crate) async fn invoke_stream_with_principal(
    state: Arc<ApiState>,
    binding_id: &'static str,
    principal: ApplicationPrincipal,
    input: CompatibilityBlockingInput,
    projection: crate::routes::application_public_api::compat_sse::CompatibleProtocolProjection,
) -> Result<Response, NativeApiError> {
    let application_id = principal.application_id();
    let invocation =
        invoke_typed_stream_with_principal(Arc::clone(&state), binding_id, principal, input)
            .await?;
    project_stream_invocation(state, application_id, invocation, projection)
}

fn project_stream_invocation(
    state: Arc<ApiState>,
    application_id: uuid::Uuid,
    invocation: CompatibilityTypedStreamInvocation,
    mut projection: crate::routes::application_public_api::compat_sse::CompatibleProtocolProjection,
) -> Result<Response, NativeApiError> {
    let (mut events, completion) = invocation.into_parts();
    let (sender, receiver) = tokio::sync::mpsc::channel(32);
    let sse_activity = state.runtime_activity.start(
        application_id,
        crate::runtime_activity::ApplicationActivityKind::SseConnection,
    );
    tokio::spawn(async move {
        let _sse_activity = sse_activity;
        let mut projection_open = true;
        while let Some(event) = events.recv().await {
            let (run, envelope) = event.into_parts();
            for event in projection.runtime_event_to_sse(&run, envelope) {
                if projection_open && sender.send(event).await.is_err() {
                    projection_open = false;
                }
            }
        }
        if let Ok(terminal) = completion.complete().await {
            let _receipt = terminal.receipt().clone().projected();
        }
    });
    Ok(
        Sse::new(tokio_stream::wrappers::ReceiverStream::new(receiver))
            .keep_alive(KeepAlive::default())
            .into_response(),
    )
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

pub(crate) async fn invoke_blocking_with_principal(
    state: Arc<ApiState>,
    binding_id: &'static str,
    principal: ApplicationPrincipal,
    input: CompatibilityBlockingInput,
) -> Result<NativeRunResult, NativeApiError> {
    let snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|boot| boot.interface_registry())
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "compatibility interface is unavailable",
            )
        })?;
    let binding_id = BindingId::new(binding_id).expect("static binding id is valid");
    let authentication_activation = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "authentication_activation_unavailable",
                "compatibility authentication activation is unavailable",
            )
        })?
        .activation()
        .clone();
    let dispatch_target = native::application_runtime_target(
        state.as_ref(),
        snapshot.as_ref(),
        &binding_id,
        principal.application_id(),
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

pub(crate) async fn invoke_stream(
    state: Arc<ApiState>,
    binding_id: &'static str,
    bearer_token: String,
    input: CompatibilityBlockingInput,
    mut projection: crate::routes::application_public_api::compat_sse::CompatibleProtocolProjection,
) -> Result<Response, NativeApiError> {
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
    let invocation = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        CompatibilityBlockingAuthorization,
    ))
    .invoke_server_stream_with_dispatch_target::<
        CompatibilityBlockingInput,
        CompatibilityStreamEvent,
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
    let (mut events, completion) = invocation.into_parts();
    let (sender, receiver) = tokio::sync::mpsc::channel(32);
    let sse_activity = state.runtime_activity.start(
        application_id,
        crate::runtime_activity::ApplicationActivityKind::SseConnection,
    );
    tokio::spawn(async move {
        let _sse_activity = sse_activity;
        let mut projection_open = true;
        while let Some(event) = events.recv().await {
            for event in projection.runtime_event_to_sse(&event.run, event.envelope) {
                if projection_open && sender.send(event).await.is_err() {
                    projection_open = false;
                }
            }
        }
        if let Ok(terminal) = completion.complete().await {
            let _receipt = terminal.receipt().clone().projected();
        }
    });
    Ok(
        Sse::new(tokio_stream::wrappers::ReceiverStream::new(receiver))
            .keep_alive(KeepAlive::default())
            .into_response(),
    )
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

pub(crate) fn application_actor(principal: &ApplicationPrincipal) -> ApplicationApiKeyActor {
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

fn stream_bindings() -> [BindingDeclaration; 7] {
    [
        (
            "compat.openai.chat-completions.stream",
            OPENAI_CHAT_STREAM_BINDING_ID,
            "api-server.compat.openai-chat.stream",
            "POST",
            "/v1/chat/completions",
        ),
        (
            "compat.openai.chat-completions-root.stream",
            OPENAI_CHAT_ROOT_STREAM_BINDING_ID,
            "api-server.compat.openai-chat-root.stream",
            "POST",
            "/chat/completions",
        ),
        (
            "compat.openai.responses.stream",
            OPENAI_RESPONSES_STREAM_BINDING_ID,
            "api-server.compat.openai-responses.stream",
            "POST",
            "/v1/responses",
        ),
        (
            "compat.openai.responses-root.stream",
            OPENAI_RESPONSES_ROOT_STREAM_BINDING_ID,
            "api-server.compat.openai-responses-root.stream",
            "POST",
            "/responses",
        ),
        (
            "compat.anthropic.messages.stream",
            ANTHROPIC_MESSAGES_STREAM_BINDING_ID,
            "api-server.compat.anthropic-messages.stream",
            "POST",
            "/v1/messages",
        ),
        (
            "application.native.runs.websocket.stream",
            NATIVE_WEBSOCKET_STREAM_BINDING_ID,
            "api-server.application-native-websocket.stream",
            "GET",
            "/api/agent/v1/runs/websocket",
        ),
        (
            "compat.openai.responses.websocket.stream",
            OPENAI_RESPONSES_WEBSOCKET_STREAM_BINDING_ID,
            "api-server.compat.openai-responses-websocket.stream",
            "GET",
            "/v1/responses",
        ),
    ]
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static compatibility contract is valid")
}
