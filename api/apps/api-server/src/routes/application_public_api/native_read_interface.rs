use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    model_catalog::extract_agent_model_catalog_from_start_node,
    native::{ApplicationNativeRunService, NativeRunValidationError},
    publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
};
use control_plane::ports::CacheStore;
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    GraphFingerprint, HandlerReference, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceIdentity,
    InterfaceLifecycle, InterfaceOwner, InterfaceScope, InterfaceTargetFailure, InterfaceVersion,
    InvocationAdapterPlan, ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity,
    TargetReference,
};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::native::{
    application_actor_from_principal, native_error, NativeApiError, NativeModelListResponse,
    NativeModelObject, NativeRunResponse,
};

pub(crate) const MODELS_BINDING_ID: &str = "http.application.native.models.list.v1";
pub(crate) const GET_RUN_BINDING_ID: &str = "http.application.native.runs.get.v1";

pub(crate) struct NativeModelsInput;
impl InterfaceContract for NativeModelsInput {
    const CONTRACT_ID: &'static str = "application-native-models-list-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeModelsOutput(pub(crate) NativeModelListResponse);
impl InterfaceContract for NativeModelsOutput {
    const CONTRACT_ID: &'static str = "application-native-models-list-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeGetRunInput(pub(crate) Uuid);
impl InterfaceContract for NativeGetRunInput {
    const CONTRACT_ID: &'static str = "application-native-run-get-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeGetRunOutput(pub(crate) NativeRunResponse);
impl InterfaceContract for NativeGetRunOutput {
    const CONTRACT_ID: &'static str = "application-native-run-get-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeReadTargetError(pub(crate) NativeApiError);
impl InterfaceContract for NativeReadTargetError {
    const CONTRACT_ID: &'static str = "application-native-read-error";
    const CONTRACT_VERSION: &'static str = "1";
}

type ReadFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, NativeReadTargetError>> + Send + 'a>>;

pub(crate) trait NativeReadPort: Send + Sync + 'static {
    fn list_models<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeModelsInput,
    ) -> ReadFuture<'a, NativeModelsOutput>;

    fn get_run<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeGetRunInput,
    ) -> ReadFuture<'a, NativeGetRunOutput>;
}

struct NativeReadAdapter {
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
}

impl NativeReadPort for NativeReadAdapter {
    fn list_models<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        _input: NativeModelsInput,
    ) -> ReadFuture<'a, NativeModelsOutput> {
        Box::pin(async move {
            let publication = ApplicationPublicationService::new(self.store.clone())
                .load_active_publication(LoadActiveApplicationPublicationCommand {
                    application_id: principal.application_id(),
                })
                .await
                .map_err(|_| native_error(NativeRunValidationError::ApplicationNotPublished))
                .map_err(NativeReadTargetError)?;
            let data = extract_agent_model_catalog_from_start_node(&publication.document_snapshot)
                .into_iter()
                .map(NativeModelObject::from)
                .collect();
            Ok(NativeModelsOutput(NativeModelListResponse {
                object: "list",
                data,
            }))
        })
    }

    fn get_run<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeGetRunInput,
    ) -> ReadFuture<'a, NativeGetRunOutput> {
        Box::pin(async move {
            ApplicationNativeRunService::new(self.store.clone())
                .with_last_used_cache(Arc::clone(&self.cache_store))
                .get_native_run_for_actor(application_actor_from_principal(principal), input.0)
                .await
                .map(super::native::to_native_run_response)
                .map(NativeGetRunOutput)
                .map_err(native_error)
                .map_err(NativeReadTargetError)
        })
    }
}

pub(crate) fn native_read_port(
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
) -> Arc<dyn NativeReadPort> {
    Arc::new(NativeReadAdapter { store, cache_store })
}

struct NativeReadHandler {
    port: Arc<dyn NativeReadPort>,
}

impl
    InterfaceHandler<
        NativeModelsInput,
        NativeModelsOutput,
        NativeReadTargetError,
        ApplicationPrincipal,
    > for NativeReadHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: NativeModelsInput,
    ) -> InterfaceHandlerFuture<NativeModelsOutput, NativeReadTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.list_models(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("application_native_models", error))
        })
    }
}

