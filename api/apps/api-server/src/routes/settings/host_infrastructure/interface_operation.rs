use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use access_control::{
    ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup, SettingsFeatureOwnerKind,
};
use anyhow::{bail, Result};
use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleKind, OrderingSemantics, OverridePolicy, Provenance, ScopeSemantics,
};
use plugin_framework::{
    HostExtensionInterfaceOperationAuditPolicy, HostExtensionInterfaceOperationAuthPolicy,
    HostExtensionInterfaceOperationErrorPolicy, HostExtensionInterfaceOperationManifest,
    HostExtensionInterfaceOperationMethod,
};

use super::{list_host_infrastructure_providers_typed, HostInfrastructureProviderConfigResponse};
use crate::{app_state::ApiState, error_response::ApiError};

pub const INTERFACE_OPERATION_POINT_ID: &str = "1flowbase.application.interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_ID: &str = "interface-operation";
pub const INTERFACE_OPERATION_CONTRACT_VERSION: &str = "1";
pub const HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID: &str =
    "host_infrastructure.providers.view";
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

pub trait InterfaceOperationSchema: Send + Sync + 'static {
    type Value: Send + 'static;
    const SCHEMA_ID: &'static str;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostInfrastructureProvidersViewInput;

#[derive(Debug, Clone)]
pub struct HostInfrastructureProvidersViewInputSchema;

impl InterfaceOperationSchema for HostInfrastructureProvidersViewInputSchema {
    type Value = HostInfrastructureProvidersViewInput;
    const SCHEMA_ID: &'static str = HOST_INFRASTRUCTURE_PROVIDERS_VIEW_INPUT_CONTRACT_ID;
}

#[derive(Debug, Clone)]
pub struct HostInfrastructureProvidersViewOutputSchema;

impl InterfaceOperationSchema for HostInfrastructureProvidersViewOutputSchema {
    type Value = Vec<HostInfrastructureProviderConfigResponse>;
    const SCHEMA_ID: &'static str = HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OUTPUT_CONTRACT_ID;
}

#[derive(Debug, Clone)]
pub struct InterfaceOperationDefinition<I, O>
where
    I: InterfaceOperationSchema,
    O: InterfaceOperationSchema,
{
    descriptor: HostExtensionInterfaceOperationManifest,
    marker: PhantomData<fn(I) -> O>,
}

impl<I, O> InterfaceOperationDefinition<I, O>
where
    I: InterfaceOperationSchema,
    O: InterfaceOperationSchema,
{
    pub fn descriptor(&self) -> &HostExtensionInterfaceOperationManifest {
        &self.descriptor
    }
}

type InterfaceOperationFuture<O> =
    Pin<Box<dyn Future<Output = std::result::Result<O, ApiError>> + Send + 'static>>;
type InterfaceOperationHandler<I, O> = fn(Arc<ApiState>, I) -> InterfaceOperationFuture<O>;

pub type HostInfrastructureProvidersViewDefinition = InterfaceOperationDefinition<
    HostInfrastructureProvidersViewInputSchema,
    HostInfrastructureProvidersViewOutputSchema,
>;

#[derive(Debug, Clone)]
pub struct InterfaceOperationBinding<I, O>
where
    I: InterfaceOperationSchema,
    O: InterfaceOperationSchema,
{
    definition: InterfaceOperationDefinition<I, O>,
    graph: Arc<EffectiveExtensionGraph>,
    provenance: Provenance,
    handler: InterfaceOperationHandler<I::Value, O::Value>,
}

impl<I, O> InterfaceOperationBinding<I, O>
where
    I: InterfaceOperationSchema,
    O: InterfaceOperationSchema,
{
    pub fn definition(&self) -> &InterfaceOperationDefinition<I, O> {
        &self.definition
    }

    pub fn graph_fingerprint(&self) -> &str {
        self.graph.fingerprint().as_str()
    }

    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn owner_module_id(&self) -> &'static str {
        INTERFACE_OPERATION_OWNER_MODULE_ID
    }

    pub async fn dispatch(
        &self,
        state: Arc<ApiState>,
        input: I::Value,
    ) -> std::result::Result<O::Value, ApiError> {
        (self.handler)(state, input).await
    }
}

pub type HostInfrastructureProvidersViewBinding = InterfaceOperationBinding<
    HostInfrastructureProvidersViewInputSchema,
    HostInfrastructureProvidersViewOutputSchema,
>;

