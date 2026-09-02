use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::auth::{LoginResult, SignUpCommand};
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

use super::auth::AuthProviderResponse;
use crate::error_response::ApiError;

pub(crate) const PROVIDERS_BINDING_ID: &str = "http.public.auth.providers.v1";
pub(crate) const SIGN_UP_BINDING_ID: &str = "http.public.auth.sign-up.v1";

pub(crate) struct PublicProvidersInput {
    pub(crate) locale: domain::CatalogLocale,
}

impl InterfaceContract for PublicProvidersInput {
    const CONTRACT_ID: &'static str = "public-auth-providers-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicProvidersOutput(pub(crate) Vec<AuthProviderResponse>);

impl InterfaceContract for PublicProvidersOutput {
    const CONTRACT_ID: &'static str = "public-auth-providers-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignUpInput(pub(crate) SignUpCommand);

impl InterfaceContract for PublicSignUpInput {
    const CONTRACT_ID: &'static str = "public-sign-up-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignUpOutput(pub(crate) LoginResult);

impl InterfaceContract for PublicSignUpOutput {
    const CONTRACT_ID: &'static str = "public-sign-up-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicResidualTargetError(pub(crate) ApiError);

impl InterfaceContract for PublicResidualTargetError {
    const CONTRACT_ID: &'static str = "public-auth-residual-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type PublicProvidersFuture<'a> = Pin<
    Box<dyn Future<Output = Result<PublicProvidersOutput, PublicResidualTargetError>> + Send + 'a>,
>;
pub(crate) type PublicSignUpFuture<'a> = Pin<
    Box<dyn Future<Output = Result<PublicSignUpOutput, PublicResidualTargetError>> + Send + 'a>,
>;

pub(crate) trait PublicProvidersPort: Send + Sync + 'static {
    fn list(&self, input: PublicProvidersInput) -> PublicProvidersFuture<'_>;
}

pub(crate) trait PublicSignUpPort: Send + Sync + 'static {
    fn sign_up(&self, input: PublicSignUpInput) -> PublicSignUpFuture<'_>;
}

struct ProvidersHandler(Arc<dyn PublicProvidersPort>);
struct SignUpHandler(Arc<dyn PublicSignUpPort>);

impl
    InterfaceHandler<
        PublicProvidersInput,
        PublicProvidersOutput,
        PublicResidualTargetError,
        PublicPrincipal,
    > for ProvidersHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicProvidersInput,
    ) -> InterfaceHandlerFuture<PublicProvidersOutput, PublicResidualTargetError> {
        let port = Arc::clone(&self.0);
        Box::pin(async move {
            port.list(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("public_auth_providers", error))
        })
    }
}

impl
    InterfaceHandler<
        PublicSignUpInput,
        PublicSignUpOutput,
        PublicResidualTargetError,
        PublicPrincipal,
    > for SignUpHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicSignUpInput,
    ) -> InterfaceHandlerFuture<PublicSignUpOutput, PublicResidualTargetError> {
        let port = Arc::clone(&self.0);
        Box::pin(async move {
            port.sign_up(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("public_sign_up", error))
        })
    }
}

pub(crate) struct PublicResidualAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for PublicResidualAuthorization {
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
    providers: Arc<dyn PublicProvidersPort>,
    sign_up: Arc<dyn PublicSignUpPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new("api-server.public-auth").expect("static owner is valid");
    let operations = [
        AuthorizationOperation::new("public.auth.providers.read")
            .expect("static providers authorization operation is valid"),
        AuthorizationOperation::new("public.auth.sign-up")
            .expect("static sign-up authorization operation is valid"),
    ];
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-auth-residual-v1")
            .expect("static registry graph fingerprint is valid"),
        operations.clone(),
        [owner.clone()],
    );
    register::<PublicProvidersInput, PublicProvidersOutput>(
        &mut compiler,
        "public.auth.providers.list",
        "api-server.public-auth.providers",
        "control-plane.authenticator.public-provider",
        "GET",
        "/api/public/auth/providers",
        PROVIDERS_BINDING_ID,
        operations[0].clone(),
        owner.clone(),
        InterfaceAuditPolicy::ReadOnly,
    )?;
    register::<PublicSignUpInput, PublicSignUpOutput>(
        &mut compiler,
        "public.auth.sign-up",
        "api-server.public-auth.sign-up",
        "control-plane.auth-kernel.sign-up",
        "POST",
        "/api/public/auth/sign-up",
        SIGN_UP_BINDING_ID,
        operations[1].clone(),
        owner,
        InterfaceAuditPolicy::Mutating,
    )?;
    compiler.bind_handler::<PublicProvidersInput, PublicProvidersOutput, PublicResidualTargetError, PublicPrincipal>(
        &InterfaceId::new("public.auth.providers.list")
            .expect("static providers interface id is valid"),
        HandlerReference::new("api-server.public-auth.providers")
            .expect("static providers handler reference is valid"),
        Arc::new(ProvidersHandler(providers)),
    )?;
    compiler.bind_handler::<PublicSignUpInput, PublicSignUpOutput, PublicResidualTargetError, PublicPrincipal>(
        &InterfaceId::new("public.auth.sign-up").expect("static sign-up interface id is valid"),
        HandlerReference::new("api-server.public-auth.sign-up")
            .expect("static sign-up handler reference is valid"),
        Arc::new(SignUpHandler(sign_up)),
    )?;
    compiler.compile()
}

#[allow(clippy::too_many_arguments)]
fn register<I: InterfaceContract, O: InterfaceContract>(
    compiler: &mut RegistryCompiler,
    interface: &str,
    handler: &str,
    target: &str,
    method: &str,
    path: &str,
    binding: &str,
    operation: AuthorizationOperation,
    owner: InterfaceOwner,
    audit: InterfaceAuditPolicy,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(interface).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<I>(),
        contract::<O>(),
        contract::<PublicResidualTargetError>(),
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
            HandlerReference::new(handler).expect("static handler reference is valid"),
            TargetReference::new(target).expect("static target reference is valid"),
        ),
        audit,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.public-authentication")
                .expect("static authentication plugin identity is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::System,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("built-in authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.public-authentication")
                .expect("static authentication plugin identity is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.public")
                .expect("static authentication adapter reference is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.public.activation.v1",
            )
            .expect("static authentication activation identity is valid"),
            interface_runtime::PrincipalProfile::Public,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding).expect("static binding id is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new(method, path).expect("static route identity is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.public")
                .expect("static authentication adapter reference is valid"),
            AuthorizationAdapterReference::new("api-server.public")
                .expect("static authorization adapter reference is valid"),
            None,
        ),
    )
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
