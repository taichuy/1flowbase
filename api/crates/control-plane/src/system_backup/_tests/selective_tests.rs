use super::*;
use crate::system_backup::SelectiveSystemBackupService;
use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane_contracts::system_backup::{
    selective::{
        PreparedSelectiveRestore, SelectiveBackupCategory, SelectiveBackupObjectScope,
        SelectiveBackupPreview, SelectiveBackupRepository, SelectiveBackupSelection,
        SelectiveObjectStorage, SELECTIVE_BACKUP_CONTENT_TYPE,
    },
    BackupComponentSource,
};
use domain::RecoveryJobId;
use std::{collections::BTreeMap, path::PathBuf};

const ROWS: &[u8] = br#"{"rows":[{"id":"original-primary-key","value":"source"}]}"#;
const OBJECT: &[u8] = b"replacement object";

struct MemoryBackup {
    sealed: SealedBackupManifest,
    encrypted: Mutex<BTreeMap<String, Vec<u8>>>,
}
#[async_trait]
impl BackupRepository for MemoryBackup {
    async fn begin_staging(&self, _: BackupSetId) -> Result<(), BackupRepositoryError> {
        unreachable!()
    }
    async fn open_staging_component(
        &self,
        _: BackupSetId,
        _: &BackupComponentId,
    ) -> Result<BackupComponentWriter, BackupRepositoryError> {
        unreachable!()
    }
    async fn abort_staging(&self, _: BackupSetId) -> Result<(), BackupRepositoryError> {
        unreachable!()
    }
    async fn seal(&self, _: &SealedBackupManifest) -> Result<(), BackupRepositoryError> {
        unreachable!()
    }
    async fn list(&self) -> Result<Vec<BackupSetCatalogEntry>, BackupRepositoryError> {
        unreachable!()
    }
    async fn load_manifest(
        &self,
        _: BackupSetId,
    ) -> Result<SealedBackupManifest, BackupRepositoryError> {
        Ok(self.sealed.clone())
    }
    async fn open_component(
        &self,
        _: BackupSetId,
        id: &BackupComponentId,
    ) -> Result<BackupComponentReader, BackupRepositoryError> {
        Ok(Box::pin(std::io::Cursor::new(
            self.encrypted.lock().unwrap()[id.as_str()].clone(),
        )))
    }
    async fn delete(&self, _: BackupSetId) -> Result<(), BackupRepositoryError> {
        unreachable!()
    }
    async fn append_journal_event(
        &self,
        _: &BackupJournalEvent,
    ) -> Result<(), BackupRepositoryError> {
        Ok(())
    }
    async fn read_journal(
        &self,
        _: BackupJournalSubject,
    ) -> Result<Vec<BackupJournalEvent>, BackupRepositoryError> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct Observed {
    prepared: usize,
    committed: usize,
    rolled_back: usize,
    received_rows: Vec<u8>,
    source_key: String,
    target_key: String,
    promoted_bytes_at_commit: Option<Vec<u8>>,
}
#[derive(Default)]
struct RowRepository {
    observed: Arc<Mutex<Observed>>,
    missing_plugin: bool,
    fail_commit: bool,
    storages: Vec<SelectiveObjectStorage>,
    object_path: Option<PathBuf>,
    corrupt_object_after_prepare: Option<Arc<MemoryBackup>>,
}
impl RowRepository {
    fn preview(&self) -> SelectiveBackupPreview {
        SelectiveBackupPreview {
            table_count: 1,
            row_count: 1,
            selected_tables: vec!["settings".into()],
            missing_plugins: if self.missing_plugin {
                vec!["example.plugin".into()]
            } else {
                vec![]
            },
            ..Default::default()
        }
    }
}
#[async_trait]
impl SelectiveBackupRepository for RowRepository {
    async fn catalog(&self) -> Result<Vec<SelectiveBackupCategory>> {
        unreachable!()
    }
    async fn source(
        &self,
        _: Vec<SelectiveBackupSelection>,
    ) -> Result<Arc<dyn BackupComponentSource>> {
        unreachable!()
    }
    async fn object_scope(
        &self,
        _: &[SelectiveBackupSelection],
    ) -> Result<SelectiveBackupObjectScope> {
        unreachable!()
    }
    async fn preflight(
        &self,
        mut reader: BackupComponentReader,
        _: &str,
        _: &str,
    ) -> Result<SelectiveBackupPreview> {
        let mut rows = vec![];
        reader.read_to_end(&mut rows).await?;
        assert_eq!(rows, ROWS);
        Ok(self.preview())
    }
    async fn prepare_restore(
        &self,
        mut reader: BackupComponentReader,
        source: &str,
        target: &str,
        confirmed: bool,
    ) -> Result<Box<dyn PreparedSelectiveRestore>> {
        if self.missing_plugin && !confirmed {
            bail!("missing plugins require confirmation");
        }
        let mut rows = vec![];
        reader.read_to_end(&mut rows).await?;
        let mut observed = self.observed.lock().unwrap();
        if let Some(backup) = &self.corrupt_object_after_prepare {
            *backup
                .encrypted
                .lock()
                .unwrap()
                .get_mut("objects/file")
                .unwrap()
                .last_mut()
                .unwrap() ^= 1;
        }
        observed.prepared += 1;
        observed.received_rows = rows;
        observed.source_key = source.to_owned();
        observed.target_key = target.to_owned();
        Ok(Box::new(PreparedRows {
            observed: self.observed.clone(),
            preview: self.preview(),
            storages: self.storages.clone(),
            fail_commit: self.fail_commit,
            object_path: self.object_path.clone(),
        }))
    }
    async fn restore(
        &self,
        _: BackupComponentReader,
        _: &str,
        _: &str,
        _: bool,
    ) -> Result<SelectiveBackupPreview> {
        panic!("service must coordinate prepare/commit rather than bypassing object compensation")
    }
}
struct PreparedRows {
    observed: Arc<Mutex<Observed>>,
    preview: SelectiveBackupPreview,
    storages: Vec<SelectiveObjectStorage>,
    fail_commit: bool,
    object_path: Option<PathBuf>,
}
#[async_trait]
impl PreparedSelectiveRestore for PreparedRows {
    fn preview(&self) -> &SelectiveBackupPreview {
        &self.preview
    }
    fn object_storages(&self) -> &[SelectiveObjectStorage] {
        &self.storages
    }
    async fn commit(self: Box<Self>) -> Result<SelectiveBackupPreview> {
        let mut observed = self.observed.lock().unwrap();
        observed.promoted_bytes_at_commit = self
            .object_path
            .as_ref()
            .map(|path| std::fs::read(path).unwrap());
        if self.fail_commit {
            bail!("database commit rejected");
        }
        observed.committed += 1;
        Ok(self.preview)
    }
    async fn rollback(self: Box<Self>) -> Result<()> {
        self.observed.lock().unwrap().rolled_back += 1;
        Ok(())
    }
}

async fn backup_fixture(
    password: Option<&str>,
    storage_id: Option<uuid::Uuid>,
) -> Arc<MemoryBackup> {
    let id = BackupSetId::new();
    let source = STANDARD.encode("source-master-key");
    let protection = password.map(|password| {
        crate::system_backup::password_protection(password, Some(&source)).unwrap()
    });
    let mut derivation = Sha256::new();
    derivation.update(b"1flowbase/system-backup/key/v1\0");
    derivation.update(b"source-master-key");
    let bytes = derivation.finalize().to_vec();
    let portable_key = BackupKeyMaterial::new(
        KeyFingerprint::try_from(format!("{:x}", Sha256::digest(&bytes))).unwrap(),
        bytes,
    )
    .unwrap();
    let key = protection
        .as_ref()
        .map_or(&portable_key, |value| &value.key);
    let mut components = vec![];
    let mut encrypted = BTreeMap::new();
    let mut specifications = vec![(
        "postgres/settings",
        ROWS,
        BackupComponentKind::PostgreSql,
        SELECTIVE_BACKUP_CONTENT_TYPE,
        BackupComponentRestoreTarget::PostgreSql,
    )];
    if let Some(storage_id) = storage_id {
        specifications.push((
            "objects/file",
            OBJECT,
            BackupComponentKind::BusinessObject,
            "text/plain",
            BackupComponentRestoreTarget::BusinessObject {
                storage_id,
                object_path: "files/example.txt".into(),
            },
        ));
    }
    for (name, bytes, kind, content_type, restore_target) in specifications {
        let component_id = BackupComponentId::try_from(name).unwrap();
        let mut buffer = vec![];
        let receipt = encrypt_backup_stream(
            std::io::Cursor::new(bytes),
            &mut buffer,
            key,
            id,
            &component_id,
        )
        .await
        .unwrap();
        encrypted.insert(name.to_owned(), buffer);
        components.push(BackupComponent {
            component_id,
            kind,
            source_identity: BackupSourceIdentity::try_from(name).unwrap(),
            content_type: content_type.into(),
            size_bytes: receipt.plaintext_size_bytes,
            content_digest: receipt.plaintext_digest,
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target,
        });
    }
    let build = ApplicationBuild::try_from("test.selective").unwrap();
    let migration = MigrationHead::try_from("test.migration").unwrap();
    let master_fingerprint = KeyFingerprint::try_from(fingerprint('a')).unwrap();
    let total = components
        .iter()
        .map(|component| component.size_bytes)
        .sum();
    let digest = ContentDigest::try_from(fingerprint('e')).unwrap();
    let manifest = if let Some(protection) = protection.as_ref() {
        BackupManifest::try_new_password_encrypted(
            id,
            OffsetDateTime::UNIX_EPOCH,
            build,
            migration,
            master_fingerprint,
            key.fingerprint().clone(),
            protection.salt_base64.clone(),
            protection.encrypted_source_master_key_base64.clone(),
            components,
            total,
            digest,
        )
    } else {
        BackupManifest::try_new_portable(
            id,
            OffsetDateTime::UNIX_EPOCH,
            build,
            migration,
            master_fingerprint,
            key.fingerprint().clone(),
            source,
            components,
            total,
            digest,
        )
    }
    .unwrap();
    Arc::new(MemoryBackup {
        sealed: authenticate_backup_manifest(manifest, key).unwrap(),
        encrypted: Mutex::new(encrypted),
    })
}
fn service(backup: Arc<MemoryBackup>, rows: Arc<RowRepository>) -> SelectiveSystemBackupService {
    SelectiveSystemBackupService::new(
        rows,
        Arc::new(SystemBackupService::new(
            backup,
            Arc::new(RejectingKeyProvider),
        )),
        Arc::new(
            storage_object::FileStorageDriverRegistry::default()
                .register(Arc::new(storage_object::LocalFileStorageDriver)),
        ),
    )
}

#[tokio::test]
async fn wrong_password_never_prepares_or_commits_rows() {
    let backup = backup_fixture(Some("correct-password"), None).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository::default());
    assert!(service(backup, rows.clone())
        .restore(
            id,
            Some("wrong-password"),
            "target-key",
            false,
            RecoveryJobId::new()
        )
        .await
        .is_err());
    let observed = rows.observed.lock().unwrap();
    assert_eq!((observed.prepared, observed.committed), (0, 0));
}
#[tokio::test]
async fn corrupt_payload_never_prepares_or_commits_rows() {
    let backup = backup_fixture(None, None).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository::default());
    *backup
        .encrypted
        .lock()
        .unwrap()
        .get_mut("postgres/settings")
        .unwrap()
        .last_mut()
        .unwrap() ^= 1;
    assert!(service(backup, rows.clone())
        .restore(id, None, "target-key", false, RecoveryJobId::new())
        .await
        .is_err());
    let observed = rows.observed.lock().unwrap();
    assert_eq!((observed.prepared, observed.committed), (0, 0));
}
#[tokio::test]
async fn missing_plugin_confirmation_is_required_and_forwarded() {
    let backup = backup_fixture(None, None).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository {
        missing_plugin: true,
        ..Default::default()
    });
    let service = service(backup, rows.clone());
    assert!(service
        .restore(id, None, "target-key", false, RecoveryJobId::new())
        .await
        .is_err());
    assert_eq!(rows.observed.lock().unwrap().committed, 0);
    let preview = service
        .restore(id, None, "target-key", true, RecoveryJobId::new())
        .await
        .unwrap();
    assert_eq!(preview.missing_plugins, vec!["example.plugin"]);
    assert_eq!(rows.observed.lock().unwrap().committed, 1);
}
#[tokio::test]
async fn no_objects_restore_passes_original_rows_and_keys_to_one_transaction() {
    let backup = backup_fixture(None, None).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository::default());
    let preview = service(backup, rows.clone())
        .restore(id, None, "target-key", false, RecoveryJobId::new())
        .await
        .unwrap();
    assert_eq!((preview.table_count, preview.row_count), (1, 1));
    let observed = rows.observed.lock().unwrap();
    assert_eq!(
        (observed.prepared, observed.committed, observed.rolled_back),
        (1, 1, 0)
    );
    assert_eq!(observed.received_rows, ROWS);
    assert_eq!(observed.source_key, "source-master-key");
    assert_eq!(observed.target_key, "target-key");
}

