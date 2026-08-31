use std::{future::Future, pin::Pin, sync::Arc};

use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract,
    InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution,
    InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceProtocol,
    InterfaceScope, InterfaceStreamHandler, InterfaceStreamHandlerFuture, InterfaceTargetFailure,
    InterfaceVersion, InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
    UserPrincipal,
};

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(Clone)]
pub(crate) struct ConsoleLocaleHints {
    explicit_header_locale: Option<String>,
    accept_language: Option<String>,
}

impl ConsoleLocaleHints {
    pub(crate) fn from_headers(headers: &axum::http::HeaderMap) -> Self {
        Self {
            explicit_header_locale: headers
                .get("x-1flowbase-locale")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            accept_language: headers
                .get(axum::http::header::ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        }
    }

    pub(crate) fn resolve(&self, preferred_locale: Option<String>) -> domain::CatalogLocale {
        self.resolve_with_query(None, preferred_locale)
    }

    pub(crate) fn resolve_with_query(
        &self,
        query_locale: Option<String>,
        preferred_locale: Option<String>,
    ) -> domain::CatalogLocale {
        let resolved = runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
            query_locale,
            explicit_header_locale: self.explicit_header_locale.clone(),
            user_preferred_locale: preferred_locale,
            accept_language: self.accept_language.clone(),
            fallback_locale: runtime_profile::FALLBACK_LOCALE,
            supported_locales: runtime_profile::SUPPORTED_LOCALES
                .iter()
                .map(|value| value.to_string())
                .collect(),
        });
        domain::CatalogLocale::new(resolved.resolved_locale)
            .expect("runtime profile must resolve a supported catalog locale")
    }
}

const AUTHENTICATION_ADAPTER: &str = "api-server.console.require-session";
const AUTHENTICATION_ACTIVATION: &str = "api-server.console.require-session.activation.v1";
const AUTHORIZATION_ADAPTER: &str = "api-server.console.compiled-operation";

pub(crate) struct ConsoleInterfaceDeclaration {
    pub(crate) interface_id: &'static str,
    pub(crate) binding_id: &'static str,
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) mutating: bool,
}

pub(crate) struct ConsoleInterfaceTargetError(pub(crate) ApiError);

impl InterfaceContract for ConsoleInterfaceTargetError {
    const CONTRACT_ID: &'static str = "console-interface-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type ConsoleInterfaceFuture<'a, O> =
    Pin<Box<dyn Future<Output = Result<O, ConsoleInterfaceTargetError>> + Send + 'a>>;

pub(crate) trait ConsoleInterfacePort<I, O>: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: I,
    ) -> ConsoleInterfaceFuture<'a, O>;
}

pub(crate) type ConsoleServerStreamFuture<S, O> =
    InterfaceStreamHandlerFuture<S, O, ConsoleInterfaceTargetError>;

pub(crate) trait ConsoleServerStreamPort<I, S, O>: Send + Sync + 'static
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
{
    fn execute_stream(
        &self,
        principal: &UserPrincipal,
        input: I,
    ) -> ConsoleServerStreamFuture<S, O>;
}

struct ConsoleInterfaceHandler<I, O> {
    port: Arc<dyn ConsoleInterfacePort<I, O>>,
}

struct ConsoleServerStreamHandler<I, S, O> {
    port: Arc<dyn ConsoleServerStreamPort<I, S, O>>,
}

impl<I, S, O> InterfaceStreamHandler<I, S, O, ConsoleInterfaceTargetError, UserPrincipal>
    for ConsoleServerStreamHandler<I, S, O>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
{
    fn invoke_stream(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: I,
    ) -> InterfaceStreamHandlerFuture<S, O, ConsoleInterfaceTargetError> {
        self.port.execute_stream(context.principal(), input)
    }
}

impl<I, O> InterfaceHandler<I, O, ConsoleInterfaceTargetError, UserPrincipal>
    for ConsoleInterfaceHandler<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: I,
    ) -> InterfaceHandlerFuture<O, ConsoleInterfaceTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("console_interface", error))
        })
    }
}

