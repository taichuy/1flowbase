use std::{future::Future, pin::Pin, sync::Arc};

use axum::{
    body::Bytes,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceExecutionMode, InterfaceHandler, InterfaceHandlerContext,
    InterfaceHandlerFuture, InterfaceId, InterfaceIdentity, InterfaceLifecycle, InterfaceOwner,
    InterfaceProtocol, InterfaceScope, InterfaceTargetFailure, InterfaceVersion,
    InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage, ProtocolBinding,
    ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference, UserPrincipal,
};
use serde_json::Value;

use crate::{
    app_state::ApiState, error_response::ApiError,
    extension_bus::RuntimeModelAuthenticationCredential, response::ApiSuccess,
};

const AUTHENTICATION_ADAPTER: &str = "api-server.runtime-user";
const AUTHORIZATION_ADAPTER: &str = "api-server.runtime-model-authorization";
const ACTIVATION: &str = "api-server.runtime-user.activation.v1";
const ROUTE: &str = "/api/runtime/models/:model_code/*operation_path";

pub(crate) struct RuntimeModelOperationInput {
    pub(crate) method: plugin_framework::DataModelOperationMethod,
    pub(crate) model_code: String,
    pub(crate) path: String,
    pub(crate) query: Option<String>,
    pub(crate) body: Vec<u8>,
}

impl InterfaceContract for RuntimeModelOperationInput {
    const CONTRACT_ID: &'static str = "runtime-model-operation-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone, Copy)]
pub(crate) enum RuntimeModelOperationStatus {
    Ok,
    Created,
}

pub(crate) struct RuntimeModelOperationOutput {
    pub(crate) status: RuntimeModelOperationStatus,
    pub(crate) data: Value,
}

impl InterfaceContract for RuntimeModelOperationOutput {
    const CONTRACT_ID: &'static str = "runtime-model-operation-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct RuntimeModelOperationTargetError(pub(crate) ApiError);

impl InterfaceContract for RuntimeModelOperationTargetError {
    const CONTRACT_ID: &'static str = "runtime-model-operation-error";
    const CONTRACT_VERSION: &'static str = "1";
}

type RuntimeModelOperationFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<RuntimeModelOperationOutput, RuntimeModelOperationTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait RuntimeModelOperationPort: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: RuntimeModelOperationInput,
    ) -> RuntimeModelOperationFuture<'a>;
}

struct RuntimeModelOperationHandler {
    port: Arc<dyn RuntimeModelOperationPort>,
}

impl
    InterfaceHandler<
        RuntimeModelOperationInput,
        RuntimeModelOperationOutput,
        RuntimeModelOperationTargetError,
        UserPrincipal,
    > for RuntimeModelOperationHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: RuntimeModelOperationInput,
    ) -> InterfaceHandlerFuture<RuntimeModelOperationOutput, RuntimeModelOperationTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("runtime_model_operation", error))
        })
    }
}

pub(crate) struct RuntimeModelAuthorization;

impl InterfaceAuthorizationPort<UserPrincipal> for RuntimeModelAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
            .expect("static runtime model authorization adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<UserPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        // Runtime model ACL and row scope remain mandatory inside RuntimeEngine. This baseline
        // authorization stage cannot grant around those model-specific decisions.
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn RuntimeModelOperationPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let owner = InterfaceOwner::new("api-server.runtime-models")
        .expect("static runtime model owner is valid");
    let operation = AuthorizationOperation::new("runtime.models.invoke")
        .expect("static runtime model operation is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:runtime-model-operations-v1")
            .expect("static runtime model graph is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    for (method, method_name, binding_id) in binding_declarations() {
        let interface_id = InterfaceId::new(format!("runtime.models.invoke.{method_name}"))
            .expect("static runtime model interface id is valid");
        let identity = InterfaceIdentity::new(
            interface_id.clone(),
            InterfaceVersion::new("1").expect("static runtime model version is valid"),
        );
        let contracts = InterfaceContracts::unary(
            contract::<RuntimeModelOperationInput>(),
            contract::<RuntimeModelOperationOutput>(),
            contract::<RuntimeModelOperationTargetError>(),
        );
        let handler =
            HandlerReference::new(format!("api-server.runtime-models.{method_name}.handler"))
                .expect("static runtime model handler is valid");
        compiler.register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                interface_runtime::PrincipalProfile::User,
                InterfaceAuthenticationPolicy::Authenticated,
                operation.clone(),
                InterfaceScope::Workspace,
            ),
            InterfaceExecution::new(
                InterfaceExecutionMode::Unary,
                handler.clone(),
                TargetReference::new("runtime-core.model-operation.execute")
                    .expect("static runtime model target is valid"),
            ),
            if matches!(method, Method::GET) {
                InterfaceAuditPolicy::ReadOnly
            } else {
                InterfaceAuditPolicy::Mutating
            },
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        compiler.register_authentication_adapter(
            &interface_id,
            1,
            interface_runtime::InterfaceExtensionRegistration::new(
                interface_runtime::PluginIdentity::new("api-server.runtime-model-authentication")
                    .expect("static runtime authentication plugin is valid"),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
                interface_runtime::InterfaceExtensionPermission::Authenticate,
                InterfaceScope::Workspace,
                interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .expect("runtime authentication registration is valid"),
            interface_runtime::ActivatedAuthenticationAdapter::new(
                interface_runtime::PluginIdentity::new("api-server.runtime-model-authentication")
                    .expect("static runtime authentication plugin is valid"),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                    .expect("static runtime authentication adapter is valid"),
                interface_runtime::AuthenticationActivationIdentity::new(ACTIVATION)
                    .expect("static runtime authentication activation is valid"),
                interface_runtime::PrincipalProfile::User,
            ),
        )?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(binding_id).expect("static runtime model binding is valid"),
                identity,
                contracts,
                ProtocolProjection::http(
                    RouteIdentity::new(method.as_str(), ROUTE)
                        .expect("static runtime model route is valid"),
                ),
            ),
            InvocationAdapterPlan::new(
                AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                    .expect("static runtime authentication adapter is valid"),
                AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                    .expect("static runtime authorization adapter is valid"),
                None,
            ),
        )?;
        compiler.bind_handler::<RuntimeModelOperationInput, RuntimeModelOperationOutput, RuntimeModelOperationTargetError, UserPrincipal>(
            &interface_id,
            handler,
            Arc::new(RuntimeModelOperationHandler {
                port: Arc::clone(&port),
            }),
        )?;
    }
    compiler.compile()
}

