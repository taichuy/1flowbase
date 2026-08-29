use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    native::{NativeRunRequest, NativeRunResult, NativeRunValidationError},
    protocol_translation::TranslationProtocol,
};
use interface_runtime::{
    AdmissionAdapterReference, ApplicationPrincipal, AuthenticationAdapterReference,
    AuthorizationAdapterReference, AuthorizationOperation, BindingId, CompiledInterfaceRegistry,
    ContractIdentity, ExtensionPlanFingerprint, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner,
    InterfaceScope, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};

pub(crate) const INTERFACE_ID: &str = "application.native.runs.create";
const INTERFACE_VERSION: &str = "1";
const HANDLER_REFERENCE: &str = "api-server.application-native-run.create";
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

pub(crate) struct ApplicationNativeRunTargetError(pub(crate) NativeRunValidationError);

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

pub(crate) trait ApplicationNativeRunPort: Send + Sync + 'static {
    fn create<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a>;
}

struct ApplicationNativeRunHandler {
    port: Arc<dyn ApplicationNativeRunPort>,
}

impl
    InterfaceHandler<
        ApplicationNativeRunInput,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    > for ApplicationNativeRunHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: ApplicationNativeRunInput,
    ) -> InterfaceHandlerFuture<ApplicationNativeRunOutput, ApplicationNativeRunTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.create(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("application_native_run", error))
        })
    }
}

pub(crate) struct ApplicationNativeRunAuthorization;

impl InterfaceAuthorizationPort<ApplicationPrincipal> for ApplicationNativeRunAuthorization {
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
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new(INTERFACE_VERSION).expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<ApplicationNativeRunInput>(),
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
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.application-api-key")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.application-native-run")
                .expect("static adapter is valid"),
            AdmissionAdapterReference::new("api-server.application-native-run")
                .expect("static adapter is valid"),
            ExtensionPlanFingerprint::new("graph:application-native-run-hooks-v1")
                .expect("static extension plan is valid"),
        ),
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
            TargetReference::new(TARGET_REFERENCE).expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_binding(ProtocolBinding::new(
        BindingId::new("http.application.native.runs.create.v1").expect("static binding is valid"),
        identity,
        contracts,
        ProtocolProjection::http(
            RouteIdentity::new("POST", "/api/agent/v1/runs").expect("static route is valid"),
        ),
    ))?;
    compiler.bind_handler::<
        ApplicationNativeRunInput,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(ApplicationNativeRunHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
