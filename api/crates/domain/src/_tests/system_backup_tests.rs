use std::collections::BTreeSet;

use time::OffsetDateTime;

use crate::{
    strict_backup_compatibility, ApplicationBuild, ArtifactRebuildability,
    BackupCompatibilityTarget, BackupComponent, BackupComponentDisposition, BackupComponentId,
    BackupComponentKind, BackupExcludedDomain, BackupIncompatibility, BackupJob, BackupJobId,
    BackupJobState, BackupJobTransitionError, BackupManifest, BackupSetId, BackupSourceIdentity,
    ContentDigest, KeyFingerprint, MigrationHead, RecoveryJob, RecoveryJobId, RecoveryJobState,
    SYSTEM_BACKUP_CHUNK_SIZE_BYTES, SYSTEM_BACKUP_FORMAT_VERSION,
    SYSTEM_BACKUP_MAX_PARALLEL_STREAMS,
};

fn fingerprint(value: char) -> String {
    std::iter::repeat(value).take(64).collect()
}

fn postgres_component() -> BackupComponent {
    BackupComponent {
        component_id: BackupComponentId::try_from("postgres/main").unwrap(),
        kind: BackupComponentKind::PostgreSql,
        source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
        content_type: "application/vnd.postgresql.custom-dump".to_owned(),
        size_bytes: 42,
        content_digest: ContentDigest::try_from(fingerprint('a')).unwrap(),
        disposition: BackupComponentDisposition::Embedded,
        rebuildability: ArtifactRebuildability::NotApplicable,
    }
}

fn manifest() -> BackupManifest {
    BackupManifest::try_new(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.5f906803").unwrap(),
        MigrationHead::try_from("202608110001").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        KeyFingerprint::try_from(fingerprint('c')).unwrap(),
        vec![postgres_component()],
        42,
        ContentDigest::try_from(fingerprint('d')).unwrap(),
    )
    .unwrap()
}

