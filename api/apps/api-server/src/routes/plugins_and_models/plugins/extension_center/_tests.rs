use std::collections::{BTreeMap, HashMap};

use serde_json::json;

use crate::official_extension_catalog::{
    OfficialExtensionArtifactDescriptor, OfficialExtensionCatalogEntry,
    OfficialExtensionCatalogEntrySource,
};

use super::upload::upload_challenge;
use super::{
    artifact_preflight_challenge, project_catalog_entry, validate_preflight_overrides,
    InstalledCatalogJoin, PreflightDecision, UploadedExtensionArtifact,
};

#[test]
fn root_1545_ac_4_catalog_projection_joins_real_local_version_by_stable_plugin_identity() {
    let entry = runtime_entry();
    let installed = HashMap::from([(
        "1flowbase.anthropic".to_string(),
        InstalledCatalogJoin {
            current_version: "0.1.32-dev".to_string(),
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
    assert!(response.artifact_kind.is_none());
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

#[test]
fn root_1545_ac_5_preflight_requires_exact_integrity_and_compatibility_acknowledgement() {
    let mut entry = runtime_entry();
    entry.host_version_requirement = ">=99.0.0".to_string();
    let descriptor = OfficialExtensionArtifactDescriptor {
        locator_kind: "release_asset".to_string(),
        locator: "https://example.test/plugin.1flowbasepkg".to_string(),
        expected_checksum: None,
        signature: None,
    };
    let challenge = artifact_preflight_challenge(&entry, &descriptor, &[]);
    assert_eq!(
        challenge
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["checksum_missing", "signature_missing"]
    );
    assert!(matches!(
        validate_preflight_overrides(&challenge, None, None).unwrap(),
        PreflightDecision::Challenge
    ));
    let risk = super::PluginRiskOverrideBody {
        reason: "Operator accepts development artifact".to_string(),
        acknowledged_warnings: vec![
            "checksum_missing".to_string(),
            "signature_missing".to_string(),
        ],
    };
    let compatibility = challenge.compatibility.as_ref().unwrap();
    let compatibility_override = super::PluginCompatibilityOverrideBody {
        reason: compatibility.reason.clone(),
        acknowledged_current_host_version: compatibility.current_host_version.clone(),
        acknowledged_minimum_host_version: compatibility.minimum_host_version.clone(),
    };
    assert!(matches!(
        validate_preflight_overrides(&challenge, Some(&risk), Some(&compatibility_override))
            .unwrap(),
        PreflightDecision::Accepted(Some(_))
    ));
}

#[test]
fn root_1545_ac_5_uploaded_invalid_signature_remains_overridable() {
    let challenge = upload_challenge(&UploadedExtensionArtifact {
        category: super::ExtensionCatalogCategory::I18n,
        organization: "taichuy".to_string(),
        artifact_id: "platform".to_string(),
        version: "2.0.1".to_string(),
        minimum_host_version: None,
        node_plugin: false,
        signature_status: domain::ExtensionSignatureStatus::Invalid,
        signature_algorithm: Some("ed25519".to_string()),
        signing_key_id: Some("unknown".to_string()),
    });
    assert_eq!(
        challenge
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["checksum_missing", "signature_invalid"]
    );
    assert!(challenge.warnings.iter().all(|warning| warning.overridable));
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
