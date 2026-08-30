use std::{collections::BTreeSet, future::Future, pin::Pin, sync::Arc};

use access_control::{ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup};
use anyhow::{bail, Result};
use control_plane::{
    host_infrastructure_config::HostInfrastructureProviderConfigList,
    ports::RoleConsolePolicyReader,
};
use interface_runtime::{
    AuthenticationAdapterReference, AuthorizationAdapterReference, AuthorizationOperation,
    BindingId, CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, HandlerReference,
    InterfaceAccess, InterfaceAuditPolicy, InterfaceAuthenticationPolicy,
    InterfaceAuthorizationError, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode, InterfaceExtensionFact,
    InterfaceExtensionIsolation, InterfaceExtensionPermission, InterfaceExtensionPoint,
    InterfaceExtensionRegistration, InterfaceExtensionTier, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId, InterfaceIdentity,
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceLifecycle, InterfaceOwner,
    InterfaceProtocol, InterfaceScope, InterfaceTargetFailure, InterfaceVersion,
    InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage, PluginIdentity,
    PrincipalProfile, ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity,
    TargetReference, UserPrincipal,
};
use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleKind, OrderingSemantics, OverridePolicy, ScopeSemantics,
};
use plugin_framework::{
    HostExtensionContributionManifest, HostExtensionInterfaceAuthenticationManifest,
    HostExtensionInterfaceAuthenticationPrincipalProfile,
    HostExtensionInterfaceOperationAuditPolicy, HostExtensionInterfaceOperationAuthPolicy,
    HostExtensionInterfaceOperationErrorPolicy, HostExtensionInterfaceOperationManifest,
    HostExtensionInterfaceOperationMethod,
};

use super::{to_provider_response, HostInfrastructureProviderConfigResponse};
use crate::app_state::ApiState;

pub const INTERFACE_OPERATION_POINT_ID: &str = "1flowbase.application.interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_ID: &str = "interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_VERSION: &str = "1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID: &str =
    "host_infrastructure.providers.view";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERFACE_VERSION: &str = "1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID: &str =
    "http.host_infrastructure.providers.view.v1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_MCP_BINDING_ID: &str =
    "mcp.host_infrastructure.providers.view.v1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERNAL_BINDING_ID: &str =
    "internal.host_infrastructure.providers.view.v1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID: &str =
    "official.local-infra-host.interface-operation.host_infrastructure.providers.view";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH: &str =
    "/api/console/settings/host-infrastructure/providers";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION: &str =
    "core.interface-operation.host-infrastructure-providers-view";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_ID: &str = "none";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_VERSION: &str = "1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_ID: &str =
    "host-infrastructure-provider-config-list";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_VERSION: &str = "1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_ID: &str =
    "host-infrastructure-provider-config-error";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_VERSION: &str = "1";
pub const INTERFACE_OPERATION_OWNER_MODULE_ID: &str = "1flowbase.boot-core";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID: &str = "official.local-infra-host";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONSOLE_OWNER_ID: &str = "boot-core";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_HANDLER_REFERENCE: &str =
    "api-server.host-infrastructure.providers.view";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_REFERENCE: &str =
    "control-plane.host-infrastructure.providers.view";

pub struct HostInfrastructureProvidersViewInput;

impl HostInfrastructureProvidersViewInput {
    pub fn new() -> Self {
        Self
    }
}

impl InterfaceContract for HostInfrastructureProvidersViewInput {
    const CONTRACT_ID: &'static str = HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_ID;
    const CONTRACT_VERSION: &'static str =
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_VERSION;
}

pub struct HostInfrastructureProvidersViewOutput {
    providers: Vec<HostInfrastructureProviderConfigResponse>,
}

impl HostInfrastructureProvidersViewOutput {
    pub fn into_providers(self) -> Vec<HostInfrastructureProviderConfigResponse> {
        self.providers
    }
}

impl InterfaceContract for HostInfrastructureProvidersViewOutput {
    const CONTRACT_ID: &'static str = HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_ID;
    const CONTRACT_VERSION: &'static str =
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_VERSION;
}

