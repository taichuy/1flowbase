use std::{future::Future, pin::Pin, sync::Arc};

use interface_runtime::{
    AdmissionAdapterReference, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    ExtensionPlanFingerprint, GraphFingerprint, HandlerReference, InterfaceAccess,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture,
    InterfaceAuthorizationPort, InterfaceAuthorizationRequest, InterfaceContract,
    InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution,
    InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope,
    InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan, ProtocolBinding,
    ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference, UserPrincipal,
};

use super::{McpCallOutcome, McpToolArguments};
use crate::error_response::ApiError;

pub(super) const INTERFACE_ID: &str = "mcp.user-api-key.invoke";
const HANDLER_REFERENCE: &str = "api-server.mcp.invoke";

pub(super) enum McpInvocationInput {
    Initialize {
        instance_name: String,
    },
    InitializedNotification,
    ToolsList {
        path_regex_enabled: bool,
    },
    ToolCall {
        name: String,
        arguments: McpToolArguments,
    },
}

impl InterfaceContract for McpInvocationInput {
    const CONTRACT_ID: &'static str = "mcp-invocation-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) enum McpInvocationOutput {
    Initialized { instance_name: String },
    NotificationAccepted,
    ToolsListed { path_regex_enabled: bool },
    ToolCalled(McpCallOutcome),
}

impl InterfaceContract for McpInvocationOutput {
    const CONTRACT_ID: &'static str = "mcp-invocation-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) struct McpInvocationTargetError(pub(super) ApiError);

impl InterfaceContract for McpInvocationTargetError {
    const CONTRACT_ID: &'static str = "mcp-invocation-target-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) type McpCallFuture<'a> =
    Pin<Box<dyn Future<Output = Result<McpCallOutcome, ApiError>> + Send + 'a>>;

pub(super) trait McpToolCallPort: Send + Sync + 'static {
    fn call(&self, name: String, arguments: McpToolArguments) -> McpCallFuture<'_>;
}

struct McpInvocationHandler {
    tool_call: Arc<dyn McpToolCallPort>,
}

impl InterfaceHandler<McpInvocationInput, McpInvocationOutput, McpInvocationTargetError>
    for McpInvocationHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        input: McpInvocationInput,
    ) -> InterfaceHandlerFuture<McpInvocationOutput, McpInvocationTargetError> {
        let tool_call = Arc::clone(&self.tool_call);
        Box::pin(async move {
            let output = match input {
                McpInvocationInput::Initialize { instance_name } => {
                    McpInvocationOutput::Initialized { instance_name }
                }
                McpInvocationInput::InitializedNotification => {
                    McpInvocationOutput::NotificationAccepted
                }
                McpInvocationInput::ToolsList { path_regex_enabled } => {
                    McpInvocationOutput::ToolsListed { path_regex_enabled }
                }
                McpInvocationInput::ToolCall { name, arguments } => {
                    match tool_call.call(name, arguments).await {
                        Ok(outcome) => McpInvocationOutput::ToolCalled(outcome),
                        Err(error) => {
                            return Err(InterfaceTargetFailure::new(
                                "mcp_tool_call",
                                McpInvocationTargetError(error),
                            ));
                        }
                    }
                }
            };
            Ok(output)
        })
    }
}

pub(super) struct McpInvocationAuthorization;

impl InterfaceAuthorizationPort for McpInvocationAuthorization {
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let authorized = matches!(
            request.principal().credential_kind(),
            interface_runtime::UserCredentialKind::UserApiKey { .. }
        );
        Box::pin(async move {
            if authorized {
                Ok(())
            } else {
                Err(interface_runtime::InterfaceAuthorizationError::classified(
                    "user_api_key_required",
                ))
            }
        })
    }
}

pub(super) fn compile_registry(
    tool_call: Arc<dyn McpToolCallPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<McpInvocationInput>(),
        contract::<McpInvocationOutput>(),
        contract::<McpInvocationTargetError>(),
    );
    let operation =
        AuthorizationOperation::new("mcp.tools.invoke").expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.mcp-protocol").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:mcp-protocol-v1").expect("static graph is valid"),
        [operation.clone()],
        [owner.clone()],
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.user-api-key")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.mcp-user-api-key")
                .expect("static adapter is valid"),
            AdmissionAdapterReference::new("api-server.mcp-instance-enabled")
                .expect("static adapter is valid"),
            ExtensionPlanFingerprint::new("graph:mcp-protocol-hooks-v1")
                .expect("static plan is valid"),
        ),
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
            HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
            TargetReference::new("api-server.mcp.virtual-ui").expect("static target is valid"),
        ),
        InterfaceAuditPolicy::Mutating,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_binding(ProtocolBinding::new(
        BindingId::new("http.mcp.user-api-key.invoke.v1").expect("static binding is valid"),
        identity,
        contracts,
        ProtocolProjection::http(
            RouteIdentity::new("POST", "/api/mcp/:instance_id").expect("static route is valid"),
        ),
    ))?;
    compiler.bind_handler::<McpInvocationInput, McpInvocationOutput, McpInvocationTargetError, UserPrincipal>(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(McpInvocationHandler { tool_call }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).expect("static contract is valid")
}
