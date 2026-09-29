use super::{ident, qualified, run_array};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sqlx::{PgConnection, PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Serialize)]
pub(super) struct Table {
    pub name: String,
    pub columns: Vec<String>,
    writable: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct CopyReceipt {
    pub initial_task_tables: BTreeMap<String, u64>,
    pub copied: BTreeMap<String, u64>,
    pub supporting_fk_tables: BTreeMap<String, u64>,
    pub domain: BTreeSet<String>,
    pub source_foreign_keys: usize,
    pub closure_passes: u64,
    pub missing_foreign_key_parents: i64,
    #[serde(skip)]
    pub tables: Vec<Table>,
}

struct ForeignKey {
    child: String,
    parent: String,
    parent_schema: String,
    child_columns: Vec<String>,
    parent_columns: Vec<String>,
}

pub(super) const DOMAIN: &str = "flow_runs node_runs node_run_details flow_run_checkpoints flow_run_events flow_run_tool_callback_inbox flow_run_callback_tasks flow_run_callback_resume_attempts flow_run_resume_claims flow_run_recovery_history runtime_spans runtime_events runtime_items runtime_context_projections runtime_usage_ledger runtime_cost_ledger runtime_credit_ledger billing_sessions runtime_artifacts runtime_audit_hashes runtime_canonical_contents runtime_observation_body_ownership runtime_native_snapshot_items runtime_native_snapshot_manifests runtime_native_snapshot_references runtime_invocation_context_bindings runtime_debug_artifacts runtime_legacy_shadow_batches runtime_legacy_shadow_rows application_conversations application_conversation_messages application_public_conversations application_run_conversation_message_items application_run_log_summaries application_run_log_tasks application_run_trace_projection_statuses application_run_trace_nodes application_run_trace_node_contents application_run_trace_refresh_queue external_agent_sessions external_agent_telemetry_events gateway_log_conversations gateway_log_turns gateway_log_invocations gateway_log_output_items gateway_log_tool_results provider_protocol_capsules provider_protocol_trajectory_events provider_semantic_trajectory_steps native_trajectory_integrity client_trajectory_archive_heads client_trajectory_archive_parts client_trajectory_captures client_trajectory_steps client_trajectory_sections client_trajectory_node_links assistant_conversations capability_invocations debug_variable_cache_entries model_provider_request_logs model_failover_attempt_ledger lifecycle_outbox lifecycle_outbox_deliveries runtime_event_deliveries";

async fn tables(connection: &mut PgConnection, schema: &str) -> Result<Vec<Table>> {
    let rows = sqlx::query("select c.relname,array_agg(a.attname::text order by a.attnum) columns,array_agg(a.attname::text order by a.attnum) filter(where a.attgenerated='') writable from pg_class c join pg_namespace n on n.oid=c.relnamespace join pg_attribute a on a.attrelid=c.oid and a.attnum>0 and not a.attisdropped where n.nspname=$1 and c.relkind in ('r','p') and not c.relispartition and c.relname<>'_sqlx_migrations' group by c.relname order by c.relname")
        .bind(schema).fetch_all(&mut *connection).await?;
    rows.into_iter()
        .map(|row| {
            Ok(Table {
                name: row.try_get("relname")?,
                columns: row.try_get("columns")?,
                writable: row.try_get("writable")?,
            })
        })
        .collect()
}

async fn foreign_keys(connection: &mut PgConnection, schema: &str) -> Result<Vec<ForeignKey>> {
    let rows = sqlx::query("select child.relname child,parent.relname parent,pn.nspname parent_schema,array(select a.attname::text from unnest(f.conkey) with ordinality k(num,position) join pg_attribute a on a.attrelid=f.conrelid and a.attnum=k.num order by k.position) child_columns,array(select a.attname::text from unnest(f.confkey) with ordinality k(num,position) join pg_attribute a on a.attrelid=f.confrelid and a.attnum=k.num order by k.position) parent_columns from pg_constraint f join pg_class child on child.oid=f.conrelid join pg_namespace cn on cn.oid=child.relnamespace join pg_class parent on parent.oid=f.confrelid join pg_namespace pn on pn.oid=parent.relnamespace where f.contype='f' and cn.nspname=$1 order by child.relname,parent.relname,f.conname")
        .bind(schema).fetch_all(&mut *connection).await?;
    rows.into_iter()
        .map(|row| {
            Ok(ForeignKey {
                child: row.try_get("child")?,
                parent: row.try_get("parent")?,
                parent_schema: row.try_get("parent_schema")?,
                child_columns: row.try_get("child_columns")?,
                parent_columns: row.try_get("parent_columns")?,
            })
        })
        .collect()
}

fn join_columns(
    left: &str,
    right: &str,
    left_columns: &[String],
    right_columns: &[String],
) -> String {
    left_columns
        .iter()
        .zip(right_columns)
        .map(|(l, r)| format!("{left}.{}={right}.{}", ident(l), ident(r)))
        .collect::<Vec<_>>()
        .join(" and ")
}
fn nonnull(alias: &str, columns: &[String]) -> String {
    columns
        .iter()
        .map(|c| format!("{alias}.{} is not null", ident(c)))
        .collect::<Vec<_>>()
        .join(" and ")
}

fn task_predicate(table: &Table, runs: &[Uuid]) -> Option<String> {
    let ids = run_array(runs);
    let mut predicates = Vec::new();
    if table.name == "flow_runs" {
        return Some(format!("t.id=any({ids})"));
    }
    // Direct run ownership and indirect directories are a finite inventory,
    // not application/scope-wide copying. FK support is filled separately.
    if table.columns.iter().any(|c| c == "flow_run_id") {
        predicates.push(format!("t.flow_run_id::text=any({ids}::text[])"));
    }
    if table.columns.iter().any(|c| c == "node_run_id") {
        predicates.push(format!(
            "t.node_run_id::text in(select id::text from public.node_runs where flow_run_id=any({ids}))"
        ));
    }
    if table.columns.iter().any(|c| c == "request_id") {
        predicates.push(format!("t.request_id::text in(select request_id::text from public.client_trajectory_archive_heads where flow_run_id=any({ids}))"));
    }
    for column in ["event_id", "runtime_event_id"] {
        if table.columns.iter().any(|c| c == column) {
            predicates.push(format!(
                "t.{}::text in(select id::text from public.runtime_events where flow_run_id=any({ids}))",
                ident(column)
            ));
        }
    }
    if table.name == "application_run_log_tasks" {
        predicates.push(format!("t.id=any({ids}) or t.member_run_ids && {ids}"));
    }
    if table.name == "node_run_details" {
        predicates.push(format!(
            "t.node_run_id::text in(select id::text from public.node_runs where flow_run_id=any({ids}))"
        ));
    }
    if table.name == "runtime_legacy_shadow_rows" {
        predicates.push(format!("t.source_row_id=any({ids}) or t.source_row_id in(select id from public.node_runs where flow_run_id=any({ids})) or t.source_row_id in(select id from public.flow_run_checkpoints where flow_run_id=any({ids}))"));
    }
    (!predicates.is_empty()).then(|| predicates.join(" or "))
}

async fn insert_selected(
    connection: &mut PgConnection,
    schema: &str,
    table: &Table,
    predicate: &str,
) -> Result<u64> {
    let columns = table
        .writable
        .iter()
        .map(|c| ident(c))
        .collect::<Vec<_>>()
        .join(",");
    let values = table
        .writable
        .iter()
        .map(|c| format!("t.{}", ident(c)))
        .collect::<Vec<_>>()
        .join(",");
    // Source remains qualified public SELECT only; no commands ever target a
    // public relation. No JSON/text reserialization of bytea, originals or bodies.
    let sql = format!("insert into {}({columns}) overriding system value select {values} from {} t where ({predicate}) on conflict do nothing",qualified(schema,&table.name),qualified("public",&table.name));
    Ok(sqlx::query(&sql)
        .execute(&mut *connection)
        .await?
        .rows_affected())
}

async fn missing(connection: &mut PgConnection, schema: &str, fk: &ForeignKey) -> Result<i64> {
    let sql = format!(
        "select count(*) from {} c where {} and not exists(select 1 from {} p where {})",
        qualified(schema, &fk.child),
        nonnull("c", &fk.child_columns),
        qualified(schema, &fk.parent),
        join_columns("c", "p", &fk.child_columns, &fk.parent_columns)
    );
    Ok(sqlx::query_scalar(&sql).fetch_one(&mut *connection).await?)
}

pub(super) async fn copy_sample(pool: &PgPool, schema: &str, runs: &[Uuid]) -> Result<CopyReceipt> {
    ensure!(
        super::isolated_schema(pool).await? == schema,
        "copy destination guard failed"
    );
    let mut tx = pool.begin().await?;
    sqlx::query("set transaction isolation level repeatable read")
        .execute(&mut *tx)
        .await?;
    // Local to this guarded destination connection/transaction. Source tables
    // are only read, so duplicate projection triggers cannot fabricate rows.
    sqlx::query("set local session_replication_role=replica")
        .execute(&mut *tx)
        .await?;
    let destination = tables(&mut tx, schema).await?;
    let source = tables(&mut tx, "public").await?;
    let source_by_name: BTreeMap<_, _> = source.iter().map(|t| (t.name.as_str(), t)).collect();
    let destination_by_name: BTreeMap<_, _> =
        destination.iter().map(|t| (t.name.as_str(), t)).collect();
    let keys = foreign_keys(&mut tx, "public").await?;
    let mut copied = BTreeMap::new();
    let mut initial = BTreeMap::new();
    let mut domain: BTreeSet<String> = DOMAIN.split_whitespace().map(str::to_owned).collect();
    for table in &destination {
        let Some(predicate) = task_predicate(table, runs) else {
            continue;
        };
        let source = source_by_name
            .get(table.name.as_str())
            .context("task table unavailable in public source")?;
        ensure!(
            table.writable.iter().all(|c| source.columns.contains(c)),
            "task table source/prefix column mismatch: {}",
            table.name
        );
        let rows = insert_selected(&mut tx, schema, table, &predicate).await?;
        initial.insert(table.name.clone(), rows);
        copied.insert(table.name.clone(), rows);
        domain.insert(table.name.clone());
    }
    let mut passes = 0;
    loop {
        passes += 1;
        let mut added = 0;
        for fk in &keys {
            let Some(child) = destination_by_name.get(fk.child.as_str()) else {
                continue;
            };
            if !copied.contains_key(&child.name) {
                continue;
            }
            ensure!(
                fk.parent_schema == "public",
                "finite closure requires a non-public parent table: {}",
                fk.parent
            );
            let parent = destination_by_name
                .get(fk.parent.as_str())
                .context("FK parent table unavailable in prefix schema")?;
            let source_parent = source_by_name
                .get(fk.parent.as_str())
                .context("FK parent table unavailable in public source")?;
            ensure!(
                parent
                    .writable
                    .iter()
                    .all(|c| source_parent.columns.contains(c)),
                "FK parent source/prefix column mismatch: {}",
                parent.name
            );
            let predicate = format!("exists(select 1 from {} c where {} and {}) and not exists(select 1 from {} p where {})",qualified(schema,&fk.child),nonnull("c",&fk.child_columns),join_columns("c","t",&fk.child_columns,&fk.parent_columns),qualified(schema,&fk.parent),join_columns("p","t",&fk.parent_columns,&fk.parent_columns));
            if fk.parent == "flow_runs" {
                let forbidden: i64 = sqlx::query_scalar(&format!("select count(*) from public.flow_runs t where ({predicate}) and not(t.id=any({}))",run_array(runs))).fetch_one(&mut *tx).await?;
                ensure!(
                    forbidden == 0,
                    "finite closure requires an additional unapproved flow run"
                );
            }
            let rows = insert_selected(&mut tx, schema, parent, &predicate).await?;
            if rows > 0 {
                *copied.entry(parent.name.clone()).or_default() += rows;
            }
            added += rows;
        }
        // Explicit task-associated child closure. These rows may lack a direct
        // run key; copy only children of the selected conversation/body parents.
        for (child, parent, child_key, parent_key) in [
            (
                "application_conversation_messages",
                "application_conversations",
                "conversation_id",
                "id",
            ),
            (
                "runtime_observation_body_ownership",
                "runtime_canonical_contents",
                "content_id",
                "id",
            ),
        ] {
            if !copied.contains_key(parent) {
                continue;
            }
            let Some(table) = destination_by_name.get(child) else {
                continue;
            };
            let predicate = format!(
                "t.{} in(select {} from {})",
                ident(child_key),
                ident(parent_key),
                qualified(schema, parent)
            );
            let rows = insert_selected(&mut tx, schema, table, &predicate).await?;
            if rows > 0 {
                *copied.entry(child.into()).or_default() += rows;
            }
            added += rows;
        }
        if added == 0 {
            break;
        }
    }
    // Restore all triggers before accepting the snapshot. PostgreSQL does not
    // retrospectively validate rows inserted with replica mode, so audit every
    // actual destination FK explicitly, including composite ownership keys.
    sqlx::query("set local session_replication_role=origin")
        .execute(&mut *tx)
        .await?;
    let destination_keys = foreign_keys(&mut tx, schema).await?;
    for fk in &destination_keys {
        ensure!(
            fk.parent_schema == schema,
            "isolated schema FK escapes destination"
        );
        ensure!(
            missing(&mut tx, schema, fk).await? == 0,
            "finite closure has missing FK parents: {} -> {}",
            fk.child,
            fk.parent
        );
    }
    audit_connection(&mut tx, schema, runs).await?;
    tx.commit().await?;
    let supporting_fk_tables = copied
        .iter()
        .filter(|(t, _)| !domain.contains(*t))
        .map(|(t, n)| (t.clone(), *n))
        .collect();
    Ok(CopyReceipt {
        initial_task_tables: initial,
        copied,
        supporting_fk_tables,
        domain,
        source_foreign_keys: keys.len(),
        closure_passes: passes,
        missing_foreign_key_parents: 0,
        tables: destination,
    })
}

async fn audit_connection(
    connection: &mut PgConnection,
    schema: &str,
    runs: &[Uuid],
) -> Result<()> {
    let ids = run_array(runs);
    let counts: (i64, i64) = sqlx::query_as(&format!(
        "select count(*),count(*) filter(where not(id=any({ids}))) from {}",
        qualified(schema, "flow_runs")
    ))
    .fetch_one(&mut *connection)
    .await?;
    ensure!(
        counts == (15, 0),
        "copy/maintenance escaped exact 15-run owner set"
    );
    for table in tables(connection, schema).await? {
        if table.columns.iter().any(|c| c == "flow_run_id") {
            let outside:i64=sqlx::query_scalar(&format!("select count(*) from {} where flow_run_id is not null and not(flow_run_id::text=any({ids}::text[]))",qualified(schema,&table.name))).fetch_one(&mut *connection).await?;
            ensure!(
                outside == 0,
                "associated table has unapproved run owner: {}",
                table.name
            );
        }
    }
    let outside_tasks: i64 = sqlx::query_scalar(&format!(
        "select count(*) from {} where not(member_run_ids <@ {ids})",
        qualified(schema, "application_run_log_tasks")
    ))
    .fetch_one(&mut *connection)
    .await?;
    ensure!(
        outside_tasks == 0,
        "task member_run_ids exceed exact allowlist"
    );
    let archive_owners:i64=sqlx::query_scalar(&format!("select count(*) from {} h left join {} c using(request_id) where c.request_id is null or h.flow_run_id is distinct from c.flow_run_id",qualified(schema,"client_trajectory_archive_heads"),qualified(schema,"client_trajectory_captures"))).fetch_one(&mut *connection).await?;
    ensure!(
        archive_owners == 0,
        "archive/capture request ownership mismatch"
    );
    Ok(())
}

pub(super) async fn audit(pool: &PgPool, schema: &str, runs: &[Uuid]) -> Result<()> {
    ensure!(
        super::isolated_schema(pool).await? == schema,
        "post-copy schema guard failed"
    );
    let mut connection = pool.acquire().await?;
    for fk in foreign_keys(&mut connection, schema).await? {
        ensure!(
            fk.parent_schema == schema && missing(&mut connection, schema, &fk).await? == 0,
            "post-copy missing/escaping FK parent"
        );
    }
    audit_connection(&mut connection, schema, runs).await
}