#[test]
fn manifest_freezes_streaming_and_exclusion_contract() {
    let manifest = manifest();

    assert_eq!(manifest.format_version(), SYSTEM_BACKUP_FORMAT_VERSION);
    assert_eq!(manifest.chunk_size_bytes(), SYSTEM_BACKUP_CHUNK_SIZE_BYTES);
    assert_eq!(
        manifest.max_parallel_streams(),
        SYSTEM_BACKUP_MAX_PARALLEL_STREAMS
    );
    assert_eq!(
        manifest.excluded_domains(),
        &BackupExcludedDomain::ALL
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
}

#[test]
fn source_identity_preserves_rebuildable_artifact_coordinates() {
    let identity =
        BackupSourceIdentity::try_from("plugin:capability-plugins/taichuy/fixture_provider@0.1.0")
            .unwrap();
    assert_eq!(
        identity.as_str(),
        "plugin:capability-plugins/taichuy/fixture_provider@0.1.0"
    );
}

#[test]
fn manifest_deserialization_rejects_missing_postgres_and_incomplete_exclusions() {
    let manifest_json = serde_json::to_value(manifest()).unwrap();

    let mut missing_postgres = manifest_json.clone();
    missing_postgres["components"] = serde_json::json!([]);
    missing_postgres["total_size_bytes"] = serde_json::json!(0);
    assert!(serde_json::from_value::<BackupManifest>(missing_postgres).is_err());

    let mut incomplete_exclusions = manifest_json;
    incomplete_exclusions["excluded_domains"] = serde_json::json!(["ephemeral_state"]);
    assert!(serde_json::from_value::<BackupManifest>(incomplete_exclusions).is_err());
}

#[test]
fn non_rebuildable_artifact_must_be_embedded() {
    let mut artifact = postgres_component();
    artifact.component_id = BackupComponentId::try_from("extension/uploaded.demo").unwrap();
    artifact.kind = BackupComponentKind::ExtensionArtifact;
    artifact.source_identity = BackupSourceIdentity::try_from("extension/uploaded.demo").unwrap();
    artifact.disposition = BackupComponentDisposition::IdentityOnly;
    artifact.rebuildability = ArtifactRebuildability::NonRebuildable;

    assert!(BackupManifest::try_new(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.5f906803").unwrap(),
        MigrationHead::try_from("202608110001").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        KeyFingerprint::try_from(fingerprint('c')).unwrap(),
        vec![postgres_component(), artifact],
        84,
        ContentDigest::try_from(fingerprint('d')).unwrap(),
    )
    .is_err());
}

#[test]
fn empty_business_object_is_valid_but_empty_postgres_is_not() {
    let mut empty_object = postgres_component();
    empty_object.component_id = BackupComponentId::try_from("object/empty").unwrap();
    empty_object.kind = BackupComponentKind::BusinessObject;
    empty_object.source_identity = BackupSourceIdentity::try_from("object/empty").unwrap();
    empty_object.size_bytes = 0;
    let valid = BackupManifest::try_new(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.5f906803").unwrap(),
        MigrationHead::try_from("202608110001").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        KeyFingerprint::try_from(fingerprint('c')).unwrap(),
        vec![postgres_component(), empty_object],
        42,
        ContentDigest::try_from(fingerprint('d')).unwrap(),
    );
    assert!(valid.is_ok());

    let mut empty_postgres = postgres_component();
    empty_postgres.size_bytes = 0;
    assert!(BackupManifest::try_new(
        BackupSetId::new(),
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.5f906803").unwrap(),
        MigrationHead::try_from("202608110001").unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        KeyFingerprint::try_from(fingerprint('c')).unwrap(),
        vec![empty_postgres],
        0,
        ContentDigest::try_from(fingerprint('d')).unwrap(),
    )
    .is_err());
}

#[test]
fn backup_job_rejects_skipped_and_post_terminal_transitions() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut job = BackupJob::new(BackupJobId::new(), BackupSetId::new(), now);

    assert_eq!(
        job.transition(BackupJobState::Capturing, now, None),
        Err(BackupJobTransitionError::InvalidTransition)
    );
    job.transition(
        BackupJobState::Failed,
        now,
        Some("capture_failed".to_owned()),
    )
    .unwrap();
    assert_eq!(
        job.transition(BackupJobState::Fencing, now, None),
        Err(BackupJobTransitionError::InvalidTransition)
    );
}

#[test]
fn recovery_job_requires_safety_backup_before_restoring_path() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut job = RecoveryJob::new(RecoveryJobId::new(), BackupSetId::new(), now);

    job.transition(RecoveryJobState::AwaitingConfirmation, now, None)
        .unwrap();
    job.transition(RecoveryJobState::SafetyBackup, now, None)
        .unwrap();
    assert!(job
        .transition(RecoveryJobState::Fencing, now, None)
        .is_err());
    assert!(job.safety_backup_set_id().is_none());
    job.record_safety_backup(BackupSetId::new()).unwrap();
    job.transition(RecoveryJobState::Fencing, now, None)
        .unwrap();
}

#[test]
fn strict_compatibility_reports_every_mismatch() {
    let manifest = manifest();
    let result = strict_backup_compatibility(
        &manifest,
        &BackupCompatibilityTarget {
            format_version: SYSTEM_BACKUP_FORMAT_VERSION + 1,
            application_build: ApplicationBuild::try_from("git.other").unwrap(),
            migration_head: MigrationHead::try_from("202608120001").unwrap(),
            master_key_fingerprint: KeyFingerprint::try_from(fingerprint('e')).unwrap(),
        },
    );

    assert_eq!(
        result,
        Err(vec![
            BackupIncompatibility::FormatVersion,
            BackupIncompatibility::ApplicationBuild,
            BackupIncompatibility::MigrationHead,
            BackupIncompatibility::MasterKeyFingerprint,
        ])
    );
}
