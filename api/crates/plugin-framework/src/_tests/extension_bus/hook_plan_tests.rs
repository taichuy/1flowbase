use std::{collections::BTreeSet, num::NonZeroU64};

use plugin_framework::extension_bus::{
    compile_extension_graph, compile_hook_plans, Cardinality, ContractDescriptor, ContractVersion,
    ContributionDescriptor, ContributionId, ContributionMode, ContributionOrdering,
    DeliverySemantics, ExtensionBusVersion, ExtensionPointDescriptor, ExtensionPointId,
    ExtensionPointKind, FailureSemantics, HookHandlerBinding, HookHandlerContract,
    HookMutationCapability, HookPhase, HookPlanCompilationError, HookPointBinding,
    HookPointContract, LifecycleSemantics, ModuleActivationDeclaration, ModuleDescriptor,
    ModuleDisableReason, ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy,
    ScopeSemantics,
};

fn modules(disable_beta: bool) -> Vec<ModuleDescriptor> {
    let context = ContractDescriptor::new("interface.request", "v1").unwrap();
    let mut core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("core").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: Vec::new(),
    };
    core.extension_points.push(ExtensionPointDescriptor {
        point_id: ExtensionPointId::new("interface.before").unwrap(),
        owner_module_id: core.module_id.clone(),
        point_kind: ExtensionPointKind::Pipeline,
        contract: context,
        scope: ScopeSemantics::Workspace,
        cardinality: Cardinality::OneOrMore,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    });

    let alpha = contribution_module("alpha", false, None);
    let beta = contribution_module("beta", disable_beta, Some("alpha.handler"));
    vec![core, alpha, beta]
}

fn contribution_module(id: &str, disabled: bool, after: Option<&str>) -> ModuleDescriptor {
    let mut ordering = ContributionOrdering::default();
    if let Some(after) = after {
        ordering.after.insert(ContributionId::new(after).unwrap());
    }
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(id).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::Runtime,
        activation: if disabled {
            ModuleActivationDeclaration::Disabled {
                reason: ModuleDisableReason::DesiredState,
            }
        } else {
            ModuleActivationDeclaration::Active
        },
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new(format!("{id}.handler")).unwrap(),
            contributor_module_id: ModuleId::new(id).unwrap(),
            point_id: ExtensionPointId::new("interface.before").unwrap(),
            contract_version: ContractVersion::new("v1").unwrap(),
            required_permissions: BTreeSet::new(),
            mode: ContributionMode::Append,
            ordering,
        }],
    }
}

fn point_contract() -> HookPointContract {
    HookPointContract {
        context: ContractDescriptor::new("interface.request", "v1").unwrap(),
        decision: None,
        phase: HookPhase::Before,
        timeout_ms: NonZeroU64::new(1_000).unwrap(),
        mutation: HookMutationCapability::ObserveOnly,
    }
}

fn handler(id: &str) -> HookHandlerBinding {
    HookHandlerBinding::new(
        ContributionId::new(id).unwrap(),
        HookHandlerContract {
            context: ContractDescriptor::new("interface.request", "v1").unwrap(),
            decision: None,
            phase: HookPhase::Before,
        },
    )
}

#[test]
fn lcf_002_compiles_forward_and_reverse_orders_against_one_fingerprint() {
    let graph = compile_extension_graph(modules(false)).unwrap();
    let plans = compile_hook_plans(
        &graph,
        vec![HookPointBinding::new(
            ExtensionPointId::new("interface.before").unwrap(),
            point_contract(),
        )],
        vec![handler("alpha.handler"), handler("beta.handler")],
    )
    .unwrap();
    let plan = &plans[0];

    assert_eq!(plan.graph_fingerprint(), graph.fingerprint());
    assert_eq!(
        plan.before_handlers()
            .iter()
            .map(|entry| entry.contribution_id().as_str())
            .collect::<Vec<_>>(),
        vec!["alpha.handler", "beta.handler"]
    );
    assert_eq!(
        plan.after_handlers()
            .iter()
            .map(|entry| entry.contribution_id().as_str())
            .collect::<Vec<_>>(),
        vec!["beta.handler", "alpha.handler"]
    );
}

#[test]
fn lcf_008_missing_or_mismatched_handler_fails_closed() {
    let graph = compile_extension_graph(modules(false)).unwrap();
    let point = HookPointBinding::new(
        ExtensionPointId::new("interface.before").unwrap(),
        point_contract(),
    );
    assert!(matches!(
        compile_hook_plans(&graph, vec![point.clone()], vec![handler("alpha.handler")]),
        Err(HookPlanCompilationError::MissingHandlerBinding { .. })
    ));

    let mismatched = HookHandlerBinding::new(
        ContributionId::new("beta.handler").unwrap(),
        HookHandlerContract {
            context: ContractDescriptor::new("interface.request", "v1").unwrap(),
            decision: None,
            phase: HookPhase::After,
        },
    );
    assert!(matches!(
        compile_hook_plans(
            &graph,
            vec![point],
            vec![handler("alpha.handler"), mismatched]
        ),
        Err(HookPlanCompilationError::HandlerContractMismatch { .. })
    ));
}

#[test]
fn lcf_009_disabled_handler_cannot_enter_the_active_plan() {
    let graph = compile_extension_graph(modules(true)).unwrap();
    assert!(matches!(
        compile_hook_plans(
            &graph,
            vec![HookPointBinding::new(
                ExtensionPointId::new("interface.before").unwrap(),
                point_contract(),
            )],
            vec![handler("alpha.handler"), handler("beta.handler")]
        ),
        Err(HookPlanCompilationError::InactiveHandlerBinding { .. })
    ));
}