pub(crate) type HostInfrastructureProvidersViewQueryFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    HostInfrastructureProviderConfigList,
                    crate::error_response::ApiError,
                >,
            > + Send
            + 'a,
    >,
>;

pub(crate) trait HostInfrastructureProvidersViewQuery: Send + Sync {
    fn list(&self) -> HostInfrastructureProvidersViewQueryFuture<'_>;
}

struct HostInfrastructureProvidersViewHandler {
    query: Arc<dyn HostInfrastructureProvidersViewQuery>,
}

struct HostInfrastructureProvidersViewTargetError(crate::error_response::ApiError);

impl InterfaceContract for HostInfrastructureProvidersViewTargetError {
    const CONTRACT_ID: &'static str = HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_ID;
    const CONTRACT_VERSION: &'static str =
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_VERSION;
}

struct ProvidersViewAuthorizationGuard;

impl interface_runtime::InterfaceAuthorizationContribution for ProvidersViewAuthorizationGuard {
    fn authorize(
        &self,
        _request: interface_runtime::InterfaceAuthorizationContributionRequest,
    ) -> interface_runtime::InterfaceAuthorizationContributionFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct ProvidersViewAdmissionGuard;

impl interface_runtime::InterfaceAdmissionContribution for ProvidersViewAdmissionGuard {
    fn admit(
        &self,
        _request: interface_runtime::InterfaceAdmissionContributionRequest,
    ) -> interface_runtime::InterfaceAdmissionContributionFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

impl
    InterfaceHandler<
        HostInfrastructureProvidersViewInput,
        HostInfrastructureProvidersViewOutput,
        HostInfrastructureProvidersViewTargetError,
    > for HostInfrastructureProvidersViewHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: HostInfrastructureProvidersViewInput,
    ) -> InterfaceHandlerFuture<
        HostInfrastructureProvidersViewOutput,
        HostInfrastructureProvidersViewTargetError,
    > {
        let query = Arc::clone(&self.query);
        Box::pin(async move {
            query
                .list()
                .await
                .map(|list| HostInfrastructureProvidersViewOutput {
                    providers: list
                        .providers
                        .into_iter()
                        .map(to_provider_response)
                        .collect(),
                })
                .map_err(|error| {
                    InterfaceTargetFailure::new(
                        "host_infrastructure_providers_view",
                        HostInfrastructureProvidersViewTargetError(error),
                    )
                })
        })
    }
}

struct ConsoleInterfaceAuthorizationPort {
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
}

impl InterfaceAuthorizationPort for ConsoleInterfaceAuthorizationPort {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.console.compiled-operation")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let policy_reader = Arc::clone(&self.policy_reader);
        let console_registry = Arc::clone(&self.console_registry);
        Box::pin(async move {
            // All protocol projections retain the canonical Console operation's authorization
            // truth. Non-HTTP bindings do not manufacture an HTTP projection, but they must
            // resolve the same compiled operation and policy group before dispatch.
            let (method, path) = request
                .binding()
                .projection()
                .http_route()
                .map(|route| (route.method(), route.path()))
                .unwrap_or(("GET", HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH));
            let access = crate::middleware::require_settings_feature_permission::compiled_console_route_access(
                &console_registry,
                method,
                path,
            )
            .map_err(InterfaceAuthorizationError::classified)?;
            if request.principal().actor().is_root {
                return Ok(());
            }
            let policies = policy_reader
                .load_role_console_policies_for_user(request.principal().actor())
                .await
                .map_err(|error| {
                    InterfaceAuthorizationError::with_source(
                        "console_authorization_unavailable",
                        crate::error_response::ApiError::from(error),
                    )
                })?;
            if crate::middleware::require_settings_feature_permission::authorize_compiled_console_access(
                &access,
                request.principal().actor(),
                &policies,
            ) {
                Ok(())
            } else {
                Err(InterfaceAuthorizationError::with_source(
                    "console_operation_permission_denied",
                    crate::error_response::ApiError::from(
                        control_plane::errors::ControlPlaneError::PermissionDenied(
                            "console_operation_permission_denied",
                        ),
                    ),
                ))
            }
        })
    }
}

