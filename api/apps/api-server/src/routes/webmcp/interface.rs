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
impl InterfaceContract for WebMcpInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Registrations"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tool")),
                ("instance_id", mp::text_schema()),
                ("operation", mp::text_schema()),
                ("arguments", mp::json_summary_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Registrations => mp::object_value(&[(
                "variant",
                serde_json::Value::String("Registrations".to_owned()),
            )]),
            Self::Tool {
                instance_id: _field_instance_id,
                operation: _field_operation,
                arguments: _field_arguments,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Tool".to_owned())),
                ("instance_id", mp::text(_field_instance_id)?),
                ("operation", mp::text(_field_operation)?),
                ("arguments", mp::json_summary(_field_arguments)),
            ]),
        })
    }
    const CONTRACT_ID: &'static str = "webmcp-input";
    const CONTRACT_VERSION: &'static str = "1";
}
impl InterfaceContract for WebMcpOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Registrations")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("instance_id",mp::text_schema()), ("tools",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("operation",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("input_schema",mp::json_summary_schema()), ("annotations",mp::object_schema(&[("read_only_hint",serde_json::json!({"type":"boolean"})), ("untrusted_content_hint",serde_json::json!({"type":"boolean"}))]))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tool")),
                (
                    "0",
                    mp::object_schema(&[
                        ("content", mp::json_summary_schema()),
                        ("is_error", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Registrations(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Registrations".to_owned()),
                ),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("instance_id",mp::text(&(item).instance_id)?), ("tools",{ if (&(item).tools).len() > 32 { return None; } serde_json::Value::Array((&(item).tools).iter().map(|item| Some(mp::object_value(&[("operation",mp::text(&(item).operation)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("input_schema",mp::json_summary(&(item).input_schema)), ("annotations",mp::object_value(&[("read_only_hint",serde_json::Value::Bool(*(&(&(item).annotations).read_only_hint))), ("untrusted_content_hint",serde_json::Value::Bool(*(&(&(item).annotations).untrusted_content_hint)))]))]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?)
                }),
            ]),
            Self::Tool(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Tool".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("content", mp::json_summary(&(_field_0).content)),
                        ("is_error", serde_json::Value::Bool(*(&(_field_0).is_error))),
                    ]),
                ),
            ]),
        })
    }
    const CONTRACT_ID: &'static str = "webmcp-output";
    const CONTRACT_VERSION: &'static str = "1";
}
impl InterfaceContract for WebMcpTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("WebMcpTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("WebMcpTargetError".to_owned()),
        )]))
    }
    const CONTRACT_ID: &'static str = "webmcp-error";
    const CONTRACT_VERSION: &'static str = "1";
}

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
