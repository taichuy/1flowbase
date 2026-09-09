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
    ProtocolBinding, ProtocolProjection, RegistryCompiler, TargetReference, UserPrincipal,
};

use super::{McpCallOutcome, McpToolArguments};
use crate::error_response::ApiError;

pub(super) const INTERFACE_ID: &str = "mcp.user-api-key.invoke";
const HANDLER_REFERENCE: &str = "api-server.mcp.invoke";

#[expect(
    clippy::large_enum_variant,
    reason = "the MCP command is consumed once by the typed protocol adapter"
)]
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
        context: McpToolInvocationContext,
    },
}

pub(crate) struct McpForwardHeader {
    pub(super) name: String,
    pub(super) value: Vec<u8>,
}

pub(crate) struct McpToolInvocationContext {
    pub(super) headers: Vec<McpForwardHeader>,
    pub(super) user: domain::UserRecord,
    pub(super) actor: domain::ActorContext,
    pub(super) catalog: domain::McpCatalogSnapshot,
    pub(super) scope: crate::routes::mcp_protocol::virtual_ui::VirtualMcpScope,
}

impl InterfaceContract for McpInvocationInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Initialize")),
                (
                    "instance_name",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("InitializedNotification"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolsList")),
                ("path_regex_enabled", serde_json::json!({"type":"boolean"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolCall")),
                (
                    "name",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "context",
                    mp::object_schema(&[
                        (
                            "user",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "nickname",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "introduction",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "preferred_locale",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("meta", mp::json_summary_schema()),
                                (
                                    "default_display_role",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "status",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Active"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Disabled"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "roles",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "actor",
                            mp::object_schema(&[
                                ("user_id", mp::text_schema()),
                                ("tenant_id", mp::text_schema()),
                                ("current_workspace_id", mp::text_schema()),
                                (
                                    "effective_display_role",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("is_root", serde_json::json!({"type":"boolean"})),
                                (
                                    "permissions",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "catalog",
                            mp::object_schema(&[
                                (
                                    "instances",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "groups",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "tools",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "bindings",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "discovery_policies",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Initialize {
                instance_name: _field_instance_name,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Initialize".to_owned()),
                ),
                (
                    "instance_name",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((_field_instance_name).len()),
                    )]),
                ),
            ]),
            Self::InitializedNotification => mp::object_value(&[(
                "variant",
                serde_json::Value::String("InitializedNotification".to_owned()),
            )]),
            Self::ToolsList {
                path_regex_enabled: _field_path_regex_enabled,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("ToolsList".to_owned())),
                (
                    "path_regex_enabled",
                    serde_json::Value::Bool(*(_field_path_regex_enabled)),
                ),
            ]),
            Self::ToolCall {
                name: _field_name,
                context: _field_context,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("ToolCall".to_owned())),
                (
                    "name",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_name).len()))]),
                ),
                (
                    "context",
                    mp::object_value(&[
                        (
                            "user",
                            mp::object_value(&[
                                (
                                    "id",
                                    serde_json::Value::String(
                                        (&(&(_field_context).user).id).to_string(),
                                    ),
                                ),
                                (
                                    "name",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_context).user).name).len()),
                                    )]),
                                ),
                                (
                                    "nickname",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_context).user).nickname).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "introduction",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_context).user).introduction).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "preferred_locale",
                                    match (&(&(_field_context).user).preferred_locale).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                ("meta", mp::json_summary(&(&(_field_context).user).meta)),
                                (
                                    "default_display_role",
                                    match (&(&(_field_context).user).default_display_role).as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "status",
                                    match &(&(_field_context).user).status {
                                        domain::auth::UserStatus::Active => mp::object_value(&[(
                                            "variant",
                                            serde_json::Value::String("Active".to_owned()),
                                        )]),
                                        domain::auth::UserStatus::Disabled => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Disabled".to_owned()),
                                            )])
                                        }
                                    },
                                ),
                                (
                                    "roles",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_context).user).roles).len()),
                                    )]),
                                ),
                            ]),
                        ),
                        (
                            "actor",
                            mp::object_value(&[
                                (
                                    "user_id",
                                    serde_json::Value::String(
                                        (&(&(_field_context).actor).user_id).to_string(),
                                    ),
                                ),
                                (
                                    "tenant_id",
                                    serde_json::Value::String(
                                        (&(&(_field_context).actor).tenant_id).to_string(),
                                    ),
                                ),
                                (
                                    "current_workspace_id",
                                    serde_json::Value::String(
                                        (&(&(_field_context).actor).current_workspace_id)
                                            .to_string(),
                                    ),
                                ),
                                (
                                    "effective_display_role",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_context).actor).effective_display_role)
                                                .len()
                                        ),
                                    )]),
                                ),
                                (
                                    "is_root",
                                    serde_json::Value::Bool(*(&(&(_field_context).actor).is_root)),
                                ),
                                (
                                    "permissions",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).actor).permissions).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                        (
                            "catalog",
                            mp::object_value(&[
                                (
                                    "instances",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).catalog).instances).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "groups",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).catalog).groups).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "tools",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).catalog).tools).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "bindings",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).catalog).bindings).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "discovery_policies",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_context).catalog).discovery_policies).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
        })
    }

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
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Initialized")),
                (
                    "instance_name",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NotificationAccepted"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolsListed")),
                ("path_regex_enabled", serde_json::json!({"type":"boolean"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ToolCalled")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Success")),
                            ("0", mp::json_summary_schema()),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("Error")),
                            ("code", serde_json::json!({"type":"integer"})),
                            (
                                "message",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "data",
                                serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                            ),
                        ]),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Initialized {
                instance_name: _field_instance_name,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Initialized".to_owned()),
                ),
                (
                    "instance_name",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((_field_instance_name).len()),
                    )]),
                ),
            ]),
            Self::NotificationAccepted => mp::object_value(&[(
                "variant",
                serde_json::Value::String("NotificationAccepted".to_owned()),
            )]),
            Self::ToolsListed {
                path_regex_enabled: _field_path_regex_enabled,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ToolsListed".to_owned()),
                ),
                (
                    "path_regex_enabled",
                    serde_json::Value::Bool(*(_field_path_regex_enabled)),
                ),
            ]),
            Self::ToolCalled(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ToolCalled".to_owned()),
                ),
                (
                    "0",
                    match _field_0 {
                        crate::routes::mcp_protocol::McpCallOutcome::Success(_field_0) => {
                            mp::object_value(&[
                                ("variant", serde_json::Value::String("Success".to_owned())),
                                ("0", mp::json_summary(_field_0)),
                            ])
                        }
                        crate::routes::mcp_protocol::McpCallOutcome::Error {
                            code: _field_code,
                            message: _field_message,
                            data: _field_data,
                            ..
                        } => mp::object_value(&[
                            ("variant", serde_json::Value::String("Error".to_owned())),
                            ("code", serde_json::json!(*(_field_code))),
                            (
                                "message",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((_field_message).len()),
                                )]),
                            ),
                            (
                                "data",
                                match (_field_data).as_ref() {
                                    Some(item) => mp::json_summary(item),
                                    None => serde_json::Value::Null,
                                },
                            ),
                        ]),
                    },
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "mcp-invocation-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) struct McpInvocationTargetError(pub(super) ApiError);

