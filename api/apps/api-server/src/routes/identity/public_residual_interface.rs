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

    const CONTRACT_ID: &'static str = "public-auth-providers-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicProvidersOutput(pub(crate) Vec<AuthProviderResponse>);

impl InterfaceContract for PublicProvidersOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("auth_type",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("0", {
            if (&(self).0).len() > 32 {
                return None;
            }
            serde_json::Value::Array(
                (&(self).0)
                    .iter()
                    .map(|item| {
                        Some(mp::object_value(&[
                            ("id", serde_json::Value::String((&(item).id).to_string())),
                            ("auth_type", mp::text(&(item).auth_type)?),
                            (
                                "title",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).title).len()),
                                )]),
                            ),
                        ]))
                    })
                    .collect::<Option<Vec<_>>>()?,
            )
        })]))
    }

    const CONTRACT_ID: &'static str = "public-auth-providers-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignUpInput(pub(crate) SignUpCommand);

impl InterfaceContract for PublicSignUpInput {
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

    const CONTRACT_ID: &'static str = "public-sign-up-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicSignUpOutput(pub(crate) LoginResult);

impl InterfaceContract for PublicSignUpOutput {
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

    const CONTRACT_ID: &'static str = "public-sign-up-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct PublicResidualTargetError(pub(crate) ApiError);

impl InterfaceContract for PublicResidualTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("PublicResidualTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("PublicResidualTargetError".to_owned()),
        )]))
    }

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
