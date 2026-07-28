use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct Inventory {
    count: usize,
    consumers: Vec<Consumer>,
}

#[derive(Deserialize)]
struct Consumer {
    module: String,
    msgid: String,
    path: String,
}

#[derive(Deserialize)]
struct InterfaceInventory {
    module: String,
    projected_interface_count: usize,
    count: usize,
    description_template: String,
    consumer_mapping: InterfaceConsumerMapping,
    columns: Vec<String>,
    interfaces: Vec<(String, String, String, String)>,
}

#[derive(Deserialize)]
struct InterfaceConsumerMapping {
    dto: String,
    api_client: String,
    ui: String,
}

#[test]
fn ac_010_d3_p3_console_interface_projection_inventory_is_exact() {
    const MODULE: &str = "@taichuy/platform/console/interfaces";
    let fixture: InterfaceInventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p3.json"
    ))
    .unwrap();
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let registry = crate::app_state::compile_core_console_operation_registry(&settings).unwrap();
    let projected_interfaces = registry
        .inventory()
        .interfaces
        .iter()
        .filter(|interface| interface.authorization_operation_id.is_some())
        .collect::<Vec<_>>();

    assert_eq!(fixture.module, MODULE);
    assert_eq!(fixture.projected_interface_count, 254);
    assert_eq!(
        fixture.projected_interface_count,
        projected_interfaces.len()
    );
    assert_eq!(fixture.count, 508);
    assert_eq!(fixture.projected_interface_count, fixture.interfaces.len());
    assert_eq!(
        fixture.description_template,
        "{summary} in the system backend."
    );
    assert_eq!(
        fixture.columns,
        ["operation_id", "method", "path", "summary"]
    );
    assert_eq!(
        fixture.consumer_mapping.dto,
        "api/apps/api-server/src/routes/settings/roles.rs"
    );
    assert_eq!(
        fixture.consumer_mapping.api_client,
        "web/packages/api-client/src/console-roles.ts"
    );
    assert_eq!(
        fixture.consumer_mapping.ui,
        "web/app/src/features/settings/components/RolePermissionPanel.tsx"
    );

    let fixture_entries = fixture
        .interfaces
        .iter()
        .flat_map(|(operation_id, method, path, summary)| {
            let description = fixture.description_template.replace("{summary}", summary);
            [
                (
                    MODULE.to_string(),
                    summary.clone(),
                    "summary".to_string(),
                    operation_id.clone(),
                    method.clone(),
                    path.clone(),
                ),
                (
                    MODULE.to_string(),
                    description,
                    "description".to_string(),
                    operation_id.clone(),
                    method.clone(),
                    path.clone(),
                ),
            ]
        })
        .collect::<BTreeSet<_>>();
    let compiled_entries = projected_interfaces
        .iter()
        .flat_map(|interface| {
            let operation_id = interface.authorization_operation_id.as_deref().unwrap();
            [
                (
                    MODULE.to_string(),
                    interface.summary.clone(),
                    "summary".to_string(),
                    operation_id.to_string(),
                    interface.route.method.clone(),
                    interface.route.path.clone(),
                ),
                (
                    MODULE.to_string(),
                    interface.description.clone(),
                    "description".to_string(),
                    operation_id.to_string(),
                    interface.route.method.clone(),
                    interface.route.path.clone(),
                ),
            ]
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(fixture_entries.len(), fixture.count);
    assert_eq!(fixture_entries, compiled_entries);

    let message_identities = fixture_entries
        .iter()
        .map(|entry| (entry.0.as_str(), entry.1.as_str()))
        .collect::<BTreeSet<_>>();
    assert_eq!(message_identities.len(), fixture.count);
    assert!(fixture
        .interfaces
        .iter()
        .all(|(operation_id, method, path, summary)| {
            !operation_id.trim().is_empty()
                && !method.trim().is_empty()
                && path.starts_with("/api/console/")
                && !summary.trim().is_empty()
        }));
}

#[test]
fn ac_010_dynamic_backend_consumer_inventory_is_exact_and_english_owned() {
    let inventory: Inventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p6.json"
    ))
    .unwrap();
    assert_eq!(inventory.count, 18);
    assert_eq!(inventory.consumers.len(), inventory.count);
    assert!(inventory.consumers.iter().all(|consumer| {
        consumer.module.starts_with("@taichuy/platform/")
            && consumer.path.starts_with("api/")
            && !consumer
                .msgid
                .chars()
                .any(|character| matches!(character, '\u{4e00}'..='\u{9fff}'))
    }));
}

#[test]
fn ac_010_d3_p4_console_display_inventory_is_exact_and_english_owned() {
    let inventory: Inventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p4.json"
    ))
    .unwrap();
    assert_eq!(inventory.count, 70);
    assert_eq!(inventory.consumers.len(), inventory.count);
    let fixture = inventory
        .consumers
        .iter()
        .map(|consumer| (consumer.module.as_str(), consumer.msgid.as_str()))
        .collect::<BTreeSet<_>>();
    let compiled = crate::routes::core_console_i18n::core_console_display_inventory()
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(fixture.len(), 68);
    assert_eq!(fixture, compiled);
    assert!(inventory.consumers.iter().all(|consumer| {
        matches!(
            consumer.module.as_str(),
            "@taichuy/platform/console/settings"
                | "@taichuy/platform/console/settings/policy"
                | "@taichuy/platform/console/settings/resources"
        ) && consumer.path.starts_with("api/")
            && !consumer
                .msgid
                .chars()
                .any(|character| matches!(character, '\u{4e00}'..='\u{9fff}'))
    }));
    let core_catalog = include_str!("../routes/core_console_i18n/catalog.rs");
    assert!(!core_catalog
        .chars()
        .any(|character| matches!(character, '\u{4e00}'..='\u{9fff}')));
    assert!(!core_catalog.contains("auto."));
    assert!(!core_catalog.contains("console.policy"));
    assert!(!core_catalog.contains("console.resources"));
}