pub(crate) fn compile_registry<I, O>(
    owner: &'static str,
    graph: &'static str,
    declarations: &'static [ConsoleInterfaceDeclaration],
    port: Arc<dyn ConsoleInterfacePort<I, O>>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    let owner = InterfaceOwner::new(owner).expect("static Console family owner is valid");
    let operations = declarations
        .iter()
        .map(|declaration| AuthorizationOperation::new(declaration.interface_id))
        .collect::<Result<Vec<_>, _>>()
        .expect("static Console family operations are valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new(graph).expect("static Console family graph is valid"),
        operations.clone(),
        [owner.clone()],
    );
    for (declaration, operation) in declarations.iter().zip(operations) {
        let interface_id = InterfaceId::new(declaration.interface_id)
            .expect("static Console family interface is valid");
        let identity = InterfaceIdentity::new(
            interface_id.clone(),
            InterfaceVersion::new("1").expect("static Console family version is valid"),
        );
        let contracts = InterfaceContracts::unary(
            contract::<I>(),
            contract::<O>(),
            contract::<ConsoleInterfaceTargetError>(),
        );
        let handler = HandlerReference::new(format!("{}.handler", declaration.interface_id))
            .expect("static Console family handler is valid");
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
                handler.clone(),
                TargetReference::new(format!("control-plane.{}", declaration.interface_id))
                    .expect("static Console family target is valid"),
            ),
            if declaration.mutating {
                InterfaceAuditPolicy::Mutating
            } else {
                InterfaceAuditPolicy::ReadOnly
            },
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        register_authentication(&mut compiler, &interface_id)?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(declaration.binding_id)
                    .expect("static Console family binding is valid"),
                identity,
                contracts,
                ProtocolProjection::http(
                    RouteIdentity::new(declaration.method, declaration.path)
                        .expect("static Console family route is valid"),
                ),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                    .expect("static Console authentication adapter is valid"),
                AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                    .expect("static Console authorization adapter is valid"),
                None,
            ),
        )?;
        compiler.bind_handler::<I, O, ConsoleInterfaceTargetError, UserPrincipal>(
            &interface_id,
            handler,
            Arc::new(ConsoleInterfaceHandler {
                port: Arc::clone(&port),
            }),
        )?;
    }
    compiler.compile()
}

pub(crate) fn compile_server_stream_registry<I, S, O>(
    owner: &'static str,
    graph: &'static str,
    declarations: &'static [ConsoleInterfaceDeclaration],
    port: Arc<dyn ConsoleServerStreamPort<I, S, O>>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
{
    let owner = InterfaceOwner::new(owner).expect("static Console family owner is valid");
    let operations = declarations
        .iter()
        .map(|declaration| AuthorizationOperation::new(declaration.interface_id))
        .collect::<Result<Vec<_>, _>>()
        .expect("static Console family operations are valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new(graph).expect("static Console family graph is valid"),
        operations.clone(),
        [owner.clone()],
    );
    for (declaration, operation) in declarations.iter().zip(operations) {
        let interface_id = InterfaceId::new(declaration.interface_id)
            .expect("static Console family interface is valid");
        let identity = InterfaceIdentity::new(
            interface_id.clone(),
            InterfaceVersion::new("1").expect("static Console family version is valid"),
        );
        let contracts = InterfaceContracts::server_stream(
            contract::<I>(),
            contract::<S>(),
            contract::<O>(),
            contract::<ConsoleInterfaceTargetError>(),
        );
        let handler = HandlerReference::new(format!("{}.handler", declaration.interface_id))
            .expect("static Console family handler is valid");
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
                InterfaceExecutionMode::ServerStream,
                handler.clone(),
                TargetReference::new(format!("control-plane.{}", declaration.interface_id))
                    .expect("static Console family target is valid"),
            ),
            if declaration.mutating {
                InterfaceAuditPolicy::Mutating
            } else {
                InterfaceAuditPolicy::ReadOnly
            },
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        register_authentication(&mut compiler, &interface_id)?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(declaration.binding_id)
                    .expect("static Console family binding is valid"),
                identity,
                contracts,
                ProtocolProjection::http(
                    RouteIdentity::new(declaration.method, declaration.path)
                        .expect("static Console family route is valid"),
                ),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                    .expect("static Console authentication adapter is valid"),
                AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                    .expect("static Console authorization adapter is valid"),
                None,
            ),
        )?;
        compiler.bind_stream_handler::<I, S, O, ConsoleInterfaceTargetError, UserPrincipal>(
            &interface_id,
            handler,
            Arc::new(ConsoleServerStreamHandler {
                port: Arc::clone(&port),
            }),
        )?;
    }
    compiler.compile()
}

