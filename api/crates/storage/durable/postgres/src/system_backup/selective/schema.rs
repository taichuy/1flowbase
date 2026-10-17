use super::archive::{Column, Table};
use anyhow::{Result, ensure};
use sqlx::{PgConnection, Row};
pub(super) fn quote(name: &str) -> Result<String> {
    ensure!(
        !name.is_empty() && name.len() <= 63 && !name.contains('\0'),
        "invalid PostgreSQL identifier"
    );
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}
pub(super) async fn columns(connection: &mut PgConnection, table: &str) -> Result<Vec<Column>> {
    let rows=sqlx::query("select a.attname, format_type(a.atttypid,a.atttypmod) as sql_type, not a.attnotnull as nullable, a.attgenerated <> '' as generated, case when a.attgenerated <> '' then pg_get_expr(d.adbin,d.adrelid) else null end as generated_expression from pg_attribute a left join pg_attrdef d on d.adrelid=a.attrelid and d.adnum=a.attnum join pg_class c on c.oid=a.attrelid join pg_namespace n on n.oid=c.relnamespace where n.nspname=current_schema() and c.relname=$1 and c.relkind in ('r','p') and a.attnum>0 and not a.attisdropped order by a.attnum").bind(table).fetch_all(connection).await?;
    rows.into_iter()
        .map(|r| {
            Ok(Column {
                name: r.try_get("attname")?,
                sql_type: r.try_get("sql_type")?,
                nullable: r.try_get("nullable")?,
                generated: r.try_get("generated")?,
                generated_expression: r.try_get("generated_expression")?,
            })
        })
        .collect()
}
pub(super) async fn primary_key(connection: &mut PgConnection, table: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("select a.attname::text from pg_index i join pg_class c on c.oid=i.indrelid join pg_namespace n on n.oid=c.relnamespace cross join lateral unnest(i.indkey) with ordinality as k(attnum,ord) join pg_attribute a on a.attrelid=c.oid and a.attnum=k.attnum where n.nspname=current_schema() and c.relname=$1 and i.indisprimary order by k.ord").bind(table).fetch_all(connection).await?)
}
pub(super) async fn prerequisites(
    connection: &mut PgConnection,
    table: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar("select distinct parent.relname::text from pg_constraint f join pg_class child on child.oid=f.conrelid join pg_class parent on parent.oid=f.confrelid join pg_namespace n on n.oid=child.relnamespace where n.nspname=current_schema() and child.relname=$1 and f.contype='f' order by 1").bind(table).fetch_all(connection).await?)
}
pub(super) fn upsert_sql(table: &Table) -> Result<String> {
    if table.name == "legacy_event_recovery_ledger" {
        // Keep existing entry identities, and let explicit-take state dominate any
        // older pending snapshot. State merges high-water without replacing a live
        // target cutover; a pristine target adopts the original state provenance.
        return Ok("insert into legacy_event_recovery_ledger as target select r.* from jsonb_populate_record(null::legacy_event_recovery_ledger,$1) r where true on conflict(sequence) do update set last_sequence=case when target.sequence=0 then greatest(target.last_sequence,excluded.last_sequence) else null end,cutover_id=case when target.sequence=0 and target.last_sequence=0 and not exists(select 1 from legacy_event_recovery_ledger where sequence>0) then excluded.cutover_id else target.cutover_id end,created_at=case when target.sequence=0 and target.last_sequence=0 and not exists(select 1 from legacy_event_recovery_ledger where sequence>0) then excluded.created_at else target.created_at end,disposition=case when target.disposition='taken' or excluded.disposition='taken' then 'taken' else target.disposition end,payload=case when target.disposition='taken' or excluded.disposition='taken' then null else target.payload end".into());
    }
    if table.name == "extension_installations" {
        return super::plugin_identity::insert_sql();
    }
    let names = table
        .columns
        .iter()
        .filter(|c| !c.generated)
        .map(|c| quote(&c.name))
        .collect::<Result<Vec<_>>>()?;
    let pk = table
        .primary_key
        .iter()
        .map(|s| quote(s))
        .collect::<Result<Vec<_>>>()?;
    ensure!(!pk.is_empty(), "table {} has no primary key", table.name);
    let writable = table
        .columns
        .iter()
        .filter(|c| !c.generated && !table.primary_key.contains(&c.name))
        .filter(|c| {
            table.name != "lifecycle_outbox"
                || !matches!(
                    c.name.as_str(),
                    "transaction_id"
                        | "contract_id"
                        | "contract_version"
                        | "occurred_at"
                        | "created_at"
                        | "native_publication_version"
                        | "graph_fingerprint"
                )
        })
        .map(|c| quote(&c.name))
        .collect::<Result<Vec<_>>>()?;
    let action = if writable.is_empty() {
        "do nothing".to_owned()
    } else {
        format!(
            "do update set {} where ({}) is distinct from ({})",
            writable
                .iter()
                .map(|c| {
                    if table.name == "lifecycle_outbox" && c == "\"canonical_payload\"" {
                        format!("{c}=case when octet_length(excluded.{c})=0 then excluded.{c} else target.{c} end")
                    } else if table.name == "lifecycle_outbox" && matches!(c.as_str(), "\"legacy_handoff_sequence\"" | "\"notification_payload_digest\"") {
                        format!("{c}=coalesce(target.{c},excluded.{c})")
                    } else {
                        format!("{c}=excluded.{c}")
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
            writable
                .iter()
                .map(|c| format!("target.{c}"))
                .collect::<Vec<_>>()
                .join(","),
            writable
                .iter()
                .map(|c| format!("excluded.{c}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let action = if table.name == "lifecycle_outbox" {
        format!(
            "{action} and not (target.status='delivered' and octet_length(target.canonical_payload)=0 and target.notification_payload_digest is not null and target.contract_version='v1' and target.contract_id in ('credit.integration_notification','model_definition.integration_notification'))"
        )
    } else if table.name == "lifecycle_outbox_deliveries" {
        format!(
            "{action} and not (target.status='delivered' and exists(select 1 from lifecycle_outbox p where p.event_id=target.event_id and p.status='delivered' and octet_length(p.canonical_payload)=0 and p.notification_payload_digest is not null and p.contract_version='v1' and p.contract_id in ('credit.integration_notification','model_definition.integration_notification')))"
        )
    } else {
        action
    };
    let name = quote(&table.name)?;
    Ok(format!(
        "insert into {name} as target ({}) overriding system value select {} from jsonb_populate_record(null::{name},$1) where true on conflict ({}) {action}",
        names.join(","),
        names.join(","),
        pk.join(",")
    ))
}

/// Definitions are rows, but their referenced physical schema must already exist on the target.
pub(super) async fn dependencies(
    connection: &mut PgConnection,
    selected: &std::collections::BTreeSet<String>,
) -> Result<Vec<super::archive::SchemaDependency>> {
    if !selected.contains("model_definitions") && !selected.contains("file_tables") {
        return Ok(Vec::new());
    }
    let file_only = !selected.contains("model_definitions");
    let names = sqlx::query_scalar::<_,String>("select distinct d.physical_table_name from model_definitions d where d.source_kind='main_source' and d.data_source_instance_id is null and d.physical_table_name is not null and (not $1 or exists(select 1 from file_tables f where f.model_definition_id=d.id)) order by 1")
        .bind(file_only).fetch_all(&mut *connection).await?;
    let mut dependencies = Vec::new();
    for name in names {
        if selected.contains(&name) {
            continue;
        }
        let columns = columns(connection, &name).await?;
        ensure!(
            !columns.is_empty(),
            "model definition references missing physical table: {name}"
        );
        dependencies.push(super::archive::SchemaDependency {
            primary_key: primary_key(connection, &name).await?,
            name,
            columns,
        });
    }
    Ok(dependencies)
}
