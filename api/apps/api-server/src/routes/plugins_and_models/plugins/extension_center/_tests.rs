use std::collections::{BTreeMap, HashMap};

use serde_json::json;

use crate::official_extension_catalog::{
    OfficialExtensionCatalogEntry, OfficialExtensionCatalogEntrySource,
};

use super::{project_catalog_entry, InstalledCatalogJoin};

#[test]
fn root_1545_ac_4_catalog_projection_joins_real_local_version_by_stable_plugin_identity() {
    let entry = runtime_entry();
    let installed = HashMap::from([(
        "1flowbase.anthropic".to_string(),
        InstalledCatalogJoin {
            current_version: "0.1.32-dev".to_string(),
            artifact_kind: Some("model_provider".to_string()),
            source: "upload".to_string(),
            trust: "unknown".to_string(),
        },
    )]);

    let response = project_catalog_entry(
        entry,
        "official",
        &installed,
        &["official-key-2026-04".to_string()],
    );

    assert_eq!(response.version, "0.2.0");
    assert_eq!(response.current_version.as_deref(), Some("0.1.32-dev"));
    assert_eq!(response.installation_status, "installed");
    assert_eq!(response.installation_source.as_deref(), Some("upload"));
    assert_eq!(response.trust, "unknown");
    assert_eq!(response.artifact_kind.as_deref(), Some("model_provider"));
}

#[test]
fn root_1545_ac_4_catalog_projection_does_not_fabricate_uninstalled_artifact_kind() {
    let mut entry = runtime_entry();
    entry.signature = None;
    entry.checksum = None;
    entry.host_version_requirement = ">=99.0.0".to_string();

    let response = project_catalog_entry(entry, "official", &HashMap::new(), &[]);

    assert!(response.current_version.is_none());
    assert_eq!(response.installation_status, "not_installed");
    assert!(response.artifact_kind.is_none());
    assert_eq!(response.trust, "unknown");
    assert_eq!(
        response
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec![
            "signature_missing",
            "checksum_missing",
            "below_minimum_host_version"
        ]
    );
    assert!(response
        .warnings
        .iter()
        .all(|warning| warning.overridable && !warning.message.is_empty()));
    let compatibility = response.compatibility.unwrap();
    assert_eq!(compatibility.reason, "below_minimum_host_version");
    assert_eq!(compatibility.minimum_host_version, "99.0.0");
    assert!(!compatibility.current_host_version.is_empty());
}

fn runtime_entry() -> OfficialExtensionCatalogEntry {
    OfficialExtensionCatalogEntry {
        id: "runtime-extensions:taichuy/anthropic".to_string(),
        name: "Anthropic".to_string(),
        category: "runtime-extensions".to_string(),
        organization: "taichuy".to_string(),
        artifact: "anthropic".to_string(),
        version: "0.2.0".to_string(),
        description: "Anthropic runtime extension".to_string(),
        host_version_requirement: ">=0.3.0".to_string(),
        source: OfficialExtensionCatalogEntrySource {
            kind: "runtime_extension_manifest".to_string(),
            locator: "runtime-extensions/@taichuy/anthropic/manifest.yaml".to_string(),
            metadata: BTreeMap::from([("plugin_id".to_string(), json!("1flowbase.anthropic"))]),
        },
        signature: Some(json!({
            "algorithm": "ed25519",
            "key_id": "official-key-2026-04"
        })),
        checksum: Some(format!("sha256:{}", "a".repeat(64))),
        download_locator: json!({
            "kind": "platform_release_assets",
            "artifacts": []
        }),
        catalog_page: 1,
    }
}
