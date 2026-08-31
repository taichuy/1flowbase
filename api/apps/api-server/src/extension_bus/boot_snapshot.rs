use std::{num::NonZeroU64, sync::Arc};

use control_plane::host_infrastructure_config::HostInfrastructureConfigService;
use plugin_framework::extension_bus::{
    compile_hook_plans, ContractDescriptor, ContributionId, ContributionResolutionReceipt,
    EffectiveExtensionGraph, EffectiveExtensionPoint, HookHandlerBinding, HookHandlerContract,
    HookMutationCapability, HookPhase, HookPointBinding, HookPointContract, ModuleId,
    ModuleResolutionReceipt, Provenance,
};
use serde::Serialize;
use storage_durable_postgres::MainDurableStore;

use super::{
    input_assembly::ExtensionGraphInputAssembly,
    production_host_extension_authentication_factories, production_interface_contributions,
    AuthenticationAdapterFactoryBinding, AuthenticationAdapterFactoryRegistry,
    InterfaceContributionCollector, INTERFACE_COMPLETION_HOOK_CONTRIBUTION_ID,
    INTERFACE_COMPLETION_HOOK_POINT_ID,
};
const INTERFACE_COMPLETION_CONTEXT_CONTRACT_ID: &str = "interface-invocation-completion";
const INTERFACE_COMPLETION_CONTEXT_CONTRACT_VERSION: &str = "1";

struct ProvidersViewCompletionObserver;

impl interface_runtime::InterfaceCompletionHook for ProvidersViewCompletionObserver {
    fn completed(
        &self,
        context: interface_runtime::InterfaceHookContext,
        terminal: interface_runtime::InterfaceInvocationTerminal,
    ) -> interface_runtime::InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            tracing::debug!(
                invocation_id = %context.invocation_id().value(),
                graph_fingerprint = context.graph_fingerprint().as_str(),
                ?terminal,
                "interface invocation completed"
            );
        })
    }
}

pub(crate) struct DurableHostInfrastructureProvidersViewQuery {
    store: MainDurableStore,
    node_id: String,
}

impl DurableHostInfrastructureProvidersViewQuery {
    pub(crate) fn new(store: MainDurableStore, node_id: String) -> Self {
        Self { store, node_id }
    }
}

impl crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQuery
    for DurableHostInfrastructureProvidersViewQuery
{
    fn list(
        &self,
    ) -> crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQueryFuture<'_>
    {
        Box::pin(async move {
            Ok(
                HostInfrastructureConfigService::new(self.store.clone(), self.node_id.clone())
                    .list_providers()
                    .await?,
            )
        })
    }
}

pub const EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1: &str = "1flowbase.effective-extension-plan/v1";

pub struct ExtensionBootSnapshot {
    graph: Arc<EffectiveExtensionGraph>,
    interface_registry: Option<Arc<interface_runtime::DynamicInterfaceRegistry>>,
    authentication_factories: AuthenticationAdapterFactoryRegistry,
}

pub fn compile_extension_boot_snapshot(
    graph: Arc<EffectiveExtensionGraph>,
    assembly: &ExtensionGraphInputAssembly,
    store: MainDurableStore,
    api_node_id: String,
) -> anyhow::Result<ExtensionBootSnapshot> {
    let host_authentication_factories = production_host_extension_authentication_factories()
        .activate(assembly.host_extension_manifests())?;
    ExtensionBootSnapshot::compile(
        graph,
        assembly.interface_operations(),
        assembly.host_extension_manifests(),
        Arc::new(DurableHostInfrastructureProvidersViewQuery::new(
            store,
            api_node_id,
        )),
        host_authentication_factories,
    )
}

impl std::fmt::Debug for ExtensionBootSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtensionBootSnapshot")
            .field("graph_fingerprint", &self.graph.fingerprint().as_str())
            .field("has_interface_registry", &self.interface_registry.is_some())
            .finish()
    }
}

impl ExtensionBootSnapshot {
    #[cfg(test)]
    pub(crate) fn new(graph: Arc<EffectiveExtensionGraph>) -> Self {
        Self {
            graph,
            interface_registry: None,
            authentication_factories: AuthenticationAdapterFactoryRegistry::built_in()
                .expect("built-in authentication factories must be valid"),
        }
    }

