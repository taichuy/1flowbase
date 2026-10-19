//! Canonical business bindings. OAuth credentials terminate in `verify_callback`.
use super::transaction::{self, BindingIntent};
use crate::{app_state::ApiState, error_response::ApiError};
use control_plane::{
    auth::{AuthKernel, CompleteExternalLoginCommand, LoginResult, SessionIssuer},
    ports::SessionStore,
};
use domain::LoginEntryRecord;
use interface_runtime::{
    ActivatedAuthenticationAdapter, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract,
    InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution,
    InterfaceExecutionMode, InterfaceExtensionIsolation, InterfaceExtensionPermission,
    InterfaceExtensionPoint, InterfaceExtensionRegistration, InterfaceExtensionTier,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId,
    InterfaceIdentity, InterfaceInvocationError, InterfaceInvocationKernel, InterfaceLifecycle,
    InterfaceOwner, InterfaceProtocol, InterfaceScope, InterfaceTargetFailure, InterfaceVersion,
    InvocationAdapterPlan, PluginIdentity, PrincipalProfile, ProtocolBinding, ProtocolProjection,
    PublicPrincipal, RegistryCompilationError, RegistryCompiler, RouteIdentity, TargetReference,
    UserPrincipal,
};
use std::sync::{Arc, Weak};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

pub(crate) const OWNER: &str = "api-server.external-auth";
pub(crate) const START_BINDING: &str = "http.public.auth.external.start.v1";
pub(crate) const CALLBACK_BINDING: &str = "http.public.auth.external.callback.v1";
pub(crate) const CALLBACK_AUTH_PLUGIN: &str = "api-server.external-callback-authentication";
pub(crate) const CALLBACK_AUTH_ADAPTER: &str = "api-server.external-callback";
pub(crate) const CALLBACK_AUTH_ACTIVATION: &str = "api-server.external-callback.activation.v1";
pub(crate) const AUTHORIZE_BINDING: &str = "http.console.identity.external-authorize.v1";
pub(crate) const OPERATIONS: &[&str] = &[
    "public.auth.external.start",
    "public.auth.external.callback",
    "console.identity.external-authorize",
];

/// Same declarations drive registry bindings and API-owner static documentation.
#[derive(Clone, Copy)]
pub(crate) struct BindingMetadata {
    pub operation: &'static str,
    pub binding_id: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub summary: &'static str,
    pub description: &'static str,
    pub console: bool,
}
const BINDINGS: &[BindingMetadata] = &[
    BindingMetadata {
        operation: OPERATIONS[0],
        binding_id: START_BINDING,
        method: "GET",
        path: "/api/public/auth/external/start",
        summary: "Start external sign-in",
        description: "Start a browser-bound external authentication transaction for an enabled login entry.",
        console: false,
    },
    BindingMetadata {
        operation: OPERATIONS[1],
        binding_id: CALLBACK_BINDING,
        method: "GET",
        path: "/api/public/auth/external/callback",
        summary: "Complete external sign-in",
        description: "Verify the one-time browser transaction and complete sign-in or explicit identity binding.",
        console: false,
    },
    BindingMetadata {
        operation: OPERATIONS[2],
        binding_id: AUTHORIZE_BINDING,
        method: "POST",
        path: "/api/console/me/external-identities/authorize",
        summary: "Authorize an external identity binding",
        description: "Create an external authentication transaction bound to the current cookie session and return its authorization URL.",
        console: true,
    },
];
pub(crate) fn bindings_metadata() -> &'static [BindingMetadata] {
    BINDINGS
}

pub(super) enum PublicInput {
    Start { login_entry_id: Uuid },
    Complete(super::VerifiedCallback),
}
pub(super) enum PublicOutput {
    Authorization(AuthorizeOutput),
    Login(LoginResult),
    Bound,
}
pub(super) struct AuthorizeInput(pub Uuid);
pub(super) struct AuthorizeOutput {
    pub cookies: axum_extra::extract::cookie::CookieJar,
    pub authorization_url: String,
}
pub(super) struct TargetError(pub ApiError);