pub(crate) fn invocation_kernel(
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
) -> Arc<InterfaceInvocationKernel> {
    Arc::new(InterfaceInvocationKernel::new(Arc::new(
        ConsoleInterfaceAuthorizationPort {
            policy_reader,
            console_registry,
        },
    )))
}

pub(crate) async fn invoke_providers_view(
    state: Arc<ApiState>,
    credential: crate::extension_bus::ConsoleAuthenticationCredential,
    protocol: InterfaceProtocol,
) -> Result<
    (
        HostInfrastructureProvidersViewOutput,
        interface_runtime::InterfaceInvocationReceipt,
    ),
    crate::error_response::ApiError,
> {
    let boot_snapshot = state.extension_boot_snapshot.as_ref().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("interface_operation"),
    )?;
    let snapshot = boot_snapshot
        .interface_registry()
        .map(|registry| registry.snapshot())
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "interface_operation",
        ))?;
    let kernel = invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    );
    let binding_id = BindingId::new(match protocol {
        InterfaceProtocol::Mcp => HOST_INFRASTRUCTURE_PROVIDERS_VIEW_MCP_BINDING_ID,
        InterfaceProtocol::Http => HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID,
        InterfaceProtocol::Internal | InterfaceProtocol::Worker => {
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERNAL_BINDING_ID
        }
    })
    .expect("built-in binding identity must remain valid");
    let activated_authentication = snapshot.authentication(&binding_id).cloned().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("authentication_activation"),
    )?;
    let principal: UserPrincipal = boot_snapshot
        .authenticate(&activated_authentication, credential)
        .await
        .map_err(crate::error_response::ApiError::from)?;
    let authentication_activation = activated_authentication.activation().clone();
    match kernel
        .invoke::<
            HostInfrastructureProvidersViewInput,
            HostInfrastructureProvidersViewOutput,
            HostInfrastructureProvidersViewTargetError,
        >(
            snapshot,
            InvocationEnvelope::with_principal(
                InvocationLineage::root(InvocationId::now_v7()),
                binding_id,
                protocol,
                activated_authentication.adapter().clone(),
                authentication_activation,
                principal,
                None,
                HostInfrastructureProvidersViewInput::new(),
            ),
        )
        .await
    {
        Ok(outcome) => {
            let receipt = outcome.receipt().clone().projected();
            Ok((outcome.into_value(), receipt))
        }
        Err(failure) => Err(invocation_failure_api_error(failure.into_error())),
    }
}

fn invocation_failure_api_error(
    error: InterfaceInvocationError,
) -> crate::error_response::ApiError {
    match error {
        InterfaceInvocationError::AuthorizationRejected(error) => error
            .into_source::<crate::error_response::ApiError>()
            .unwrap_or_else(|| {
                control_plane::errors::ControlPlaneError::PermissionDenied(
                    "console_operation_permission_denied",
                )
                .into()
            }),
        InterfaceInvocationError::AuthorizationContributionRejected(_) => {
            control_plane::errors::ControlPlaneError::PermissionDenied(
                "console_operation_permission_denied",
            )
            .into()
        }
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<HostInfrastructureProvidersViewTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("host infrastructure providers view failed").into()),
        InterfaceInvocationError::UnknownBinding => {
            control_plane::errors::ControlPlaneError::NotFound("interface_operation").into()
        }
        InterfaceInvocationError::ContractMismatch
        | InterfaceInvocationError::ProtocolBindingMismatch
        | InterfaceInvocationError::AuthenticationAdapterMismatch
        | InterfaceInvocationError::AuthenticationActivationMismatch
        | InterfaceInvocationError::AuthorizationAdapterMismatch
        | InterfaceInvocationError::AdmissionAdapterMismatch
        | InterfaceInvocationError::PrincipalProfileMismatch
        | InterfaceInvocationError::HookPlanFingerprintMismatch => {
            control_plane::errors::ControlPlaneError::Conflict("interface_contract").into()
        }
        InterfaceInvocationError::BeforeHookRejected(error) => {
            anyhow::anyhow!(error.to_string()).into()
        }
        InterfaceInvocationError::AdmissionRejected(error) => {
            anyhow::anyhow!(error.to_string()).into()
        }
        InterfaceInvocationError::AdmissionContributionRejected(error) => {
            anyhow::anyhow!(error.to_string()).into()
        }
        InterfaceInvocationError::DeadlineElapsed => {
            anyhow::anyhow!("interface invocation deadline elapsed").into()
        }
        InterfaceInvocationError::Cancelled => {
            anyhow::anyhow!("interface invocation was cancelled").into()
        }
    }
}

