use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    model_catalog::extract_agent_model_catalog_from_start_node,
    native::{ApplicationNativeRunService, NativeRunValidationError},
    publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
};
use control_plane::{
    orchestration_runtime::OrchestrationRuntimeService,
    ports::{CacheStore, ProviderTransportStore, RuntimeEventStream, TaskQueue},
};
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
use orchestration_runtime::execution_engine::RuntimeInternalToolInvoker;
use runtime_core::runtime_engine::RuntimeEngine;
use serde_json::Value;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::native::{
    application_actor_from_principal, native_error, NativeApiError, NativeModelListResponse,
    NativeModelObject, NativeRunResponse,
};
use crate::{
    provider_runtime::ApiProviderRuntime, runtime_activity::ApplicationRuntimeActivityTracker,
};

pub(crate) const MODELS_BINDING_ID: &str = "http.application.native.models.list.v1";
pub(crate) const GET_RUN_BINDING_ID: &str = "http.application.native.runs.get.v1";
pub(crate) const CANCEL_RUN_BINDING_ID: &str = "http.application.native.runs.cancel.v1";
pub(crate) const RESUME_RUN_BINDING_ID: &str = "http.application.native.runs.resume.v1";

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

pub(crate) struct NativeCancelRunInput(pub(crate) Uuid);
impl InterfaceContract for NativeCancelRunInput {
    const CONTRACT_ID: &'static str = "application-native-run-cancel-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeCancelRunOutput(pub(crate) NativeRunResponse);
impl InterfaceContract for NativeCancelRunOutput {
    const CONTRACT_ID: &'static str = "application-native-run-cancel-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeResumeRunInput {
    pub(crate) run_id: Uuid,
    pub(crate) callback_task_id: Uuid,
    pub(crate) response_payload: Value,
    pub(crate) response_mode: Option<String>,
}
impl InterfaceContract for NativeResumeRunInput {
    const CONTRACT_ID: &'static str = "application-native-run-resume-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NativeResumeRunOutput(pub(crate) NativeRunResponse);
impl InterfaceContract for NativeResumeRunOutput {
    const CONTRACT_ID: &'static str = "application-native-run-resume-output";
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

    fn cancel_run<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeCancelRunInput,
    ) -> ReadFuture<'a, NativeCancelRunOutput>;
}

pub(crate) type RuntimeInvokerFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<Arc<dyn RuntimeInternalToolInvoker>, NativeApiError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait NativeRuntimeInvokerFactory: Send + Sync + 'static {
    fn for_actor<'a>(
        &'a self,
        actor: &'a control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> RuntimeInvokerFuture<'a>;
}

pub(crate) trait NativeResumePort: Send + Sync + 'static {
    fn resume<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeResumeRunInput,
    ) -> ReadFuture<'a, NativeResumeRunOutput>;
}

struct NativeReadAdapter {
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
}

struct NativeResumeAdapter {
    store: MainDurableStore,
    provider_runtime: ApiProviderRuntime,
    runtime_engine: Arc<RuntimeEngine>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    cache_store: Arc<dyn CacheStore>,
    task_queue: Arc<dyn TaskQueue>,
    provider_transport_store: Arc<dyn ProviderTransportStore>,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    runtime_invoker_factory: Arc<dyn NativeRuntimeInvokerFactory>,
}

impl NativeResumePort for NativeResumeAdapter {
    fn resume<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeResumeRunInput,
    ) -> ReadFuture<'a, NativeResumeRunOutput> {
        Box::pin(async move {
            let actor = application_actor_from_principal(principal);
            let runtime_invoker = self
                .runtime_invoker_factory
                .for_actor(&actor)
                .await
                .map_err(NativeReadTargetError)?;
            let runtime_service = OrchestrationRuntimeService::new(
                self.store.clone(),
                self.provider_runtime.clone(),
                Arc::clone(&self.runtime_engine),
                self.provider_secret_master_key.clone(),
            )
            .with_node_artifact_context(
                self.api_node_id.clone(),
                self.provider_install_root.clone(),
            )
            .with_file_storage_registry(Arc::clone(&self.file_storage_registry))
            .with_runtime_internal_tool_invoker(runtime_invoker)
            .with_llm_routing_counter_store(Arc::clone(&self.cache_store))
            .with_provider_request_log_queue(Arc::clone(&self.task_queue))
            .with_provider_transport_store(Arc::clone(&self.provider_transport_store))
            .with_runtime_event_stream(Arc::clone(&self.runtime_event_stream));
            let _activity = self.runtime_activity.start(
                actor.application_id,
                crate::runtime_activity::ApplicationActivityKind::ApplicationExecution,
            );
            ApplicationPublishedCallbackResumeService::new(self.store.clone(), runtime_service)
                .with_last_used_cache(Arc::clone(&self.cache_store))
                .resume_callback_for_actor(
                    actor,
                    ResumePublishedCallbackCommand {
                        bearer_token: String::new(),
                        target: PublishedCallbackResumeTarget::FlowRun {
                            flow_run_id: input.run_id,
                            callback_task_id: input.callback_task_id,
                        },
                        source: PublishedCallbackResumeSource::NativeAgent,
                        response_payload: input.response_payload,
                        response_mode: input.response_mode,
                    },
                )
                .await
                .map(|result| {
                    NativeResumeRunOutput(super::native::to_native_run_response(result.run))
                })
                .map_err(super::native::service_error)
                .map_err(NativeReadTargetError)
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_resume_port(
    store: MainDurableStore,
    provider_runtime: ApiProviderRuntime,
    runtime_engine: Arc<RuntimeEngine>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    cache_store: Arc<dyn CacheStore>,
    task_queue: Arc<dyn TaskQueue>,
    provider_transport_store: Arc<dyn ProviderTransportStore>,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    runtime_invoker_factory: Arc<dyn NativeRuntimeInvokerFactory>,
) -> Arc<dyn NativeResumePort> {
    Arc::new(NativeResumeAdapter {
        store,
        provider_runtime,
        runtime_engine,
        provider_secret_master_key,
        api_node_id,
        provider_install_root,
        file_storage_registry,
        cache_store,
        task_queue,
        provider_transport_store,
        runtime_event_stream,
        runtime_activity,
        runtime_invoker_factory,
    })
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

    fn cancel_run<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: NativeCancelRunInput,
    ) -> ReadFuture<'a, NativeCancelRunOutput> {
        Box::pin(async move {
            ApplicationNativeRunService::new(self.store.clone())
                .with_last_used_cache(Arc::clone(&self.cache_store))
                .with_runtime_event_stream(Arc::clone(&self.runtime_event_stream))
                .cancel_native_run_for_actor(application_actor_from_principal(principal), input.0)
                .await
                .map(super::native::to_native_run_response)
                .map(NativeCancelRunOutput)
                .map_err(native_error)
                .map_err(NativeReadTargetError)
        })
    }
}

