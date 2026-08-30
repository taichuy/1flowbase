use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    native::{NativeRunRequest, NativeRunResult},
    protocol_translation::TranslationProtocol,
};
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    GraphFingerprint, HandlerReference, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceEventStream, InterfaceExecution, InterfaceExecutionMode,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId,
    InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope, InterfaceStreamHandler,
    InterfaceStreamHandlerFuture, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};

use super::native::NativeApiError;

pub(crate) const ASYNC_INTERFACE_ID: &str = "application.native.runs.create-async";
pub(crate) const BLOCKING_INTERFACE_ID: &str = "application.native.runs.execute-blocking";
pub(crate) const STREAM_INTERFACE_ID: &str = "application.native.runs.execute-stream";
pub(crate) const ASYNC_BINDING_ID: &str = "http.application.native.runs.create-async.v1";
pub(crate) const BLOCKING_BINDING_ID: &str = "http.application.native.runs.execute-blocking.v1";
pub(crate) const STREAM_BINDING_ID: &str = "http.application.native.runs.execute-stream.v1";
const INTERFACE_VERSION: &str = "1";
const ASYNC_HANDLER_REFERENCE: &str = "api-server.application-native-run.create-async";
const BLOCKING_HANDLER_REFERENCE: &str = "api-server.application-native-run.execute-blocking";
const STREAM_HANDLER_REFERENCE: &str = "api-server.application-native-run.execute-stream";
const TARGET_REFERENCE: &str = "control-plane.application-native-run.create";
const OWNER: &str = "api-server.application-public-api";
const OPERATION: &str = "application.native.runs.create";

pub(crate) struct ApplicationNativeRunInput {
    pub(crate) request: NativeRunRequest,
    pub(crate) protocol: TranslationProtocol,
}

impl InterfaceContract for ApplicationNativeRunInput {
    const CONTRACT_ID: &'static str = "application-native-run-create-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationNativeRunOutput(pub(crate) NativeRunResult);

impl InterfaceContract for ApplicationNativeRunOutput {
    const CONTRACT_ID: &'static str = "application-native-run-create-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationNativeRunTargetError(pub(crate) NativeApiError);

impl InterfaceContract for ApplicationNativeRunTargetError {
    const CONTRACT_ID: &'static str = "application-native-run-create-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type ApplicationNativeRunFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<ApplicationNativeRunOutput, ApplicationNativeRunTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) struct ApplicationNativeRunStreamEvent(
    pub(crate) Result<axum::response::sse::Event, std::convert::Infallible>,
);

impl InterfaceContract for ApplicationNativeRunStreamEvent {
    const CONTRACT_ID: &'static str = "application-native-run-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type ApplicationNativeRunStreamFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    InterfaceEventStream<
                        ApplicationNativeRunStreamEvent,
                        ApplicationNativeRunOutput,
                        ApplicationNativeRunTargetError,
                    >,
                    ApplicationNativeRunTargetError,
                >,
            > + Send
            + 'a,
    >,
>;

pub(crate) trait ApplicationNativeRunPort: Send + Sync + 'static {
    fn create<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a>;

    fn execute_blocking<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a>;

    fn execute_stream<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunStreamFuture<'a>;
}

enum ApplicationNativeRunHandlerMode {
    Async,
    Blocking,
}

struct ApplicationNativeUnaryHandler {
    port: Arc<dyn ApplicationNativeRunPort>,
    mode: ApplicationNativeRunHandlerMode,
}

impl
    InterfaceHandler<
        ApplicationNativeRunInput,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    > for ApplicationNativeUnaryHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: ApplicationNativeRunInput,
    ) -> InterfaceHandlerFuture<ApplicationNativeRunOutput, ApplicationNativeRunTargetError> {
        let port = Arc::clone(&self.port);
        let blocking = matches!(self.mode, ApplicationNativeRunHandlerMode::Blocking);
        Box::pin(async move {
            let result = if blocking {
                port.execute_blocking(context.principal(), input).await
            } else {
                port.create(context.principal(), input).await
            };
            result.map_err(|error| InterfaceTargetFailure::new("application_native_run", error))
        })
    }
}

struct ApplicationNativeStreamHandler {
    port: Arc<dyn ApplicationNativeRunPort>,
}