fn register_authentication(
    compiler: &mut RegistryCompiler,
    interface_id: &InterfaceId,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    compiler.register_authentication_adapter(
        interface_id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static Console authentication plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("Console authentication registration is valid"),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.console-authentication")
                .expect("static Console authentication plugin is valid"),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static Console authentication adapter is valid"),
            interface_runtime::AuthenticationActivationIdentity::new(AUTHENTICATION_ACTIVATION)
                .expect("static Console authentication activation is valid"),
            interface_runtime::PrincipalProfile::User,
        ),
    )
}

pub(crate) async fn invoke<I, O>(
    state: Arc<ApiState>,
    binding_id: &'static str,
    credential: crate::extension_bus::ConsoleAuthenticationCredential,
    input: I,
) -> Result<O, ApiError>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("interface_operation"),
    )?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "interface_operation",
        ))?;
    let binding_id = BindingId::new(binding_id).expect("static Console binding is valid");
    let activated = snapshot.authentication(&binding_id).cloned().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("authentication_activation"),
    )?;
    let principal: UserPrincipal = boot_snapshot
        .authenticate(&activated, credential)
        .await
        .map_err(ApiError::from)?;
    let kernel = crate::routes::host_infrastructure::interface_operation::invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    );
    match kernel
        .invoke::<I, O, ConsoleInterfaceTargetError>(
            snapshot,
            InvocationEnvelope::with_principal(
                InvocationLineage::root(InvocationId::now_v7()),
                binding_id,
                InterfaceProtocol::Http,
                activated.adapter().clone(),
                activated.activation().clone(),
                principal,
                None,
                input,
            ),
        )
        .await
    {
        Ok(outcome) => {
            let _receipt = outcome.receipt().clone().projected();
            Ok(outcome.into_value())
        }
        Err(failure) => match failure.into_error() {
            interface_runtime::InterfaceInvocationError::TargetFailed(error) => Err(error
                .into_source::<ConsoleInterfaceTargetError>()
                .map(|error| error.0)
                .unwrap_or_else(|| anyhow::anyhow!("Console target contract mismatch").into())),
            interface_runtime::InterfaceInvocationError::AuthorizationRejected(error) => Err(error
                .into_source::<ApiError>()
                .unwrap_or_else(|| anyhow::anyhow!("Console authorization failed").into())),
            _ => Err(anyhow::anyhow!("Console interface invocation failed").into()),
        },
    }
}

/// Invokes a unary Console interface with an already authenticated principal.
/// The binding is still resolved from the frozen boot snapshot and dispatched
/// through the same Console authorization/admission/hook kernel as HTTP.
pub(crate) async fn invoke_with_principal<I, O>(
    state: Arc<ApiState>,
    binding_id: &'static str,
    principal: UserPrincipal,
    input: I,
) -> Result<O, ApiError>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    let snapshot = frozen_snapshot(&state)?;
    let binding_id = BindingId::new(binding_id).expect("static Console binding is valid");
    let activated = activated_authentication(&snapshot, &binding_id)?;
    let kernel = console_invocation_kernel(&state);
    match kernel
        .invoke::<I, O, ConsoleInterfaceTargetError>(
            snapshot,
            InvocationEnvelope::with_principal(
                InvocationLineage::root(InvocationId::now_v7()),
                binding_id,
                InterfaceProtocol::Http,
                activated.adapter().clone(),
                activated.activation().clone(),
                principal,
                None,
                input,
            ),
        )
        .await
    {
        Ok(outcome) => Ok(outcome.into_value()),
        Err(failure) => Err(console_invocation_error(failure.into_error())),
    }
}