    pub(crate) fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        active_extensions: &[(
            plugin_framework::PluginManifestV1,
            plugin_framework::HostExtensionContributionManifest,
        )],
        providers_view_query: Arc<
            dyn crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQuery,
        >,
        host_authentication_factories: Vec<AuthenticationAdapterFactoryBinding>,
    ) -> anyhow::Result<Self> {
        let hook_contract = HookPointContract {
            context: ContractDescriptor::new(
                INTERFACE_COMPLETION_CONTEXT_CONTRACT_ID,
                INTERFACE_COMPLETION_CONTEXT_CONTRACT_VERSION,
            )?,
            decision: None,
            phase: HookPhase::Completion,
            timeout_ms: NonZeroU64::new(1_000).expect("hook timeout is non-zero"),
            mutation: HookMutationCapability::ObserveOnly,
        };
        let effective_plans = compile_hook_plans(
            &graph,
            vec![HookPointBinding::new(
                plugin_framework::extension_bus::ExtensionPointId::new(
                    INTERFACE_COMPLETION_HOOK_POINT_ID,
                )?,
                hook_contract.clone(),
            )],
            vec![HookHandlerBinding::new(
                ContributionId::new(INTERFACE_COMPLETION_HOOK_CONTRIBUTION_ID)?,
                HookHandlerContract {
                    context: hook_contract.context,
                    decision: None,
                    phase: HookPhase::Completion,
                },
            )],
        )?;
        if effective_plans.len() != 1 {
            anyhow::bail!("providers view completion hook plan is not unique");
        }
        let providers_view_hook_plan = Arc::new(
            interface_runtime::TypedInterfaceHookPlan::new(
                interface_runtime::GraphFingerprint::new(graph.fingerprint().as_str())?,
            )
            .bind_completion(
                interface_runtime::PluginIdentity::new("api-server.providers-view-completion")?,
                Arc::new(ProvidersViewCompletionObserver),
            ),
        );
        let interface_snapshot =
            crate::routes::host_infrastructure::interface_operation::compile_interface_registry(
                Arc::clone(&graph),
                descriptors,
                active_extensions,
                providers_view_query,
                Arc::clone(&providers_view_hook_plan),
            )?;
        let interface_registry = Arc::new(interface_runtime::DynamicInterfaceRegistry::new(
            interface_snapshot,
        ));
        let mut authentication_factories = AuthenticationAdapterFactoryRegistry::built_in()?;
        authentication_factories.activate_host_extensions(host_authentication_factories)?;
        Ok(Self {
            graph,
            interface_registry: Some(interface_registry),
            authentication_factories,
        })
    }

    #[cfg(test)]
    pub(crate) fn compile_for_test(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
    ) -> anyhow::Result<Self> {
        Self::compile(
            graph,
            descriptors,
            &[],
            Arc::new(
                crate::routes::host_infrastructure::interface_operation::UnavailableHostInfrastructureProvidersViewQuery,
            ),
            Vec::new(),
        )
    }

    pub fn graph(&self) -> &EffectiveExtensionGraph {
        self.graph.as_ref()
    }

    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }

    pub fn fingerprint(&self) -> &str {
        self.graph.fingerprint().as_str()
    }

    pub fn interface_registry(&self) -> Option<&Arc<interface_runtime::DynamicInterfaceRegistry>> {
        self.interface_registry.as_ref()
    }

    pub(crate) fn publish_complete_catalog(
        &self,
        state: &Arc<crate::app_state::ApiState>,
    ) -> anyhow::Result<()> {
        let registry = self
            .interface_registry
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interface registry is absent"))?;
        let mut collector = InterfaceContributionCollector::new(
            interface_runtime::GraphFingerprint::new(self.graph.fingerprint().as_str())?,
        );
        collector.absorb_published_interface(
            registry.snapshot(),
            interface_runtime::InterfaceId::new(
                crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID,
            )?,
            interface_runtime::AuthorizationOperation::new(
                crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PERMISSION,
            )?,
            interface_runtime::InterfaceOwner::new(
                crate::routes::host_infrastructure::interface_operation::HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID,
            )?,
        );
        for contribution in production_interface_contributions() {
            collector.add(contribution)?;
        }
        let candidate = collector.compile(Arc::downgrade(state))?;
        self.authentication_factories
            .validate_registry(&candidate)?;
        registry.publish(candidate);
        Ok(())
    }

    pub(crate) async fn authenticate<C, P>(
        &self,
        activation: &interface_runtime::ActivatedAuthenticationAdapter,
        credential: C,
    ) -> anyhow::Result<P>
    where
        C: std::any::Any + Send + 'static,
        P: interface_runtime::InvocationPrincipal,
    {
        self.authentication_factories
            .authenticate(activation, credential)
            .await
    }

    pub fn effective_plan(&self) -> EffectiveExtensionPlan<'_> {
        EffectiveExtensionPlan {
            schema_version: EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1,
            graph_fingerprint: self.graph.fingerprint().as_str(),
            bus_version: self.graph.bus_version().as_str(),
            module_order: self.graph.module_order(),
            module_provenance: self.graph.module_provenance(),
            module_receipts: self.graph.module_receipts(),
            points: self.graph.points(),
            contribution_receipts: self.graph.contribution_receipts(),
        }
    }

    pub fn render_effective_plan(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.effective_plan())
    }
}

#[derive(Debug, Serialize)]
pub struct EffectiveExtensionPlan<'a> {
    pub schema_version: &'static str,
    pub graph_fingerprint: &'a str,
    pub bus_version: &'static str,
    pub module_order: &'a [ModuleId],
    pub module_provenance: &'a [Provenance],
    pub module_receipts: &'a [ModuleResolutionReceipt],
    pub points: &'a [EffectiveExtensionPoint],
    pub contribution_receipts: &'a [ContributionResolutionReceipt],
}
