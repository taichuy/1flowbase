use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    BackupObjectDatabaseReference, BackupObjectInventoryRecord, BackupObjectInventoryRepository,
};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

#[derive(Clone)]
struct StoredObjectStorage {
    driver_type: String,
    config_json: serde_json::Value,
}

struct RegisteredFileTable {
    id: Uuid,
    physical_table_name: String,
}

#[async_trait]
impl BackupObjectInventoryRepository for PgControlPlaneStore {
    async fn list_backup_object_inventory(&self) -> Result<Vec<BackupObjectInventoryRecord>> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("set transaction isolation level repeatable read read only")
            .execute(&mut *transaction)
            .await?;
        let storages = load_object_storages(&mut transaction).await?;
        let file_tables = load_registered_file_tables(&mut transaction).await?;
        let mut records = Vec::new();
        for file_table in file_tables {
            records
                .extend(load_file_table_records(&mut transaction, &storages, &file_table).await?);
        }
        records.extend(load_runtime_debug_artifacts(&mut transaction, &storages).await?);
        transaction.commit().await?;
        records.sort_by(|left, right| {
            left.storage_id
                .cmp(&right.storage_id)
                .then_with(|| left.object_path.cmp(&right.object_path))
                .then_with(|| left.reference.cmp(&right.reference))
        });
        Ok(records)
    }
}

async fn load_object_storages(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<BTreeMap<Uuid, StoredObjectStorage>> {
    let rows = sqlx::query(
        r#"
        select id, driver_type, config_json
        from file_storages
        order by id
        "#,
    )
    .fetch_all(&mut **transaction)
    .await?;
    let mut storages = BTreeMap::new();
    for row in rows {
        let id: Uuid = row.try_get("id")?;
        let storage = StoredObjectStorage {
            driver_type: required_text(row.try_get("driver_type")?)?,
            config_json: row.try_get("config_json")?,
        };
        if storages.insert(id, storage).is_some() {
            bail!("duplicate file storage in backup inventory");
        }
    }
    Ok(storages)
}

async fn load_registered_file_tables(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Vec<RegisteredFileTable>> {
    let rows = sqlx::query(
        r#"
        select
            tables.id,
            definitions.physical_table_name,
            definitions.source_kind,
            definitions.data_source_instance_id
        from file_tables tables
        join model_definitions definitions on definitions.id = tables.model_definition_id
        order by tables.id
        "#,
    )
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter()
        .map(|row| {
            let source_kind: String = row.try_get("source_kind")?;
            let data_source_instance_id: Option<Uuid> = row.try_get("data_source_instance_id")?;
            if source_kind != "main_source" || data_source_instance_id.is_some() {
                bail!("file table backup inventory must use the durable main source");
            }
            let physical_table_name: String = row.try_get("physical_table_name")?;
            validate_identifier(&physical_table_name)?;
            Ok(RegisteredFileTable {
                id: row.try_get("id")?,
                physical_table_name,
            })
        })
        .collect()
}

async fn load_file_table_records(
    transaction: &mut Transaction<'_, Postgres>,
    storages: &BTreeMap<Uuid, StoredObjectStorage>,
    file_table: &RegisteredFileTable,
) -> Result<Vec<BackupObjectInventoryRecord>> {
    let statement = format!(
        r#"
        select
            id::text as record_id,
            storage_id::text as storage_id,
            path::text as object_path,
            mimetype::text as content_type,
            size::text as size_bytes
        from "{}"
        order by storage_id::text, path::text, id::text
        "#,
        file_table.physical_table_name
    );
    let rows = sqlx::query(&statement)
        .fetch_all(&mut **transaction)
        .await
        .with_context(|| {
            format!(
                "failed to enumerate registered file table {}",
                file_table.id
            )
        })?;
    rows.into_iter()
        .map(|row| {
            let record_id = parse_uuid(required_text(row.try_get("record_id")?)?)?;
            let storage_id = parse_uuid(required_text(row.try_get("storage_id")?)?)?;
            let storage = storages
                .get(&storage_id)
                .context("file record references a missing storage")?;
            Ok(BackupObjectInventoryRecord {
                reference: BackupObjectDatabaseReference::FileRecord {
                    file_table_id: file_table.id,
                    record_id,
                },
                storage_id,
                driver_type: storage.driver_type.clone(),
                storage_config: storage.config_json.clone(),
                object_path: required_text(row.try_get("object_path")?)?,
                content_type: required_text(row.try_get("content_type")?)?,
                size_bytes: parse_size(required_text(row.try_get("size_bytes")?)?)?,
            })
        })
        .collect()
}

async fn load_runtime_debug_artifacts(
    transaction: &mut Transaction<'_, Postgres>,
    storages: &BTreeMap<Uuid, StoredObjectStorage>,
) -> Result<Vec<BackupObjectInventoryRecord>> {
    let rows = sqlx::query(
        r#"
        select
            id,
            storage_id,
            storage_ref,
            content_type,
            original_size_bytes
        from runtime_debug_artifacts
        where retention_state in ('active', 'pending_delete')
        order by storage_id, storage_ref, id
        "#,
    )
    .fetch_all(&mut **transaction)
    .await?;
    rows.into_iter()
        .map(|row| {
            let storage_id: Uuid = row.try_get("storage_id")?;
            let storage = storages
                .get(&storage_id)
                .context("runtime debug artifact references a missing storage")?;
            let original_size_bytes: i64 = row.try_get("original_size_bytes")?;
            Ok(BackupObjectInventoryRecord {
                reference: BackupObjectDatabaseReference::RuntimeDebugArtifact {
                    artifact_id: row.try_get("id")?,
                },
                storage_id,
                driver_type: storage.driver_type.clone(),
                storage_config: storage.config_json.clone(),
                object_path: required_text(row.try_get("storage_ref")?)?,
                content_type: required_text(row.try_get("content_type")?)?,
                size_bytes: u64::try_from(original_size_bytes)
                    .context("runtime debug artifact has an invalid size")?,
            })
        })
        .collect()
}

fn validate_identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 63
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("invalid registered file table name");
    }
    Ok(())
}

fn required_text(value: Option<String>) -> Result<String> {
    value
        .filter(|value| !value.is_empty() && value.trim() == value)
        .context("backup object inventory field is missing or invalid")
}

fn parse_uuid(value: String) -> Result<Uuid> {
    Uuid::parse_str(&value).context("backup object inventory UUID is invalid")
}

fn parse_size(value: String) -> Result<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value.as_str(), ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte == b'0')
    {
        bail!("backup object inventory size is invalid");
    }
    whole
        .parse::<u64>()
        .context("backup object inventory size is out of range")
}