struct TemporaryRoot(PathBuf);
impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[tokio::test]
async fn uncertain_database_commit_retains_promoted_object_and_rollback_copy() {
    let root = TemporaryRoot(
        std::env::temp_dir().join(format!("selective-test-{}", uuid::Uuid::now_v7())),
    );
    let object_path = root.0.join("files/example.txt");
    std::fs::create_dir_all(object_path.parent().unwrap()).unwrap();
    std::fs::write(&object_path, b"target original").unwrap();
    let storage_id = uuid::Uuid::now_v7();
    let backup = backup_fixture(None, Some(storage_id)).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository {
        fail_commit: true,
        object_path: Some(object_path.clone()),
        storages: vec![SelectiveObjectStorage {
            storage_id,
            driver_type: "local".into(),
            config_json: serde_json::json!({"root_path": root.0}),
        }],
        ..Default::default()
    });
    let recovery_job_id = RecoveryJobId::new();
    let error = service(backup, rows.clone())
        .restore(id, None, "target-key", false, recovery_job_id)
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::system_backup::SelectiveRestoreNeedsRepair>()
            .unwrap()
            .0,
        "database_commit_uncertain"
    );
    let observed = rows.observed.lock().unwrap();
    assert_eq!(observed.promoted_bytes_at_commit.as_deref(), Some(OBJECT));
    assert_eq!(observed.committed, 0);
    assert_eq!(std::fs::read(object_path).unwrap(), OBJECT);
    let mut hasher = Sha256::new();
    hasher.update(storage_id.as_bytes());
    hasher.update(b"files/example.txt");
    hasher.update(b"objects/file");
    let identity = format!("{:x}", hasher.finalize());
    let rollback = root.0.join(format!(
        "__1flowbase_recovery/{}/{identity}/rollback",
        recovery_job_id.as_uuid()
    ));
    assert_eq!(std::fs::read(rollback).unwrap(), b"target original");
}

