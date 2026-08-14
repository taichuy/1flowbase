use std::sync::Arc;

use access_control::{ConsoleAuthorization, ConsolePolicyGroup};
use plugin_framework::{
    extension_bus::{
        Cardinality, DeliverySemantics, ExtensionPointKind, FailureSemantics, LifecycleSemantics,
        ModuleDisableReason, ModuleId, ModuleKind,
    },
    HostExtensionInterfaceOperationAuthPolicy, HostExtensionInterfaceOperationMethod,
};

use crate::{
    app_state::compile_core_console_operation_registry,
    extension_bus::{
        assemble_extension_graph_input, ExtensionBootSnapshot, ModuleActivationFact,
        DEFAULT_PLUGIN_SET_PATH,
    },
    routes::{
        console_route_assembly::migrated_core_console_route_assembly,
        host_infrastructure::interface_operation::{
            HostInfrastructureProvidersViewInputSchema,
            HostInfrastructureProvidersViewOutputSchema, InterfaceOperationBinding,
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID,
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID,
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH, INTERFACE_OPERATION_POINT_ID,
        },
    },
};

fn assembly() -> crate::extension_bus::ExtensionGraphInputAssembly {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    assemble_extension_graph_input(root, DEFAULT_PLUGIN_SET_PATH, Vec::new()).unwrap()
}

#[test]
fn manifest_descriptor_graph_and_boot_snapshot_compile_one_typed_binding() {
    let assembly = assembly();
    let descriptors = assembly.interface_operations().to_vec();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let snapshot = ExtensionBootSnapshot::compile(Arc::clone(&graph), &descriptors).unwrap();
    let binding = snapshot.interface_operations().unwrap().providers_view();
    let descriptor = binding.definition().descriptor();

    assert!(Arc::ptr_eq(binding.graph_arc(), snapshot.graph_arc()));
    assert_eq!(binding.graph_fingerprint(), snapshot.fingerprint());
    assert_eq!(
        descriptor.operation_id,
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    );
    assert_eq!(descriptor.path, HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH);
    assert_eq!(
        binding.provenance().module_id().as_str(),
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
    );
    assert_eq!(binding.owner_module_id(), "1flowbase.boot-core");
    assert_eq!(binding.provenance().module_kind(), ModuleKind::TrustedHost);
    assert_eq!(
        descriptor.auth_policy,
        HostExtensionInterfaceOperationAuthPolicy::CoreConsoleOperation
    );

    let point = graph
        .points()
        .iter()
        .find(|point| point.descriptor().point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .unwrap();
    assert_eq!(point.contributions().len(), 1);
    assert_eq!(
        point.descriptor().point_kind,
        ExtensionPointKind::Contribution
    );
    assert_eq!(point.descriptor().cardinality, Cardinality::Many);
    assert_eq!(point.descriptor().failure, FailureSemantics::FailClosed);
    assert_eq!(point.descriptor().delivery, DeliverySemantics::Synchronous);
    assert_eq!(
        point.descriptor().lifecycle,
        LifecycleSemantics::BootSnapshot
    );
}

#[test]
fn missing_disabled_or_wrong_owner_interface_operation_fails_closed() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    assert!(ExtensionBootSnapshot::compile(Arc::clone(&graph), &[]).is_err());

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let disabled = assemble_extension_graph_input(
        root,
        DEFAULT_PLUGIN_SET_PATH,
        vec![ModuleActivationFact::disabled(
            HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID,
            ModuleDisableReason::DeploymentPolicy,
        )
        .unwrap()],
    )
    .unwrap();
    let disabled_graph = Arc::new(disabled.compile_graph().unwrap());
    assert!(
        ExtensionBootSnapshot::compile(disabled_graph, disabled.interface_operations(),).is_err()
    );

    let mut modules = assembly.module_descriptors().to_vec();
    let point = modules
        .iter_mut()
        .flat_map(|module| module.extension_points.iter_mut())
        .find(|point| point.point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .unwrap();
    point.owner_module_id = ModuleId::new("wrong.owner").unwrap();
    assert!(plugin_framework::extension_bus::compile_extension_graph(modules).is_err());
}

#[test]
fn non_owner_host_extension_cannot_claim_the_core_interface_permission() {
    let assembly = assembly();
    let mut modules = assembly.module_descriptors().to_vec();
    let mut contribution = {
        let owner = modules
            .iter_mut()
            .find(|module| {
                module.module_id.as_str() == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
            })
            .unwrap();
        let index = owner
            .contributions
            .iter()
            .position(|contribution| contribution.point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
            .unwrap();
        owner.contributions.remove(index)
    };
    let claimant = modules
        .iter_mut()
        .find(|module| module.module_id.as_str() == "official.identity-host")
        .unwrap();
    contribution.contributor_module_id = claimant.module_id.clone();
    claimant.contributions.push(contribution);

    assert!(plugin_framework::extension_bus::compile_extension_graph(modules).is_err());
}

#[test]
fn method_path_permission_and_schema_mismatch_are_rejected_before_binding() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let canonical = assembly.interface_operations()[0].clone();

    let mut candidates = Vec::new();
    let mut wrong_method = canonical.clone();
    wrong_method.method = HostExtensionInterfaceOperationMethod::Post;
    candidates.push(wrong_method);
    let mut wrong_path = canonical.clone();
    wrong_path.path.push_str("/wrong");
    candidates.push(wrong_path);
    let mut wrong_permission = canonical.clone();
    wrong_permission.required_core_permission = "core.wrong".to_string();
    candidates.push(wrong_permission);
    let mut wrong_input = canonical.clone();
    wrong_input.input.contract_id = "dynamic-json".to_string();
    candidates.push(wrong_input);
    let mut wrong_output = canonical;
    wrong_output.output.contract_version = "2".to_string();
    candidates.push(wrong_output);

    for candidate in candidates {
        assert!(ExtensionBootSnapshot::compile(Arc::clone(&graph), &[candidate]).is_err());
    }
}

#[test]
fn binding_matches_core_console_operation_auth_and_route_without_second_literal_registry() {
    let assembly = assembly();
    let descriptors = assembly.interface_operations().to_vec();
    let snapshot =
        ExtensionBootSnapshot::compile(Arc::new(assembly.compile_graph().unwrap()), &descriptors)
            .unwrap();
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let binding = snapshot.interface_operations().unwrap().providers_view();
    binding.validate_console_registry(&registry).unwrap();

    let access = registry
        .access_for_console_route("GET", HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH)
        .unwrap();
    assert_eq!(
        access.operation_id,
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    );
    assert_eq!(access.authorization, &ConsoleAuthorization::Simple);
    assert_eq!(
        access.policy_group,
        &ConsolePolicyGroup::SettingsFeature("system.host-infrastructure".to_string())
    );
    assert!(migrated_core_console_route_assembly()
        .bindings()
        .iter()
        .any(|route| {
            route.route.method == "GET"
                && route.route.path == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
        }));

    let route_source = include_str!("../../routes/settings/host_infrastructure.rs");
    let binding_source =
        include_str!("../../routes/settings/host_infrastructure/interface_operation.rs");
    assert!(!route_source.contains("host_infrastructure.providers.view"));
    assert!(!binding_source.contains("serde_json"));
}

fn _binding_type_is_statically_tied_to_existing_dto(
    binding: &InterfaceOperationBinding<
        HostInfrastructureProvidersViewInputSchema,
        HostInfrastructureProvidersViewOutputSchema,
    >,
) {
    let _ = binding;
}
