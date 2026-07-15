use api_server::app_state::compile_core_console_operation_inventory_snapshot;

#[test]
fn ac_009_013_compiled_snapshot_uses_boot_registry_routes_and_locales_deterministically() {
    let first = compile_core_console_operation_inventory_snapshot()
        .expect("Core console inventory snapshot should compile from the boot registry");
    let second = compile_core_console_operation_inventory_snapshot()
        .expect("Core console inventory snapshot should be deterministic");

    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert!(!first.compiled_inventory.operations.is_empty());
    assert!(!first.compiled_inventory.resources.is_empty());
    assert!(!first.route_assembly.is_empty());
    for locale in ["zh_Hans", "en_US"] {
        assert!(
            first
                .locales
                .get(locale)
                .is_some_and(|references| !references.is_empty()),
            "compiled snapshot must carry non-empty {locale} locale evidence"
        );
    }
}