struct NativeGetRunHandler {
    port: Arc<dyn NativeReadPort>,
}

impl
    InterfaceHandler<
        NativeGetRunInput,
        NativeGetRunOutput,
        NativeReadTargetError,
        ApplicationPrincipal,
    > for NativeGetRunHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: NativeGetRunInput,
    ) -> InterfaceHandlerFuture<NativeGetRunOutput, NativeReadTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.get_run(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("application_native_get_run", error))
        })
    }
}

pub(crate) struct NativeReadAuthorization;
impl InterfaceAuthorizationPort<ApplicationPrincipal> for NativeReadAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.application-native-read").unwrap()
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn NativeReadPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new("api-server.application-public-api").unwrap();
    let operations = [
        AuthorizationOperation::new("application.native.models.list").unwrap(),
        AuthorizationOperation::new("application.native.runs.read").unwrap(),
    ];
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-native-read-v1").unwrap(),
        operations.clone(),
        [owner.clone()],
    );
    register::<NativeModelsInput, NativeModelsOutput>(
        &mut compiler,
        &owner,
        operations[0].clone(),
        "application.native.models.list",
        MODELS_BINDING_ID,
        "api-server.application-native-models.list",
        RouteIdentity::new("GET", "/api/agent/v1/models").unwrap(),
        "control-plane.application-publication.load-active",
    )?;
    register::<NativeGetRunInput, NativeGetRunOutput>(
        &mut compiler,
        &owner,
        operations[1].clone(),
        "application.native.runs.get",
        GET_RUN_BINDING_ID,
        "api-server.application-native-run.get",
        RouteIdentity::new("GET", "/api/agent/v1/runs/:run_id").unwrap(),
        "control-plane.application-native-run.get",
    )?;
    compiler.bind_handler::<NativeModelsInput, NativeModelsOutput, NativeReadTargetError, ApplicationPrincipal>(
        &InterfaceId::new("application.native.models.list").unwrap(),
        HandlerReference::new("api-server.application-native-models.list").unwrap(),
        Arc::new(NativeReadHandler {
            port: Arc::clone(&port),
        }),
    )?;
    compiler.bind_handler::<NativeGetRunInput, NativeGetRunOutput, NativeReadTargetError, ApplicationPrincipal>(
        &InterfaceId::new("application.native.runs.get").unwrap(),
        HandlerReference::new("api-server.application-native-run.get").unwrap(),
        Arc::new(NativeGetRunHandler { port }),
    )?;
    compiler.compile()
}

#[allow(clippy::too_many_arguments)]
fn register<I: InterfaceContract, O: InterfaceContract>(
    compiler: &mut RegistryCompiler,
    owner: &InterfaceOwner,
    operation: AuthorizationOperation,
    interface_id: &str,
    binding_id: &str,
    handler: &str,
    route: RouteIdentity,
    target: &str,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let id = InterfaceId::new(interface_id).unwrap();
    let identity = InterfaceIdentity::new(id.clone(), InterfaceVersion::new("1").unwrap());
    let contracts = InterfaceContracts::unary(
        contract::<I>(),
        contract::<O>(),
        contract::<NativeReadTargetError>(),
    );
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            interface_runtime::PrincipalProfile::Application,
            InterfaceAuthenticationPolicy::Authenticated,
            operation,
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(handler).unwrap(),
            TargetReference::new(target).unwrap(),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    compiler.register_authentication_adapter(
        &id,
        1,
        interface_runtime::InterfaceExtensionRegistration::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .unwrap(),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
            interface_runtime::InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .unwrap(),
        interface_runtime::ActivatedAuthenticationAdapter::new(
            interface_runtime::PluginIdentity::new("api-server.application-authentication")
                .unwrap(),
            interface_runtime::InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.application-api-key").unwrap(),
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.application-api-key.activation.v1",
            )
            .unwrap(),
            interface_runtime::PrincipalProfile::Application,
        ),
    )?;
    compiler.register_binding(
        ProtocolBinding::new(
            BindingId::new(binding_id).unwrap(),
            identity,
            contracts,
            ProtocolProjection::http(route),
        ),
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.application-api-key").unwrap(),
            AuthorizationAdapterReference::new("api-server.application-native-read").unwrap(),
            None,
        ),
    )
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION).unwrap()
}
