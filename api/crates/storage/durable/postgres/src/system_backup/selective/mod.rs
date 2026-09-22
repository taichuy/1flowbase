//! Selectable row backups, deliberately separate from whole-database disaster recovery.
mod archive;
mod inventory;
mod key_index;
mod plugin_identity;
mod schema;
mod validation;

use anyhow::{ensure, Context, Result};
use archive::{Header, Record, Table};
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{BackupComponentReader, BackupComponentWriter},
    system_backup::{
        selective::*, BackupComponentDescriptor, BackupComponentSource, BackupSourceError,
    },
};
use domain::{
    ArtifactRebuildability, BackupComponentDisposition, BackupComponentId, BackupComponentKind,
    BackupComponentRestoreTarget, BackupSourceIdentity,
};
use futures_util::TryStreamExt;
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone)]
pub struct PgSelectiveBackupRepository {
    pool: PgPool,
}
impl PgSelectiveBackupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn export(
        &self,
        selection: &[SelectiveBackupSelection],
        destination: BackupComponentWriter,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("set transaction isolation level repeatable read read only")
            .execute(&mut *tx)
            .await?;
        let inv = inventory::inventory(&mut tx).await?;
        let mut names = inventory::selected(&inv, selection)?;
        let explicit_identity = names.contains("extension_installations");
        let mut identity_ids = BTreeSet::<uuid::Uuid>::new();
        let mut plugins = BTreeSet::new();
        for name in &names {
            plugins.extend(inv.plugins.get(name).into_iter().flatten().cloned());
            let references = sqlx::query_scalar::<_, String>("select a.attname::text from pg_constraint f join pg_class c on c.oid=f.conrelid join pg_namespace n on n.oid=c.relnamespace join pg_class p on p.oid=f.confrelid join pg_attribute a on a.attrelid=c.oid and a.attnum=f.conkey[1] where n.nspname=current_schema() and c.relname=$1 and p.relname='extension_installations' and f.contype='f' and cardinality(f.conkey)=1").bind(name).fetch_all(&mut *tx).await?;
            for column in references {
                let sql = format!(
                    "select distinct {} from {} where {} is not null",
                    schema::quote(&column)?,
                    schema::quote(name)?,
                    schema::quote(&column)?
                );
                identity_ids.extend(
                    sqlx::query_scalar::<_, uuid::Uuid>(&sql)
                        .fetch_all(&mut *tx)
                        .await?,
                );
            }
        }
        // Only identity rows referenced by selected config/data or selected plugin-owned tables.
        if !plugins.is_empty() {
            identity_ids.extend(
                sqlx::query_scalar::<_, uuid::Uuid>(
                    "select id from extension_installations where plugin_id=any($1)",
                )
                .bind(plugins.iter().cloned().collect::<Vec<_>>())
                .fetch_all(&mut *tx)
                .await?,
            );
        }
        if explicit_identity {
            identity_ids.extend(
                sqlx::query_scalar::<_, uuid::Uuid>("select id from extension_installations")
                    .fetch_all(&mut *tx)
                    .await?,
            );
        }
        if !identity_ids.is_empty() {
            names.insert("extension_installations".into());
            plugins.extend(sqlx::query_scalar::<_,String>("select plugin_id from extension_installations where id=any($1) and plugin_id is not null").bind(identity_ids.iter().copied().collect::<Vec<_>>()).fetch_all(&mut *tx).await?);
        }
        let schema_dependencies = schema::dependencies(&mut tx, &names).await?;
        let mut tables = Vec::new();
        for name in names {
            let (feature_id, data) = inv
                .owners
                .get(&name)
                .context("missing table owner")?
                .clone();
            let primary_key = schema::primary_key(&mut tx, &name).await?;
            ensure!(!primary_key.is_empty(), "table {name} has no primary key");
            tables.push(Table {
                columns: {
                    let columns = schema::columns(&mut tx, &name).await?;
                    if name == "extension_installations" {
                        plugin_identity::columns(columns)
                    } else {
                        columns
                    }
                },
                primary_key,
                feature_id,
                data,
                prerequisite: name == "extension_installations" && !explicit_identity,
                plugin_owners: inv.plugins.get(&name).cloned().unwrap_or_default(),
                prerequisites: schema::prerequisites(&mut tx, &name).await?,
                name,
            });
        }
        let header = Header {
            format: archive::FORMAT.into(),
            selection: selection.to_vec(),
            tables,
            plugins: plugins.into_iter().collect(),
            schema_dependencies,
        };
        let mut encoder = archive::Encoder::new(destination);
        encoder.write(&header).await?;
        for table in &header.tables {
            let name = schema::quote(&table.name)?;
            let order = table
                .primary_key
                .iter()
                .map(|c| schema::quote(c))
                .collect::<Result<Vec<_>>>()?
                .join(",");
            let filter = if table.prerequisite {
                " where id=any($1)"
            } else {
                ""
            };
            let sql = format!("select to_jsonb(t) as row from {name} t{filter} order by {order}");
            let mut query = sqlx::query(&sql);
            if table.prerequisite {
                query = query.bind(identity_ids.iter().copied().collect::<Vec<_>>());
            }
            let mut stream = query.fetch(&mut *tx);
            while let Some(row) = stream.try_next().await? {
                encoder
                    .write(&Record {
                        table: table.name.clone(),
                        row: {
                            let value = row.try_get("row")?;
                            if table.name == "extension_installations" {
                                plugin_identity::row(value)?
                            } else {
                                value
                            }
                        },
                    })
                    .await?;
            }
        }
        encoder.finish().await?;
        tx.commit().await?;
        Ok(())
    }
}
struct Source {
    repository: PgSelectiveBackupRepository,
    selection: Vec<SelectiveBackupSelection>,
}
#[async_trait]
impl BackupComponentSource for Source {
    fn descriptor(&self) -> BackupComponentDescriptor {
        BackupComponentDescriptor {
            component_id: BackupComponentId::try_from("postgresql").expect("static id"),
            kind: BackupComponentKind::PostgreSql,
            source_identity: BackupSourceIdentity::try_from("postgresql/settings-rows")
                .expect("static source"),
            content_type: SELECTIVE_BACKUP_CONTENT_TYPE.into(),
            disposition: BackupComponentDisposition::Embedded,
            rebuildability: ArtifactRebuildability::NotApplicable,
            restore_target: BackupComponentRestoreTarget::PostgreSql,
        }
    }
    async fn write_to(
        &self,
        destination: BackupComponentWriter,
    ) -> std::result::Result<(), BackupSourceError> {
        self.repository
            .export(&self.selection, destination)
            .await
            .map_err(|_| BackupSourceError::Unavailable)
    }
}
struct Prepared {
    transaction: Transaction<'static, Postgres>,
    preview: SelectiveBackupPreview,
    storages: Vec<SelectiveObjectStorage>,
}
#[async_trait]
impl PreparedSelectiveRestore for Prepared {
    fn preview(&self) -> &SelectiveBackupPreview {
        &self.preview
    }
    fn object_storages(&self) -> &[SelectiveObjectStorage] {
        &self.storages
    }
    async fn commit(self: Box<Self>) -> Result<SelectiveBackupPreview> {
        self.transaction.commit().await?;
        Ok(self.preview)
    }
    async fn rollback(self: Box<Self>) -> Result<()> {
        self.transaction.rollback().await?;
        Ok(())
    }
}
#[async_trait]
impl SelectiveBackupRepository for PgSelectiveBackupRepository {
    async fn catalog(&self) -> Result<Vec<SelectiveBackupCategory>> {
        let mut connection = self.pool.acquire().await?;
        Ok(inventory::inventory(&mut connection).await?.categories)
    }
    async fn source(
        &self,
        selection: Vec<SelectiveBackupSelection>,
    ) -> Result<Arc<dyn BackupComponentSource>> {
        let mut connection = self.pool.acquire().await?;
        let tables =
            inventory::selected(&inventory::inventory(&mut connection).await?, &selection)?;
        let _ = schema::dependencies(&mut connection, &tables).await?;
        for table in tables {
            ensure!(
                !schema::primary_key(&mut connection, &table)
                    .await?
                    .is_empty(),
                "table {table} has no primary key"
            );
        }
        Ok(Arc::new(Source {
            repository: self.clone(),
            selection,
        }))
    }
    async fn object_scope(
        &self,
        selection: &[SelectiveBackupSelection],
    ) -> Result<SelectiveBackupObjectScope> {
        let mut connection = self.pool.acquire().await?;
        let tables = inventory::selected(&inventory::inventory(&mut connection).await?, selection)?;
        let file_table_ids=sqlx::query_scalar("select f.id from file_tables f join model_definitions d on d.id=f.model_definition_id where d.physical_table_name=any($1) order by f.id").bind(tables.iter().cloned().collect::<Vec<_>>()).fetch_all(&mut *connection).await?;
        Ok(SelectiveBackupObjectScope {
            file_table_ids,
            include_runtime_debug_artifacts: tables.contains("runtime_debug_artifacts"),
        })
    }
    async fn preflight(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
    ) -> Result<SelectiveBackupPreview> {
        let scratch = archive::unpack(reader).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("set transaction isolation level repeatable read read only")
            .execute(&mut *tx)
            .await?;
        let (_, preview) =
            validation::validate(&mut tx, &scratch, source_master_key, target_master_key).await?;
        tx.rollback().await?;
        Ok(preview)
    }
    async fn prepare_restore(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
        confirm_missing_plugins: bool,
    ) -> Result<Box<dyn PreparedSelectiveRestore>> {
        let scratch = archive::unpack(reader).await?;
        let mut tx = self.pool.begin().await?;
        // Serializable protects the validation/write boundary against concurrent target changes.
        sqlx::query("set transaction isolation level serializable")
            .execute(&mut *tx)
            .await?;
        let (header, preview) =
            validation::validate(&mut tx, &scratch, source_master_key, target_master_key).await?;
        ensure!(
            preview.failures.is_empty(),
            "settings backup preflight failed: {}",
            preview.failures.join("; ")
        );
        ensure!(
            confirm_missing_plugins || preview.missing_plugins.is_empty(),
            "missing plugins require confirmation: {}",
            preview.missing_plugins.join(", ")
        );
        sqlx::query("set constraints all deferred")
            .execute(&mut *tx)
            .await?;
        let tables: BTreeMap<_, _> = header.tables.iter().map(|t| (t.name.as_str(), t)).collect();
        let statements = tables
            .iter()
            .map(|(name, t)| Ok((*name, schema::upsert_sql(t)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        // FK dependencies can include self references and cycles. Retry only FK failures using
        // savepoints and bounded disk spools; no constraint or trigger is disabled.
        let mut pending = scratch;
        let mut passes = 0u32;
        loop {
            passes += 1;
            ensure!(
                passes <= 256,
                "settings backup foreign key dependency depth exceeds 256 passes"
            );
            let (next, mut next_file) = archive::Scratch::new()?;
            use tokio::io::AsyncWriteExt;
            let mut reader = pending.reader().await?;
            let _ = archive::header(&mut reader).await?;
            let mut header_bytes = serde_json::to_vec(&header)?;
            header_bytes.push(b'\n');
            next_file.write_all(&header_bytes).await?;
            let mut progressed = 0u64;
            let mut deferred = 0u64;
            while let Some(bytes) = archive::line(&mut reader).await? {
                let mut record: Record = serde_json::from_slice(&bytes)?;
                validation::rekey(
                    &record.table,
                    &mut record.row,
                    source_master_key,
                    target_master_key,
                )?;
                sqlx::query("savepoint selective_row")
                    .execute(&mut *tx)
                    .await?;
                match sqlx::query(
                    statements
                        .get(record.table.as_str())
                        .context("unknown archive table")?,
                )
                .bind(&record.row)
                .execute(&mut *tx)
                .await
                {
                    Ok(_) => {
                        sqlx::query("release savepoint selective_row")
                            .execute(&mut *tx)
                            .await?;
                        progressed += 1;
                    }
                    Err(error)
                        if error.as_database_error().and_then(|e| e.code()).as_deref()
                            == Some("23503") =>
                    {
                        sqlx::query("rollback to savepoint selective_row")
                            .execute(&mut *tx)
                            .await?;
                        sqlx::query("release savepoint selective_row")
                            .execute(&mut *tx)
                            .await?;
                        next_file.write_all(&bytes).await?;
                        deferred += 1;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            next_file.flush().await?;
            if deferred == 0 {
                break;
            }
            ensure!(
                progressed > 0,
                "settings backup has unresolved foreign key dependencies ({deferred} rows)"
            );
            pending = next;
        }
        // Force deferred constraints while rollback remains possible for both database and files.
        sqlx::query("set constraints all immediate")
            .execute(&mut *tx)
            .await?;
        let storages =
            sqlx::query("select id,driver_type,config_json from file_storages order by id")
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(|r| {
                    Ok(SelectiveObjectStorage {
                        storage_id: r.try_get("id")?,
                        driver_type: r.try_get("driver_type")?,
                        config_json: r.try_get("config_json")?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        Ok(Box::new(Prepared {
            transaction: tx,
            preview,
            storages,
        }))
    }
    async fn restore(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
        confirm_missing_plugins: bool,
    ) -> Result<SelectiveBackupPreview> {
        self.prepare_restore(
            reader,
            source_master_key,
            target_master_key,
            confirm_missing_plugins,
        )
        .await?
        .commit()
        .await
    }
}

#[cfg(test)]
mod _tests;
