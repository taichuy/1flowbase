use crate::extension_bus::{
    assemble_extension_graph_input, ExtensionBootSnapshot, DEFAULT_PLUGIN_SET_PATH,
};
use std::sync::Arc;

#[test]
fn boot_no_longer_requires_the_retired_provider_configuration_interface() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assembly =
        assemble_extension_graph_input(root, DEFAULT_PLUGIN_SET_PATH, Vec::new()).unwrap();
    assert!(assembly.interface_operations().is_empty());
    let snapshot =
        ExtensionBootSnapshot::compile_for_test(Arc::new(assembly.compile_graph().unwrap()), &[])
            .unwrap();
    let registry = snapshot.interface_registry().unwrap().snapshot();
    assert_eq!(registry.definitions().len(), 0);
    assert_eq!(
        registry.graph_fingerprint().as_str(),
        snapshot.fingerprint()
    );
}

#[test]
fn console_inventory_retires_providers_and_preserves_cache_and_memory_ownership() {
    use crate::routes::console_route_assembly::{
        compile_migrated_core_console_operation_registry, migrated_core_console_route_assembly,
    };
    let assembly = migrated_core_console_route_assembly();
    assert!(!assembly.bindings().iter().any(|binding| binding
        .route
        .path
        .contains("/host-infrastructure/providers")));
    let features = access_control::SettingsFeatureRegistry::compile(
        access_control::core_settings_feature_registrations(),
    )
    .unwrap();
    let registry =
        compile_migrated_core_console_operation_registry(&features, assembly.bindings()).unwrap();
    for (method, path) in [
        ("GET", "/api/console/settings/host-infrastructure/memory"),
        (
            "POST",
            "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/clear",
        ),
    ] {
        let access = registry.access_for_console_route(method, path).unwrap();
        assert_eq!(
            access.policy_group,
            &access_control::ConsolePolicyGroup::SettingsFeature(
                "system.memory-observation".to_owned()
            )
        );
    }
}