pub(crate) fn compile_interface_registry(
    graph: Arc<EffectiveExtensionGraph>,
    descriptors: &[HostExtensionInterfaceOperationManifest],
    active_extensions: &[(
        plugin_framework::PluginManifestV1,
        HostExtensionContributionManifest,
    )],
    providers_view_query: Arc<dyn HostInfrastructureProvidersViewQuery>,
    providers_view_hooks: Arc<
        interface_runtime::TypedInterfaceHookPlan<
            HostInfrastructureProvidersViewInput,
            HostInfrastructureProvidersViewOutput,
        >,
    >,
) -> Result<Arc<CompiledInterfaceRegistry>> {
    let descriptor = validate_active_providers_view(&graph, descriptors)?;
    let binding = binding_from_descriptor(&descriptor)?;
    let definition = definition_from_descriptor(descriptor)?;
    let interface_id = definition.interface_id().clone();
    let activated_authentication =
        providers_view_authentication(&graph, active_extensions, &interface_id)?;
    let handler_reference = definition.handler_reference().clone();
    let graph_fingerprint = GraphFingerprint::new(graph.fingerprint().as_str())?;
    let owner = InterfaceOwner::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID)?;
    let mut compiler = RegistryCompiler::new(
        graph_fingerprint.clone(),
        [AuthorizationOperation::new(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
        )?],
        [owner],
    );
    let adapter_plan = || {
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.console.require-session")
                .expect("static authentication adapter is valid"),
            AuthorizationAdapterReference::new("api-server.console.compiled-operation")
                .expect("static authorization adapter is valid"),
            None,
        )
    };
    let definition_contribution = interface_runtime::TypedInterfaceDefinitionContribution::<
        HostInfrastructureProvidersViewInput,
        HostInfrastructureProvidersViewOutput,
        HostInfrastructureProvidersViewTargetError,
        UserPrincipal,
    >::new(
        definition.clone(),
        [
            interface_runtime::ContributedProtocolBinding::new(binding, adapter_plan()),
            interface_runtime::ContributedProtocolBinding::new(
                ProtocolBinding::new(
                    BindingId::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_MCP_BINDING_ID)?,
                    definition.identity().clone(),
                    definition.contracts().clone(),
                    ProtocolProjection::mcp(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID),
                ),
                adapter_plan(),
            ),
            interface_runtime::ContributedProtocolBinding::new(
                ProtocolBinding::new(
                    BindingId::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERNAL_BINDING_ID)?,
                    definition.identity().clone(),
                    definition.contracts().clone(),
                    ProtocolProjection::internal(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID),
                ),
                adapter_plan(),
            ),
        ],
    )?;
    compiler.register_definition_contribution(
        0,
        InterfaceExtensionRegistration::new(
            PluginIdentity::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID)?,
            InterfaceExtensionTier::HostExtension,
            InterfaceExtensionPoint::Definition,
            InterfaceExtensionPermission::Define,
            InterfaceScope::System,
            InterfaceExtensionIsolation::TrustedInProcess,
            [
                InterfaceExtensionFact::DefinitionIdentity,
                InterfaceExtensionFact::BindingIdentity,
            ],
        )?,
        Arc::new(definition_contribution),
    );
    compiler.register_authentication_adapter(
        &interface_id,
        1,
        InterfaceExtensionRegistration::new(
            activated_authentication.plugin().clone(),
            activated_authentication.tier(),
            InterfaceExtensionPoint::AuthenticationAdapter,
            InterfaceExtensionPermission::Authenticate,
            InterfaceScope::System,
            InterfaceExtensionIsolation::TrustedInProcess,
            [],
        )?,
        activated_authentication,
    )?;
    let authorization_plugin = PluginIdentity::new("api-server.providers-view-authorization")?;
    compiler.register_extension(
        &interface_id,
        10,
        InterfaceExtensionRegistration::new(
            authorization_plugin.clone(),
            InterfaceExtensionTier::HostExtension,
            InterfaceExtensionPoint::Authorization,
            InterfaceExtensionPermission::Authorize,
            InterfaceScope::System,
            InterfaceExtensionIsolation::TrustedInProcess,
            [
                InterfaceExtensionFact::DefinitionIdentity,
                InterfaceExtensionFact::PrincipalSummary,
            ],
        )?,
    )?;
    compiler.bind_authorization_plan(
        &interface_id,
        Arc::new(
            interface_runtime::TypedInterfaceAuthorizationPlan::<
                HostInfrastructureProvidersViewInput,
                HostInfrastructureProvidersViewOutput,
            >::new(graph_fingerprint.clone())
            .bind(
                authorization_plugin,
                Arc::new(ProvidersViewAuthorizationGuard),
            ),
        ),
    )?;
    let admission_plugin = PluginIdentity::new("api-server.providers-view-admission")?;
    compiler.register_extension(
        &interface_id,
        20,
        InterfaceExtensionRegistration::new(
            admission_plugin.clone(),
            InterfaceExtensionTier::HostExtension,
            InterfaceExtensionPoint::Admission,
            InterfaceExtensionPermission::Admit,
            InterfaceScope::System,
            InterfaceExtensionIsolation::TrustedInProcess,
            [
                InterfaceExtensionFact::DefinitionIdentity,
                InterfaceExtensionFact::PrincipalSummary,
                InterfaceExtensionFact::AuthorizationDecision,
            ],
        )?,
    )?;
    compiler.bind_admission_plan(
        &interface_id,
        Arc::new(
            interface_runtime::TypedInterfaceAdmissionPlan::<
                HostInfrastructureProvidersViewInput,
                HostInfrastructureProvidersViewOutput,
            >::new(graph_fingerprint)
            .bind(admission_plugin, Arc::new(ProvidersViewAdmissionGuard)),
        ),
    )?;
    compiler.register_extension(
        &interface_id,
        100,
        InterfaceExtensionRegistration::new(
            PluginIdentity::new("api-server.providers-view-completion")?,
            InterfaceExtensionTier::BuiltIn,
            InterfaceExtensionPoint::Completion,
            InterfaceExtensionPermission::ObserveCompletion,
            InterfaceScope::System,
            InterfaceExtensionIsolation::TrustedInProcess,
            [
                InterfaceExtensionFact::Terminal,
                InterfaceExtensionFact::InvocationIdentity,
            ],
        )?,
    )?;
    compiler.bind_handler::<
        HostInfrastructureProvidersViewInput,
        HostInfrastructureProvidersViewOutput,
        HostInfrastructureProvidersViewTargetError,
        UserPrincipal,
    >(
        &interface_id,
        handler_reference,
        Arc::new(HostInfrastructureProvidersViewHandler {
            query: providers_view_query,
        }),
    )?;
    compiler.bind_hook_plan(&interface_id, providers_view_hooks)?;
    Ok(compiler.compile()?)
}

