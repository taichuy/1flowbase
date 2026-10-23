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
pub(crate) const INTERFACE_ID: &str = "public.files.temporary.read";
const HANDLER_REFERENCE: &str = "api-server.temporary-files.read";
pub(crate) const BINDING_ID: &str = "http.public.files.temporary.read.v1";
pub(crate) struct TemporaryFileInput(pub control_plane::file_management::FileAccessKey, pub bool);
impl InterfaceContract for TemporaryFileInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "preview",
            serde_json::json!({"type":"boolean"}),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"preview":self.1}))
    }
    const CONTRACT_ID: &'static str = "temporary-file-access-input";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct TemporaryFileOutput(pub control_plane::file_management::FileContent);
impl InterfaceContract for TemporaryFileOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"type":"object","properties":{"size":{"type":"integer"}},"required":["size"],"additionalProperties":false}),
        )
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"size":self.0.content_length}))
    }
    const CONTRACT_ID: &'static str = "temporary-file-access-output";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct TemporaryFileTargetError(pub(crate) ApiError);

impl From<ApiError> for TemporaryFileTargetError {
    fn from(error: ApiError) -> Self {
        Self(error)
    }
}

impl InterfaceContract for TemporaryFileTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([(
            "kind",
            mp::tag_schema("TemporaryFileTargetError"),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value([(
            "kind",
            serde_json::Value::String("TemporaryFileTargetError".to_owned()),
        )]))
    }

    const CONTRACT_ID: &'static str = "temporary-file-access-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type TemporaryFileFuture<'a> = Pin<
    Box<dyn Future<Output = Result<TemporaryFileOutput, TemporaryFileTargetError>> + Send + 'a>,
>;

pub(crate) trait TemporaryFilePort: Send + Sync + 'static {
    fn download(&self, input: TemporaryFileInput) -> TemporaryFileFuture<'_>;
}

struct TemporaryFileHandler {
    port: Arc<dyn TemporaryFilePort>,
}

impl
    InterfaceHandler<
        TemporaryFileInput,
        TemporaryFileOutput,
        TemporaryFileTargetError,
        PublicPrincipal,
    > for TemporaryFileHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        input: TemporaryFileInput,
    ) -> InterfaceHandlerFuture<TemporaryFileOutput, TemporaryFileTargetError> {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.download(input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("temporary_file_access", error))
        })
    }
}

pub(crate) struct TemporaryFileAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for TemporaryFileAuthorization {
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
    port: Arc<dyn TemporaryFilePort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<TemporaryFileInput>(),
        contract::<TemporaryFileOutput>(),
        contract::<TemporaryFileTargetError>(),
    );
    let operation = AuthorizationOperation::new("public.files.temporary.read")
        .expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.temporary-files").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-temporary-file-access-v1")
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
            TargetReference::new("control-plane.temporary-files.download")
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
            BindingId::new("http.public.files.temporary.read.v1").expect("static binding is valid"),
            identity,
            contracts,
            ProtocolProjection::http(
                RouteIdentity::new("GET", "/api/public/files/content")
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
        TemporaryFileInput,
        TemporaryFileOutput,
        TemporaryFileTargetError,
        PublicPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(TemporaryFileHandler { port }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
