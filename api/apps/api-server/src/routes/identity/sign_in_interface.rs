use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::auth::{
    AuthKernel, AuthenticatorRegistry, LoginCommand, LoginResult, SessionIssuer,
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
    ProtocolBinding, ProtocolProjection, PublicPrincipal, RegistryCompiler, RouteIdentity,
    TargetReference,
};

use crate::error_response::ApiError;
use control_plane::ports::SessionStore;
use storage_durable_postgres::MainDurableStore;

pub(crate) const BINDING_ID: &str = "http.public.auth.sign-in.v1";
const INTERFACE_ID: &str = "public.auth.sign-in";
const HANDLER: &str = "api-server.public-auth.sign-in";
const OPERATION: &str = "public.auth.sign-in";

pub(crate) struct PublicSignInInput(pub(crate) LoginCommand);

impl InterfaceContract for PublicSignInInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[("login_entry_id", mp::text_schema())]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[(
                "login_entry_id",
                serde_json::Value::String((&(&(self).0).login_entry_id).to_string()),
            )]),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-sign-in-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignInOutput(pub(crate) LoginResult);

impl InterfaceContract for PublicSignInOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[(
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
            )]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[(
                "actor",
                mp::object_value(&[
                    (
                        "user_id",
                        serde_json::Value::String((&(&(&(self).0).actor).user_id).to_string()),
                    ),
                    (
                        "tenant_id",
                        serde_json::Value::String((&(&(&(self).0).actor).tenant_id).to_string()),
                    ),
                    (
                        "current_workspace_id",
                        serde_json::Value::String(
                            (&(&(&(self).0).actor).current_workspace_id).to_string(),
                        ),
                    ),
                    (
                        "effective_display_role",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(&(&(self).0).actor).effective_display_role).len()),
                        )]),
                    ),
                    (
                        "is_root",
                        serde_json::Value::Bool(*(&(&(&(self).0).actor).is_root)),
                    ),
                    (
                        "permissions",
                        mp::object_value(&[(
                            "item_count",
                            serde_json::json!((&(&(&(self).0).actor).permissions).len()),
                        )]),
                    ),
                ]),
            )]),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-sign-in-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignInTargetError(pub(crate) ApiError);

impl InterfaceContract for PublicSignInTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("PublicSignInTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("PublicSignInTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-sign-in-error";
    const CONTRACT_VERSION: &'static str = "1";
}

type PublicSignInFuture<'a> =
    Pin<Box<dyn Future<Output = Result<PublicSignInOutput, PublicSignInTargetError>> + Send + 'a>>;

pub(crate) trait PublicSignInPort: Send + Sync + 'static {
    fn sign_in(&self, input: PublicSignInInput) -> PublicSignInFuture<'_>;
}

struct PublicSignInAdapter {
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    session_ttl_days: i64,
    authenticator_registry: Arc<AuthenticatorRegistry>,
}

impl PublicSignInPort for PublicSignInAdapter {
    fn sign_in(&self, input: PublicSignInInput) -> PublicSignInFuture<'_> {
        let store = self.store.clone();
        let session_store = Arc::clone(&self.session_store);
        let session_ttl_days = self.session_ttl_days;
        let registry = Arc::clone(&self.authenticator_registry);
        Box::pin(async move {
            AuthKernel::with_registry(
                store,
                SessionIssuer::new(session_store, session_ttl_days),
                registry.as_ref().clone(),
            )
            .login(input.0)
            .await
            .map(PublicSignInOutput)
            .map_err(ApiError::from)
            .map_err(PublicSignInTargetError)
        })
    }
}

struct PublicSignInHandler {
    port: Arc<dyn PublicSignInPort>,
}

impl
    InterfaceHandler<
        PublicSignInInput,
        PublicSignInOutput,
        PublicSignInTargetError,
        PublicPrincipal,
    > for PublicSignInHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicSignInInput,
    ) -> InterfaceHandlerFuture<PublicSignInOutput, PublicSignInTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.sign_in(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("public_sign_in", error))
        })
    }
}

pub(crate) struct PublicSignInAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for PublicSignInAuthorization {
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
    port: Arc<dyn PublicSignInPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<PublicSignInInput>(),
        contract::<PublicSignInOutput>(),
        contract::<PublicSignInTargetError>(),
    );
    let operation = AuthorizationOperation::new(OPERATION).expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.public-auth").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-sign-in-v1").expect("static graph is valid"),
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
            HandlerReference::new(HANDLER).expect("static handler is valid"),
            TargetReference::new("control-plane.auth-kernel.login")
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
            BindingId::new(BINDING_ID).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("POST", "/api/public/auth/sign-in")
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
    compiler.bind_handler::<PublicSignInInput, PublicSignInOutput, PublicSignInTargetError, PublicPrincipal>(
        &interface_id,
        HandlerReference::new(HANDLER).expect("static handler is valid"),
        Arc::new(PublicSignInHandler {
            port,
        }),
    )?;
    compiler.compile()
}

pub(crate) fn public_sign_in_port(
    store: MainDurableStore,
    session_store: Arc<dyn SessionStore>,
    session_ttl_days: i64,
    authenticator_registry: Arc<AuthenticatorRegistry>,
) -> Arc<dyn PublicSignInPort> {
    Arc::new(PublicSignInAdapter {
        store,
        session_store,
        session_ttl_days,
        authenticator_registry,
    })
}

#[cfg(test)]
pub(crate) fn compile_registry_for_test(
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    compile_registry(Arc::new(UnavailablePublicSignInPort))
}

#[cfg(test)]
struct UnavailablePublicSignInPort;

#[cfg(test)]
impl PublicSignInPort for UnavailablePublicSignInPort {
    fn sign_in(&self, _input: PublicSignInInput) -> PublicSignInFuture<'_> {
        Box::pin(async {
            Err(PublicSignInTargetError(
                anyhow::anyhow!("test public sign-in port is unavailable").into(),
            ))
        })
    }
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
