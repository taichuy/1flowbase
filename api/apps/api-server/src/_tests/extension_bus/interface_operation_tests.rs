use std::sync::Arc;

use access_control::{ConsoleAuthorization, ConsolePolicyGroup};
use plugin_framework::extension_bus::{Cardinality, ExtensionPointKind, ModuleId};

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
            providers_view_definition, validate_console_registry,
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
fn effective_graph_compiles_one_typed_registry_definition_and_handler() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let snapshot = ExtensionBootSnapshot::compile_for_test(
        Arc::clone(&graph),
        assembly.interface_operations(),
    )
    .unwrap();
    let registry = snapshot.interface_registry().unwrap().snapshot();
    let definition = providers_view_definition(registry.as_ref()).unwrap();

    assert_eq!(registry.definitions().len(), 1);
    assert_eq!(
        registry.graph_fingerprint().as_str(),
        snapshot.fingerprint()
    );
    assert!(registry.fingerprint().as_str().starts_with("sha256:"));
    assert_eq!(
        definition.interface_id().as_str(),
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_OPERATION_ID
    );
    assert_eq!(
        definition.route().unwrap().path(),
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_PATH
    );
    assert_eq!(
        definition.owner().as_str(),
        HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
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
}

#[test]
fn missing_duplicate_mismatched_or_disabled_definition_fails_closed() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    assert!(ExtensionBootSnapshot::compile_for_test(Arc::clone(&graph), &[]).is_err());

    let duplicate = assembly.interface_operations()[0].clone();
    assert!(ExtensionBootSnapshot::compile_for_test(
        Arc::clone(&graph),
        &[duplicate.clone(), duplicate],
    )
    .is_err());

    let canonical = assembly.interface_operations()[0].clone();
    let mut candidates = Vec::new();
    let mut wrong_method = canonical.clone();
    wrong_method.method = plugin_framework::HostExtensionInterfaceOperationMethod::Post;
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
        assert!(ExtensionBootSnapshot::compile_for_test(Arc::clone(&graph), &[candidate]).is_err());
    }

    let mut modules = assembly.module_descriptors().to_vec();
    modules
        .iter_mut()
        .find(|module| {
            module.module_id.as_str() == HOST_INFRASTRUCTURE_PROVIDERS_VIEW_CONTRIBUTOR_ID
        })
        .unwrap()
        .contributions
        .retain(|contribution| contribution.point_id.as_str() != INTERFACE_OPERATION_POINT_ID);
    let disabled =
        Arc::new(plugin_framework::extension_bus::compile_extension_graph(modules).unwrap());
    assert!(
        ExtensionBootSnapshot::compile_for_test(disabled, assembly.interface_operations()).is_err()
    );
}

#[test]
fn wrong_graph_owner_is_rejected_before_registry_publish() {
    let assembly = assembly();
    let mut modules = assembly.module_descriptors().to_vec();
    modules
        .iter_mut()
        .flat_map(|module| module.extension_points.iter_mut())
        .find(|point| point.point_id.as_str() == INTERFACE_OPERATION_POINT_ID)
        .unwrap()
        .owner_module_id = ModuleId::new("wrong.owner").unwrap();
    assert!(plugin_framework::extension_bus::compile_extension_graph(modules).is_err());
}

#[test]
fn registry_definition_projects_route_console_openapi_and_mcp_identity() {
    let assembly = assembly();
    let snapshot = ExtensionBootSnapshot::compile_for_test(
        Arc::new(assembly.compile_graph().unwrap()),
        assembly.interface_operations(),
    )
    .unwrap();
    let registry = snapshot.interface_registry().unwrap().snapshot();
    let definition = providers_view_definition(registry.as_ref()).unwrap();
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let route_assembly =
        migrated_core_console_route_assembly_with_interface_operations(Some(registry.as_ref()));
    let console_registry =
        compile_migrated_core_console_operation_registry(&settings, route_assembly.bindings())
            .unwrap();
    validate_console_registry(registry.as_ref(), &console_registry).unwrap();

    let route = definition.route().unwrap();
    let access = console_registry
        .access_for_console_route(route.method(), route.path())
        .unwrap();
    assert_eq!(access.operation_id, definition.interface_id().as_str());
    assert_eq!(access.authorization, &ConsoleAuthorization::Simple);
    assert_eq!(
        access.policy_group,
        &ConsolePolicyGroup::SettingsFeature("system.host-infrastructure".to_string())
    );

    let route_source = include_str!("../../routes/settings/host_infrastructure.rs");
    let openapi_source = include_str!("../../openapi_interface/capability_catalog.rs");
    let mcp_catalog_source =
        include_str!("../../routes/settings/mcp_management/interface_catalog.rs");
    let mcp_dispatch_source = include_str!("../../routes/settings/mcp_management/debug_execute.rs");
    assert!(!route_source.contains("host_infrastructure.providers.view"));
    assert!(openapi_source.contains("providers_view_definition(registry)"));
    assert!(mcp_catalog_source.contains("mcp_interface_entry_from_capability"));
    assert!(mcp_dispatch_source.contains("invoke_providers_view"));
    assert!(!mcp_dispatch_source.contains("HostInfrastructureProvidersViewBinding"));
}
