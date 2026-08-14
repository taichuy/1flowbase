use std::{collections::BTreeSet, sync::Arc};

use access_control::{ConsoleAuthorization, ConsolePolicyGroup};
use plugin_framework::{
    extension_bus::{
        Cardinality, ContractVersion, ContributionDescriptor, ContributionId, ContributionMode,
        ContributionOrdering, DeliverySemantics, ExtensionPointId, ExtensionPointKind,
        FailureSemantics, LifecycleSemantics, ModuleId, ModuleKind, PermissionCode,
    },
    HostExtensionInterfaceOperationAuthPolicy, HostExtensionInterfaceOperationMethod,
};

use crate::{
    extension_bus::{
        assemble_extension_graph_input, ExtensionBootSnapshot, DEFAULT_PLUGIN_SET_PATH,
    },
    routes::{
        console_route_assembly::{
            compile_migrated_core_console_operation_registry,
            migrated_core_console_route_assembly_with_interface_operations,
        },
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
fn unrelated_future_module_and_operation_do_not_block_the_known_binding() {
    let assembly = assembly();
    let mut descriptors = assembly.interface_operations().to_vec();
    let mut future_descriptor = descriptors[0].clone();
    future_descriptor.operation_id = "identity.sessions.view".to_string();
    future_descriptor.path = "/api/console/identity/sessions".to_string();
    future_descriptor.output.contract_id = "identity-session-list".to_string();
    future_descriptor.required_core_permission =
        "core.interface-operation.identity-sessions-view".to_string();
    descriptors.push(future_descriptor);

    let mut modules = assembly.module_descriptors().to_vec();
    let future_permission =
        PermissionCode::new("core.interface-operation.identity-sessions-view").unwrap();
    modules
        .iter_mut()
        .flat_map(|module| module.extension_points.iter_mut())
        .find(|point| point.point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .unwrap()
        .allowed_permissions
        .insert(future_permission.clone());
    let future_module = modules
        .iter_mut()
        .find(|module| module.module_id.as_str() == "official.identity-host")
        .unwrap();
    future_module
        .granted_permissions
        .insert(future_permission.clone());
    future_module.contributions.push(ContributionDescriptor {
        contribution_id: ContributionId::new(
            "official.identity-host.interface-operation.identity.sessions.view",
        )
        .unwrap(),
        contributor_module_id: future_module.module_id.clone(),
        point_id: ExtensionPointId::new(INTERFACE_OPERATION_POINT_ID).unwrap(),
        contract_version: ContractVersion::new("1").unwrap(),
        required_permissions: BTreeSet::from([future_permission]),
        mode: ContributionMode::Append,
        ordering: ContributionOrdering::default(),
    });

    let graph =
        Arc::new(plugin_framework::extension_bus::compile_extension_graph(modules).unwrap());
    let snapshot = ExtensionBootSnapshot::compile(Arc::clone(&graph), &descriptors).unwrap();
    assert_eq!(
        snapshot
            .interface_operations()
            .unwrap()
            .providers_view()
            .definition()
            .descriptor()
            .operation_id,
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    );
    assert_eq!(
        graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
            .unwrap()
            .contributions()
            .len(),
        2
    );
}

#[test]
fn missing_disabled_or_wrong_owner_interface_operation_fails_closed() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    assert!(ExtensionBootSnapshot::compile(Arc::clone(&graph), &[]).is_err());
    let duplicate = assembly.interface_operations()[0].clone();
    assert!(
        ExtensionBootSnapshot::compile(Arc::clone(&graph), &[duplicate.clone(), duplicate],)
            .is_err()
    );

    let mut modules_without_interface_operation = assembly.module_descriptors().to_vec();
    modules_without_interface_operation
        .iter_mut()
        .find(|module| {
            module.module_id.as_str() == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        })
        .unwrap()
        .contributions
        .retain(|contribution| contribution.point_id.as_str() != INTERFACE_OPERATION_POINT_ID);
    let disabled_graph = Arc::new(
        plugin_framework::extension_bus::compile_extension_graph(
            modules_without_interface_operation,
        )
        .unwrap(),
    );
    assert!(
        ExtensionBootSnapshot::compile(disabled_graph, assembly.interface_operations(),).is_err()
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
    let binding = snapshot.interface_operations().unwrap().providers_view();
    let absent_assembly = migrated_core_console_route_assembly_with_interface_operations(None);
    assert!(!absent_assembly.bindings().iter().any(|route| {
        route.route.method == "GET" && route.route.path == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
    }));
    let activated_assembly = migrated_core_console_route_assembly_with_interface_operations(
        snapshot.interface_operations(),
    );
    let registry =
        compile_migrated_core_console_operation_registry(&settings, activated_assembly.bindings())
            .unwrap();
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
    assert!(activated_assembly.bindings().iter().any(|route| {
        route.route.method == "GET" && route.route.path == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
    }));

    let route_source = include_str!("../../routes/settings/host_infrastructure.rs");
    let binding_source =
        include_str!("../../routes/settings/host_infrastructure/interface_operation.rs");
    let mcp_catalog_source =
        include_str!("../../routes/settings/mcp_management/interface_catalog.rs");
    let mcp_dispatch_source = include_str!("../../routes/settings/mcp_management/debug_execute.rs");
    assert!(!route_source.contains("host_infrastructure.providers.view"));
    assert!(!binding_source.contains("serde_json"));
    assert!(!mcp_catalog_source.contains("parse_host_extension"));
    assert!(!mcp_dispatch_source.contains("parse_host_extension"));
    assert!(!mcp_dispatch_source.contains("HostInfrastructureProvidersView"));
}

fn _binding_type_is_statically_tied_to_existing_dto(
    binding: &InterfaceOperationBinding<
        HostInfrastructureProvidersViewInputSchema,
        HostInfrastructureProvidersViewOutputSchema,
    >,
) {
    let _ = binding;
}
