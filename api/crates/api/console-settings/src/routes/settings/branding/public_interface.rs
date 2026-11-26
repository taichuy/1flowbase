use std::{path::PathBuf, sync::Arc};

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

use super::BrandingInput;
use crate::{
    app_state::ApiState, error_response::ApiError, openapi_interface::json_contract::JsonApiOutput,
    response::ApiSuccess, routes::console_interface::ConsoleInterfaceTargetError,
};
use axum::{extract::State, Json};
use control_plane::system_branding::SystemBrandingService;
use control_plane_contracts::ports::{CommercialBrandingConfig, PublicSystemMetadata};
const INTERFACE_ID: &str = "public.system-metadata.get";
const HANDLER_REFERENCE: &str = "api-server.public-branding.read";

#[derive(Debug, Clone)]
pub struct CommercialLicenseBrandingSource {
    pub config_path: PathBuf,
    pub packaged_default_path: PathBuf,
}

#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct PublicBrandingOutput(pub PublicSystemMetadata);
impl JsonApiOutput for PublicBrandingOutput {}

impl InterfaceContract for PublicBrandingOutput {
    const CONTRACT_ID: &'static str = "public-system-metadata-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "system_name",
            crate::extension_bus::managed_projection::text_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"system_name": crate::extension_bus::managed_projection::text(&self.0.system_name)?}),
        )
    }
}

async fn read_commercial_branding(
    source: Option<&CommercialLicenseBrandingSource>,
) -> Option<CommercialBrandingConfig> {
    let source = source?;
    match tokio::fs::read(&source.config_path).await {
        Ok(bytes) => Some(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match tokio::fs::read(&source.packaged_default_path).await {
                Ok(bytes) => Some(serde_json::from_slice(&bytes).unwrap_or_default()),
                Err(_) => Some(CommercialBrandingConfig::default()),
            }
        }
        Err(_) => Some(CommercialBrandingConfig::default()),
    }
}

struct BrandingHandler {
    store: storage_durable_postgres::MainDurableStore,
    commercial_source: Option<CommercialLicenseBrandingSource>,
}
impl
    InterfaceHandler<
        BrandingInput,
        PublicBrandingOutput,
        ConsoleInterfaceTargetError,
        PublicPrincipal,
    > for BrandingHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext<PublicPrincipal>,
        _input: BrandingInput,
    ) -> InterfaceHandlerFuture<PublicBrandingOutput, ConsoleInterfaceTargetError> {
        let store = self.store.clone();
        let commercial_source = self.commercial_source.clone();
        Box::pin(async move {
            let base = SystemBrandingService::new(store)
                .read()
                .await
                .map_err(|error| {
                    InterfaceTargetFailure::new(
                        "system_branding",
                        ConsoleInterfaceTargetError(ApiError::from(error)),
                    )
                })?;
            let commercial = read_commercial_branding(commercial_source.as_ref()).await;
            Ok(PublicBrandingOutput(PublicSystemMetadata::compose(
                base,
                commercial.as_ref(),
            )))
        })
    }
}

pub fn route_assembly() -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        "/api/public/system-metadata",
        crate::external_route_assembly::get(get_system_metadata),
    )
}

/// Read public system display metadata without signing in.
/// Returns synthesized installation branding, filing HTML, and active commercial white-label projection.
#[utoipa::path(get, path = "/api/public/system-metadata", responses((status = 200, body = serde_json::Value)))]
pub async fn get_system_metadata(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ApiSuccess<PublicSystemMetadata>>, ApiError> {
    use interface_runtime::{
        InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol,
    };
    let boot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding =
        BindingId::new("http.public.system-metadata.get.v1").expect("static binding is valid");
    let authenticated = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            Arc::clone(&snapshot),
            &binding,
            InterfaceProtocol::Http,
            crate::extension_bus::PublicAuthenticationCredential,
        )
        .await?;
    let outcome = InterfaceInvocationKernel::new(Arc::new(BrandingAuthorization))
        .invoke::<BrandingInput, PublicBrandingOutput, ConsoleInterfaceTargetError>(
            snapshot,
            authenticated.into_envelope(BrandingInput::Read),
        )
        .await
        .map_err(|failure| match failure.into_error() {
            InterfaceInvocationError::TargetFailed(error) => error
                .into_source::<ConsoleInterfaceTargetError>()
                .map(|error| error.0)
                .unwrap_or_else(|| anyhow::anyhow!("public branding read failed").into()),
            error => anyhow::anyhow!(error.to_string()).into(),
        })?;
    let _receipt = outcome.receipt().clone().projected();
    Ok(Json(ApiSuccess::new(outcome.into_value().0)))
}

pub(crate) struct BrandingAuthorization;

impl InterfaceAuthorizationPort<PublicPrincipal> for BrandingAuthorization {
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

pub fn compile_registry(
    store: storage_durable_postgres::MainDurableStore,
    commercial_source: Option<CommercialLicenseBrandingSource>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let interface_id = InterfaceId::new(INTERFACE_ID).expect("static interface id is valid");
    let identity = InterfaceIdentity::new(
        interface_id.clone(),
        InterfaceVersion::new("1").expect("static interface version is valid"),
    );
    let contracts = InterfaceContracts::unary(
        contract::<BrandingInput>(),
        contract::<PublicBrandingOutput>(),
        contract::<ConsoleInterfaceTargetError>(),
    );
    let operation = AuthorizationOperation::new("public.system-metadata.read")
        .expect("static operation is valid");
    let owner = InterfaceOwner::new("api-server.public-branding").expect("static owner is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:public-branding-v1")
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
            TargetReference::new("control-plane.system-branding.read")
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
    let binding = ProtocolBinding::new(
        BindingId::new("http.public.system-metadata.get.v1").expect("static binding is valid"),
        identity,
        contracts,
        ProtocolProjection::http(
            RouteIdentity::new("GET", "/api/public/system-metadata")
                .expect("static route is valid"),
        ),
    );
    crate::openapi_interface::contract::register_static_api_operation::<
        BrandingInput,
        PublicBrandingOutput,
    >(&mut compiler, &binding, "graph:public-branding-v1")?;
    compiler.register_binding(
        binding,
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            AuthorizationAdapterReference::new("api-server.public")
                .expect("static adapter is valid"),
            None,
        ),
    )?;
    compiler.bind_handler::<
        BrandingInput,
        PublicBrandingOutput,
        ConsoleInterfaceTargetError,
        PublicPrincipal,
    >(
        &interface_id,
        HandlerReference::new(HANDLER_REFERENCE).expect("static handler is valid"),
        Arc::new(BrandingHandler {
            store,
            commercial_source,
        }),
    )?;
    compiler.compile()
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
