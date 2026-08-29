use std::{future::Future, pin::Pin, sync::Arc};

use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner,
    InterfaceScope, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, PublicPrincipal, RegistryCompiler, RouteIdentity,
    TargetReference,
};

use super::auth::PublicLoginInstancesResponse;
use crate::error_response::ApiError;

pub(crate) const INTERFACE_ID: &str = "public.auth.login-instances.list";
const HANDLER_REFERENCE: &str = "api-server.public-auth.login-instances";

pub(crate) struct PublicLoginInstancesInput {
    pub(crate) locale: domain::CatalogLocale,
}

impl InterfaceContract for PublicLoginInstancesInput {
    const CONTRACT_ID: &'static str = "public-login-instances-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicLoginInstancesOutput(pub(crate) PublicLoginInstancesResponse);

impl InterfaceContract for PublicLoginInstancesOutput {
    const CONTRACT_ID: &'static str = "public-login-instances-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicLoginInstancesTargetError(pub(crate) ApiError);

impl From<ApiError> for PublicLoginInstancesTargetError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl InterfaceContract for PublicLoginInstancesTargetError {
    const CONTRACT_ID: &'static str = "public-login-instances-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type PublicLoginInstancesFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<PublicLoginInstancesOutput, PublicLoginInstancesTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait PublicLoginInstancesPort: Send + Sync + 'static {
    fn list(&self, input: PublicLoginInstancesInput) -> PublicLoginInstancesFuture<'_>;
}

struct PublicLoginInstancesHandler {
    port: Arc<dyn PublicLoginInstancesPort>,
}

impl
    InterfaceHandler<
        PublicLoginInstancesInput,
        PublicLoginInstancesOutput,
        PublicLoginInstancesTargetError,
        PublicPrincipal,
    > for PublicLoginInstancesHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicLoginInstancesInput,
    ) -> InterfaceHandlerFuture<PublicLoginInstancesOutput, PublicLoginInstancesTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.list(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("public_login_instances", error))
        })
    }
}

pub(crate) struct PublicLoginInstancesAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for PublicLoginInstancesAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.public").expect("static adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<PublicPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn PublicLoginInstancesPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<PublicLoginInstancesInput>(),
        contract::<PublicLoginInstancesOutput>(),
        contract::<PublicLoginInstancesTargetError>(),
    );
    let operation = AuthorizationOperation::new("public.auth.login-instances.read")
        .expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.public-auth").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-login-instances-v1")
            .expect("static graph fingerprint is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Public,
            InterfaceAuthenticationPolicy::Anonymous,
            operation,
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
            TargetReference::new("control-plane.authenticator.list-public")
                .expect("static target is valid"),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new("http.public.auth.login-instances.v1").expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("GET", "/api/public/auth/login-instances")
                    .expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<
        PublicLoginInstancesInput,
        PublicLoginInstancesOutput,
        PublicLoginInstancesTargetError,
        PublicPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(PublicLoginInstancesHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
