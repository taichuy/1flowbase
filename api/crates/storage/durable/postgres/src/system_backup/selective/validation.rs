use super::{
    archive::{self, Header, Record, Scratch, Table},
    inventory, schema,
};
use crate::secret_crypto::{
    decrypt_secret_json, decrypt_secret_json_with_aad, encrypt_secret_json,
    encrypt_secret_json_with_aad,
};
use anyhow::{ensure, Context, Result};
use control_plane_contracts::system_backup::selective::SelectiveBackupPreview;
use serde_json::Value;
use sqlx::{PgConnection, Row};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn rekey(table: &str, row: &mut Value, source: &str, target: &str) -> Result<()> {
    if matches!(
        table,
        "model_provider_instance_secrets"
            | "data_source_secrets"
            | "network_egress_provider_secrets"
            | "mcp_upstream_connection_secrets"
            | "mcp_client_credentials"
    ) {
        let value = row
            .get("encrypted_secret_json")
            .context("secret row missing ciphertext")?;
        let plain = decrypt_secret_json(value, source)
            .context("source secret key cannot decrypt selected row")?;
        if source != target {
            row["encrypted_secret_json"] = encrypt_secret_json(&plain, target)?;
        }
    }
    if table == "provider_protocol_capsules" {
        let field = |name| {
            row.get(name)
                .and_then(Value::as_str)
                .context("invalid capsule identity")
        };
        let aad = format!(
            "provider_protocol_capsule:v1\0{}\0{}\0{}",
            field("flow_run_id")?,
            field("capsule_kind")?,
            field("slot_key")?
        );
        let plain = decrypt_secret_json_with_aad(
            row.get("encrypted_payload")
                .context("missing capsule payload")?,
            source,
            aad.as_bytes(),
        )?;
        if source != target {
            row["encrypted_payload"] =
                encrypt_secret_json_with_aad(&plain, target, aad.as_bytes())?;
        }
    }
    Ok(())
}
struct ForeignKey {
    parent: String,
    columns: Vec<String>,
    parent_columns: Vec<String>,
}
async fn foreign_keys(connection: &mut PgConnection, table: &str) -> Result<Vec<ForeignKey>> {
    let rows=sqlx::query("select p.relname::text as parent, array(select a.attname::text from unnest(f.conkey) with ordinality k(num,ord) join pg_attribute a on a.attrelid=f.conrelid and a.attnum=k.num order by ord) as columns, array(select a.attname::text from unnest(f.confkey) with ordinality k(num,ord) join pg_attribute a on a.attrelid=f.confrelid and a.attnum=k.num order by ord) as parent_columns from pg_constraint f join pg_class c on c.oid=f.conrelid join pg_class p on p.oid=f.confrelid join pg_namespace n on n.oid=c.relnamespace where n.nspname=current_schema() and c.relname=$1 and f.contype='f'").bind(table).fetch_all(connection).await?;
    rows.into_iter()
        .map(|r| {
            Ok(ForeignKey {
                parent: r.try_get("parent")?,
                columns: r.try_get("columns")?,
                parent_columns: r.try_get("parent_columns")?,
            })
        })
        .collect()
}
async fn missing_parent(
    connection: &mut PgConnection,
    keys: &mut super::key_index::KeyIndex,
    table: &Table,
    row: &Value,
    fk: &ForeignKey,
    selected: &BTreeSet<String>,
) -> Result<bool> {
    if fk
        .columns
        .iter()
        .any(|c| row.get(c).is_none_or(Value::is_null))
    {
        return Ok(false);
    }
    let conditions = fk
        .columns
        .iter()
        .zip(&fk.parent_columns)
        .map(|(child, parent)| {
            Ok(format!(
                "p.{}=s.{}",
                schema::quote(parent)?,
                schema::quote(child)?
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(" and ");
    let sql=format!("select exists(select 1 from {} p cross join jsonb_populate_record(null::{},$1) s where {conditions})",schema::quote(&fk.parent)?,schema::quote(&table.name)?);
    if sqlx::query_scalar::<_, bool>(&sql)
        .bind(row)
        .fetch_one(connection)
        .await?
    {
        return Ok(false);
    }
    Ok(!selected.contains(&fk.parent)
        || !keys.contains(&fk.parent, &fk.parent_columns, &fk.columns, row)?)
}
async fn unique_queries(
    connection: &mut PgConnection,
    table: &Table,
) -> Result<Vec<(String, String)>> {
    let rows=sqlx::query("select i.indexrelid::bigint as oid, ci.relname::text as name, i.indnkeyatts::int as count, pg_get_expr(i.indpred,i.indrelid) as predicate from pg_index i join pg_class c on c.oid=i.indrelid join pg_namespace n on n.oid=c.relnamespace join pg_class ci on ci.oid=i.indexrelid where n.nspname=current_schema() and c.relname=$1 and i.indisunique and not i.indisprimary and i.indisvalid").bind(&table.name).fetch_all(&mut *connection).await?;
    let mut result = Vec::new();
    for index in rows {
        let oid: i64 = index.try_get("oid")?;
        let count: i32 = index.try_get("count")?;
        let expressions = sqlx::query_scalar::<_, String>(
            "select pg_get_indexdef($1::bigint::oid,k,true) from generate_series(1,$2) k",
        )
        .bind(oid)
        .bind(count)
        .fetch_all(&mut *connection)
        .await?;
        let projection = expressions
            .iter()
            .enumerate()
            .map(|(i, e)| format!("{e} as key{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let equality = (0..count)
            .map(|i| format!("old.key{i}=new.key{i}"))
            .collect::<Vec<_>>()
            .join(" and ");
        let same_pk = table
            .primary_key
            .iter()
            .map(|p| {
                Ok(format!(
                    "old.record -> '{}' = $1 -> '{}'",
                    p.replace('\'', "''"),
                    p.replace('\'', "''")
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .join(" and ");
        let predicate = index
            .try_get::<Option<String>, _>("predicate")?
            .unwrap_or_else(|| "true".into());
        let name = schema::quote(&table.name)?;
        let sql=format!("select exists(select 1 from (select to_jsonb(t) as record,{projection} from {name} t where {predicate}) old cross join (select {projection} from jsonb_populate_record(null::{name},$1) t where {predicate}) new where {equality} and not ({same_pk}))");
        result.push((index.try_get("name")?, sql));
    }
    Ok(result)
}
fn failure(preview: &mut SelectiveBackupPreview, message: String) {
    if preview.failures.len() < 100 && !preview.failures.contains(&message) {
        preview.failures.push(message);
    }
}

pub(super) async fn validate(
    connection: &mut PgConnection,
    scratch: &Scratch,
    source: &str,
    target: &str,
) -> Result<(Header, SelectiveBackupPreview)> {
    let mut reader = scratch.reader().await?;
    let header = archive::header(&mut reader).await?;
    let mut preview = SelectiveBackupPreview::default();
    let mut names = BTreeSet::new();
    let mut valid = BTreeMap::new();
    let selections: BTreeMap<_, _> = header
        .selection
        .iter()
        .map(|s| (s.feature_id.as_str(), s))
        .collect();
    ensure!(
        selections.len() == header.selection.len(),
        "duplicate archive feature selection"
    );
    let mut dependency_names = BTreeSet::new();
    for dependency in &header.schema_dependencies {
        ensure!(
            dependency_names.insert(&dependency.name),
            "duplicate schema dependency"
        );
        let columns = schema::columns(connection, &dependency.name).await?;
        if columns.is_empty() {
            failure(
                &mut preview,
                format!("missing physical table dependency: {}", dependency.name),
            );
            continue;
        }
        if columns != dependency.columns {
            failure(
                &mut preview,
                format!("incompatible physical fields: {}", dependency.name),
            );
        }
        if schema::primary_key(connection, &dependency.name).await? != dependency.primary_key {
            failure(
                &mut preview,
                format!("incompatible physical primary key: {}", dependency.name),
            );
        }
    }
    for table in &header.tables {
        ensure!(names.insert(table.name.clone()), "duplicate archive table");
        ensure!(
            !inventory::excluded(&table.name),
            "backup contains excluded installation table"
        );
        if let Some((owner, data)) = inventory::static_owner(&table.name) {
            ensure!(
                owner == table.feature_id && data == table.data,
                "invalid archive table owner"
            );
        } else {
            ensure!(
                table.data
                    && (matches!(
                        table.feature_id.as_str(),
                        "system.data-models" | "system.files"
                    ) || (table.feature_id == "system.extension-center"
                        && !table.plugin_owners.is_empty())),
                "untrusted archive table owner"
            );
        }
        if table.prerequisite {
            ensure!(
                table.name == "extension_installations",
                "invalid implicit prerequisite table"
            );
        } else {
            let selection = selections
                .get(table.feature_id.as_str())
                .context("table is outside archive selection")?;
            ensure!(
                if table.data {
                    selection.data
                } else {
                    selection.structure
                },
                "table is outside selected row kind"
            );
        }
        let columns = schema::columns(connection, &table.name).await?;
        let columns = if table.name == "extension_installations" {
            super::plugin_identity::columns(columns)
        } else {
            columns
        };
        if columns.is_empty() {
            failure(&mut preview, format!("missing table: {}", table.name));
            continue;
        }
        if columns != table.columns {
            failure(&mut preview, format!("incompatible fields: {}", table.name));
            continue;
        }
        let pk = schema::primary_key(connection, &table.name).await?;
        if pk.is_empty() || pk != table.primary_key {
            failure(
                &mut preview,
                format!("incompatible primary key: {}", table.name),
            );
            continue;
        }
        let fks = foreign_keys(connection, &table.name).await?;
        let unique = unique_queries(connection, table).await?;
        valid.insert(table.name.as_str(), (table, fks, unique));
    }
    preview.table_count = header.tables.len() as u64;
    preview.selected_tables = names.iter().cloned().collect();
    let installed:BTreeSet<String>=sqlx::query_scalar("select plugin_id from extension_installations where plugin_id is not null and id in (select installation_id from extension_artifact_instances where artifact_status='ready')").fetch_all(&mut *connection).await?.into_iter().collect();
    preview.missing_plugins = header
        .plugins
        .iter()
        .filter(|p| !installed.contains(*p))
        .cloned()
        .collect();
    let mut parent_keys: BTreeMap<String, BTreeSet<Vec<String>>> = BTreeMap::new();
    for (_, fks, _) in valid.values() {
        for fk in fks {
            if names.contains(&fk.parent) {
                parent_keys
                    .entry(fk.parent.clone())
                    .or_default()
                    .insert(fk.parent_columns.clone());
            }
        }
    }
    let mut keys = super::key_index::KeyIndex::new()?;
    let mut index_reader = scratch.reader().await?;
    let _ = archive::header(&mut index_reader).await?;
    while let Some(bytes) = archive::line(&mut index_reader).await? {
        let record: Record = serde_json::from_slice(&bytes)?;
        if let Some(column_sets) = parent_keys.get(&record.table) {
            for columns in column_sets {
                keys.insert(&record.table, columns, &record.row)?;
            }
        }
    }
    keys.finish()?;
    while let Some(bytes) = archive::line(&mut reader).await? {
        let mut record: Record = serde_json::from_slice(&bytes)?;
        ensure!(
            names.contains(&record.table),
            "row references unknown archive table"
        );
        preview.row_count += 1;
        if record.table == "model_definitions"
            && record.row.get("source_kind").and_then(Value::as_str) == Some("main_source")
            && record
                .row
                .get("data_source_instance_id")
                .is_none_or(Value::is_null)
        {
            if let Some(physical) = record
                .row
                .get("physical_table_name")
                .and_then(Value::as_str)
            {
                if !names.contains(physical)
                    && !dependency_names
                        .iter()
                        .any(|name| name.as_str() == physical)
                {
                    failure(
                        &mut preview,
                        format!("missing physical schema description: {physical}"),
                    );
                }
            } else {
                failure(
                    &mut preview,
                    "model definition has no physical table name".into(),
                );
            }
        }

        let Some((table, fks, unique)) = valid.get(record.table.as_str()) else {
            continue;
        };
        let object = record
            .row
            .as_object()
            .context("archive row is not an object")?;
        if object.len() != table.columns.len()
            || table.columns.iter().any(|c| !object.contains_key(&c.name))
        {
            failure(&mut preview, format!("row field mismatch: {}", table.name));
            continue;
        }
        if table
            .primary_key
            .iter()
            .any(|pk| object.get(pk).is_none_or(Value::is_null))
        {
            failure(
                &mut preview,
                format!("row primary key is null: {}", table.name),
            );
            continue;
        }
        if table.name == "extension_installations" {
            if let Some(existing) =
                super::plugin_identity::existing(connection, &record.row).await?
            {
                if !super::plugin_identity::compatible(&existing, &record.row) {
                    failure(
                        &mut preview,
                        "plugin logical identity conflicts with target primary key".into(),
                    );
                }
                // Existing identity is preserved, so incoming actor/policy dependencies do not apply.
                continue;
            }
        }
        if let Err(error) = rekey(&record.table, &mut record.row, source, target) {
            failure(
                &mut preview,
                format!("secret compatibility: {}: {error}", table.name),
            );
            continue;
        }
        // PostgreSQL itself checks JSON-to-domain conversion before any row mutation.
        let cast = format!(
            "select to_jsonb(r) from jsonb_populate_record(null::{},$1) r",
            schema::quote(&table.name)?
        );
        let normalized: Value = sqlx::query_scalar(&cast)
            .bind(&record.row)
            .fetch_one(&mut *connection)
            .await?;
        if table.columns.iter().any(|c| {
            !c.nullable && !c.generated && normalized.get(&c.name).is_none_or(Value::is_null)
        }) {
            failure(
                &mut preview,
                format!("required field is null: {}", table.name),
            );
        }
        for fk in fks {
            if missing_parent(connection, &mut keys, table, &record.row, fk, &names).await? {
                failure(
                    &mut preview,
                    format!(
                        "missing prerequisite: {} -> {} ({})",
                        table.name,
                        fk.parent,
                        fk.columns.join(",")
                    ),
                );
            }
        }
        for (index, sql) in unique {
            if sqlx::query_scalar::<_, bool>(sql)
                .bind(&record.row)
                .fetch_one(&mut *connection)
                .await?
            {
                failure(
                    &mut preview,
                    format!("unique identity conflict: {} ({index})", table.name),
                );
            }
        }
        if matches!(
            table.name.as_str(),
            "runtime_canonical_contents" | "flow_run_recovery_history" | "i18n_catalog_releases"
        ) {
            let condition = table
                .primary_key
                .iter()
                .map(|pk| Ok(format!("t.{}=r.{}", schema::quote(pk)?, schema::quote(pk)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" and ");
            let sql=format!("select exists(select 1 from {} t cross join jsonb_populate_record(null::{},$1) r where {condition} and to_jsonb(t) is distinct from to_jsonb(r))",schema::quote(&table.name)?,schema::quote(&table.name)?);
            if sqlx::query_scalar::<_, bool>(&sql)
                .bind(&record.row)
                .fetch_one(&mut *connection)
                .await?
            {
                failure(
                    &mut preview,
                    format!("immutable row conflict: {}", table.name),
                );
            }
        }
    }
    Ok((header, preview))
}
