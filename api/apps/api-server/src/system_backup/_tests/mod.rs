use std::path::PathBuf;

use control_plane::{
    ports::{BackupComponentWriter, BackupKeyMaterial, BackupRepository},
    system_backup::authenticate_backup_manifest,
};
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupJobId, BackupJobState, BackupJournalEvent,
    BackupJournalEventKind, BackupJournalSubject, BackupManifest, BackupSetId,
    BackupSourceIdentity, ContentDigest, KeyFingerprint, MigrationHead,
};
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::LocalBackupRepository;

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("1flowbase-{label}-{}", Uuid::now_v7()))
}

fn fingerprint(value: char) -> String {
    std::iter::repeat(value).take(64).collect()
}

fn manifest(backup_set_id: BackupSetId) -> BackupManifest {
    BackupManifest::try_new(
        backup_set_id,
        OffsetDateTime::UNIX_EPOCH,
        ApplicationBuild::try_from("git.test").unwrap(),
        MigrationHead::try_from("migration.test").unwrap(),
        KeyFingerprint::try_from(fingerprint('a')).unwrap(),
        KeyFingerprint::try_from(fingerprint('b')).unwrap(),
        vec![BackupComponent {
            component_id: BackupComponentId::try_from("postgres/main").unwrap(),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgres/main").unwrap(),
            content_type: "application/octet-stream".to_owned(),
            size_bytes: 4,
            content_digest: ContentDigest::try_from(fingerprint('c')).unwrap(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
        }],
        4,
        ContentDigest::try_from(fingerprint('d')).unwrap(),
    )
    .unwrap()
}

async fn write_component(mut writer: BackupComponentWriter) {
    writer.write_all(b"test").await.unwrap();
    writer.shutdown().await.unwrap();
}

#[tokio::test]
async fn local_repository_seals_immutable_set_and_keeps_external_journal() {
    let root = temporary_root("backup-repository");
    let protected = temporary_root("business-storage");
    tokio::fs::create_dir_all(&protected).await.unwrap();
    let repository = LocalBackupRepository::open(&root, std::slice::from_ref(&protected))
        .await
        .unwrap();
    let backup_set_id = BackupSetId::new();
    repository.begin_staging(backup_set_id).await.unwrap();
    write_component(
        repository
            .open_staging_component(
                backup_set_id,
                &BackupComponentId::try_from("postgres/main").unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let key = BackupKeyMaterial::new(
        KeyFingerprint::try_from(fingerprint('e')).unwrap(),
        vec![9_u8; 32],
    )
    .unwrap();
    let sealed = authenticate_backup_manifest(manifest(backup_set_id), &key).unwrap();
    repository.seal(&sealed).await.unwrap();

    let subject = BackupJournalSubject::Backup(BackupJobId::new());
    repository
        .append_journal_event(&BackupJournalEvent {
            event_id: Uuid::now_v7(),
            sequence: 1,
            subject,
            backup_set_id,
            actor_user_id: None,
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            event: BackupJournalEventKind::BackupStateChanged {
                state: BackupJobState::Succeeded,
            },
        })
        .await
        .unwrap();

    assert_eq!(repository.list().await.unwrap().len(), 1);
    assert_eq!(repository.read_journal(subject).await.unwrap().len(), 1);
    tokio::fs::remove_dir_all(root).await.unwrap();
    tokio::fs::remove_dir_all(protected).await.unwrap();
}

#[tokio::test]
async fn local_repository_rejects_overlapping_protected_root() {
    let protected = temporary_root("overlap");
    let root = protected.join("backups");
    tokio::fs::create_dir_all(&protected).await.unwrap();

    assert!(
        LocalBackupRepository::open(&root, std::slice::from_ref(&protected))
            .await
            .is_err()
    );

    tokio::fs::remove_dir_all(protected).await.unwrap();
}