// Deliberately no managed projection: sealed identity/profile and session capabilities
// must never enter ordinary plugin hooks, receipts, or wire serialization.
macro_rules! contract {
    ($type:ty, $id:literal) => {
        impl InterfaceContract for $type {
            const CONTRACT_ID: &'static str = $id;
            const CONTRACT_VERSION: &'static str = "1";
        }
    };
}
contract!(PublicInput, "external-auth-public-input");
contract!(PublicOutput, "external-auth-public-output");
contract!(AuthorizeInput, "external-auth-authorize-input");
contract!(AuthorizeOutput, "external-auth-authorize-output");
contract!(TargetError, "external-auth-error");

#[derive(Clone)]
struct Adapter {
    store: MainDurableStore,
    sessions: Arc<dyn SessionStore>,
    session_ttl_days: i64,
    state: Weak<ApiState>,
}

impl Adapter {
    async fn entry(&self, id: Uuid) -> Result<LoginEntryRecord, ApiError> {
        let entry = self
            .store
            .find_login_entry(id)
            .await?
            .ok_or_else(transaction::denied)?;
        super::validate_entry(&entry)?;
        Ok(entry)
    }
    async fn public(&self, input: PublicInput) -> Result<PublicOutput, ApiError> {
        match input {
            PublicInput::Start { login_entry_id } => {
                let entry = self.entry(login_entry_id).await?;
                let state = self.state.upgrade().ok_or_else(transaction::denied)?;
                let (cookies, authorization_url) = super::begin(&state, entry, None).await?;
                Ok(PublicOutput::Authorization(AuthorizeOutput {
                    cookies,
                    authorization_url,
                }))
            }
            PublicInput::Complete(verified) => {
                let super::VerifiedCallback {
                    login_entry_id,
                    identity,
                    registration_eligible,
                    binding_actor,
                    authority,
                } = verified;
                let lease = authority.take()?;
                let entry = self.entry(login_entry_id).await?;
                if identity.connection_id() != entry.connection_id {
                    return Err(transaction::denied());
                }
                let kernel = AuthKernel::new(
                    self.store.clone(),
                    SessionIssuer::new(Arc::clone(&self.sessions), self.session_ttl_days),
                );
                let output = if let Some(actor) = binding_actor {
                    kernel.bind_external_identity(&actor, identity).await?;
                    PublicOutput::Bound
                } else {
                    PublicOutput::Login(
                        kernel
                            .complete_external_login(CompleteExternalLoginCommand {
                                login_entry_id,
                                identity,
                                registration_eligible,
                            })
                            .await?,
                    )
                };
                if lease.release().await.is_err() {
                    // Business writes/session issuance already succeeded. This is a
                    // release failure, never evidence that the business rolled back.
                    tracing::error!(
                        stage = "external_auth_authority_release",
                        business_effect_completed = true,
                        "external authentication authority release failed"
                    );
                    return Err(
                        control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                            "external_auth_authority_release",
                        )
                        .into(),
                    );
                }
                Ok(output)
            }
        }
    }
    async fn authorize(
        &self,
        principal: &UserPrincipal,
        input: AuthorizeInput,
    ) -> Result<AuthorizeOutput, ApiError> {
        let session_id = principal
            .authenticated_session()
            .ok_or_else(transaction::denied)?
            .expose_to_trusted_handler();
        let session = self
            .sessions
            .get(session_id)
            .await?
            .ok_or_else(transaction::denied)?;
        let binding = BindingIntent {
            actor_id: principal.actor().user_id,
            session_hash: transaction::digest(session_id),
        };
        if !transaction::binding_matches(
            &binding,
            &session,
            principal.actor(),
            time::OffsetDateTime::now_utc().unix_timestamp(),
        ) {
            return Err(transaction::denied());
        }
        let entry = self.entry(input.0).await?;
        let state = self.state.upgrade().ok_or_else(transaction::denied)?;
        let (cookies, authorization_url) = super::begin(&state, entry, Some(binding)).await?;
        Ok(AuthorizeOutput {
            cookies,
            authorization_url,
        })
    }
}
impl InterfaceHandler<PublicInput, PublicOutput, TargetError, PublicPrincipal> for Adapter {
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: PublicInput,
    ) -> InterfaceHandlerFuture<PublicOutput, TargetError> {
        let adapter = self.clone();
        Box::pin(async move {
            adapter
                .public(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("external_auth", TargetError(error)))
        })
    }
}
impl InterfaceHandler<AuthorizeInput, AuthorizeOutput, TargetError, UserPrincipal> for Adapter {
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: AuthorizeInput,
    ) -> InterfaceHandlerFuture<AuthorizeOutput, TargetError> {
        let adapter = self.clone();
        Box::pin(async move {
            adapter
                .authorize(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("external_auth", TargetError(error)))
        })
    }
}

