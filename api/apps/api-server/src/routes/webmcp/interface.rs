use super::{InvokeWebMcpToolResponse, WebMcpRegistrationResponse};
use crate::error_response::ApiError;
use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationError, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceExtensionIsolation,
    InterfaceExtensionPermission, InterfaceExtensionPoint, InterfaceExtensionRegistration,
    InterfaceExtensionTier, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope,
    InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan, PluginIdentity,
    PrincipalProfile, ProtocolBinding, ProtocolProjection, RegistryCompilationError,
    RegistryCompiler, RouteIdentity, TargetReference, UserCredentialKind, UserPrincipal,
};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};

pub(crate) const OWNER: &str = "api-server.webmcp";
pub(crate) const REGISTRATIONS_BINDING: &str = "http.webmcp.registrations.v1";
pub(crate) const TOOLS_BINDING: &str = "http.webmcp.tools.invoke.v1";
const AUTHENTICATION: &str = "api-server.console.require-session";
const ACTIVATION: &str = "api-server.console.require-session.activation.v1";
const AUTHORIZATION: &str = "api-server.webmcp.cookie-session";

pub(crate) struct WebMcpForwardHeader {
    pub(crate) name: String,
    pub(crate) value: Vec<u8>,
}

pub(crate) enum WebMcpInput {
    Registrations,
    Tool {
        instance_id: String,
        operation: String,
        arguments: Value,
        forward_headers: Vec<WebMcpForwardHeader>,
    },
}

pub(crate) enum WebMcpOutput {
    Registrations(Vec<WebMcpRegistrationResponse>),
    Tool(InvokeWebMcpToolResponse),
}

pub(crate) struct WebMcpTargetError(pub(crate) ApiError);
macro_rules! contract {
    ($ty:ty, $id:literal) => {
        impl InterfaceContract for $ty {
            const CONTRACT_ID: &'static str = $id;
            const CONTRACT_VERSION: &'static str = "1";
        }
    };
}
contract!(WebMcpInput, "webmcp-input");
contract!(WebMcpOutput, "webmcp-output");
contract!(WebMcpTargetError, "webmcp-error");

pub(crate) type WebMcpFuture<'a> =
    Pin<Box<dyn Future<Output = Result<WebMcpOutput, ApiError>> + Send + 'a>>;
pub(crate) trait WebMcpPort: Send + Sync + 'static {
    fn execute<'a>(&'a self, principal: &'a UserPrincipal, input: WebMcpInput) -> WebMcpFuture<'a>;
}

struct WebMcpHandler {
    port: Arc<dyn WebMcpPort>,
}
impl InterfaceHandler<WebMcpInput, WebMcpOutput, WebMcpTargetError, UserPrincipal>
    for WebMcpHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: WebMcpInput,
    ) -> InterfaceHandlerFuture<WebMcpOutput, WebMcpTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("webmcp", WebMcpTargetError(error)))
        })
    }
}

pub(super) struct WebMcpAuthorization;
impl InterfaceAuthorizationPort<UserPrincipal> for WebMcpAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(AUTHORIZATION).expect("static WebMCP authorization")
    }
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest<UserPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        let cookie = request.principal().credential_kind() == UserCredentialKind::CookieSession;
        Box::pin(async move {
            if cookie {
                Ok(())
            } else {
                Err(InterfaceAuthorizationError::with_source(
                    "cookie_session_required",
                    ApiError::from(control_plane::errors::ControlPlaneError::PermissionDenied(
                        "cookie_session_required",
                    )),
                ))
            }
        })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn WebMcpPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
    let owner = InterfaceOwner::new(OWNER).expect("static WebMCP owner");
    let declarations = [
        (
            "webmcp.registrations.list",
            REGISTRATIONS_BINDING,
            "GET",
            "/api/webmcp/registrations",
            InterfaceAuditPolicy::ReadOnly,
        ),
        (
            "webmcp.tools.invoke",
            TOOLS_BINDING,
            "POST",
            "/api/webmcp/:instance_id/tools/:operation",
            InterfaceAuditPolicy::Mutating,
        ),
    ];
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:webmcp-v1").expect("static WebMCP graph"),
        declarations
            .iter()
            .map(|(id, ..)| AuthorizationOperation::new(*id).expect("static WebMCP operation")),
        [owner.clone()],
    );
    for (id, binding, method, path, audit) in declarations {
        let interface_id = InterfaceId::new(id).expect("static WebMCP interface");
        let identity = InterfaceIdentity::new(
            interface_id.clone(),
            InterfaceVersion::new("1").expect("static version"),
        );
        let contracts = InterfaceContracts::unary(
            contract_identity::<WebMcpInput>(),
            contract_identity::<WebMcpOutput>(),
            contract_identity::<WebMcpTargetError>(),
        );
        let handler =
            HandlerReference::new(format!("api-server.{id}.handler")).expect("static handler");
        compiler.register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                PrincipalProfile::User,
                InterfaceAuthenticationPolicy::Authenticated,
                AuthorizationOperation::new(id).expect("static operation"),
                InterfaceScope::Workspace,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new(format!("control-plane.{id}")).expect("static target"),
            ),
            audit,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        // Reuse the activated Console factory. This contributes a plan reference, not another credential parser.
        compiler.register_authentication_adapter(
            &interface_id,
            1,
            InterfaceExtensionRegistration::new(
                PluginIdentity::new("api-server.console-authentication")
                    .expect("static authentication plugin"),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .expect("static authentication registration"),
            ActivatedAuthenticationAdapter::new(
                PluginIdentity::new("api-server.console-authentication")
                    .expect("static authentication plugin"),
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new(AUTHENTICATION).expect("static authentication"),
                AuthenticationActivationIdentity::new(ACTIVATION).expect("static activation"),
                PrincipalProfile::User,
            ),
        )?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(binding).expect("static binding"),
                identity,
                contracts,
                ProtocolProjection::http(RouteIdentity::new(method, path).expect("static route")),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new(AUTHENTICATION).expect("static authentication"),
                AuthorizationAdapterReference::new(AUTHORIZATION).expect("static authorization"),
                None,
            ),
        )?;
        compiler.bind_handler::<WebMcpInput, WebMcpOutput, WebMcpTargetError, UserPrincipal>(
            &interface_id,
            handler,
            Arc::new(WebMcpHandler {
                port: Arc::clone(&port),
            }),
        )?;
    }
    compiler.compile()
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).expect("static WebMCP contract")
}
