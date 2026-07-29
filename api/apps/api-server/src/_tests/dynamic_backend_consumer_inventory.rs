use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Deserialize)]
struct MetadataConsumer {
    module: String,
    msgid: String,
}

#[derive(Deserialize)]
struct OfficialCoverageFixture {
    packet: String,
    catalog_version: String,
    semantic_sha256: String,
    official_commit: String,
    expected: OfficialCoverageCounts,
    p7_references: Vec<LowCodeReference>,
    p7_source: String,
}

#[derive(Deserialize)]
struct OfficialCoverageCounts {
    catalog_modules: usize,
    catalog_identities: usize,
    frozen_identities: usize,
    p3_identities: usize,
    p4_identities: usize,
    p5_consumers: usize,
    p5_identities: usize,
    p6_consumers: usize,
    p6_identities: usize,
}

#[derive(Deserialize)]
struct LowCodeReference {
    module: String,
    msgid: String,
    classification: String,
}

fn named_placeholders(text: &str) -> BTreeSet<&str> {
    text.split('{')
        .skip(1)
        .filter_map(|suffix| suffix.split_once('}').map(|(name, _)| name))
        .collect()
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
    assert!(core_catalog.contains("settings_feature!"));
    assert!(core_catalog.contains("auto.translation_catalog_title"));
}

#[test]
fn ac_001_ac_002_ac_006_ac_010_ac_012_official_seed_covers_frozen_consumers() {
    let coverage: OfficialCoverageFixture = serde_json::from_str(include_str!(
        "fixtures/official_i18n_catalog_coverage.d3-p8.json"
    ))
    .unwrap();
    let p3: InterfaceInventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p3.json"
    ))
    .unwrap();
    let p4: Inventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p4.json"
    ))
    .unwrap();
    let p5: Vec<MetadataConsumer> = serde_json::from_str(include_str!(
        "../../../../crates/control-plane/src/_tests/fixtures/metadata_i18n_consumers.json"
    ))
    .unwrap();
    let p6: Inventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p6.json"
    ))
    .unwrap();
    let seed: serde_json::Value =
        serde_json::from_str(include_str!("../../resources/i18n/catalog-seed.json")).unwrap();
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../resources/i18n/catalog-seed.source.json"
    ))
    .unwrap();
    assert_eq!(coverage.packet, "D3-P8");
    assert_eq!(
        coverage.p7_source,
        "api/apps/api-server/src/_tests/application/application_orchestration_routes.rs"
    );

    let p3_identities = p3
        .interfaces
        .iter()
        .flat_map(|(_, _, _, summary)| {
            [
                (p3.module.clone(), summary.clone()),
                (
                    p3.module.clone(),
                    p3.description_template.replace("{summary}", summary),
                ),
            ]
        })
        .collect::<BTreeSet<_>>();
    let p4_identities = p4
        .consumers
        .iter()
        .map(|consumer| (consumer.module.clone(), consumer.msgid.clone()))
        .collect::<BTreeSet<_>>();
    let p5_identities = p5
        .iter()
        .map(|consumer| (consumer.module.clone(), consumer.msgid.clone()))
        .collect::<BTreeSet<_>>();
    let p6_identities = p6
        .consumers
        .iter()
        .map(|consumer| (consumer.module.clone(), consumer.msgid.clone()))
        .collect::<BTreeSet<_>>();
    assert_eq!(p3_identities.len(), coverage.expected.p3_identities);
    assert_eq!(p4_identities.len(), coverage.expected.p4_identities);
    assert_eq!(p5.len(), coverage.expected.p5_consumers);
    assert_eq!(p5_identities.len(), coverage.expected.p5_identities);
    assert_eq!(p6.consumers.len(), coverage.expected.p6_consumers);
    assert_eq!(p6_identities.len(), coverage.expected.p6_identities);

    assert_eq!(
        seed["manifest"]["catalog_version"].as_str(),
        Some(coverage.catalog_version.as_str())
    );
    assert_eq!(
        seed["manifest"]["semantic_sha256"].as_str(),
        Some(coverage.semantic_sha256.as_str())
    );
    assert_eq!(
        source["catalog_version"].as_str(),
        Some(coverage.catalog_version.as_str())
    );
    assert_eq!(
        source["semantic_sha256"].as_str(),
        Some(coverage.semantic_sha256.as_str())
    );
    assert_eq!(
        source["official_commit"].as_str(),
        Some(coverage.official_commit.as_str())
    );

    let catalog = seed["modules"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|module| {
            let module_id = module["id"].as_str().unwrap();
            module["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |message| {
                    (
                        (
                            module_id.to_owned(),
                            message["msgid"].as_str().unwrap().to_owned(),
                        ),
                        message["translations"]["zh_Hans"]
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    )
                })
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        seed["modules"].as_array().unwrap().len(),
        coverage.expected.catalog_modules
    );
    assert_eq!(catalog.len(), coverage.expected.catalog_identities);
    assert_eq!(
        p3_identities
            .iter()
            .chain(&p4_identities)
            .chain(&p5_identities)
            .chain(&p6_identities)
            .collect::<BTreeSet<_>>()
            .len(),
        coverage.expected.frozen_identities
    );
    for (module, msgid) in p3_identities
        .iter()
        .chain(&p4_identities)
        .chain(&p5_identities)
        .chain(&p6_identities)
    {
        let translation = catalog
            .get(&(module.clone(), msgid.clone()))
            .unwrap_or_else(|| panic!("missing official identity {module} / {msgid}"));
        assert!(!translation.trim().is_empty());
        assert_eq!(named_placeholders(msgid), named_placeholders(translation));
    }

    let official_demo = &coverage.p7_references[0];
    assert_eq!(official_demo.classification, "official_demonstration");
    assert!(catalog.contains_key(&(official_demo.module.clone(), official_demo.msgid.clone())));
    let custom_fallback = &coverage.p7_references[1];
    assert_eq!(
        custom_fallback.classification,
        "custom_fallback_not_official"
    );
    assert!(!catalog.contains_key(&(
        custom_fallback.module.clone(),
        custom_fallback.msgid.clone()
    )));
}