impl InterfaceContract for McpInvocationTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("McpInvocationTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("McpInvocationTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "mcp-invocation-target-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) type McpCallFuture<'a> =
    Pin<Box<dyn Future<Output = Result<McpCallOutcome, ApiError>> + Send + 'a>>;

pub(crate) trait McpToolCallPort: Send + Sync + 'static {
    fn call(
        &self,
        name: String,
        arguments: McpToolArguments,
        context: McpToolInvocationContext,
    ) -> McpCallFuture<'_>;
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
                McpInvocationInput::ToolCall {
                    name,
                    arguments,
                    context,
                } => match tool_call.call(name, arguments, context).await {
                    Ok(outcome) => McpInvocationOutput::ToolCalled(outcome),
                    Err(error) => {
                        return Err(InterfaceTargetFailure::new(
                            "mcp_tool_call",
                            McpInvocationTargetError(error),
                        ));
                    }
                },
            };
            Ok(output)
        })
    }
}

pub(super) struct McpInvocationAuthorization;

impl InterfaceAuthorizationPort for McpInvocationAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.mcp-user-api-key")
            .expect("static adapter is valid")
    }

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
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.mcp-authentication")
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
            interface_runtime::PluginIdentity::new("api-server.mcp-authentication")
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.user-api-key")
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.user-api-key.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::User,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new("mcp.user-api-key.invoke.v1").expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::mcp("mcp.json-rpc"),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.user-api-key")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.mcp-user-api-key")
                .expect("static adapter is valid"),
            None,
        ),
    )?;
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
