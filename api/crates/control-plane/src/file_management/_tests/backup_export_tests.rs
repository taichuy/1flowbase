use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::file_management::{
    BackupObjectDatabaseReference, BackupObjectExportError, BackupObjectInventoryRecord,
    BackupObjectInventoryRepository, BusinessObjectBackupExporter, BusinessObjectBackupInventory,
};

#[derive(Clone)]
struct FixtureRepository {
    records: Vec<BackupObjectInventoryRecord>,
}

#[async_trait]
impl BackupObjectInventoryRepository for FixtureRepository {
    async fn list_backup_object_inventory(
        &self,
    ) -> anyhow::Result<Vec<BackupObjectInventoryRecord>> {
        Ok(self.records.clone())
    }
}

fn record(
    storage_id: Uuid,
    path: &str,
    content_type: &str,
    size_bytes: u64,
    reference: BackupObjectDatabaseReference,
) -> BackupObjectInventoryRecord {
    BackupObjectInventoryRecord {
        reference,
        storage_id,
        driver_type: "local".to_string(),
        storage_config: serde_json::json!({ "root_path": "/tmp/object-backup-fixture" }),
        object_path: path.to_string(),
        content_type: content_type.to_string(),
        size_bytes,
    }
}

#[test]
fn inventory_is_stably_sorted_and_identical_object_references_are_deduplicated() {
    let storage_id = Uuid::now_v7();
    let first_record_id = Uuid::now_v7();
    let second_record_id = Uuid::now_v7();
    let file_table_id = Uuid::now_v7();
    let inventory = BusinessObjectBackupInventory::try_from_records(vec![
        record(
            storage_id,
            "z-last.bin",
            "application/octet-stream",
            9,
            BackupObjectDatabaseReference::FileRecord {
                file_table_id,
                record_id: first_record_id,
            },
        ),
        record(
            storage_id,
            "a-first.bin",
            "application/octet-stream",
            0,
            BackupObjectDatabaseReference::RuntimeDebugArtifact {
                artifact_id: Uuid::now_v7(),
            },
        ),
        record(
            storage_id,
            "z-last.bin",
            "application/octet-stream",
            9,
            BackupObjectDatabaseReference::FileRecord {
                file_table_id,
                record_id: second_record_id,
            },
        ),
    ])
    .unwrap();

    assert_eq!(inventory.objects().len(), 2);
    assert_eq!(inventory.objects()[0].identity().object_path, "a-first.bin");
    assert_eq!(inventory.objects()[0].size_bytes(), 0);
    assert_eq!(inventory.objects()[1].identity().object_path, "z-last.bin");
    assert_eq!(inventory.objects()[1].references().len(), 2);
}

#[test]
fn duplicate_object_metadata_conflict_fails_closed() {
    let storage_id = Uuid::now_v7();
    let file_table_id = Uuid::now_v7();
    let records = vec![
        record(
            storage_id,
            "same.bin",
            "application/octet-stream",
            9,
            BackupObjectDatabaseReference::FileRecord {
                file_table_id,
                record_id: Uuid::now_v7(),
            },
        ),
        record(
            storage_id,
            "same.bin",
            "text/plain",
            9,
            BackupObjectDatabaseReference::RuntimeDebugArtifact {
                artifact_id: Uuid::now_v7(),
            },
        ),
    ];
    assert_eq!(
        BusinessObjectBackupInventory::try_from_records(records),
        Err(BackupObjectExportError::ConflictingReferences)
    );
}

#[tokio::test]
async fn missing_db_referenced_object_is_reported_by_the_backup_source() {
    let storage_id = Uuid::now_v7();
    let root = std::env::temp_dir().join(format!("missing-object-fixture-{}", Uuid::now_v7()));
    let mut missing = record(
        storage_id,
        "missing.bin",
        "application/octet-stream",
        1,
        BackupObjectDatabaseReference::RuntimeDebugArtifact {
            artifact_id: Uuid::now_v7(),
        },
    );
    missing.storage_config = serde_json::json!({ "root_path": root.display().to_string() });
    let exporter = BusinessObjectBackupExporter::new(
        FixtureRepository {
            records: vec![missing],
        },
        Arc::new(
            storage_object::FileStorageDriverRegistry::default()
                .register(Arc::new(storage_object::LocalFileStorageDriver)),
        ),
    );
    let source = exporter.sources().await.unwrap().pop().unwrap();
    let result = source.write_to(Box::pin(tokio::io::sink())).await;
    assert!(matches!(
        result,
        Err(crate::system_backup::BackupSourceError::Unavailable)
    ));
}
