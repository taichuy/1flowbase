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
