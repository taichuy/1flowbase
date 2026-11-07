use super::{interface::*, metadata::RuntimeDataModelMetadata};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use interface_runtime::*;
use std::sync::Arc;

pub(crate) const BINDING_ID: &str = "http.runtime.models.metadata.get.v1";
pub(crate) const INTERFACE_ID: &str = "runtime.models.metadata.get";
pub(crate) const OPENAPI_PATH: &str = "/api/runtime/models/{model_code}/metadata";

struct RuntimeModelMetadataInput {
    model_code: String,
}
impl InterfaceContract for RuntimeModelMetadataInput {
    const CONTRACT_ID: &'static str = "runtime-model-metadata-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([("model_code", mp::text_schema())]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"model_code": self.model_code}))
    }
}

impl crate::event_center::api_projection::ApiEventInput for RuntimeModelMetadataInput {
    fn project_document_input(
        &self,
        _: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, Arc<str>> {
        Ok(serde_json::json!({"path":{"model_code":self.model_code}}))
    }
}

struct Handler(Arc<dyn RuntimeModelOperationPort>);
impl
    InterfaceHandler<
        RuntimeModelMetadataInput,
        RuntimeDataModelMetadata,
        RuntimeModelOperationTargetError,
        UserPrincipal,
    > for Handler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<UserPrincipal>,
        input: RuntimeModelMetadataInput,
    ) -> InterfaceHandlerFuture<RuntimeDataModelMetadata, RuntimeModelOperationTargetError> {
        let port = Arc::clone(&self.0);
        Box::pin(async move {
            port.metadata(context.principal(), input.model_code)
                .await
                .map_err(|error| InterfaceTargetFailure::new("runtime_model_metadata", error))
        })
    }
}

pub(super) fn register(
    compiler: &mut RegistryCompiler,
    port: Arc<dyn RuntimeModelOperationPort>,
    owner: &InterfaceOwner,
    operation: &AuthorizationOperation,
) -> Result<(), RegistryCompilationError> {
    let id = InterfaceId::new(INTERFACE_ID).expect("static metadata interface id");
    let identity = InterfaceIdentity::new(
        id.clone(),
        InterfaceVersion::new("1").expect("static runtime model version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<RuntimeModelMetadataInput>(),
        contract::<RuntimeDataModelMetadata>(),
        contract::<RuntimeModelOperationTargetError>(),
    );
    let handler = HandlerReference::new("api-server.runtime-models.metadata.handler")
        .expect("static runtime metadata handler is valid");
    compiler.register_definition(InterfaceDefinition::new(
        identity.clone(),
        contracts.clone(),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            operation.clone(),
            InterfaceScope::Workspace,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            handler.clone(),
            TargetReference::new("runtime-core.model-metadata.read")
                .expect("static runtime metadata target is valid"),
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        owner.clone(),
    ))?;
    let plugin = PluginIdentity::new("api-server.runtime-model-authentication")
        .expect("static runtime authentication plugin is valid");
    compiler.register_authentication_adapter(
        &id,
        1,
        InterfaceExtensionRegistration::new(
            plugin.clone(),
            InterfaceExtensionTier::BuiltIn,
            InterfaceExtensionPoint::AuthenticationAdapter,
            InterfaceExtensionPermission::Authenticate,
            InterfaceScope::Workspace,
            InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )
        .expect("runtime authentication registration is valid"),
        ActivatedAuthenticationAdapter::new(
            plugin,
            InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static runtime authentication adapter is valid"),
            AuthenticationActivationIdentity::new(ACTIVATION)
                .expect("static runtime authentication activation is valid"),
            PrincipalProfile::User,
        ),
    )?;
    let binding = ProtocolBinding::new(
        BindingId::new(BINDING_ID).expect("static runtime metadata binding is valid"),
        identity,
        contracts,
        ProtocolProjection::http(
            RouteIdentity::new("GET", "/api/runtime/models/:model_code/metadata")
                .expect("static runtime metadata route is valid"),
        ),
    );
    crate::event_center::api_projection::register_static_api_event::<
        RuntimeModelMetadataInput,
        RuntimeDataModelMetadata,
    >(compiler, &binding, "runtime-model-metadata-document-v1")?;
    compiler.register_binding(
        binding,
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new(AUTHENTICATION_ADAPTER)
                .expect("static runtime authentication adapter is valid"),
            AuthorizationAdapterReference::new(AUTHORIZATION_ADAPTER)
                .expect("static runtime authorization adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<RuntimeModelMetadataInput, RuntimeDataModelMetadata, RuntimeModelOperationTargetError, UserPrincipal>(&id, handler, Arc::new(Handler(port)))?;
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/runtime/models/{model_code}/metadata",
    operation_id = "runtime.models.metadata.get",
    tag = "Runtime Models",
    summary = "Read runtime model metadata",
    description = "Read field presentation and effective record capabilities using the caller's runtime data grants.",
    params(("model_code" = String, Path, description = "Runtime model code")),
    security(("sessionCookie" = []), ("patBearer" = [])),
    responses(
        (status = 200, body = RuntimeDataModelMetadata),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 409, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_model_metadata(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_code): Path<String>,
) -> Result<Json<ApiSuccess<RuntimeDataModelMetadata>>, ApiError> {
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("interface snapshot unavailable"))?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry unavailable"))?
        .snapshot();
    let binding = BindingId::new(BINDING_ID).expect("static runtime metadata binding is valid");
    let authenticated = boot
        .authenticate_invocation::<_, UserPrincipal>(
            Arc::clone(&snapshot),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::RuntimeModelAuthenticationCredential {
                state: Arc::clone(&state),
                headers,
                method: plugin_framework::DataModelOperationMethod::Get,
                path: format!("/api/runtime/models/{model_code}/metadata"),
                model_code: model_code.clone(),
            },
        )
        .await
        .map_err(ApiError::from)?;
    let outcome = InterfaceInvocationKernel::new(Arc::new(RuntimeModelAuthorization))
        .invoke::<RuntimeModelMetadataInput, RuntimeDataModelMetadata, RuntimeModelOperationTargetError>(
            snapshot, authenticated.into_envelope(RuntimeModelMetadataInput { model_code }),
        ).await.map_err(|failure| match failure.into_error() {
            InterfaceInvocationError::TargetFailed(error) => error.into_source::<RuntimeModelOperationTargetError>()
                .map(|error| error.0).unwrap_or_else(|| anyhow::anyhow!("invalid metadata target error").into()),
            _ => anyhow::anyhow!("metadata invocation failed").into(),
        })?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(Json(ApiSuccess::new(outcome.into_value())))
}
