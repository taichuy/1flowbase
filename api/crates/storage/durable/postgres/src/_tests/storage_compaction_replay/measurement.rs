use super::{copy::DOMAIN, qualified};
use anyhow::{ensure, Result};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
pub(super) struct TableCost {
    pub cost_domain: &'static str,
    pub rows: i64,
    pub composite_row_value_bytes: i64,
    pub heap_and_auxiliary_bytes: i64,
    pub main_index_bytes: i64,
    pub toast_and_toast_indexes_bytes: i64,
    pub gross_allocated_bytes: i64,
    pub indexes: BTreeMap<String, i64>,
}

#[derive(Serialize)]
pub(super) struct Snapshot {
    pub task_domain_gross_allocated_bytes: i64,
    pub fk_support_and_defaults_gross_allocated_bytes: i64,
    pub tables: BTreeMap<String, TableCost>,
}

pub(super) async fn snapshot(
    pool: &PgPool,
    schema: &str,
    domain: &BTreeSet<String>,
) -> Result<Snapshot> {
    ensure!(
        super::isolated_schema(pool).await? == schema,
        "measurement schema guard failed"
    );
    let rows=sqlx::query("select c.relname,c.relkind::text kind,pg_table_size(c.oid)::bigint table_bytes,pg_indexes_size(c.oid)::bigint index_bytes,case when c.reltoastrelid=0 then 0 else pg_total_relation_size(c.reltoastrelid) end::bigint toast_bytes,pg_total_relation_size(c.oid)::bigint total_bytes from pg_class c join pg_namespace n on n.oid=c.relnamespace where n.nspname=$1 and c.relkind in ('r','p') and not c.relispartition and c.relname<>'_sqlx_migrations' order by c.relname")
        .bind(schema).fetch_all(pool).await?;
    let mut tables = BTreeMap::new();
    let mut task = 0;
    let mut support = 0;
    for row in rows {
        let name: String = row.try_get("relname")?;
        ensure!(
            row.try_get::<String, _>("kind")? == "r",
            "physical replay requires explicit partition costing: {name}"
        );
        let (count, values): (i64, i64) = sqlx::query_as(&format!(
            "select count(*),coalesce(sum(pg_column_size(t)),0)::bigint from {} t",
            qualified(schema, &name)
        ))
        .fetch_one(pool)
        .await?;
        let total: i64 = row.try_get("total_bytes")?;
        let toast: i64 = row.try_get("toast_bytes")?;
        let table: i64 = row.try_get("table_bytes")?;
        let index: i64 = row.try_get("index_bytes")?;
        ensure!(
            table + index == total,
            "relation/index/TOAST cost accounting mismatch"
        );
        let index_rows=sqlx::query("select i.relname,pg_relation_size(i.oid)::bigint bytes from pg_index x join pg_class t on t.oid=x.indrelid join pg_namespace n on n.oid=t.relnamespace join pg_class i on i.oid=x.indexrelid where n.nspname=$1 and t.relname=$2 order by i.relname")
            .bind(schema).bind(&name).fetch_all(pool).await?;
        let indexes = index_rows
            .into_iter()
            .map(|r| Ok((r.try_get("relname")?, r.try_get("bytes")?)))
            .collect::<Result<_>>()?;
        let measured = domain.contains(&name) || DOMAIN.split_whitespace().any(|t| t == name);
        if measured {
            task += total;
        } else {
            support += total;
        }
        tables.insert(
            name,
            TableCost {
                cost_domain: if measured {
                    "task_runtime_business"
                } else {
                    "finite_fk_support_and_defaults"
                },
                rows: count,
                composite_row_value_bytes: values,
                heap_and_auxiliary_bytes: table - toast,
                main_index_bytes: index,
                toast_and_toast_indexes_bytes: toast,
                gross_allocated_bytes: total,
                indexes,
            },
        );
    }
    Ok(Snapshot {
        task_domain_gross_allocated_bytes: task,
        fk_support_and_defaults_gross_allocated_bytes: support,
        tables,
    })
}

pub(super) async fn compact_isolated_layout(
    pool: &PgPool,
    schema: &str,
    _domain: &BTreeSet<String>,
    _copied: &BTreeMap<String, u64>,
) -> Result<()> {
    ensure!(
        super::isolated_schema(pool).await? == schema,
        "physical rewrite schema guard failed"
    );
    let mut connection = pool.acquire().await?;
    let names:Vec<String>=sqlx::query_scalar("select c.relname from pg_class c join pg_namespace n on n.oid=c.relnamespace where n.nspname=$1 and c.relkind='r' and not c.relispartition and c.relname<>'_sqlx_migrations' order by c.relname")
        .bind(schema).fetch_all(&mut *connection).await?;
    for name in names {
        // Same full finite closure policy on both samples and their own empty schemas.
        // VACUUM FULL rebuilds heap/TOAST and the table's indexes. It is an
        // isolated-layout measurement only, never a live-data reclaim claim.
        sqlx::query(&format!(
            "vacuum (full,analyze) {}",
            qualified(schema, &name)
        ))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Compare compact layouts only, subtracting each schema's own equally compact
/// empty baseline. Gross and per-table costs remain available in the receipt.
pub(super) fn compare(
    before: &Snapshot,
    before_empty: &Snapshot,
    after: &Snapshot,
    after_empty: &Snapshot,
) -> Result<serde_json::Value> {
    let task_before =
        before.task_domain_gross_allocated_bytes - before_empty.task_domain_gross_allocated_bytes;
    let task_after =
        after.task_domain_gross_allocated_bytes - after_empty.task_domain_gross_allocated_bytes;
    let support_before = before.fk_support_and_defaults_gross_allocated_bytes
        - before_empty.fk_support_and_defaults_gross_allocated_bytes;
    let support_after = after.fk_support_and_defaults_gross_allocated_bytes
        - after_empty.fk_support_and_defaults_gross_allocated_bytes;
    ensure!(
        task_before >= 0 && task_after >= 0 && support_before >= 0 && support_after >= 0,
        "own-empty baseline exceeds finite closure physical cost"
    );
    let names: BTreeSet<_> = before
        .tables
        .keys()
        .chain(after.tables.keys())
        .cloned()
        .collect();
    let per_table: BTreeMap<_, _> = names
        .into_iter()
        .map(|name| {
            let allocated = |snapshot: &Snapshot| {
                snapshot
                    .tables
                    .get(&name)
                    .map_or(0, |t| t.gross_allocated_bytes)
            };
            let before_net = allocated(before) - allocated(before_empty);
            let after_net = allocated(after) - allocated(after_empty);
            (
                name,
                serde_json::json!({"before_net_bytes":before_net,"after_net_bytes":after_net,
            "saved_bytes":before_net-after_net}),
            )
        })
        .collect();
    Ok(
        serde_json::json!({"per_table":per_table,"layout_policy":"vacuum_full_all_isolated_closure_tables",
        "task_before_bytes":task_before,"task_after_bytes":task_after,
        "support_before_bytes":support_before,"support_after_bytes":support_after,
        "closure_before_bytes":task_before+support_before,"closure_after_bytes":task_after+support_after,
        "task_saved_bytes":task_before-task_after,
        "closure_saved_bytes":task_before+support_before-task_after-support_after}),
    )
}
