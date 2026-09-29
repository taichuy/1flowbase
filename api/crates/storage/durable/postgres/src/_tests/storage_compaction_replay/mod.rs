//! Opt-in, candidate-bound replay of the retained 62m sample. No fixture payload
//! is exported: the artifact contains counts, hashes and physical sizes only.
mod copy;
mod fingerprints;
mod measurement;

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{collections::BTreeSet, path::PathBuf};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const PREFIX: i64 = 20260929100000;
const EVENTS: i64 = 51081;
const FRAMES: i64 = 85201;
const CAPTURES: i64 = 96;

struct Input {
    runs: Vec<Uuid>,
    receipt_hash: String,
    costs_hash: String,
    old_core_row_value_bytes: i64,
    candidate_sha: String,
    output: PathBuf,
}

impl Input {
    fn read() -> Result<Self> {
        let output = PathBuf::from(std::env::var("STORAGE_REPLAY_OUTPUT_DIR")?);
        ensure!(
            output.is_absolute(),
            "replay output must be an absolute path"
        );
        let components: Vec<_> = output.components().collect();
        ensure!(
            components
                .windows(2)
                .any(|parts| parts[0].as_os_str() == "tmp"
                    && parts[1].as_os_str() == "test-governance"),
            "replay artifact must stay under tmp/test-governance"
        );
        let receipt = std::fs::read(std::env::var("STORAGE_REPLAY_RECEIPT")?)?;
        let costs = std::fs::read(std::env::var("STORAGE_REPLAY_COSTS")?)?;
        let source: Value = serde_json::from_slice(&receipt)?;
        let costs_value: Value = serde_json::from_slice(&costs)?;
        let mut runs = source["runs"]
            .as_array()
            .context("sample runs missing")?
            .iter()
            .map(|r| {
                Uuid::parse_str(r["id"].as_str().context("sample run ID missing")?)
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>>>()?;
        runs.sort();
        ensure!(
            runs.len() == 15 && runs.iter().collect::<BTreeSet<_>>().len() == 15,
            "sample must contain exactly 15 distinct runs"
        );
        let event_count: i64 = source["event_types"]
            .as_array()
            .context("sample events missing")?
            .iter()
            .map(|r| r["rows"].as_i64().unwrap_or(-1))
            .sum();
        ensure!(
            event_count == EVENTS
                && source["frame_parts"]["frames"] == FRAMES
                && costs_value["clientFrames"] == FRAMES,
            "receipt sample counts differ from approved sample"
        );
        let receipt_hash = hash(&receipt);
        ensure!(
            costs_value["sourceSha256"].as_str() == Some(receipt_hash.as_str()),
            "cost receipt is not attached to source receipt"
        );
        let git = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()?;
        ensure!(git.status.success(), "candidate SHA unavailable");
        let candidate_sha = String::from_utf8(git.stdout)?.trim().to_owned();
        ensure!(
            candidate_sha.len() == 40 && candidate_sha.bytes().all(|b| b.is_ascii_hexdigit()),
            "candidate SHA invalid"
        );
        if let Ok(expected) = std::env::var("STORAGE_REPLAY_CANDIDATE_SHA") {
            ensure!(expected == candidate_sha, "fixture candidate SHA mismatch");
        }
        Ok(Self {
            runs,
            receipt_hash,
            costs_hash: hash(&costs),
            old_core_row_value_bytes: costs_value["coreDataRowValuesBytes"]
                .as_i64()
                .context("core cost missing")?,
            candidate_sha,
            output,
        })
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn qualified(schema: &str, table: &str) -> String {
    format!("{}.{}", ident(schema), ident(table))
}
pub(super) fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
pub(super) fn run_array(runs: &[Uuid]) -> String {
    format!(
        "array[{}]::uuid[]",
        runs.iter()
            .map(|r| format!("'{r}'"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

async fn isolated_schema(pool: &PgPool) -> Result<String> {
    let schema: String = sqlx::query_scalar("select current_schema()")
        .fetch_one(pool)
        .await?;
    let schemas: Vec<String> = sqlx::query_scalar("select current_schemas(false)::text[]")
        .fetch_one(pool)
        .await?;
    ensure!(
        schema.starts_with("test_")
            && schema.len() == 37
            && schema[5..].bytes().all(|b| b.is_ascii_hexdigit())
            && schemas == [schema.clone()],
        "replay requires a guarded isolated test schema with no public search-path fallback"
    );
    Ok(schema)
}

async fn empty_fixture(version: Option<i64>) -> Result<PgPool> {
    let (pool, seed) =
        super::provider_protocol_capsule_store_tests::seeded_flow_run_before(version).await;
    let schema = isolated_schema(&pool).await?;
    sqlx::query(&format!(
        "delete from {} where id=$1",
        qualified(&schema, "flow_runs")
    ))
    .bind(seed)
    .execute(&pool)
    .await?;
    let remaining: i64 = sqlx::query_scalar(&format!(
        "select count(*) from {}",
        qualified(&schema, "flow_runs")
    ))
    .fetch_one(&pool)
    .await?;
    ensure!(
        remaining == 0,
        "seed runtime rows were not completely removed"
    );
    Ok(pool)
}

async fn sample_counts(pool: &PgPool, schema: &str, runs: &[Uuid]) -> Result<Value> {
    let ids = run_array(runs);
    let sql = format!("select (select count(*) from {s}.flow_runs where id=any({ids})) runs,(select count(*) from {s}.runtime_events where flow_run_id=any({ids})) events,(select coalesce(sum(jsonb_array_length(p.frames)),0)::bigint from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids})) frames,(select count(*) from {s}.client_trajectory_captures where flow_run_id=any({ids})) captures", s=ident(schema));
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let result = json!({"runs":row.try_get::<i64,_>("runs")?,"events":row.try_get::<i64,_>("events")?,"frames":row.try_get::<i64,_>("frames")?,"captures":row.try_get::<i64,_>("captures")?});
    ensure!(
        result["runs"] == 15
            && result["events"] == EVENTS
            && result["frames"] == FRAMES
            && result["captures"] == CAPTURES,
        "source/copy approved sample count mismatch"
    );
    Ok(result)
}

async fn replay(input: &Input, phase: &mut &'static str) -> Result<Value> {
    *phase = "prefix_schema";
    let pool = empty_fixture(Some(PREFIX))
        .await
        .context("prefix isolated-schema creation")?;
    let schema = isolated_schema(&pool).await?;
    *phase = "source_snapshot";
    let source_counts = sample_counts(&pool, "public", &input.runs)
        .await
        .context("public source sample count validation")?;
    let source_events = fingerprints::events(&pool, "public", &input.runs)
        .await
        .context("public original-event fingerprint")?;
    *phase = "prefix_empty_measurement";
    let prefix_empty = measurement::snapshot(&pool, &schema, &BTreeSet::new()).await?;
    *phase = "finite_copy_and_fk_audit";
    let copy = copy::copy_sample(&pool, &schema, &input.runs)
        .await
        .context("finite task/FK closure copy and audit")?;
    let copied_counts = sample_counts(&pool, &schema, &input.runs)
        .await
        .context("copied sample count validation")?;
    let copied_events = fingerprints::events(&pool, &schema, &input.runs)
        .await
        .context("prefix original-event fingerprint")?;
    ensure!(
        copied_events == source_events,
        "prefix copy original events differ"
    );
    *phase = "old_gross_measurement";
    let before = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    let retained_before = fingerprints::retained(&pool, &schema, &copy.tables).await?;

    *phase = "formal_upgrade";
    run_migrations(&pool)
        .await
        .context("formal complete schema upgrade")?;
    let upgrade_events = fingerprints::events(&pool, &schema, &input.runs).await?;
    ensure!(
        upgrade_events == source_events,
        "formal upgrade original events differ"
    );
    let store = PgControlPlaneStore::new(pool.clone());
    *phase = "legacy_reader_baseline";
    let before_readers = fingerprints::readers(&store, &input.runs)
        .await
        .context("upgraded legacy reader fingerprint")?;
    ensure!(
        before_readers.frames == FRAMES as u64,
        "legacy reader frame count differs"
    );
    let upgraded = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    *phase = "actual_mover";
    let moved = store
        .compact_retained_runtime_storage(&input.runs, 256)
        .await
        .context("actual retained-runtime mover")?;
    ensure!(
        moved.client_semantic_records_processed > 0
            && moved.native_snapshot_events > 0
            && moved.raw_archive_parts > 0,
        "sample mover did not transform each requested domain"
    );
    *phase = "lossless_after_mover";
    let after_events = fingerprints::events(&pool, &schema, &input.runs).await?;
    let after_readers = fingerprints::readers(&store, &input.runs)
        .await
        .context("compacted reader fingerprint")?;
    let retained_after = fingerprints::retained(&pool, &schema, &copy.tables).await?;
    ensure!(
        after_events == source_events,
        "compaction complete original events/Native strings differ"
    );
    ensure!(
        after_readers == before_readers,
        "compaction client pages/sections/cursors/filter/focus/raw frames differ"
    );
    ensure!(
        retained_after == retained_before,
        "compaction retained business/source-reference rows differ"
    );
    copy::audit(&pool, &schema, &input.runs)
        .await
        .context("post-mover FK and owner audit")?;
    *phase = "live_allocated_measurement";
    let live_after = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    *phase = "actual_mover_reentry";
    let repeated = store
        .compact_retained_runtime_storage(&input.runs, 256)
        .await
        .context("actual mover reentry")?;
    ensure!(
        repeated.client_semantic_records_processed == 0
            && repeated.native_snapshot_events == 0
            && repeated.raw_archive_parts == 0,
        "mover reentry was not row-idempotent"
    );
    let repeated_events = fingerprints::events(&pool, &schema, &input.runs).await?;
    ensure!(
        repeated_events == after_events,
        "mover reentry changes original events"
    );
    *phase = "isolated_compact_layout";
    measurement::compact_isolated_layout(&pool, &schema, &copy.domain, &copy.copied)
        .await
        .context("isolated physical-layout compaction")?;
    let compact_equivalent = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    *phase = "latest_empty_baseline";
    let latest_empty_pool = empty_fixture(None)
        .await
        .context("latest empty physical baseline")?;
    let latest_empty_schema = isolated_schema(&latest_empty_pool).await?;
    let latest_empty =
        measurement::snapshot(&latest_empty_pool, &latest_empty_schema, &copy.domain).await?;
    *phase = "source_unchanged_audit";
    let source_counts_after = sample_counts(&pool, "public", &input.runs).await?;
    ensure!(
        source_counts_after == source_counts,
        "public source changed during replay"
    );
    let source_events_after = fingerprints::events(&pool, "public", &input.runs).await?;
    ensure!(
        source_events_after == source_events,
        "public source event originals changed during replay"
    );
    let receipt = json!({"status":"passed","candidate_sha":input.candidate_sha,
        "source_receipt_sha256":input.receipt_hash,"source_costs_sha256":input.costs_hash,
        "run_allowlist_sha256":hash(serde_json::to_string(&input.runs)?.as_bytes()),"source_counts":source_counts,"copied_counts":copied_counts,
        "old_receipt_core_row_values_bytes":input.old_core_row_value_bytes,
        "copy":copy,"mover":moved,"reentry":repeated,"lossless_original_events":after_events,"lossless_readers":after_readers,"retained_business_rows":retained_after,
        "physical":{"prefix_empty":prefix_empty,"latest_empty":latest_empty,"old_gross":before,"formal_upgrade_before_mover":upgraded,"live_after_compaction_allocated":live_after,"isolated_compact_layout_equivalent":compact_equivalent},
        "notes":["Source public tables were selected only; all writes, migrations and VACUUM FULL targeted guarded isolated schemas.",
            "193046031-byte/184.103-MiB original core receipt excluded client steps and summary/conversation/recovery/Outbox/usage/billing costs; this replay lists them and every added directory/header/item/ref/ownership/index/TOAST cost.",
            "Gross relation totals include empty relation/index overhead. Task domain and finite FK support/default costs are separate; shared canonical parents are counted once in the isolated closure, not attributed as incremental source growth.",
            "Live allocated after-compaction sizes are recorded before VACUUM FULL. Compact-layout equivalent does not claim live source file reclamation, CPU/latency/WAL/replica/backup savings.",
            "Complete JSON values are compared as representation-preserving Value hashes; Native body Strings and every raw frame byte/kind/time/sequence are hashed exactly. IDs/order/cursors/filter/focus/source-reference identities are included. No original payloads are exported."]});
    latest_empty_pool.close().await;
    pool.close().await;
    Ok(receipt)
}

fn safe_database_error(error: &anyhow::Error) -> Value {
    for cause in error.chain() {
        if let Some(sqlx::Error::Database(database)) = cause.downcast_ref::<sqlx::Error>() {
            if let Some(pg) = database.try_downcast_ref::<sqlx::postgres::PgDatabaseError>() {
                return json!({"sqlstate":pg.code(),"constraint":pg.constraint(),"table":pg.table()});
            }
        }
    }
    Value::Null
}

#[tokio::test]
#[ignore = "Requires explicit 62m receipt/cost/output environment and an isolated PostgreSQL replay"]
async fn real_62m_storage_compaction_lossless_and_physical_receipt() {
    // Test harness errors must never print an original body, credential or query
    // parameter. A failure exposes only its digest and a fixed phase label.
    let input = Input::read().unwrap_or_else(|error| {
        panic!(
            "replay input failed; error digest {}",
            hash(format!("{error:#}").as_bytes())
        )
    });
    std::fs::create_dir_all(&input.output).expect("replay output directory unavailable");
    let mut phase = "prefix_schema";
    let (receipt, failed) = match replay(&input, &mut phase).await {
        Ok(receipt) => (receipt, false),
        Err(error) => (
            json!({"status":"failed","phase":phase,"candidate_sha":input.candidate_sha,"source_receipt_sha256":input.receipt_hash,"source_costs_sha256":input.costs_hash,"database_error":safe_database_error(&error),"error_sha256":hash(format!("{error:#}").as_bytes()),"notes":["Replay stopped before acceptance; original values and credentials are intentionally absent."]}),
            true,
        ),
    };
    std::fs::write(
        input.output.join("storage-compaction-replay.json"),
        serde_json::to_vec_pretty(&receipt).expect("receipt serialization failed"),
    )
    .expect("receipt write failed");
    assert!(
        !failed,
        "storage replay failed; see sanitized storage-compaction-replay.json"
    );
}
