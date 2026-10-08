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

use crate::error_response::ApiError;
pub(crate) const INTERFACE_ID: &str = "public.client-collectors.asset.download";
const HANDLER_REFERENCE: &str = "api-server.client-collectors.assets";
pub(crate) const BINDING_ID: &str = "http.public.client-collectors.assets.v1";
pub(crate) struct CollectorAssetInput {
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub asset: String,
}
impl InterfaceContract for CollectorAssetInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            ("organization", mp::text_schema()),
            ("artifact_id", mp::text_schema()),
            ("version", mp::text_schema()),
            ("asset", mp::text_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[
            ("organization", mp::text(&self.organization)?),
            ("artifact_id", mp::text(&self.artifact_id)?),
            ("version", mp::text(&self.version)?),
            ("asset", mp::text(&self.asset)?),
        ]))
    }
    const CONTRACT_ID: &'static str = "client-collector-asset-input";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct CollectorAssetOutput(
    pub control_plane::plugin_management::ClientCollectorDownload,
);
impl InterfaceContract for CollectorAssetOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","properties":{"size":{"type":"integer"}},"required":["size"],"additionalProperties":false}),
        )
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"size":self.0.size}))
    }
    const CONTRACT_ID: &'static str = "client-collector-asset-output";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct CollectorAssetTargetError(pub(crate) ApiError);

impl From<ApiError> for CollectorAssetTargetError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl InterfaceContract for CollectorAssetTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "kind",
            mp::tag_schema("CollectorAssetTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "kind",
            serde_json::Value::String("CollectorAssetTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "public-client-collector-assets-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type CollectorAssetFuture<'a> = Pin<
    Box<dyn Future<Output = Result<CollectorAssetOutput, CollectorAssetTargetError>> + Send + 'a>,
>;

pub(crate) trait CollectorAssetPort: Send + Sync + 'static {
    fn download(&self, input: CollectorAssetInput) -> CollectorAssetFuture<'_>;
}

struct CollectorAssetHandler {
    port: Arc<dyn CollectorAssetPort>,
}

impl
    InterfaceHandler<
        CollectorAssetInput,
        CollectorAssetOutput,
        CollectorAssetTargetError,
        PublicPrincipal,
    > for CollectorAssetHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: CollectorAssetInput,
    ) -> InterfaceHandlerFuture<CollectorAssetOutput, CollectorAssetTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.download(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("client_collector_asset", error))
        })
    }
}

pub(crate) struct CollectorAssetAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for CollectorAssetAuthorization {
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
    port: Arc<dyn CollectorAssetPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<CollectorAssetInput>(),
        contract::<CollectorAssetOutput>(),
        contract::<CollectorAssetTargetError>(),
    );
    let operation = AuthorizationOperation::new("public.client-collectors.assets.read")
        .expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.client-collectors").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-client-collector-assets-v1")
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
            TargetReference::new("control-plane.client-collectors.download")
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
            BindingId::new("http.public.client-collectors.assets.v1").expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("GET", "/api/public/client-collectors/:organization/:artifact_id/:version/assets/:asset")
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
        CollectorAssetInput,
        CollectorAssetOutput,
        CollectorAssetTargetError,
        PublicPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(CollectorAssetHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
