use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::super::{
    build_backup_artifact_inventory, BackupArtifactDisposition, BackupArtifactInventoryReason,
    BackupArtifactKind,
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

fn current_ready_artifact(installation_id: Uuid) -> domain::PluginArtifactInstanceRecord {
    domain::PluginArtifactInstanceRecord {
        node_id: "node-a".to_owned(),
        installation_id,
        local_version: Some("1.0.0".to_owned()),
        local_checksum: Some("sha256:package".to_owned()),
        local_path: Some("/tmp/example-installed".to_owned()),
        package_path: Some("/tmp/example.1flowbasepkg".to_owned()),
        manifest_fingerprint: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: domain::PluginRuntimeStatus::Inactive,
        availability_status: domain::PluginAvailabilityStatus::Disabled,
        checked_at: OffsetDateTime::UNIX_EPOCH,
        last_error: None,
        is_current: true,
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

    assert_eq!(
        error.reason,
        BackupArtifactInventoryReason::RetainedArtifactMissing
    );
}

#[test]
fn required_rebuildable_plugin_rejects_invalid_catalog_identity() {
    let mut unverified = installation("official");
    unverified.verification_status = domain::PluginVerificationStatus::Invalid;

    let error = build_backup_artifact_inventory("node-a", vec![unverified], Vec::new(), Vec::new())
        .unwrap_err();

    assert_eq!(
        error.reason,
        BackupArtifactInventoryReason::RebuildableIdentityInvalid
    );
}

#[test]
fn backup_inventory_excludes_historical_uploaded_installations_without_a_ready_current_artifact() {
    let mut historical = installation("uploaded");
    historical.desired_state = domain::PluginDesiredState::Disabled;

    let entries =
        build_backup_artifact_inventory("node-a", vec![historical], Vec::new(), Vec::new())
            .unwrap();

    assert!(entries.is_empty());
}

#[test]
fn backup_inventory_excludes_historical_invalid_registry_identity_without_a_ready_current_artifact()
{
    let mut historical = installation("official_registry");
    historical.desired_state = domain::PluginDesiredState::Disabled;
    historical.verification_status = domain::PluginVerificationStatus::Invalid;

    let entries =
        build_backup_artifact_inventory("node-a", vec![historical], Vec::new(), Vec::new())
            .unwrap();

    assert!(entries.is_empty());
}

#[test]
fn backup_inventory_retains_disabled_uploaded_installations_with_a_ready_current_artifact() {
    let mut disabled = installation("uploaded");
    disabled.desired_state = domain::PluginDesiredState::Disabled;
    let artifact = current_ready_artifact(disabled.id);

    let entries =
        build_backup_artifact_inventory("node-a", vec![disabled], vec![artifact], Vec::new())
            .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].disposition, BackupArtifactDisposition::Embedded);
}

#[test]
fn backup_inventory_excludes_disabled_installations_with_a_non_ready_current_artifact() {
    let mut disabled = installation("uploaded");
    disabled.desired_state = domain::PluginDesiredState::Disabled;
    let mut artifact = current_ready_artifact(disabled.id);
    artifact.artifact_status = domain::PluginArtifactInstanceStatus::Missing;

    let entries =
        build_backup_artifact_inventory("node-a", vec![disabled], vec![artifact], Vec::new())
            .unwrap();

    assert!(entries.is_empty());
}

#[test]
fn pending_restart_rebuildable_plugin_is_required_without_a_local_artifact() {
    let mut pending_restart = installation("official_registry");
    pending_restart.desired_state = domain::PluginDesiredState::PendingRestart;

    let entries =
        build_backup_artifact_inventory("node-a", vec![pending_restart], Vec::new(), Vec::new())
            .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].disposition,
        BackupArtifactDisposition::RebuildableIdentity
    );
}

#[test]
fn configured_proxy_uses_verified_official_identity_without_a_current_artifact() {
    let entries = build_backup_artifact_inventory(
        "node-a",
        vec![installation("configured_proxy")],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].disposition,
        BackupArtifactDisposition::RebuildableIdentity
    );
    assert_eq!(entries[0].artifact_path, None);
}