pub(crate) fn native_read_port(
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
) -> Arc<dyn NativeReadPort> {
    Arc::new(NativeReadAdapter {
        store,
        cache_store,
        runtime_event_stream,
    })
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

struct NativeCancelRunHandler {
    port: Arc<dyn NativeReadPort>,
}

struct NativeResumeRunHandler {
    port: Arc<dyn NativeResumePort>,
}

impl
    InterfaceHandler<
        NativeResumeRunInput,
        NativeResumeRunOutput,
        NativeReadTargetError,
        ApplicationPrincipal,
    > for NativeResumeRunHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: NativeResumeRunInput,
    ) -> InterfaceHandlerFuture<NativeResumeRunOutput, NativeReadTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.resume(context.principal(), input)
                .await
                .map_err(|error| {
                    InterfaceTargetFailure::new("application_native_resume_run", error)
                })
        })
    }
}

impl
    InterfaceHandler<
        NativeCancelRunInput,
        NativeCancelRunOutput,
        NativeReadTargetError,
        ApplicationPrincipal,
    > for NativeCancelRunHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: NativeCancelRunInput,
    ) -> InterfaceHandlerFuture<NativeCancelRunOutput, NativeReadTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.cancel_run(context.principal(), input)
                .await
                .map_err(|error| {
                    InterfaceTargetFailure::new("application_native_cancel_run", error)
                })
        })
    }
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
    resume_port: Arc<dyn NativeResumePort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new("api-server.application-public-api").unwrap();
    let operations = [
        AuthorizationOperation::new("application.native.models.list").unwrap(),
        AuthorizationOperation::new("application.native.runs.read").unwrap(),
        AuthorizationOperation::new("application.native.runs.cancel").unwrap(),
        AuthorizationOperation::new("application.native.runs.resume").unwrap(),
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
        InterfaceAuditPolicy::ReadOnly,
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
        InterfaceAuditPolicy::ReadOnly,
    )?;
    register::<NativeCancelRunInput, NativeCancelRunOutput>(
        &mut compiler,
        &owner,
        operations[2].clone(),
        "application.native.runs.cancel",
        CANCEL_RUN_BINDING_ID,
        "api-server.application-native-run.cancel",
        RouteIdentity::new("POST", "/api/agent/v1/runs/:run_id/cancel").unwrap(),
        "control-plane.application-native-run.cancel",
        InterfaceAuditPolicy::Mutating,
    )?;
    register::<NativeResumeRunInput, NativeResumeRunOutput>(
        &mut compiler,
        &owner,
        operations[3].clone(),
        "application.native.runs.resume",
        RESUME_RUN_BINDING_ID,
        "api-server.application-native-run.resume",
        RouteIdentity::new("POST", "/api/agent/v1/runs/:run_id/resume").unwrap(),
        "control-plane.application-native-run.resume",
        InterfaceAuditPolicy::Mutating,
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
        Arc::new(NativeGetRunHandler {
            port: Arc::clone(&port),
        }),
    )?;
    compiler.bind_handler::<NativeCancelRunInput, NativeCancelRunOutput, NativeReadTargetError, ApplicationPrincipal>(
        &InterfaceId::new("application.native.runs.cancel").unwrap(),
        HandlerReference::new("api-server.application-native-run.cancel").unwrap(),
        Arc::new(NativeCancelRunHandler { port }),
    )?;
    compiler.bind_handler::<NativeResumeRunInput, NativeResumeRunOutput, NativeReadTargetError, ApplicationPrincipal>(
        &InterfaceId::new("application.native.runs.resume").unwrap(),
        HandlerReference::new("api-server.application-native-run.resume").unwrap(),
        Arc::new(NativeResumeRunHandler { port: resume_port }),
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
    audit_policy: InterfaceAuditPolicy,
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
        audit_policy,
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