pub(crate) fn compile_registry(
    state: &Arc<ApiState>,
) -> Result<Arc<CompiledInterfaceRegistry>, RegistryCompilationError> {
    let owner = InterfaceOwner::new(OWNER).expect("static owner");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:external-auth-v1").expect("static graph"),
        OPERATIONS
            .iter()
            .map(|v| AuthorizationOperation::new(*v).expect("static operation")),
        [owner.clone()],
    );
    let adapter = Arc::new(Adapter {
        store: state.store.clone(),
        sessions: Arc::clone(&state.session_store),
        session_ttl_days: state.session_ttl_days,
        state: Arc::downgrade(state),
    });
    for declaration in bindings_metadata() {
        let BindingMetadata {
            operation,
            binding_id: binding,
            method,
            path,
            console,
            ..
        } = *declaration;
        let interface = InterfaceId::new(operation).expect("static interface");
        let identity = InterfaceIdentity::new(
            interface.clone(),
            InterfaceVersion::new("1").expect("static version"),
        );
        let contracts = if console {
            InterfaceContracts::unary(
                contract_identity::<AuthorizeInput>(),
                contract_identity::<AuthorizeOutput>(),
                contract_identity::<TargetError>(),
            )
        } else {
            InterfaceContracts::unary(
                contract_identity::<PublicInput>(),
                contract_identity::<PublicOutput>(),
                contract_identity::<TargetError>(),
            )
        };
        let handler =
            HandlerReference::new(format!("{operation}.handler")).expect("static handler");
        let profile = if console {
            PrincipalProfile::User
        } else {
            PrincipalProfile::Public
        };
        let scope = if console {
            InterfaceScope::Workspace
        } else {
            InterfaceScope::System
        };
        let callback = binding == CALLBACK_BINDING;
        let auth = if console {
            "api-server.console.require-session"
        } else if callback {
            CALLBACK_AUTH_ADAPTER
        } else {
            "api-server.public"
        };
        let authorization = if console {
            "api-server.console.compiled-operation"
        } else {
            "api-server.public"
        };
        let plugin = if console {
            "api-server.console-authentication"
        } else if callback {
            CALLBACK_AUTH_PLUGIN
        } else {
            "api-server.public-authentication"
        };
        let activation = if console {
            "api-server.console.require-session.activation.v1"
        } else if callback {
            CALLBACK_AUTH_ACTIVATION
        } else {
            "api-server.public.activation.v1"
        };
        compiler.register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                profile,
                if console {
                    InterfaceAuthenticationPolicy::Authenticated
                } else {
                    InterfaceAuthenticationPolicy::Anonymous
                },
                AuthorizationOperation::new(operation).expect("static operation"),
                scope,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new(format!("control-plane.{operation}")).expect("static target"),
            ),
            InterfaceAuditPolicy::Mutating,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        compiler.register_authentication_adapter(
            &interface,
            1,
            InterfaceExtensionRegistration::new(
                PluginIdentity::new(plugin).expect("static plugin"),
                InterfaceExtensionTier::BuiltIn,
                InterfaceExtensionPoint::AuthenticationAdapter,
                InterfaceExtensionPermission::Authenticate,
                scope,
                InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .expect("static authentication"),
            ActivatedAuthenticationAdapter::new(
                PluginIdentity::new(plugin).expect("static plugin"),
                InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new(auth).expect("static adapter"),
                AuthenticationActivationIdentity::new(activation).expect("static activation"),
                profile,
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
                AuthenticationAdapterReference::new(auth).expect("static adapter"),
                AuthorizationAdapterReference::new(authorization).expect("static authorization"),
                None,
            ),
        )?;
        if console {
            compiler.bind_handler::<AuthorizeInput, AuthorizeOutput, TargetError, UserPrincipal>(
                &interface,
                handler,
                adapter.clone(),
            )?;
        } else {
            compiler.bind_handler::<PublicInput, PublicOutput, TargetError, PublicPrincipal>(
                &interface,
                handler,
                adapter.clone(),
            )?;
        }
    }
    compiler.compile()
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).expect("static contract")
}

