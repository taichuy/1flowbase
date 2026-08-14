use std::{path::PathBuf, sync::Arc};

use control_plane::{
    ports::{
        BackupComponentWriter, BackupKeyMaterial, BackupRepository, CacheStore, SessionStore,
        TaskQueue,
    },
    system_backup::authenticate_backup_manifest,
    system_recovery::{PostRestoreRecoveryContext, RecoveryEphemeralState},
};
use domain::{
    ApplicationBuild, ArtifactRebuildability, BackupComponent, BackupComponentDisposition,
    BackupComponentId, BackupComponentKind, BackupComponentRestoreTarget, BackupJobId,
    BackupJobState, BackupJournalEvent, BackupJournalEventKind, BackupJournalSubject,
    BackupManifest, BackupSetId, BackupSourceIdentity, ContentDigest, KeyFingerprint,
    MigrationHead, RecoveryJobId, SessionRecord,
};
use storage_ephemeral::{MemoryTaskQueue, MokaCacheStore, MokaSessionStore};
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{ApiRecoveryEphemeralState, LocalBackupRepository};

mod toolchain;

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("1flowbase-{label}-{}", Uuid::now_v7()))
}

fn fingerprint(value: char) -> String {
    std::iter::repeat_n(value, 64).collect()
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
            restore_target: BackupComponentRestoreTarget::PostgreSql,
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

#[tokio::test]
async fn recovery_ephemeral_adapter_invalidates_old_sessions_cache_and_queue() {
    let sessions = Arc::new(MokaSessionStore::new("recovery:sessions", 32));
    let cache = Arc::new(MokaCacheStore::new("recovery:cache", 32));
    let queue = Arc::new(MemoryTaskQueue::new("recovery:queue"));
    let session_id = "session-before-restore";
    sessions
        .put(SessionRecord {
            session_id: session_id.to_string(),
            user_id: Uuid::now_v7(),
            tenant_id: Uuid::now_v7(),
            current_workspace_id: Uuid::now_v7(),
            active_role_code: "root".to_string(),
            session_version: 1,
            csrf_token: "fixture-token".to_string(),
            expires_at_unix: OffsetDateTime::now_utc().unix_timestamp() + 300,
        })
        .await
        .unwrap();
    CacheStore::set_json(
        &*cache,
        "catalog:old",
        serde_json::json!({"old": true}),
        None,
    )
    .await
    .unwrap();
    queue
        .enqueue("durable-rebuild", serde_json::json!({"old": true}), None)
        .await
        .unwrap();
    let adapter = ApiRecoveryEphemeralState::new(sessions.clone(), cache.clone(), queue.clone());
    let context = PostRestoreRecoveryContext {
        recovery_job_id: RecoveryJobId::new(),
        backup_set_id: BackupSetId::new(),
        safety_backup_set_id: BackupSetId::new(),
        actor_user_id: Uuid::now_v7(),
    };

    adapter.invalidate_after_restore(&context).await.unwrap();

    assert_eq!(sessions.get(session_id).await.unwrap(), None);
    assert_eq!(
        CacheStore::get_json(&*cache, "catalog:old").await.unwrap(),
        None
    );
    assert_eq!(
        queue
            .claim(
                "durable-rebuild",
                "worker-after-restore",
                time::Duration::seconds(30),
            )
            .await
            .unwrap(),
        None
    );
}
