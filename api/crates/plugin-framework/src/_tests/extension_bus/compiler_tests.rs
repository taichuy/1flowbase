use std::collections::BTreeSet;

use plugin_framework::extension_bus::{
    compile_extension_graph, Cardinality, CompilationError, ContractDescriptor, ContractVersion,
    ContributionDescriptor, ContributionId, ContributionInactivityReason, ContributionMode,
    ContributionOrdering, ContributionResolutionStatus, DeliverySemantics, ExtensionBusVersion,
    ExtensionPointDescriptor, ExtensionPointId, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleActivationDeclaration, ModuleDependency, ModuleDescriptor,
    ModuleDisableReason, ModuleId, ModuleInactivityReason, ModuleKind, ModuleResolutionStatus,
    ModuleVersion, OrderingSemantics, OverridePolicy, PermissionCode, ScopeSemantics,
};

fn module(id: &str, kind: ModuleKind) -> ModuleDescriptor {
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(id).unwrap(),
        module_version: ModuleVersion::new("1.0.0").unwrap(),
        module_kind: kind,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![],
        contributions: vec![],
    }
}

fn point(id: &str, owner: &str, cardinality: Cardinality) -> ExtensionPointDescriptor {
    ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(id).unwrap(),
        owner_module_id: ModuleId::new(owner).unwrap(),
        point_kind: ExtensionPointKind::Contribution,
        contract: ContractDescriptor::new("test.contract", "1").unwrap(),
        scope: ScopeSemantics::Global,
        cardinality,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::BootSnapshot,
        allowed_permissions: BTreeSet::new(),
        override_policy: OverridePolicy::Sealed,
    }
}

fn contribution(id: &str, contributor: &str, point_id: &str) -> ContributionDescriptor {
    ContributionDescriptor {
        contribution_id: ContributionId::new(id).unwrap(),
        contributor_module_id: ModuleId::new(contributor).unwrap(),
        point_id: ExtensionPointId::new(point_id).unwrap(),
        contract_version: ContractVersion::new("1").unwrap(),
        required_permissions: BTreeSet::new(),
        mode: ContributionMode::Append,
        ordering: ContributionOrdering::default(),
    }
}

fn valid_modules() -> Vec<ModuleDescriptor> {
    let mut core = module("core", ModuleKind::BootCore);
    core.extension_points
        .push(point("core.actions", "core", Cardinality::OneOrMore));

    let mut alpha = module("alpha", ModuleKind::Capability);
    alpha
        .contributions
        .push(contribution("alpha.action", "alpha", "core.actions"));
    alpha
        .contributions
        .push(contribution("alpha.aux", "alpha", "core.actions"));

    let mut beta = module("beta", ModuleKind::Runtime);
    let mut beta_contribution = contribution("beta.action", "beta", "core.actions");
    beta_contribution
        .ordering
        .after
        .insert(ContributionId::new("alpha.action").unwrap());
    beta.contributions.push(beta_contribution);

    vec![core, alpha, beta]
}