pub(crate) fn is_active_interface_route(state: &ApiState, method: &str, path: &str) -> bool {
    state
        .extension_boot_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.interface_registry())
        .map(|registry| registry.snapshot())
        .is_some_and(|snapshot| {
            providers_view_definition(snapshot.as_ref())
                .ok()
                .and_then(|definition| {
                    snapshot
                        .plan_for_interface(definition.interface_id())
                        .and_then(|plan| plan.binding().projection().http_route())
                })
                .is_some_and(|route| route.method() == method && route.path() == path)
        })
}

#[cfg(test)]
pub(crate) struct UnavailableHostInfrastructureProvidersViewQuery;

#[cfg(test)]
impl HostInfrastructureProvidersViewQuery for UnavailableHostInfrastructureProvidersViewQuery {
    fn list(&self) -> HostInfrastructureProvidersViewQueryFuture<'_> {
        Box::pin(async {
            Err(anyhow::anyhow!("providers view query fixture is unavailable").into())
        })
    }
}

pub fn providers_view_definition(
    registry: &CompiledInterfaceRegistry,
) -> Result<&InterfaceDefinition> {
    registry
        .definition(&InterfaceId::new(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID,
        )?)
        .ok_or_else(|| anyhow::anyhow!("host infrastructure providers view is absent"))
}