pub(crate) async fn invoke_server_stream_with_principal<I, S, O>(
    state: Arc<ApiState>,
    binding_id: &'static str,
    principal: UserPrincipal,
    input: I,
) -> Result<interface_runtime::InterfaceStreamInvocation<S, O, ConsoleInterfaceTargetError>, ApiError>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
{
    let snapshot = frozen_snapshot(&state)?;
    let binding_id = BindingId::new(binding_id).expect("static Console binding is valid");
    let activated = activated_authentication(&snapshot, &binding_id)?;
    let plan = snapshot
        .plan(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("Console binding is unavailable"))?;
    let target = interface_runtime::ExecutionTargetPin::BuiltIn {
        handler: plan.definition().handler_reference().clone(),
        target: plan.definition().target_reference().clone(),
    };
    console_invocation_kernel(&state)
        .invoke_server_stream_with_dispatch_target::<I, S, O, ConsoleInterfaceTargetError>(
            snapshot,
            InvocationEnvelope::with_principal(
                InvocationLineage::root(InvocationId::now_v7()),
                binding_id,
                InterfaceProtocol::Http,
                activated.adapter().clone(),
                activated.activation().clone(),
                principal,
                None,
                input,
            ),
            target,
        )
        .await
        .map_err(|failure| console_invocation_error(failure.into_error()))
}

pub(crate) async fn invoke_server_stream<I, S, O>(
    state: Arc<ApiState>,
    binding_id: &'static str,
    credential: crate::extension_bus::ConsoleAuthenticationCredential,
    input: I,
) -> Result<interface_runtime::InterfaceStreamInvocation<S, O, ConsoleInterfaceTargetError>, ApiError>
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
{
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("interface_operation"),
    )?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "interface_operation",
        ))?;
    let binding_id = BindingId::new(binding_id).expect("static Console binding is valid");
    let activated = snapshot.authentication(&binding_id).cloned().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("authentication_activation"),
    )?;
    let principal: UserPrincipal = boot_snapshot
        .authenticate(&activated, credential)
        .await
        .map_err(ApiError::from)?;
    let plan = snapshot
        .plan(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("Console binding is unavailable"))?;
    let target = interface_runtime::ExecutionTargetPin::BuiltIn {
        handler: plan.definition().handler_reference().clone(),
        target: plan.definition().target_reference().clone(),
    };
    console_invocation_kernel(&state)
        .invoke_server_stream_with_dispatch_target::<I, S, O, ConsoleInterfaceTargetError>(
            snapshot,
            InvocationEnvelope::with_principal(
                InvocationLineage::root(InvocationId::now_v7()),
                binding_id,
                InterfaceProtocol::Http,
                activated.adapter().clone(),
                activated.activation().clone(),
                principal,
                None,
                input,
            ),
            target,
        )
        .await
        .map_err(|failure| console_invocation_error(failure.into_error()))
}

fn frozen_snapshot(state: &ApiState) -> Result<Arc<CompiledInterfaceRegistry>, ApiError> {
    state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|boot| boot.interface_registry())
        .map(|registry| registry.snapshot())
        .ok_or_else(|| anyhow::anyhow!("Console interface registry is unavailable").into())
}

fn activated_authentication(
    snapshot: &CompiledInterfaceRegistry,
    binding_id: &BindingId,
) -> Result<interface_runtime::ActivatedAuthenticationAdapter, ApiError> {
    snapshot
        .authentication(binding_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Console authentication activation is unavailable").into())
}

fn console_invocation_kernel(
    state: &ApiState,
) -> Arc<interface_runtime::InterfaceInvocationKernel<UserPrincipal>> {
    crate::routes::host_infrastructure::interface_operation::invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    )
}

fn console_invocation_error(error: interface_runtime::InterfaceInvocationError) -> ApiError {
    match error {
        interface_runtime::InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<ConsoleInterfaceTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("Console target contract mismatch").into()),
        interface_runtime::InterfaceInvocationError::AuthorizationRejected(error) => error
            .into_source::<ApiError>()
            .unwrap_or_else(|| anyhow::anyhow!("Console authorization failed").into()),
        _ => anyhow::anyhow!("Console interface invocation failed").into(),
    }
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static Console family contract is valid")
}