pub(crate) async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    method: Method,
    model_code: String,
    uri: Uri,
    body: Bytes,
) -> Response {
    let Some((_route_method, _, raw_binding_id)) = binding_declarations()
        .into_iter()
        .find(|(candidate, _, _)| candidate == &method)
    else {
        return crate::error_response::ApiError::from(
            control_plane::errors::ControlPlaneError::NotFound("runtime_operation"),
        )
        .into_response();
    };
    let descriptor_method = super::descriptor_method(&method)
        .expect("supported runtime model method has a descriptor method");
    let Some(boot_snapshot) = state.extension_boot_snapshot.as_ref() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(snapshot) = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let binding_id = BindingId::new(raw_binding_id).expect("static runtime binding id is valid");
    let Some(activated) = snapshot.authentication(&binding_id) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let principal: UserPrincipal = match boot_snapshot
        .authenticate(
            activated,
            RuntimeModelAuthenticationCredential {
                state: Arc::clone(&state),
                headers,
                method: descriptor_method,
                model_code: model_code.clone(),
                path: uri.path().to_string(),
            },
        )
        .await
    {
        Ok(principal) => principal,
        Err(error) => return ApiError::from(error).into_response(),
    };
    let authentication_activation = activated.activation().clone();
    let outcome = interface_runtime::InterfaceInvocationKernel::new(Arc::new(
        RuntimeModelAuthorization,
    ))
    .invoke::<RuntimeModelOperationInput, RuntimeModelOperationOutput, RuntimeModelOperationTargetError>(
        snapshot,
        InvocationEnvelope::with_principal(
            InvocationLineage::root(InvocationId::now_v7()),
            binding_id,
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static runtime authentication adapter is valid"),
            authentication_activation,
            principal,
            None,
            RuntimeModelOperationInput {
                method: descriptor_method,
                model_code,
                path: uri.path().to_string(),
                query: uri.query().map(str::to_string),
                body: body.to_vec(),
            },
        ),
    )
    .await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(failure) => match failure.into_error() {
            interface_runtime::InterfaceInvocationError::TargetFailed(error) => {
                return error
                    .into_source::<RuntimeModelOperationTargetError>()
                    .map(|error| error.0.into_response())
                    .unwrap_or_else(|| StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
            _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
    };
    let _receipt = outcome.receipt().clone().projected();
    let output = outcome.into_value();
    let status = match output.status {
        RuntimeModelOperationStatus::Ok => StatusCode::OK,
        RuntimeModelOperationStatus::Created => StatusCode::CREATED,
    };
    (status, Json(ApiSuccess::new(output.data))).into_response()
}

fn binding_declarations() -> [(Method, &'static str, &'static str); 5] {
    [
        (Method::GET, "get", "http.runtime.models.dynamic.get.v1"),
        (Method::POST, "post", "http.runtime.models.dynamic.post.v1"),
        (Method::PUT, "put", "http.runtime.models.dynamic.put.v1"),
        (
            Method::PATCH,
            "patch",
            "http.runtime.models.dynamic.patch.v1",
        ),
        (
            Method::DELETE,
            "delete",
            "http.runtime.models.dynamic.delete.v1",
        ),
    ]
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static runtime model contract is valid")
}

#[cfg(test)]
struct UnavailableRuntimeModelOperationPort;

#[cfg(test)]
impl RuntimeModelOperationPort for UnavailableRuntimeModelOperationPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: RuntimeModelOperationInput,
    ) -> RuntimeModelOperationFuture<'a> {
        Box::pin(async {
            Err(RuntimeModelOperationTargetError(ApiError::from(
                control_plane::errors::ControlPlaneError::Conflict(
                    "runtime_model_test_port_unavailable",
                ),
            )))
        })
    }
}

#[cfg(test)]
pub(super) fn compile_registry_for_test(
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    compile_registry(Arc::new(UnavailableRuntimeModelOperationPort))
}