#[tokio::test]
async fn object_materialization_failure_rolls_back_prepared_rows_and_preserves_target() {
    let root = TemporaryRoot(
        std::env::temp_dir().join(format!("selective-test-{}", uuid::Uuid::now_v7())),
    );
    let object_path = root.0.join("files/example.txt");
    std::fs::create_dir_all(object_path.parent().unwrap()).unwrap();
    std::fs::write(&object_path, b"target original").unwrap();
    let storage_id = uuid::Uuid::now_v7();
    let backup = backup_fixture(None, Some(storage_id)).await;
    let id = backup.sealed.manifest().backup_set_id();
    let rows = Arc::new(RowRepository {
        corrupt_object_after_prepare: Some(backup.clone()),
        storages: vec![SelectiveObjectStorage {
            storage_id,
            driver_type: "local".into(),
            config_json: serde_json::json!({ "root_path": root.0 }),
        }],
        ..Default::default()
    });
    let error = service(backup, rows.clone())
        .restore(id, None, "target-key", false, RecoveryJobId::new())
        .await
        .unwrap_err();
    assert!(error
        .downcast_ref::<crate::system_backup::SelectiveRestoreNeedsRepair>()
        .is_none());
    let observed = rows.observed.lock().unwrap();
    assert_eq!(
        (observed.prepared, observed.committed, observed.rolled_back),
        (1, 0, 1)
    );
    assert_eq!(std::fs::read(object_path).unwrap(), b"target original");
}

#[tokio::test]
async fn corrupt_object_is_rejected_before_rows_prepare_even_when_rows_are_intact() {
    let backup = backup_fixture(None, Some(uuid::Uuid::now_v7())).await;
    let id = backup.sealed.manifest().backup_set_id();
    *backup
        .encrypted
        .lock()
        .unwrap()
        .get_mut("objects/file")
        .unwrap()
        .last_mut()
        .unwrap() ^= 1;
    let rows = Arc::new(RowRepository::default());
    let service = service(backup, rows.clone());
    assert!(service.preflight(id, None, "target-key").await.is_err());
    assert!(service
        .restore(id, None, "target-key", false, RecoveryJobId::new())
        .await
        .is_err());
    assert_eq!(rows.observed.lock().unwrap().prepared, 0);
}
