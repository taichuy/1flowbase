use std::collections::BTreeSet;

use serde::Deserialize;

#[derive(Deserialize)]
struct Inventory {
    count: usize,
    consumers: Vec<Consumer>,
}

#[derive(Deserialize)]
struct Consumer {
    key: String,
    path: String,
}

#[derive(Deserialize)]
struct InterfaceInventory {
    projected_interface_count: usize,
    description_template: String,
    interfaces: Vec<(String, String, String, String)>,
}

#[derive(Deserialize)]
struct MetadataConsumer {
    key: String,
}

#[derive(Deserialize)]
struct OfficialCoverageFixture {
    catalog_version: String,
    semantic_sha256: String,
    official_commit: String,
    expected: OfficialCoverageCounts,
    p7_references: Vec<LowCodeReference>,
}

#[derive(Deserialize)]
struct OfficialCoverageCounts {
    catalog_keys: usize,
}

#[derive(Deserialize)]
struct LowCodeReference {
    key: String,
    classification: String,
}

#[test]
fn console_interface_projection_inventory_is_key_only_and_exact() {
    let fixture: InterfaceInventory = serde_json::from_str(include_str!(
        "../fixtures/dynamic_backend_consumers.d3-p3.json"
    ))
    .unwrap();
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let registry = crate::app_state::compile_core_console_operation_registry(&settings).unwrap();
    let projected = registry
        .inventory()
        .interfaces
        .iter()
        .filter(|interface| interface.authorization_operation_id.is_some())
        .collect::<Vec<_>>();

    assert_eq!(fixture.projected_interface_count, projected.len());
    assert_eq!(fixture.projected_interface_count, fixture.interfaces.len());
    assert_eq!(
        fixture.description_template,
        "{summary} in the system backend."
    );
    let compiled = projected
        .iter()
        .map(|interface| {
            (
                interface.authorization_operation_id.clone().unwrap(),
                interface.route.method.clone(),
                interface.route.path.clone(),
                interface.summary.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(fixture.interfaces, compiled);
    // f88bd597c moved vendor exchange to managed-plugin contributions. These
    // host-owned IDs must not reappear in the exact core projection or fixture.
    for retired in [
        "applications.logs.langfuse.export",
        "applications.logs.langfuse.import",
    ] {
        assert!(!compiled.iter().any(|(id, _, _, _)| id == retired));
    }

    // Role catalog localization resolves these English metadata strings as dynamic keys.
    // A pinned official seed can predate a valid backend contract; CatalogResolver then
    // deliberately falls back to the English key. Validate the actual resolver input
    // contract without requiring every current interface to exist in that old seed.
    for interface in projected {
        for key in [&interface.summary, &interface.description] {
            assert!(
                domain::CatalogMessageIdentity::new(key.as_str()).is_ok(),
                "console interface {} has invalid catalog text: {key}",
                interface.interface_id
            );
        }
    }
}

#[test]
fn console_display_inventory_matches_key_only_fixture() {
    let inventory: Inventory = serde_json::from_str(include_str!(
        "../fixtures/dynamic_backend_consumers.d3-p4.json"
    ))
    .unwrap();
    let fixture = inventory
        .consumers
        .iter()
        .map(|consumer| consumer.key.as_str())
        .collect::<BTreeSet<_>>();
    let compiled = crate::routes::core_console_i18n::core_console_display_inventory()
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(inventory.count, inventory.consumers.len());
    assert_eq!(fixture, compiled);
    assert!(inventory
        .consumers
        .iter()
        .all(|consumer| { consumer.path.starts_with("api/") && !consumer.key.trim().is_empty() }));

    let static_references =
        crate::routes::core_console_i18n::core_console_static_reference_inventory();
    assert!(!static_references.is_empty());
    assert!(static_references.iter().all(|reference| {
        reference.starts_with("auto.")
            || reference.starts_with("console.")
            || *reference == "system_settings.title"
            // Event Center owns this existing static reference outside console.*.
            || *reference == "events.title"
    }));
    assert!(static_references
        .iter()
        .all(|reference| !compiled.contains(*reference)));
    assert!(compiled
        .iter()
        .all(|key| domain::CatalogMessageIdentity::new(*key).is_ok()));
}

#[test]
fn metadata_and_backend_consumers_are_key_only() {
    let metadata: Vec<MetadataConsumer> = serde_json::from_str(include_str!(
        "../../../../../crates/control-plane/src/_tests/fixtures/metadata_i18n_consumers.json"
    ))
    .unwrap();
    let expected = metadata
        .iter()
        .map(|consumer| consumer.key.as_str())
        .collect::<BTreeSet<_>>();
    let actual = control_plane::system_metadata::system_metadata_title_references()
        .into_iter()
        .map(|reference| reference.key)
        .chain(
            control_plane::file_management::file_metadata_title_references()
                .into_iter()
                .map(|reference| reference.key),
        )
        .collect::<BTreeSet<_>>();
    assert_eq!(expected, actual);

    let inventory: Inventory = serde_json::from_str(include_str!(
        "../fixtures/dynamic_backend_consumers.d3-p6.json"
    ))
    .unwrap();
    assert_eq!(inventory.count, inventory.consumers.len());
    assert!(inventory
        .consumers
        .iter()
        .all(|consumer| !consumer.key.trim().is_empty() && consumer.path.starts_with("api/")));
}

#[test]
fn official_seed_coverage_is_global_key_based_with_pinned_provenance() {
    let coverage: OfficialCoverageFixture = serde_json::from_str(include_str!(
        "../fixtures/official_i18n_catalog_coverage.d3-p8.json"
    ))
    .unwrap();
    let seed: serde_json::Value =
        serde_json::from_slice(crate::official_i18n_catalog_seed::OFFICIAL_SEED_BYTES).unwrap();
    assert_eq!(
        seed["manifest"]["catalog_version"],
        coverage.catalog_version
    );
    assert_eq!(
        seed["manifest"]["semantic_sha256"],
        coverage.semantic_sha256
    );
    assert_eq!(coverage.official_commit.len(), 40);
    let keys = seed["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["key"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(keys.len(), coverage.expected.catalog_keys);
    // Historical consumer counts in the pinned release fixture are not current
    // coverage evidence. The tests above compare today's complete compiled
    // interface, display and metadata inventories with their contract fixtures.
    for reference in coverage.p7_references {
        match reference.classification.as_str() {
            "official_demonstration" => assert!(keys.contains(reference.key.as_str())),
            "custom_fallback_not_official" => assert!(!keys.contains(reference.key.as_str())),
            _ => panic!("unknown low-code reference classification"),
        }
    }
}