pub fn validate_console_registry(
    interface_registry: &CompiledInterfaceRegistry,
    console_registry: &ConsoleOperationRegistry,
) -> Result<()> {
    let definition = providers_view_definition(interface_registry)?;
    let route = interface_registry
        .plan_for_interface(definition.interface_id())
        .and_then(|plan| plan.binding().projection().http_route())
        .ok_or_else(|| anyhow::anyhow!("host infrastructure providers view route is absent"))?;
    let access = console_registry.access_for_console_route(route.method(), route.path())?;
    if access.operation_id != definition.interface_id().as_str()
        || access.authorization != &ConsoleAuthorization::Simple
        || access.policy_group
            != &ConsolePolicyGroup::SettingsFeature("system.host-infrastructure".to_string())
    {
        bail!("host infrastructure providers view ConsoleOperation contract mismatch");
    }
    let operation = console_registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == definition.interface_id().as_str())
        .ok_or_else(|| {
            anyhow::anyhow!("host infrastructure providers view ConsoleOperation is absent")
        })?;
    if operation.owner.kind != access_control::SettingsFeatureOwnerKind::Core
        || operation.owner.owner_id != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONSOLE_OWNER_ID
    {
        bail!("host infrastructure providers view must retain Core authorization ownership");
    }
    Ok(())
}

pub(crate) fn official_local_infra_host_providers_view_descriptor(
) -> HostExtensionInterfaceOperationManifest {
    HostExtensionInterfaceOperationManifest {
        operation_id: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID.to_string(),
        method: HostExtensionInterfaceOperationMethod::Get,
        path: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH.to_string(),
        input: plugin_framework::HostExtensionInterfaceOperationContractManifest {
            contract_id: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_ID.to_string(),
            contract_version: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_VERSION.to_string(),
        },
        output: plugin_framework::HostExtensionInterfaceOperationContractManifest {
            contract_id: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_ID.to_string(),
            contract_version: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_VERSION
                .to_string(),
        },
        required_core_permission: HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION.to_string(),
        auth_policy: HostExtensionInterfaceOperationAuthPolicy::CoreConsoleOperation,
        audit_policy: HostExtensionInterfaceOperationAuditPolicy::ReadOnly,
        error_policy: HostExtensionInterfaceOperationErrorPolicy::CoreApiError,
    }
}

pub(crate) fn host_infrastructure_providers_view_console_path() -> &'static str {
    HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
        .strip_prefix("/api/console")
        .expect("providers view path must remain under /api/console")
}

fn definition_from_descriptor(
    descriptor: HostExtensionInterfaceOperationManifest,
) -> Result<InterfaceDefinition> {
    Ok(InterfaceDefinition::new(
        InterfaceIdentity::new(
            InterfaceId::new(&descriptor.operation_id)?,
            InterfaceVersion::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERFACE_VERSION)?,
        ),
        InterfaceContracts::unary(
            ContractIdentity::new(
                &descriptor.input.contract_id,
                &descriptor.input.contract_version,
            )?,
            ContractIdentity::new(
                &descriptor.output.contract_id,
                &descriptor.output.contract_version,
            )?,
            ContractIdentity::new(
                HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_ID,
                HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_VERSION,
            )?,
        ),
        InterfaceAccess::new(
            PrincipalProfile::User,
            InterfaceAuthenticationPolicy::Authenticated,
            AuthorizationOperation::new(&descriptor.required_core_permission)?,
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
            InterfaceExecutionMode::Unary,
            HandlerReference::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_HANDLER_REFERENCE)?,
            TargetReference::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_REFERENCE)?,
        ),
        InterfaceAuditPolicy::ReadOnly,
        InterfaceErrorPolicy::TypedTarget,
        InterfaceLifecycle::BootSnapshot,
        InterfaceOwner::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID)?,
    ))
}

