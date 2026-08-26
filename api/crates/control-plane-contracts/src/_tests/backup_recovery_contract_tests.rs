use domain::{BackupSetId, KeyFingerprint, MigrationHead, RecoveryJobId};
use uuid::Uuid;

use crate::{
    BackupComponentReader, BackupComponentSource, BackupComponentWriter, BackupKeyMaterial,
    BackupObjectDatabaseReference, BackupObjectInventoryRecord, BackupObjectInventoryRepository,
    BackupRepository, BackupRepositoryError, BackupSourceError, RecoveryStepContext,
    RecoveryStepTarget, RecoveryStepTargetError,
};

#[test]
fn backup_contract_keeps_secret_and_async_stream_bounds() {
    adapter_traits_remain_canonical::<
        dyn BackupRepository,
        dyn BackupObjectInventoryRepository,
        dyn BackupComponentSource,
        dyn RecoveryStepTarget,
    >();

    let fingerprint = KeyFingerprint::try_from("a".repeat(64)).expect("valid fingerprint");
    assert!(BackupKeyMaterial::new(fingerprint.clone(), Vec::new()).is_none());

    let material = BackupKeyMaterial::new(fingerprint.clone(), vec![7, 11, 13])
        .expect("non-empty key material must be accepted");
    assert_eq!(material.fingerprint(), &fingerprint);
    assert_eq!(material.expose_bytes(), &[7, 11, 13]);

    fn assert_reader(_: BackupComponentReader) {}
    fn assert_writer(_: BackupComponentWriter) {}
    assert_reader(Box::pin(tokio::io::empty()));
    assert_writer(Box::pin(tokio::io::sink()));
}

#[test]
fn backup_inventory_debug_redacts_storage_configuration() {
    let record = BackupObjectInventoryRecord {
        reference: BackupObjectDatabaseReference::RuntimeDebugArtifact {
            artifact_id: Uuid::nil(),
        },
        storage_id: Uuid::nil(),
        driver_type: "s3".to_string(),
        storage_config: serde_json::json!({ "secret": "must-not-leak" }),
        object_path: "debug/artifact.json".to_string(),
        content_type: "application/json".to_string(),
        size_bytes: 42,
    };

    let debug = format!("{record:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("must-not-leak"));
}

#[test]
fn recovery_contract_keeps_context_and_error_semantics() {
    let context = RecoveryStepContext {
        recovery_job_id: RecoveryJobId::new(),
        backup_set_id: BackupSetId::new(),
        migration_head: MigrationHead::try_from("migration.test").expect("valid migration head"),
    };

    assert_eq!(context.clone(), context);
    assert_eq!(
        BackupRepositoryError::PathOverlap.to_string(),
        "backup repository path overlaps a protected data root"
    );
    assert_eq!(
        BackupSourceError::Changed.to_string(),
        "backup source changed while being captured"
    );
    assert_eq!(
        RecoveryStepTargetError::Compensation.to_string(),
        "recovery target compensation failed"
    );
}

fn adapter_traits_remain_canonical<Repository, Inventory, Source, Target>()
where
    Repository: BackupRepository + ?Sized,
    Inventory: BackupObjectInventoryRepository + ?Sized,
    Source: BackupComponentSource + ?Sized,
    Target: RecoveryStepTarget + ?Sized,
{
}
