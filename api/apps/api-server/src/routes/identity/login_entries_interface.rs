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

use super::auth::PublicLoginEntriesResponse;
use crate::error_response::ApiError;

pub(crate) const INTERFACE_ID: &str = "public.auth.login-entries.list";
const HANDLER_REFERENCE: &str = "api-server.public-auth.login-entries";

pub(crate) struct PublicLoginEntriesInput {
    pub(crate) locale: domain::CatalogLocale,
}

impl InterfaceContract for PublicLoginEntriesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[("locale", mp::text_schema())]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "locale",
            mp::text(self.locale.as_str())?,
        )]))
    }

    const CONTRACT_ID: &'static str = "public-login-entries-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicLoginEntriesOutput(pub(crate) PublicLoginEntriesResponse);

impl InterfaceContract for PublicLoginEntriesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("default_login_entry_id", mp::text_schema()),
                (
                    "login_entries",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("auth_type",mp::text_schema()), ("is_builtin",serde_json::json!({"type":"boolean"})), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("sort_order",serde_json::json!({"type":"integer"})), ("public_ui_block",mp::object_schema(&[("byte_count",mp::count_schema())])), ("public_variables",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                (
                    "default_login_entry_id",
                    serde_json::Value::String((&(&(self).0).default_login_entry_id).to_string()),
                ),
                ("login_entries", {
                    if (&(&(self).0).login_entries).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (&(&(self).0).login_entries)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", serde_json::Value::String((&(item).id).to_string())),
                                    ("auth_type", mp::text(&(item).auth_type)?),
                                    ("is_builtin", serde_json::Value::Bool(*(&(item).is_builtin))),
                                    (
                                        "title",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).title).len()),
                                        )]),
                                    ),
                                    (
                                        "description",
                                        match (&(item).description).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("sort_order", serde_json::json!(*(&(item).sort_order))),
                                    (
                                        "public_ui_block",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).public_ui_block).len()),
                                        )]),
                                    ),
                                    (
                                        "public_variables",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).public_variables).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-login-entries-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicLoginEntriesTargetError(pub(crate) ApiError);

impl From<ApiError> for PublicLoginEntriesTargetError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl InterfaceContract for PublicLoginEntriesTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("PublicLoginEntriesTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("PublicLoginEntriesTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-login-entries-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type PublicLoginEntriesFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<PublicLoginEntriesOutput, PublicLoginEntriesTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait PublicLoginEntriesPort: Send + Sync + 'static {
    fn list(&self, input: PublicLoginEntriesInput) -> PublicLoginEntriesFuture<'_>;
}

struct PublicLoginEntriesHandler {
    port: Arc<dyn PublicLoginEntriesPort>,
}

impl
    InterfaceHandler<
        PublicLoginEntriesInput,
        PublicLoginEntriesOutput,
        PublicLoginEntriesTargetError,
        PublicPrincipal,
    > for PublicLoginEntriesHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicLoginEntriesInput,
    ) -> InterfaceHandlerFuture<PublicLoginEntriesOutput, PublicLoginEntriesTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.list(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("public_login_entries", error))
        })
    }
}

pub(crate) struct PublicLoginEntriesAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for PublicLoginEntriesAuthorization {
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
    port: Arc<dyn PublicLoginEntriesPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<PublicLoginEntriesInput>(),
        contract::<PublicLoginEntriesOutput>(),
        contract::<PublicLoginEntriesTargetError>(),
    );
    let operation = AuthorizationOperation::new("public.auth.login-entries.read")
        .expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.public-auth").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-login-entries-v1")
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
            TargetReference::new("control-plane.login-entry.list-public")
                .expect("static target is valid"),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner,
    ))?;
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.public-authentication")
                .expect("static plugin is valid"),
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
                .expect("static plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.public.activation.v1",
            )
            .expect("static activation is valid"),
            interface_runtime::PrincipalProfile::Public,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new("http.public.auth.login-entries.v1").expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("GET", "/api/public/auth/login-entries")
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
        PublicLoginEntriesInput,
        PublicLoginEntriesOutput,
        PublicLoginEntriesTargetError,
        PublicPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(PublicLoginEntriesHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