fn binding_from_descriptor(
    descriptor: &HostExtensionInterfaceOperationManifest,
) -> Result<ProtocolBinding> {
    Ok(ProtocolBinding::new(
        BindingId::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID)?,
        InterfaceIdentity::new(
            InterfaceId::new(&descriptor.operation_id)?,
            InterfaceVersion::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERFACE_VERSION)?,
        ),
        InterfaceContracts::unary(
            ContractIdentity::new(
                &descriptor.input.contract_id,
                &descriptor.input.contract_version,
            )?,
            ContractIdentity::new(
                &descriptor.output.contract_id,
                &descriptor.output.contract_version,
            )?,
            ContractIdentity::new(
                HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_ID,
                HOST_INFRASTRUCTURE_PROVIDERS_VIEW_TARGET_ERROR_CONTRACT_VERSION,
            )?,
        ),
        ProtocolProjection::http(RouteIdentity::new(
            descriptor.method.as_str(),
            &descriptor.path,
        )?),
    ))
}

fn validate_active_providers_view(
    graph: &EffectiveExtensionGraph,
    descriptors: &[HostExtensionInterfaceOperationManifest],
) -> Result<HostExtensionInterfaceOperationManifest> {
    let canonical = official_local_infra_host_providers_view_descriptor();
    let mut matching = descriptors.iter().filter(|descriptor| {
        descriptor.operation_id == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    });
    let descriptor = matching.next().cloned().ok_or_else(|| {
        anyhow::anyhow!("host infrastructure providers view descriptor is absent")
    })?;
    if matching.next().is_some() {
        bail!("host infrastructure providers view descriptor is not unique");
    }
    if descriptor != canonical {
        bail!("host infrastructure providers view interface operation contract mismatch");
    }
    let point = graph
        .points()
        .iter()
        .find(|point| point.descriptor().point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .ok_or_else(|| anyhow::anyhow!("interface operation extension point is unavailable"))?;
    let point_descriptor = point.descriptor();
    if point_descriptor.owner_module_id.as_str() != INTERFACE_OPERATION_OWNER_MODULE_ID
        || point_descriptor.point_kind != ExtensionPointKind::Contribution
        || point_descriptor.contract.contract_id.as_str() != INTERFACE_OPERATION_CONTRACT_ID
        || point_descriptor.contract.contract_version.as_str()
            != INTERFACE_OPERATION_CONTRACT_VERSION
        || point_descriptor.scope != ScopeSemantics::System
        || point_descriptor.cardinality != Cardinality::Many
        || point_descriptor.ordering != OrderingSemantics::Lexicographic
        || point_descriptor.failure != FailureSemantics::FailClosed
        || point_descriptor.delivery != DeliverySemantics::Synchronous
        || point_descriptor.lifecycle != LifecycleSemantics::BootSnapshot
        || point_descriptor.override_policy != OverridePolicy::Sealed
        || !point_descriptor
            .allowed_permissions
            .iter()
            .any(|permission| permission.as_str() == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION)
    {
        bail!("interface operation extension point contract mismatch");
    }
    let mut contributions = point.contributions().iter().filter(|contribution| {
        contribution.descriptor().contribution_id.as_str()
            == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID
    });
    let contribution = contributions.next().ok_or_else(|| {
        anyhow::anyhow!("host infrastructure providers view contribution is not active")
    })?;
    if contributions.next().is_some()
        || contribution.descriptor().contributor_module_id.as_str()
            != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        || contribution.provenance().module_kind() != ModuleKind::TrustedHost
        || contribution.provenance().module_id().as_str()
            != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        || contribution
            .descriptor()
            .required_permissions
            .iter()
            .map(|permission| permission.as_str())
            .collect::<Vec<_>>()
            != [HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION]
    {
        bail!("host infrastructure providers view contribution contract mismatch");
    }
    Ok(descriptor)
}

fn providers_view_authentication(
    graph: &EffectiveExtensionGraph,
    active_extensions: &[(
        plugin_framework::PluginManifestV1,
        HostExtensionContributionManifest,
    )],
    interface_id: &InterfaceId,
) -> Result<interface_runtime::ActivatedAuthenticationAdapter> {
    let mut candidates = active_extensions.iter().flat_map(|(_, contribution)| {
        contribution
            .interface_authentication_adapters
            .iter()
            .filter(move |descriptor| descriptor.interface_id == interface_id.as_str())
            .map(move |descriptor| (contribution, descriptor))
    });
    let Some((contribution, descriptor)) = candidates.next() else {
        return Ok(interface_runtime::ActivatedAuthenticationAdapter::new(
            PluginIdentity::new("api-server.console-authentication")?,
            InterfaceExtensionTier::BuiltIn,
            AuthenticationAdapterReference::new("api-server.console.require-session")?,
            interface_runtime::AuthenticationActivationIdentity::new(
                "api-server.console.require-session.activation.v1",
            )?,
            PrincipalProfile::User,
        ));
    };
    if candidates.next().is_some() {
        bail!("providers view Authentication contribution is not unique");
    }
    validate_providers_view_authentication_descriptor(graph, contribution, descriptor)?;
    crate::extension_bus::activated_host_authentication(&contribution.extension_id, descriptor)
}

fn validate_providers_view_authentication_descriptor(
    graph: &EffectiveExtensionGraph,
    contribution: &HostExtensionContributionManifest,
    descriptor: &HostExtensionInterfaceAuthenticationManifest,
) -> Result<()> {
    let expected_bindings = BTreeSet::from([
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_BINDING_ID,
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_MCP_BINDING_ID,
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERNAL_BINDING_ID,
    ]);
    let actual_bindings = descriptor
        .binding_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if descriptor.interface_version != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INTERFACE_VERSION
        || actual_bindings != expected_bindings
        || descriptor.adapter_id != "api-server.console.require-session"
        || descriptor.principal_profile
            != HostExtensionInterfaceAuthenticationPrincipalProfile::User
        || descriptor.credential.contract_id
            != crate::extension_bus::CONSOLE_SESSION_CREDENTIAL_CONTRACT_ID
        || descriptor.credential.contract_version
            != crate::extension_bus::CONSOLE_SESSION_CREDENTIAL_CONTRACT_VERSION
    {
        bail!("providers view Authentication contribution contract mismatch");
    }
    let point = graph
        .points()
        .iter()
        .find(|point| {
            point.descriptor().point_id.as_str()
                == crate::extension_bus::INTERFACE_AUTHENTICATION_ADAPTER_POINT_ID
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Interface Authentication extension point is unavailable")
        })?;
    let point_descriptor = point.descriptor();
    if point_descriptor.owner_module_id.as_str() != crate::extension_bus::BOOT_CORE_MODULE_ID
        || point_descriptor.point_kind != ExtensionPointKind::Contribution
        || point_descriptor.contract.contract_id.as_str()
            != crate::extension_bus::INTERFACE_AUTHENTICATION_ADAPTER_CONTRACT_ID
        || point_descriptor.contract.contract_version.as_str()
            != crate::extension_bus::INTERFACE_AUTHENTICATION_ADAPTER_CONTRACT_VERSION
        || point_descriptor.scope != ScopeSemantics::System
        || point_descriptor.cardinality != Cardinality::Many
        || point_descriptor.ordering != OrderingSemantics::Lexicographic
        || point_descriptor.failure != FailureSemantics::FailClosed
        || point_descriptor.delivery != DeliverySemantics::Synchronous
        || point_descriptor.lifecycle != LifecycleSemantics::BootSnapshot
        || point_descriptor.override_policy != OverridePolicy::Sealed
    {
        bail!("Interface Authentication extension point contract mismatch");
    }
    let mut graph_contributions = point.contributions().iter().filter(|candidate| {
        candidate.descriptor().contribution_id.as_str() == descriptor.contribution_id
    });
    let graph_contribution = graph_contributions.next().ok_or_else(|| {
        anyhow::anyhow!("HostExtension Authentication contribution is not active")
    })?;
    if graph_contributions.next().is_some()
        || graph_contribution
            .descriptor()
            .contributor_module_id
            .as_str()
            != contribution.extension_id
        || graph_contribution.provenance().module_kind() != ModuleKind::TrustedHost
        || graph_contribution.provenance().module_id().as_str() != contribution.extension_id
    {
        bail!("HostExtension Authentication contribution graph identity mismatch");
    }
    Ok(())
}