// Root #1688 AC-001/AC-002: compilation is independent of discovery order.
#[test]
fn compilation_is_deterministic_under_input_permutation() {
    let forward = compile_extension_graph(valid_modules()).unwrap();
    let mut reversed_input = valid_modules();
    reversed_input.reverse();
    for module in &mut reversed_input {
        module.extension_points.reverse();
        module.contributions.reverse();
    }
    let reversed = compile_extension_graph(reversed_input).unwrap();

    assert_eq!(forward, reversed);
    assert_eq!(forward.fingerprint(), reversed.fingerprint());
    assert_eq!(
        forward.module_order(),
        &[
            ModuleId::new("alpha").unwrap(),
            ModuleId::new("beta").unwrap(),
            ModuleId::new("core").unwrap(),
        ]
    );
    assert_eq!(
        forward.points()[0]
            .contributions()
            .iter()
            .map(|entry| entry.descriptor().contribution_id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha.action", "alpha.aux", "beta.action"]
    );
}

// Root #1688 AC-003: controlled negative for an unavailable module dependency.
#[test]
fn compilation_rejects_missing_dependency() {
    let mut dependent = module("dependent", ModuleKind::Runtime);
    dependent.dependencies.insert(ModuleDependency {
        module_id: ModuleId::new("missing").unwrap(),
        required_version: ModuleVersion::new("1.0.0").unwrap(),
    });

    assert!(matches!(
        compile_extension_graph(vec![dependent]),
        Err(CompilationError::MissingModuleDependency { .. })
    ));
}

// Root #1688 AC-003: a point has exactly one trusted owner.
#[test]
fn compilation_rejects_duplicate_point_owner() {
    let mut core = module("core", ModuleKind::BootCore);
    core.extension_points
        .push(point("shared.point", "core", Cardinality::Many));
    let mut host = module("host", ModuleKind::TrustedHost);
    host.extension_points
        .push(point("shared.point", "host", Cardinality::Many));

    assert!(matches!(
        compile_extension_graph(vec![core, host]),
        Err(CompilationError::DuplicatePointOwner { .. })
    ));
}

// Root #1688 AC-009: runtime, capability, and user modules are contribution-only.
#[test]
fn compilation_rejects_point_definition_from_ordinary_module() {
    for module_kind in [
        ModuleKind::Runtime,
        ModuleKind::Capability,
        ModuleKind::User,
    ] {
        let mut ordinary = module("ordinary", module_kind);
        ordinary
            .extension_points
            .push(point("ordinary.point", "ordinary", Cardinality::Many));

        assert!(matches!(
            compile_extension_graph(vec![ordinary]),
            Err(CompilationError::UnauthorizedPointDefinition { .. })
        ));
    }
}

// Root #1688 AC-009: ordinary modules may contribute, never override or gain authority.
#[test]
fn compilation_rejects_illegal_override_and_permission_escalation() {
    let permission = PermissionCode::new("resource.write").unwrap();
    let mut core = module("core", ModuleKind::BootCore);
    let mut extension_point = point("core.actions", "core", Cardinality::Many);
    extension_point.override_policy = OverridePolicy::TrustedHost;
    extension_point
        .allowed_permissions
        .insert(permission.clone());
    core.extension_points.push(extension_point);

    let mut runtime = module("runtime", ModuleKind::Runtime);
    let mut override_contribution = contribution("runtime.override", "runtime", "core.actions");
    override_contribution.mode = ContributionMode::Override;
    runtime.contributions.push(override_contribution);
    assert!(matches!(
        compile_extension_graph(vec![core.clone(), runtime]),
        Err(CompilationError::IllegalOverride { .. })
    ));

    let mut capability = module("capability", ModuleKind::Capability);
    let mut escalating = contribution("capability.write", "capability", "core.actions");
    escalating.required_permissions.insert(permission);
    capability.contributions.push(escalating);
    assert!(matches!(
        compile_extension_graph(vec![core, capability]),
        Err(CompilationError::PermissionEscalation { .. })
    ));
}

// Root #1688 AC-003: controlled negative for graph cycles.
#[test]
fn compilation_rejects_dependency_cycle() {
    let mut alpha = module("alpha", ModuleKind::Runtime);
    alpha.dependencies.insert(ModuleDependency {
        module_id: ModuleId::new("beta").unwrap(),
        required_version: ModuleVersion::new("1.0.0").unwrap(),
    });
    let mut beta = module("beta", ModuleKind::Capability);
    beta.dependencies.insert(ModuleDependency {
        module_id: ModuleId::new("alpha").unwrap(),
        required_version: ModuleVersion::new("1.0.0").unwrap(),
    });

    assert!(matches!(
        compile_extension_graph(vec![alpha, beta]),
        Err(CompilationError::DependencyCycle { .. })
    ));
}

// Root #1688 AC-003: point cardinality is checked after override selection.
#[test]
fn compilation_rejects_cardinality_conflict() {
    let mut core = module("core", ModuleKind::BootCore);
    core.extension_points
        .push(point("core.slot", "core", Cardinality::ExactlyOne));

    assert!(matches!(
        compile_extension_graph(vec![core]),
        Err(CompilationError::CardinalityConflict { actual: 0, .. })
    ));
}

// Root #1688 AC-010: every effective entry retains source identity in the fingerprinted graph.
#[test]
fn effective_graph_carries_provenance_and_stable_fingerprint() {
    let graph = compile_extension_graph(valid_modules()).unwrap();
    let point = &graph.points()[0];

    assert_eq!(point.provenance().module_id().as_str(), "core");
    assert_eq!(graph.module_provenance().len(), 3);
    assert_eq!(
        point.contributions()[0].provenance().module_id().as_str(),
        "alpha"
    );
    assert_eq!(graph.fingerprint().as_str().len(), 64);
    assert!(graph
        .fingerprint()
        .as_str()
        .chars()
        .all(|c| c.is_ascii_hexdigit()));
}

// Root #1688 AC-002/AC-010: override resolution remains deterministic and inspectable.
#[test]
fn trusted_override_preserves_deterministic_resolution_receipts() {
    let mut modules = valid_modules();
    modules[0].extension_points[0].override_policy = OverridePolicy::TrustedHost;
    let mut host = module("trusted", ModuleKind::TrustedHost);
    let mut winner = contribution("trusted.override", "trusted", "core.actions");
    winner.mode = ContributionMode::Override;
    host.contributions.push(winner);
    modules.push(host);

    let forward = compile_extension_graph(modules.clone()).unwrap();
    let mut fewer_receipts = modules.clone();
    fewer_receipts
        .iter_mut()
        .find(|module| module.module_id.as_str() == "alpha")
        .unwrap()
        .contributions
        .retain(|entry| entry.contribution_id.as_str() != "alpha.aux");
    let fewer_receipts = compile_extension_graph(fewer_receipts).unwrap();
    assert_ne!(forward.fingerprint(), fewer_receipts.fingerprint());

    modules.reverse();
    for module in &mut modules {
        module.contributions.reverse();
    }
    let reversed = compile_extension_graph(modules).unwrap();

    assert_eq!(forward.fingerprint(), reversed.fingerprint());
    assert_eq!(forward.points()[0].contributions().len(), 1);
    assert_eq!(forward.contribution_receipts().len(), 4);
    assert_eq!(
        forward.points()[0].contributions()[0]
            .descriptor()
            .contribution_id
            .as_str(),
        "trusted.override"
    );
    let alpha_receipt = forward
        .contribution_receipts()
        .iter()
        .find(|receipt| receipt.descriptor().contribution_id.as_str() == "alpha.action")
        .unwrap();
    assert_eq!(
        alpha_receipt.status(),
        &ContributionResolutionStatus::SupersededBy {
            contribution_id: ContributionId::new("trusted.override").unwrap(),
        }
    );
    assert!(forward.contribution_receipts().iter().any(|receipt| {
        receipt.descriptor().contribution_id.as_str() == "trusted.override"
            && receipt.status() == &ContributionResolutionStatus::Active
    }));
}

// Root #1688 AC-010: D1-P2 activation facts remain visible without becoming consumer handles.
#[test]
fn inactive_module_and_contribution_keep_typed_receipts() {
    let mut modules = valid_modules();
    let alpha = modules
        .iter_mut()
        .find(|module| module.module_id.as_str() == "alpha")
        .unwrap();
    alpha.activation = ModuleActivationDeclaration::Disabled {
        reason: ModuleDisableReason::DesiredState,
    };

    let graph = compile_extension_graph(modules).unwrap();
    let alpha_module = graph
        .module_receipts()
        .iter()
        .find(|receipt| receipt.provenance().module_id().as_str() == "alpha")
        .unwrap();
    let inactive_reason = ModuleInactivityReason::Disabled {
        reason: ModuleDisableReason::DesiredState,
    };
    assert_eq!(
        alpha_module.status(),
        &ModuleResolutionStatus::Inactive {
            reason: inactive_reason.clone(),
        }
    );
    assert!(graph.points()[0].contributions().iter().all(|entry| entry
        .provenance()
        .module_id()
        .as_str()
        != "alpha"));
    assert!(graph.contribution_receipts().iter().any(|receipt| {
        receipt.descriptor().contribution_id.as_str() == "alpha.action"
            && receipt.status()
                == &ContributionResolutionStatus::Inactive {
                    reason: ContributionInactivityReason::ModuleInactive {
                        reason: inactive_reason.clone(),
                    },
                }
    }));
}

#[test]
fn contract_exposes_all_five_stable_point_kinds() {
    assert_eq!(
        [
            ExtensionPointKind::Slot,
            ExtensionPointKind::Pipeline,
            ExtensionPointKind::EventStream,
            ExtensionPointKind::Contribution,
            ExtensionPointKind::ResourceAction,
        ]
        .map(ExtensionPointKind::as_str),
        [
            "slot",
            "pipeline",
            "event_stream",
            "contribution",
            "resource_action",
        ]
    );
}

#[test]
fn contract_exposes_stable_lifecycle_and_delivery_inventories() {
    assert_eq!(
        [
            LifecycleSemantics::BootSnapshot,
            LifecycleSemantics::Invocation,
            LifecycleSemantics::RuntimeWorker,
            LifecycleSemantics::WorkspaceAssignment,
            LifecycleSemantics::UiMount,
        ]
        .map(LifecycleSemantics::as_str),
        [
            "boot_snapshot",
            "invocation",
            "runtime_worker",
            "workspace_assignment",
            "ui_mount",
        ]
    );
    assert_eq!(
        [
            DeliverySemantics::Synchronous,
            DeliverySemantics::Asynchronous,
            DeliverySemantics::AfterCommitDurable,
            DeliverySemantics::RequiredStream,
            DeliverySemantics::DiagnosticBestEffort,
        ]
        .map(DeliverySemantics::as_str),
        [
            "synchronous",
            "asynchronous",
            "after_commit_durable",
            "required_stream",
            "diagnostic_best_effort",
        ]
    );
}

#[test]
fn compilation_rejects_event_delivery_on_non_event_point_and_inverse() {
    let mut core = module("core", ModuleKind::BootCore);
    let mut non_event = point("core.actions", "core", Cardinality::Many);
    non_event.delivery = DeliverySemantics::RequiredStream;
    core.extension_points.push(non_event);
    assert!(matches!(
        compile_extension_graph(vec![core]),
        Err(CompilationError::IncompatibleDeliverySemantics { .. })
    ));

    let mut core = module("core", ModuleKind::BootCore);
    let mut event_stream = point("core.events", "core", Cardinality::Many);
    event_stream.point_kind = ExtensionPointKind::EventStream;
    event_stream.delivery = DeliverySemantics::Synchronous;
    core.extension_points.push(event_stream);
    assert!(matches!(
        compile_extension_graph(vec![core]),
        Err(CompilationError::IncompatibleDeliverySemantics { .. })
    ));
}
