use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    model_catalog::{extract_agent_model_catalog_from_start_node, AgentModelDescriptor},
    publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
};
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, ContractIdentity, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceIdentity,
    InterfaceLifecycle, InterfaceOwner, InterfaceProtocol, InterfaceScope, InterfaceTargetFailure,
    InterfaceVersion, InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};
use storage_durable_postgres::MainDurableStore;

use super::{
    invocation_error, register_authentication, CompatibilityBlockingAuthorization,
    CompatibilityBlockingTargetError, AUTHENTICATION_ADAPTER, AUTHORIZATION_ADAPTER,
    OPENAI_CHAT_MODELS_BINDING_ID, OPENAI_MODELS_BINDING_ID, OPENAI_MODELS_ROOT_BINDING_ID,
};
use crate::{
    app_state::ApiState,
    extension_bus::ApplicationApiKeyAuthenticationCredential,
    routes::application_public_api::native::{self, NativeApiError},
};

pub(crate) struct CompatibilityModelsInput;

impl InterfaceContract for CompatibilityModelsInput {
    const CONTRACT_ID: &'static str = "application-compatibility-models-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct CompatibilityModelsOutput {
    pub(crate) models: Vec<AgentModelDescriptor>,
    pub(crate) publication_created_at: i64,
}

impl InterfaceContract for CompatibilityModelsOutput {
    const CONTRACT_ID: &'static str = "application-compatibility-models-output";
    const CONTRACT_VERSION: &'static str = "1";
}

type CompatibilityModelsFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<CompatibilityModelsOutput, CompatibilityBlockingTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait CompatibilityModelsPort: Send + Sync + 'static {
    fn list<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: CompatibilityModelsInput,
    ) -> CompatibilityModelsFuture<'a>;
}

struct CompatibilityModelsAdapter {
    store: MainDurableStore,
}

impl CompatibilityModelsPort for CompatibilityModelsAdapter {
    fn list<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        _input: CompatibilityModelsInput,
    ) -> CompatibilityModelsFuture<'a> {
        Box::pin(async move {
            let publication = ApplicationPublicationService::new(self.store.clone())
                .load_active_publication(LoadActiveApplicationPublicationCommand {
                    application_id: principal.application_id(),
                })
                .await
                .map_err(|_| {
                    native::native_error(
                        control_plane::application_public_api::native::NativeRunValidationError::ApplicationNotPublished,
                    )
                })
                .map_err(CompatibilityBlockingTargetError)?;
            Ok(CompatibilityModelsOutput {
                models: extract_agent_model_catalog_from_start_node(&publication.document_snapshot),
                publication_created_at: publication.created_at.unix_timestamp(),
            })
        })
    }
}

pub(crate) fn compatibility_models_port(
    store: MainDurableStore,
) -> Arc<dyn CompatibilityModelsPort> {
    Arc::new(CompatibilityModelsAdapter { store })
}

struct CompatibilityModelsHandler {
    port: Arc<dyn CompatibilityModelsPort>,
}

impl
    InterfaceHandler<
        CompatibilityModelsInput,
        CompatibilityModelsOutput,
        CompatibilityBlockingTargetError,
        ApplicationPrincipal,
    > for CompatibilityModelsHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: CompatibilityModelsInput,
    ) -> InterfaceHandlerFuture<CompatibilityModelsOutput, CompatibilityBlockingTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.list(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("compatibility_models", error))
        })
    }
}

pub(super) fn register_bindings(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
    port: Arc<dyn CompatibilityModelsPort>,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    for (interface_id, binding_id, handler, path) in [
        (
            "compat.openai.models-root.list",
            OPENAI_MODELS_ROOT_BINDING_ID,
            "api-server.compat.openai-models-root.list",
            "/models",
        ),
        (
            "compat.openai.models.list",
            OPENAI_MODELS_BINDING_ID,
            "api-server.compat.openai-models.list",
            "/v1/models",
        ),
        (
            "compat.openai.chat-completions.models.list",
            OPENAI_CHAT_MODELS_BINDING_ID,
            "api-server.compat.openai-chat-models.list",
            "/v1/chat/completions/models",
        ),
    ] {
        register_binding(
            compiler,
            owner,
            operation,
            Arc::clone(&port),
            interface_id,
            binding_id,
            handler,
            path,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn register_binding(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
    port: Arc<dyn CompatibilityModelsPort>,
    interface_id: &str,
    binding_id: &str,
    handler: &str,
    path: &str,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(interface_id).expect("static interface id is valid");
    let contracts = InterfaceContracts::unary(
        contract::<CompatibilityModelsInput>(),
        contract::<CompatibilityModelsOutput>(),
        contract::<CompatibilityBlockingTargetError>(),
    );
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let handler = interface_runtime::HandlerReference::new(handler)
        .expect("static compatibility models handler is valid");
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation.clone(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            handler.clone(),
            TargetReference::new("control-plane.application-publication.load-active")
                .expect("static target is valid"),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    register_authentication(compiler, &interface_id)?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("GET", path).expect("static route is valid"),
            ),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<CompatibilityModelsInput, CompatibilityModelsOutput, CompatibilityBlockingTargetError, ApplicationPrincipal>(
        &interface_id,
        handler,
        Arc::new(CompatibilityModelsHandler { port }),
    )?;
    Ok(())
}

pub(crate) async fn invoke_models(
    state: Arc<ApiState>,
    binding_id: &'static str,
    bearer_token: String,
) -> Result<CompatibilityModelsOutput, NativeApiError> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or_else(|| {
        NativeApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "interface_registry_unavailable",
            "compatibility models interface is unavailable",
        )
    })?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or_else(|| {
            NativeApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "interface_registry_unavailable",
                "compatibility models interface is unavailable",
            )
        })?;
    let binding_id = BindingId::new(binding_id).expect("static binding id is valid");
    let activated = snapshot.authentication(&binding_id).ok_or_else(|| {
        NativeApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "authentication_activation_unavailable",
            "compatibility models authentication activation is unavailable",
        )
    })?;
    let principal: ApplicationPrincipal = boot_snapshot
        .authenticate(
            activated,
            ApplicationApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                bearer_token,
            },
        )
        .await
        .map_err(|_| {
            native::native_error(
                control_plane::application_public_api::native::NativeRunValidationError::NotAuthenticated,
            )
        })?;
    let authentication_activation = activated.activation().clone();
    let outcome = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        CompatibilityBlockingAuthorization,
    ))
    .invoke::<CompatibilityModelsInput, CompatibilityModelsOutput, CompatibilityBlockingTargetError>(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static adapter is valid"),
            authentication_activation,
            principal,
            None,
            CompatibilityModelsInput,
        ),
    )
    .await
    .map_err(|failure| invocation_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(outcome.into_value())
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static compatibility models contract is valid")
}

#[cfg(test)]
struct UnavailableCompatibilityModelsPort;

#[cfg(test)]
impl CompatibilityModelsPort for UnavailableCompatibilityModelsPort {
    fn list<'a>(
        &'a self,
        _principal: &'a ApplicationPrincipal,
        _input: CompatibilityModelsInput,
    ) -> CompatibilityModelsFuture<'a> {
        Box::pin(async {
            Err(CompatibilityBlockingTargetError(NativeApiError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "test_port_unavailable",
                "test compatibility models port is unavailable",
            )))
        })
    }
}

#[cfg(test)]
pub(super) fn unavailable_port() -> Arc<dyn CompatibilityModelsPort> {
    Arc::new(UnavailableCompatibilityModelsPort)
}
