use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::super::{
    build_backup_artifact_inventory, BackupArtifactDisposition, BackupArtifactKind,
};

fn installation(source_kind: &str) -> domain::PluginInstallationRecord {
    domain::PluginInstallationRecord {
        id: Uuid::from_u128(1),
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::CapabilityPlugins,
        organization: "onebase".to_owned(),
        provider_code: "builtin_blocks".to_owned(),
        plugin_id: "builtin_blocks@1.0.0".to_owned(),
        plugin_version: "1.0.0".to_owned(),
        contract_version: "1flowbase.plugin/v1".to_owned(),
        protocol: "declarative".to_owned(),
        display_name: "Builtin Blocks".to_owned(),
        source_kind: source_kind.to_owned(),
        trust_level: "verified_official".to_owned(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::ActiveRequested,
        expected_checksum: Some("sha256:catalog".to_owned()),
        signature_status: domain::ExtensionSignatureStatus::Verified,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({}),
        is_system_reserved: source_kind == "builtin",
        created_by: Uuid::from_u128(2),
        updated_by: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn rebuildable_plugin_uses_verified_immutable_identity_without_current_artifact() {
    let entries = build_backup_artifact_inventory(
        "node-a",
        vec![installation("builtin")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.kind, BackupArtifactKind::Extension);
    assert_eq!(
        entry.disposition,
        BackupArtifactDisposition::RebuildableIdentity
    );
    assert_eq!(
        entry.identity,
        "plugin:capability-plugins/onebase/builtin_blocks@1.0.0@1.0.0"
    );
    assert_eq!(entry.expected_checksum.as_deref(), Some("sha256:catalog"));
    assert_eq!(entry.artifact_path, None);
}

#[test]
fn embedded_plugin_requires_a_current_retained_artifact() {
    let error = build_backup_artifact_inventory(
        "node-a",
        vec![installation("uploaded")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("non-rebuildable plugin artifact instance is missing"),
        "{error:#}"
    );
}

#[test]
fn rebuildable_plugin_rejects_unverified_catalog_identity() {
    let mut unverified = installation("official");
    unverified.verification_status = domain::PluginVerificationStatus::Pending;

    let error = build_backup_artifact_inventory("node-a", vec![unverified], Vec::new(), Vec::new())
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "rebuildable plugin identity is not verifiable"
    );
}