impl
    InterfaceStreamHandler<
        ApplicationNativeRunInput,
        ApplicationNativeRunStreamEvent,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    > for ApplicationNativeStreamHandler
{
    fn invoke_stream(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: ApplicationNativeRunInput,
    ) -> InterfaceStreamHandlerFuture<
        ApplicationNativeRunStreamEvent,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
    > {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute_stream(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("application_native_stream", error))
        })
    }
}

pub(crate) struct ApplicationNativeRunAuthorization;

impl InterfaceAuthorizationPort<ApplicationPrincipal> for ApplicationNativeRunAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.application-native-run")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn ApplicationNativeRunPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let unary_contracts = InterfaceContracts::unary(
        contract::<ApplicationNativeRunInput>(),
        contract::<ApplicationNativeRunOutput>(),
        contract::<ApplicationNativeRunTargetError>(),
    );
    let stream_contracts = InterfaceContracts::server_stream(
        contract::<ApplicationNativeRunInput>(),
        contract::<ApplicationNativeRunStreamEvent>(),
        contract::<ApplicationNativeRunOutput>(),
        contract::<ApplicationNativeRunTargetError>(),
    );
    let owner = InterfaceOwner::new(OWNER).expect("static owner is valid");
    let operation = AuthorizationOperation::new(OPERATION).expect("static operation is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-native-run-v1")
            .expect("static graph fingerprint is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    for (interface_id, binding_id, handler_reference, mode, projection, contracts) in [
        (
            ASYNC_INTERFACE_ID,
            ASYNC_BINDING_ID,
            ASYNC_HANDLER_REFERENCE,
            InterfaceExecutionMode::Unary,
            ProtocolProjection::http_variant(route(), "async"),
            unary_contracts.clone(),
        ),
        (
            BLOCKING_INTERFACE_ID,
            BLOCKING_BINDING_ID,
            BLOCKING_HANDLER_REFERENCE,
            InterfaceExecutionMode::Unary,
            ProtocolProjection::http(route()),
            unary_contracts.clone(),
        ),
        (
            STREAM_INTERFACE_ID,
            STREAM_BINDING_ID,
            STREAM_HANDLER_REFERENCE,
            InterfaceExecutionMode::ServerStream,
            ProtocolProjection::http_variant(route(), "streaming"),
            stream_contracts.clone(),
        ),
    ] {
        let id = InterfaceId::new(interface_id).expect("static interface id is valid");
        let identity = InterfaceIdentity::new(
            id.clone(),
            InterfaceVersion::new(INTERFACE_VERSION).expect("static interface version is valid"),
        );
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
                mode,
                HandlerReference::new(handler_reference).expect("static handler is valid"),
                TargetReference::new(TARGET_REFERENCE).expect("static target is valid"),
            ),
            InterfaceAuditPolicy::Mutating,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        compiler.register_authentication_adapter(
            &id,
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
                AuthenticationAdapterReference::new("api-server.application-api-key")
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
                projection,
            ),
            adapter_plan(),
        )?;
    }
    compiler.bind_handler::<ApplicationNativeRunInput, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(ASYNC_INTERFACE_ID).unwrap(),
        HandlerReference::new(ASYNC_HANDLER_REFERENCE).unwrap(),
        Arc::new(ApplicationNativeUnaryHandler { port: Arc::clone(&port), mode: ApplicationNativeRunHandlerMode::Async }),
    )?;
    compiler.bind_handler::<ApplicationNativeRunInput, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(BLOCKING_INTERFACE_ID).unwrap(),
        HandlerReference::new(BLOCKING_HANDLER_REFERENCE).unwrap(),
        Arc::new(ApplicationNativeUnaryHandler { port: Arc::clone(&port), mode: ApplicationNativeRunHandlerMode::Blocking }),
    )?;
    compiler.bind_stream_handler::<ApplicationNativeRunInput, ApplicationNativeRunStreamEvent, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(STREAM_INTERFACE_ID).unwrap(),
        HandlerReference::new(STREAM_HANDLER_REFERENCE).unwrap(),
        Arc::new(ApplicationNativeStreamHandler { port }),
    )?;
    compiler.compile()
}

fn route() -> RouteIdentity {
    RouteIdentity::new("POST", "/api/agent/v1/runs").expect("static route is valid")
}

fn adapter_plan() -> InvocationAdapterPlan {
    InvocationAdapterPlan::new(
        AuthenticationAdapterReference::new("api-server.application-api-key")
            .expect("static adapter is valid"),
        AuthorizationAdapterReference::new("api-server.application-native-run")
            .expect("static adapter is valid"),
        None,
    )
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
