use std::{collections::BTreeSet, sync::Arc};

use plugin_framework::extension_bus::{
    compile_extension_graph, Cardinality, ContractDescriptor, ContractVersion,
    ContributionDescriptor, ContributionId, ContributionMode, ContributionOrdering,
    DeliverySemantics, ExtensionBusVersion, ExtensionPointDescriptor, ExtensionPointId,
    ExtensionPointKind, FailureSemantics, LifecycleSemantics, ModuleActivationDeclaration,
    ModuleDescriptor, ModuleDisableReason, ModuleId, ModuleKind, ModuleVersion, OrderingSemantics,
    OverridePolicy, ScopeSemantics,
};

use crate::extension_bus::{
    assemble_extension_graph_input, ExtensionBootSnapshot, DEFAULT_PLUGIN_SET_PATH,
    EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1,
};

#[test]
fn activated_graph_and_effective_plan_share_exact_arc_and_fingerprint() {
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assembly =
        assemble_extension_graph_input(workspace_root, DEFAULT_PLUGIN_SET_PATH, Vec::new())
            .unwrap();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let graph_identity = Arc::as_ptr(&graph);
    let manifests = assembly.into_host_extension_manifests();
    let host_extensions =
        control_plane::host_extension_boot::register_builtin_host_extension_contributions(
            &manifests,
        )
        .unwrap();
    let _infrastructure =
        crate::host_infrastructure::build_local_host_infrastructure_from_host_extensions(
            &host_extensions,
            &graph,
        )
        .unwrap();

    let snapshot = ExtensionBootSnapshot::new(graph);
    let first_dump = snapshot.render_effective_plan().unwrap();
    let second_dump = snapshot.render_effective_plan().unwrap();
    let rendered: serde_json::Value = serde_json::from_str(&first_dump).unwrap();

    assert_eq!(Arc::as_ptr(snapshot.graph_arc()), graph_identity);
    assert_eq!(
        snapshot.fingerprint(),
        snapshot.graph().fingerprint().as_str()
    );
    assert_eq!(first_dump, second_dump);
    assert_eq!(
        rendered["schema_version"],
        EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1
    );
    assert_eq!(rendered["graph_fingerprint"], snapshot.fingerprint());
    assert!(!rendered["module_receipts"].as_array().unwrap().is_empty());
    assert!(!rendered["contribution_receipts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        rendered["points"][0]["descriptor"]["lifecycle"],
        "boot_snapshot"
    );
    assert_eq!(
        rendered["points"][0]["descriptor"]["delivery"],
        "synchronous"
    );
    assert_eq!(
        rendered["points"][0]["descriptor"]["failure"],
        "fail_closed"
    );
}

#[test]
fn effective_plan_renders_active_inactive_and_superseded_receipts() {
    let graph = Arc::new(
        compile_extension_graph(vec![
            boot_module(),
            contributor("fixture.append", ContributionMode::Append, true),
            contributor("fixture.override", ContributionMode::Override, true),
            contributor("fixture.disabled", ContributionMode::Append, false),
        ])
        .unwrap(),
    );
    let snapshot = ExtensionBootSnapshot::new(graph);
    let rendered = snapshot.render_effective_plan().unwrap();

    assert!(rendered.contains("\"status\": \"active\""));
    assert!(rendered.contains("\"status\": \"inactive\""));
    assert!(rendered.contains("\"status\": \"superseded_by\""));
    assert!(rendered.contains("\"reason\": \"desired_state\""));
    assert!(rendered.contains("\"module_kind\": \"trusted_host\""));
}

fn boot_module() -> ModuleDescriptor {
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("fixture.boot").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![ExtensionPointDescriptor {
            point_id: ExtensionPointId::new("fixture.cache-store").unwrap(),
            owner_module_id: ModuleId::new("fixture.boot").unwrap(),
            point_kind: ExtensionPointKind::Slot,
            contract: ContractDescriptor::new("cache-store", "1").unwrap(),
            scope: ScopeSemantics::System,
            cardinality: Cardinality::ExactlyOne,
            ordering: OrderingSemantics::Lexicographic,
            failure: FailureSemantics::FailClosed,
            delivery: DeliverySemantics::Synchronous,
            lifecycle: LifecycleSemantics::BootSnapshot,
            allowed_permissions: BTreeSet::new(),
            override_policy: OverridePolicy::TrustedHost,
        }],
        contributions: Vec::new(),
    }
}

fn contributor(id: &str, mode: ContributionMode, active: bool) -> ModuleDescriptor {
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(id).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::TrustedHost,
        activation: if active {
            ModuleActivationDeclaration::Active
        } else {
            ModuleActivationDeclaration::Disabled {
                reason: ModuleDisableReason::DesiredState,
            }
        },
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new(format!("{id}.cache-store")).unwrap(),
            contributor_module_id: ModuleId::new(id).unwrap(),
            point_id: ExtensionPointId::new("fixture.cache-store").unwrap(),
            contract_version: ContractVersion::new("1").unwrap(),
            required_permissions: BTreeSet::new(),
            mode,
            ordering: ContributionOrdering::default(),
        }],
    }
}
