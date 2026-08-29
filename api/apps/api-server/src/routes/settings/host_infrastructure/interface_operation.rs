use std::{future::Future, pin::Pin, sync::Arc};

use access_control::{ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup};
use anyhow::{bail, Result};
use control_plane::{
    host_infrastructure_config::HostInfrastructureProviderConfigList,
    ports::RoleConsolePolicyReader,
};
use interface_runtime::{
    AdmissionAdapterReference, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    ExtensionPlanFingerprint, GraphFingerprint, HandlerReference, InterfaceAccess,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceAuthorizationError,
    InterfaceAuthorizationFuture, InterfaceAuthorizationPort, InterfaceAuthorizationRequest,
    InterfaceContract, InterfaceContracts, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceExecution, InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture,
    InterfaceId, InterfaceIdentity, InterfaceInvocationError, InterfaceInvocationKernel,
    InterfaceLifecycle, InterfaceOwner, InterfaceProtocol, InterfaceScope, InterfaceTargetError,
    InterfaceVersion, InvocationAdapterPlan, InvocationEnvelope, InvocationId, InvocationLineage,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};
use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleKind, OrderingSemantics, OverridePolicy, ScopeSemantics,
};
use plugin_framework::{
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

impl InterfaceHandler<HostInfrastructureProvidersViewInput, HostInfrastructureProvidersViewOutput>
    for HostInfrastructureProvidersViewHandler
{
    fn invoke(
        &self,
        _context: InterfaceHandlerContext,
        _input: HostInfrastructureProvidersViewInput,
    ) -> InterfaceHandlerFuture<HostInfrastructureProvidersViewOutput> {
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
                    InterfaceTargetError::with_source("host_infrastructure_providers_view", error)
                })
        })
    }
}

struct ConsoleInterfaceAuthorizationPort {
    policy_reader: Arc<dyn RoleConsolePolicyReader>,
    console_registry: Arc<ConsoleOperationRegistry>,
}

impl InterfaceAuthorizationPort for ConsoleInterfaceAuthorizationPort {
    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest,
    ) -> InterfaceAuthorizationFuture<'_> {
        let policy_reader = Arc::clone(&self.policy_reader);
        let console_registry = Arc::clone(&self.console_registry);
        Box::pin(async move {
            let route = request.binding().projection().http_route().ok_or_else(|| {
                InterfaceAuthorizationError::classified("console_route_unregistered")
            })?;
            let access = crate::middleware::require_settings_feature_permission::compiled_console_route_access(
                &console_registry,
                route.method(),
                route.path(),
            )
            .map_err(InterfaceAuthorizationError::classified)?;
            if request.actor().is_root {
                return Ok(());
            }
            let policies = policy_reader
                .load_role_console_policies_for_user(request.actor())
                .await
                .map_err(|error| {
                    InterfaceAuthorizationError::with_source(
                        "console_authorization_unavailable",
                        crate::error_response::ApiError::from(error),
                    )
                })?;
            if crate::middleware::require_settings_feature_permission::authorize_compiled_console_access(
                &access,
                request.actor(),
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

pub async fn invoke_providers_view(
    state: Arc<ApiState>,
    actor: domain::ActorContext,
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
    let hook_plan = boot_snapshot.providers_view_hook_plan().ok_or(
        control_plane::errors::ControlPlaneError::NotFound("interface_hook_plan"),
    )?;
    let kernel = invocation_kernel(
        Arc::clone(&state.console_policy_reader),
        Arc::clone(&state.console_operation_registry),
    );
    match kernel
        .invoke_with_hook_plan::<
            HostInfrastructureProvidersViewInput,
            HostInfrastructureProvidersViewOutput,
        >(
            snapshot,
            InvocationEnvelope::new(
                InvocationLineage::root(InvocationId::now_v7()),
                InterfaceId::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID)
                    .expect("built-in interface identity must remain valid"),
                protocol,
                actor,
                None,
                HostInfrastructureProvidersViewInput::new(),
            ),
            hook_plan,
        )
        .await
    {
        Ok(outcome) => {
            let receipt = outcome.receipt().clone();
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
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<crate::error_response::ApiError>()
            .unwrap_or_else(|| anyhow::anyhow!("host infrastructure providers view failed").into()),
        InterfaceInvocationError::UnknownInterface => {
            control_plane::errors::ControlPlaneError::NotFound("interface_operation").into()
        }
        InterfaceInvocationError::ContractMismatch
        | InterfaceInvocationError::HookPlanFingerprintMismatch => {
            control_plane::errors::ControlPlaneError::Conflict("interface_contract").into()
        }
        InterfaceInvocationError::BeforeHookRejected(error) => {
            anyhow::anyhow!(error.to_string()).into()
        }
        InterfaceInvocationError::AdmissionRejected(error) => {
            anyhow::anyhow!(error.to_string()).into()
        }
        InterfaceInvocationError::DeadlineElapsed => {
            anyhow::anyhow!("interface invocation deadline elapsed").into()
        }
    }
}

pub(crate) fn compile_interface_registry(
    graph: Arc<EffectiveExtensionGraph>,
    descriptors: &[HostExtensionInterfaceOperationManifest],
    providers_view_query: Arc<dyn HostInfrastructureProvidersViewQuery>,
) -> Result<Arc<CompiledInterfaceRegistry>> {
    let descriptor = validate_active_providers_view(&graph, descriptors)?;
    let binding = binding_from_descriptor(&descriptor)?;
    let definition = definition_from_descriptor(descriptor)?;
    let interface_id = definition.interface_id().clone();
    let handler_reference = definition.handler_reference().clone();
    let graph_fingerprint = GraphFingerprint::new(graph.fingerprint().as_str())?;
    let owner = InterfaceOwner::new(HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID)?;
    let mut compiler = RegistryCompiler::new(
        graph_fingerprint,
        [AuthorizationOperation::new(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
        )?],
        [owner],
        InvocationAdapterPlan::new(
            AuthenticationAdapterReference::new("api-server.console.require-session")?,
            AuthorizationAdapterReference::new("api-server.console.compiled-operation")?,
            AdmissionAdapterReference::new("api-server.interface.no-admission")?,
            ExtensionPlanFingerprint::new("graph:providers-view-hooks")?,
        ),
    );
    compiler.register_binding(binding)?;
    compiler.register_definition(definition)?;
    compiler.bind_handler::<
        HostInfrastructureProvidersViewInput,
        HostInfrastructureProvidersViewOutput,
    >(
        &interface_id,
        handler_reference,
        Arc::new(HostInfrastructureProvidersViewHandler {
            query: providers_view_query,
        }),
    )?;
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
        ),
        InterfaceAccess::new(
            InterfaceAuthenticationPolicy::Authenticated,
            AuthorizationOperation::new(&descriptor.required_core_permission)?,
            InterfaceScope::System,
        ),
        InterfaceExecution::new(
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
        ContractIdentity::new(
            &descriptor.input.contract_id,
            &descriptor.input.contract_version,
        )?,
        ContractIdentity::new(
            &descriptor.output.contract_id,
            &descriptor.output.contract_version,
        )?,
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