impl
    InterfaceOperationBinding<
        HostInfrastructureProvidersViewInputSchema,
        HostInfrastructureProvidersViewOutputSchema,
    >
{
    pub fn validate_console_registry(&self, registry: &ConsoleOperationRegistry) -> Result<()> {
        let descriptor = self.definition.descriptor();
        let access =
            registry.access_for_console_route(descriptor.method.as_str(), &descriptor.path)?;
        if access.operation_id != descriptor.operation_id
            || access.authorization != &ConsoleAuthorization::Simple
            || access.policy_group
                != &ConsolePolicyGroup::SettingsFeature("system.host-infrastructure".to_string())
        {
            bail!("host infrastructure providers view ConsoleOperation contract mismatch");
        }
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == descriptor.operation_id)
            .ok_or_else(|| {
                anyhow::anyhow!("host infrastructure providers view ConsoleOperation is absent")
            })?;
        if operation.owner.kind != SettingsFeatureOwnerKind::Core
            || operation.owner.owner_id != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONSOLE_OWNER_ID
        {
            bail!("host infrastructure providers view must retain Core authorization ownership");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceOperationCatalog {
    providers_view: HostInfrastructureProvidersViewBinding,
}

impl InterfaceOperationCatalog {
    pub(crate) fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[HostExtensionInterfaceOperationManifest],
    ) -> Result<Self> {
        let mut providers_view_descriptors = descriptors.iter().filter(|descriptor| {
            descriptor.operation_id == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
        });
        let descriptor = providers_view_descriptors.next().cloned().ok_or_else(|| {
            anyhow::anyhow!("host infrastructure providers view descriptor is absent")
        })?;
        if providers_view_descriptors.next().is_some() {
            bail!("host infrastructure providers view descriptor is not unique");
        }
        let providers_view = bind_providers_view(graph, descriptor)?;
        Ok(Self { providers_view })
    }

    pub fn providers_view(&self) -> &HostInfrastructureProvidersViewBinding {
        &self.providers_view
    }
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

pub fn host_infrastructure_providers_view_definition() -> HostInfrastructureProvidersViewDefinition
{
    InterfaceOperationDefinition {
        descriptor: official_local_infra_host_providers_view_descriptor(),
        marker: PhantomData,
    }
}

pub(crate) fn host_infrastructure_providers_view_console_path() -> &'static str {
    HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
        .strip_prefix("/api/console")
        .expect("providers view path must remain under /api/console")
}

fn bind_providers_view(
    graph: Arc<EffectiveExtensionGraph>,
    descriptor: HostExtensionInterfaceOperationManifest,
) -> Result<HostInfrastructureProvidersViewBinding> {
    let canonical = official_local_infra_host_providers_view_descriptor();
    if descriptor != canonical
        || canonical.input.contract_id != HostInfrastructureProvidersViewInputSchema::SCHEMA_ID
        || canonical.output.contract_id != HostInfrastructureProvidersViewOutputSchema::SCHEMA_ID
    {
        bail!("host infrastructure providers view interface operation contract mismatch");
    }
    let point = graph
        .points()
        .iter()
        .find(|point| point.descriptor().point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .ok_or_else(|| anyhow::anyhow!("interface operation extension point is unavailable"))?;
    let point_descriptor = point.descriptor();
    let allowed_permission = point_descriptor
        .allowed_permissions
        .iter()
        .map(|permission| permission.as_str())
        .collect::<Vec<_>>();
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
        || !allowed_permission.contains(&HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION)
    {
        bail!("interface operation extension point contract mismatch");
    }
    let mut target_contributions = point.contributions().iter().filter(|contribution| {
        contribution.descriptor().contribution_id.as_str()
            == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID
    });
    let contribution = target_contributions.next().ok_or_else(|| {
        anyhow::anyhow!("host infrastructure providers view contribution is not active")
    })?;
    if target_contributions.next().is_some() {
        bail!("host infrastructure providers view contribution is not unique");
    }
    let required_permission = contribution
        .descriptor()
        .required_permissions
        .iter()
        .map(|permission| permission.as_str())
        .collect::<Vec<_>>();
    if contribution.descriptor().contribution_id.as_str()
        != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTION_ID
        || contribution.descriptor().contributor_module_id.as_str()
            != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        || contribution.provenance().module_kind() != ModuleKind::TrustedHost
        || contribution.provenance().module_id().as_str()
            != HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        || required_permission != [HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION]
    {
        bail!("host infrastructure providers view contribution contract mismatch");
    }

    let provenance = contribution.provenance().clone();
    Ok(InterfaceOperationBinding {
        definition: host_infrastructure_providers_view_definition(),
        graph,
        provenance,
        handler: dispatch_host_infrastructure_providers_view,
    })
}

fn dispatch_host_infrastructure_providers_view(
    state: Arc<ApiState>,
    _input: HostInfrastructureProvidersViewInput,
) -> InterfaceOperationFuture<Vec<HostInfrastructureProviderConfigResponse>> {
    Box::pin(async move { list_host_infrastructure_providers_typed(state.as_ref()).await })
}
