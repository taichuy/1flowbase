use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    mapping::WorkflowExtensionHttpMethod,
    workflow_extension::{WorkflowExtensionRequestParameters, WorkflowHttpPrincipal},
};
use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner,
    InterfaceScope, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
    UserCredentialKind, UserPrincipal,
};

use super::native::NativeApiError;

pub(crate) const BINDING_ID: &str = "http.workflow-extension.invoke.v1";
const INTERFACE_ID: &str = "workflow-extension.invoke";
const HANDLER: &str = "api-server.workflow-extension.invoke";
const OPERATION: &str = "workflow-extension.invoke";

pub(crate) struct WorkflowExtensionInput {
    pub(crate) request_path: String,
    pub(crate) method: WorkflowExtensionHttpMethod,
    pub(crate) parameters: WorkflowExtensionRequestParameters,
}

impl InterfaceContract for WorkflowExtensionInput {
    const CONTRACT_ID: &'static str = "workflow-extension-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum WorkflowExtensionOutput {
    Accepted {
        run_id: uuid::Uuid,
        status: domain::FlowRunStatus,
    },
    Completed(domain::ApplicationRunDetail),
}

impl InterfaceContract for WorkflowExtensionOutput {
    const CONTRACT_ID: &'static str = "workflow-extension-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct WorkflowExtensionTargetError(pub(crate) NativeApiError);

impl InterfaceContract for WorkflowExtensionTargetError {
    const CONTRACT_ID: &'static str = "workflow-extension-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type WorkflowExtensionFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<WorkflowExtensionOutput, WorkflowExtensionTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait WorkflowExtensionPort: Send + Sync + 'static {
    fn invoke<'a>(
        &'a self,
        actor: &'a domain::ActorContext,
        principal: WorkflowHttpPrincipal,
        input: WorkflowExtensionInput,
    ) -> WorkflowExtensionFuture<'a>;
}

struct WorkflowExtensionHandler {
    port: Arc<dyn WorkflowExtensionPort>,
}

impl
    InterfaceHandler<
        WorkflowExtensionInput,
        WorkflowExtensionOutput,
        WorkflowExtensionTargetError,
        UserPrincipal,
    > for WorkflowExtensionHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: WorkflowExtensionInput,
    ) -> InterfaceHandlerFuture<WorkflowExtensionOutput, WorkflowExtensionTargetError> {
        let port = Arc::clone(&self.port);
        let actor = context.principal().actor().clone();
        let principal = match context.principal().credential_kind() {
            UserCredentialKind::UserApiKey { api_key_id } => {
                WorkflowHttpPrincipal::UserApiKey { api_key_id }
            }
            UserCredentialKind::CookieSession | UserCredentialKind::ServerDelegation => {
                WorkflowHttpPrincipal::User
            }
        };
        Box::pin(async move {
            port.invoke(&actor, principal, input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("workflow_extension", error))
        })
    }
}

pub(crate) struct WorkflowExtensionAuthorization;

impl InterfaceAuthorizationPort<UserPrincipal> for WorkflowExtensionAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.workflow-extension")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<UserPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn WorkflowExtensionPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<WorkflowExtensionInput>(),
        contract::<WorkflowExtensionOutput>(),
        contract::<WorkflowExtensionTargetError>(),
    );
    let operation = AuthorizationOperation::new(OPERATION).expect("static operation is valid");
    let owner =
        InterfaceOwner::new("api-server.workflow-extension").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:workflow-extension-v1").expect("static graph is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(HANDLER).expect("static handler is valid"),
            TargetReference::new("control-plane.workflow-extension-run")
                .expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
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
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.console.require-session")
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.console.require-session.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::User,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(BINDING_ID).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("ANY", "/api/ex/*slug").expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.console.require-session")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.workflow-extension")
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<WorkflowExtensionInput, WorkflowExtensionOutput, WorkflowExtensionTargetError, UserPrincipal>(
        &interface_id,
        HandlerReference::new(HANDLER).expect("static handler is valid"),
        Arc::new(WorkflowExtensionHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