fn target_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<TargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| transaction::denied()),
        InterfaceInvocationError::AuthorizationRejected(error) => error
            .into_source::<ApiError>()
            .unwrap_or_else(|| transaction::denied()),
        _ => transaction::denied(),
    }
}

pub(super) async fn invoke_public(
    state: &Arc<ApiState>,
    binding: &str,
    input: PublicInput,
) -> Result<PublicOutput, ApiError> {
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(transaction::denied)?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(transaction::denied)?
        .snapshot();
    let binding = BindingId::new(binding).expect("static binding");
    let authenticated = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            Arc::clone(&snapshot),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let outcome = InterfaceInvocationKernel::new(Arc::new(
        super::super::public_residual_interface::PublicResidualAuthorization,
    ))
    .invoke::<PublicInput, PublicOutput, TargetError>(snapshot, authenticated.into_envelope(input))
    .await
    .map_err(|failure| target_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value())
}

pub(super) async fn invoke_callback(
    state: &Arc<ApiState>,
    headers: axum::http::HeaderMap,
    jar: axum_extra::extract::cookie::CookieJar,
    raw_query: Option<String>,
) -> (Result<PublicOutput, ApiError>, Option<String>) {
    let admission = super::CallbackAdmission::default();
    let result = async {
        let boot = state
            .extension_boot_snapshot
            .as_ref()
            .ok_or_else(transaction::denied)?;
        let snapshot = boot
            .interface_registry()
            .ok_or_else(transaction::denied)?
            .snapshot();
        let binding = BindingId::new(CALLBACK_BINDING).expect("static callback binding");
        let authenticated = boot
            .authenticate_invocation::<_, PublicPrincipal>(
                Arc::clone(&snapshot),
                &binding,
                InterfaceProtocol::Http,
                super::ExternalCallbackCredential {
                    state: Arc::clone(state),
                    headers,
                    jar,
                    raw_query,
                    admission: admission.clone(),
                },
            )
            .await?;
        let verified = admission.take()?;
        let outcome = InterfaceInvocationKernel::new(Arc::new(
            super::super::public_residual_interface::PublicResidualAuthorization,
        ))
        .invoke::<PublicInput, PublicOutput, TargetError>(
            snapshot,
            authenticated.into_envelope(PublicInput::Complete(verified)),
        )
        .await
        .map_err(|failure| target_error(failure.into_error()))?;
        let _receipt = outcome.receipt().clone().projected();
        Ok(outcome.into_value())
    }
    .await;
    (result, admission.cookie_name())
}

pub(super) async fn invoke_authorize(
    state: &Arc<ApiState>,
    headers: axum::http::HeaderMap,
    id: Uuid,
) -> Result<AuthorizeOutput, ApiError> {
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(transaction::denied)?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(transaction::denied)?
        .snapshot();
    let binding = BindingId::new(AUTHORIZE_BINDING).expect("static binding");
    let authenticated = boot
        .authenticate_invocation::<_, UserPrincipal>(
            Arc::clone(&snapshot),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state: Arc::clone(state),
                headers,
            },
        )
        .await?;
    let kernel = crate::routes::host_infrastructure::interface_operation::invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    );
    let outcome = kernel
        .invoke::<AuthorizeInput, AuthorizeOutput, TargetError>(
            snapshot,
            authenticated.into_envelope(AuthorizeInput(id)),
        )
        .await
        .map_err(|failure| target_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value())
}
