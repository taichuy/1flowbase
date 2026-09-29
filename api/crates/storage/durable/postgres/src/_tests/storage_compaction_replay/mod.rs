//! Opt-in, candidate-bound replay of an explicitly approved finite sample. No fixture payload
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
const DENSE_PREFIX: i64 = 20260929143000;
const CURRENT_PREFIX: i64 = 20260929143001;

struct Input {
    runs: Vec<Uuid>,
    events: i64,
    frames: i64,
    captures: i64,
    prefix_version: i64,
    finite_manifest: bool,
    inventory: Option<Value>,
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
        if let Ok(path) = std::env::var("STORAGE_REPLAY_SAMPLE_MANIFEST") {
            let path = PathBuf::from(path);
            ensure!(path.is_absolute(), "sample manifest must be absolute");
            let bytes = std::fs::read(path)?;
            let source: Value = serde_json::from_slice(&bytes)?;
            let mut runs = source["run_ids"]
                .as_array()
                .context("run_ids missing")?
                .iter()
                .map(|id| {
                    Uuid::parse_str(id.as_str().context("run ID missing")?).map_err(Into::into)
                })
                .collect::<Result<Vec<_>>>()?;
            runs.sort();
            ensure!(
                !runs.is_empty() && runs.iter().collect::<BTreeSet<_>>().len() == runs.len(),
                "finite sample run IDs must be nonempty and unique"
            );
            let events = source["events"].as_i64().context("exact events missing")?;
            let frames = source["frames"].as_i64().context("exact frames missing")?;
            let captures = source["captures"]
                .as_i64()
                .context("exact captures missing")?;
            let prefix_version = source["prefix_version"]
                .as_i64()
                .context("prefix_version missing")?;
            ensure!(
                events > 0 && frames > 0 && captures > 0,
                "approved finite baseline counts must be positive"
            );
            ensure!(
                [20260929120000, 20260929120001, DENSE_PREFIX, CURRENT_PREFIX]
                    .contains(&prefix_version),
                "finite sample requires approved prior or current schema prefix"
            );
            let candidate_sha = candidate_sha()?;
            ensure!(
                source["candidate_sha"].as_str() == Some(candidate_sha.as_str()),
                "sample manifest candidate SHA mismatch"
            );
            return Ok(Self {
                runs,
                events,
                frames,
                captures,
                prefix_version,
                finite_manifest: true,
                inventory: source.get("inventory").cloned(),
                receipt_hash: hash(&bytes),
                costs_hash: String::new(),
                old_core_row_value_bytes: 0,
                candidate_sha,
                output,
            });
        }
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
        let candidate_sha = candidate_sha()?;
        Ok(Self {
            events: EVENTS,
            frames: FRAMES,
            captures: CAPTURES,
            prefix_version: PREFIX,
            finite_manifest: false,
            inventory: None,
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

fn candidate_sha() -> Result<String> {
    let git = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    ensure!(git.status.success(), "candidate SHA unavailable");
    let sha = String::from_utf8(git.stdout)?.trim().to_owned();
    ensure!(
        sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "candidate SHA invalid"
    );
    if let Ok(expected) = std::env::var("STORAGE_REPLAY_CANDIDATE_SHA") {
        ensure!(expected == sha, "fixture candidate SHA mismatch");
    }
    Ok(sha)
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

async fn sample_counts(pool: &PgPool, schema: &str, input: &Input) -> Result<Value> {
    let ids = run_array(&input.runs);
    let sql = format!("select (select count(*) from {s}.flow_runs where id=any({ids})) runs,(select count(*) from {s}.runtime_events where flow_run_id=any({ids})) events,(select coalesce(sum(case when coalesce((to_jsonb(p)->>'codec_version')::int,0)=0 then jsonb_array_length(p.frames) when (to_jsonb(p)->>'codec_version')::int in (1,2,3) then p.last_sequence-p.first_sequence+1 else null end),0)::bigint from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids})) frames,(select count(*) from {s}.client_trajectory_captures where flow_run_id=any({ids})) captures", s=ident(schema));
    let unknown: i64 = sqlx::query_scalar(&format!("select count(*) from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}) and coalesce((to_jsonb(p)->>'codec_version')::int,0) not in (0,1,2,3)",s=ident(schema))).fetch_one(pool).await?;
    ensure!(
        unknown == 0,
        "unknown archive encoding cannot enter frame counts"
    );
    let row = sqlx::query(&sql).fetch_one(pool).await?;
    let mut result = json!({"runs":row.try_get::<i64,_>("runs")?,"events":row.try_get::<i64,_>("events")?,"frames":row.try_get::<i64,_>("frames")?,"captures":row.try_get::<i64,_>("captures")?});
    ensure!(
        result["runs"] == input.runs.len()
            && result["events"] == input.events
            && result["frames"] == input.frames
            && result["captures"] == input.captures,
        "source/copy approved sample count mismatch"
    );
    if input.finite_manifest && input.prefix_version == 20260929120000 {
        let inventory: (i64, i64, i64) = sqlx::query_as(&format!("select (select count(*) from {s}.client_trajectory_steps where flow_run_id=any({ids})),(select count(*) from {s}.client_trajectory_sections where flow_run_id=any({ids})),(select count(*) from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}))",s=ident(schema))).fetch_one(pool).await?;
        ensure!(
            input.runs.len() == 8
                && input.events == 1941
                && input.frames == 39392
                && input.captures == 55
                && inventory == (4326, 13167, 17989),
            "prior 31m approved finite inventory differs"
        );
        result["steps"] = json!(inventory.0);
        result["sections"] = json!(inventory.1);
        result["raw_parts"] = json!(inventory.2);
    }
    if let Some(expected) = &input.inventory {
        ensure!(
            expected.as_object().is_some_and(|fields| fields.len() == 3)
                && ["steps", "sections", "raw_parts"]
                    .iter()
                    .all(|key| expected[*key].as_i64().is_some_and(|n| n > 0)),
            "manifest inventory must contain exact positive steps/sections/raw_parts counts"
        );
        let actual: (i64,i64,i64) = sqlx::query_as(&format!("select (select count(*) from {s}.client_trajectory_steps where flow_run_id=any({ids})),(select count(*) from {s}.client_trajectory_sections where flow_run_id=any({ids})),(select count(*) from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}))",s=ident(schema))).fetch_one(pool).await?;
        ensure!(
            json!({"steps":actual.0,"sections":actual.1,"raw_parts":actual.2}) == *expected,
            "approved finite inventory differs"
        );
        result["inventory"] = expected.clone();
    }
    if matches!(
        input.prefix_version,
        20260929120001 | DENSE_PREFIX | CURRENT_PREFIX
    ) {
        let layout = layout_inventory(pool, schema, &input.runs).await?;
        if input.prefix_version == CURRENT_PREFIX {
            ensure!(
                layout["dense_steps"].as_i64().unwrap_or(0) > 0
                    && layout["packed_parts"].as_i64().unwrap_or(0) > 0
                    && layout["packed_blocks"].as_i64().unwrap_or(0) > 0,
                "current sample must exercise dense metadata and packed CAD directories"
            );
        } else {
            ensure!(
                layout["compact_layout0_steps"].as_i64().unwrap_or(0) > 0
                    && layout["sealed_inline_directory_parts"]
                        .as_i64()
                        .unwrap_or(0)
                        > 0,
                "C1 baseline must exercise compact layout0 and codec2 source parts"
            );
        }
        result["layout_inventory"] = layout;
    }
    Ok(result)
}

async fn layout_inventory(pool: &PgPool, schema: &str, runs: &[Uuid]) -> Result<Value> {
    let ids = run_array(runs);
    // to_jsonb inspects only the new typed layout marker. Originals and bodies
    // never pass through this inventory projection.
    let row = sqlx::query(&format!("select (select count(*) from {s}.client_trajectory_steps t where flow_run_id=any({ids}) and metadata_compact and coalesce((to_jsonb(t)->>'metadata_layout')::int,0)=0) compact_layout0_steps,(select count(*) from {s}.client_trajectory_steps t where flow_run_id=any({ids}) and metadata_compact and (to_jsonb(t)->>'metadata_layout')::int=1) dense_steps,(select count(*) from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}) and p.codec_version=2) sealed_inline_directory_parts,(select count(*) from {s}.client_trajectory_archive_parts p join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}) and p.codec_version=3) packed_parts,(select count(*) from {s}.client_trajectory_archive_blocks b join {s}.client_trajectory_archive_heads h using(request_id) where h.flow_run_id=any({ids}) and b.codec_version=2) packed_blocks",s=ident(schema))).fetch_one(pool).await?;
    Ok(
        json!({"compact_layout0_steps":row.try_get::<i64,_>("compact_layout0_steps")?,
        "dense_steps":row.try_get::<i64,_>("dense_steps")?,
        "sealed_inline_directory_parts":row.try_get::<i64,_>("sealed_inline_directory_parts")?,
        "packed_parts":row.try_get::<i64,_>("packed_parts")?,
        "packed_blocks":row.try_get::<i64,_>("packed_blocks")?}),
    )
}

async fn replay(input: &Input, phase: &mut &'static str) -> Result<Value> {
    *phase = "prefix_schema";
    let pool = empty_fixture(Some(input.prefix_version))
        .await
        .context("prefix isolated-schema creation")?;
    let schema = isolated_schema(&pool).await?;
    *phase = "source_snapshot";
    let source_counts = sample_counts(&pool, "public", input)
        .await
        .context("public source sample count validation")?;
    let source_events = fingerprints::events(&pool, "public", &input.runs, input.events)
        .await
        .context("public original-event fingerprint")?;
    let source_anchors = fingerprints::archive_anchors(&pool, "public", &input.runs).await?;
    *phase = "finite_copy_and_fk_audit";
    let copy = copy::copy_sample(&pool, &schema, &input.runs)
        .await
        .context("finite task/FK closure copy and audit")?;
    *phase = "prefix_empty_measurement";
    let prefix_empty_pool = empty_fixture(Some(input.prefix_version)).await?;
    let prefix_empty_schema = isolated_schema(&prefix_empty_pool).await?;
    measurement::compact_isolated_layout(
        &prefix_empty_pool,
        &prefix_empty_schema,
        &copy.domain,
        &Default::default(),
    )
    .await?;
    let prefix_empty =
        measurement::snapshot(&prefix_empty_pool, &prefix_empty_schema, &copy.domain).await?;
    prefix_empty_pool.close().await;
    let copied_counts = sample_counts(&pool, &schema, input)
        .await
        .context("copied sample count validation")?;
    let copied_events = fingerprints::events(&pool, &schema, &input.runs, input.events)
        .await
        .context("prefix original-event fingerprint")?;
    ensure!(
        copied_events == source_events,
        "prefix copy original events differ"
    );
    let copied_anchors = fingerprints::archive_anchors(&pool, &schema, &input.runs).await?;
    ensure!(
        copied_anchors == source_anchors,
        "finite copy changed original part anchors"
    );
    *phase = "old_gross_measurement";
    let before = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    *phase = "prefix_compact_layout";
    measurement::compact_isolated_layout(&pool, &schema, &copy.domain, &copy.copied).await?;
    let before_compact = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    let retained_before = fingerprints::retained(&pool, &schema, &copy.tables).await?;
    let queue_before = fingerprints::refresh_queue(&pool).await?;
    let mut retained_audit = json!({"prefix":&retained_before});
    let audit_path = input.output.join("retained-stage-audit.json");
    std::fs::write(&audit_path, serde_json::to_vec_pretty(&retained_audit)?)?;

    *phase = "formal_upgrade";
    let upgrade_started: time::OffsetDateTime = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    run_migrations(&pool)
        .await
        .context("formal complete schema upgrade")?;
    let upgrade_finished: time::OffsetDateTime = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&pool)
        .await?;
    let retained_upgraded = fingerprints::retained(&pool, &schema, &copy.tables).await?;
    retained_audit["post_formal_upgrade"] = serde_json::to_value(&retained_upgraded)?;
    retained_audit["upgrade_difference"] =
        fingerprints::retained_difference(&retained_before, &retained_upgraded);
    std::fs::write(&audit_path, serde_json::to_vec_pretty(&retained_audit)?)?;
    let queue_after = fingerprints::refresh_queue(&pool).await?;
    let queue_transition = if input.finite_manifest {
        ensure!(
            queue_after == queue_before,
            "finite sample formal upgrade changed trace queue"
        );
        json!({"verified":true,"unchanged":true,"row_count":queue_after.len()})
    } else {
        fingerprints::verify_upgrade_queue(
            &pool,
            &input.runs,
            &queue_before,
            &queue_after,
            upgrade_started,
            upgrade_finished,
        )
        .await?
    };
    ensure!(
        retained_before.len() == retained_upgraded.len()
            && retained_before
                .iter()
                .all(|(name, rows)| name == "application_run_trace_refresh_queue"
                    || retained_upgraded.get(name) == Some(rows)),
        "formal upgrade changed retained business/source-reference rows"
    );
    retained_audit["queue_transition"] = queue_transition.clone();
    std::fs::write(&audit_path, serde_json::to_vec_pretty(&retained_audit)?)?;
    let upgrade_events = fingerprints::events(&pool, &schema, &input.runs, input.events).await?;
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
        before_readers.frames == input.frames as u64,
        "legacy reader frame count differs"
    );
    let upgraded = measurement::snapshot(&pool, &schema, &copy.domain).await?;
    let layout_before = layout_inventory(&pool, &schema, &input.runs).await?;
    *phase = "actual_mover";
    let moved = store
        .compact_retained_runtime_storage(&input.runs, 256)
        .await
        .context("actual retained-runtime mover")?;
    ensure!(
        if input.prefix_version == CURRENT_PREFIX {
            true // This sample exercises the current writer, already compact at EOF.
        } else if input.finite_manifest {
            moved.compact_directory_records > 0 && moved.raw_archive_parts_sealed > 0
        } else {
            moved.client_semantic_records_processed > 0
                && moved.native_snapshot_events > 0
                && moved.raw_archive_parts > 0
        },
        "sample mover did not transform each requested domain"
    );
    *phase = "lossless_after_mover";
    let after_events = fingerprints::events(&pool, &schema, &input.runs, input.events).await?;
    let after_readers = fingerprints::readers(&store, &input.runs)
        .await
        .context("compacted reader fingerprint")?;
    let retained_after = fingerprints::retained(&pool, &schema, &copy.tables).await?;
    retained_audit["post_mover"] = serde_json::to_value(&retained_after)?;
    retained_audit["mover_difference"] =
        fingerprints::retained_difference(&retained_upgraded, &retained_after);
    std::fs::write(&audit_path, serde_json::to_vec_pretty(&retained_audit)?)?;
    ensure!(
        after_events == source_events,
        "compaction complete original events/Native strings differ"
    );
    ensure!(
        after_readers == before_readers,
        "compaction client pages/sections/cursors/filter/focus/raw frames differ"
    );
    ensure!(
        retained_after == retained_upgraded,
        "compaction retained business/source-reference rows differ"
    );
    copy::audit(&pool, &schema, &input.runs)
        .await
        .context("post-mover FK and owner audit")?;
    let after_anchors = fingerprints::archive_anchors(&pool, &schema, &input.runs).await?;
    ensure!(
        after_anchors == source_anchors,
        "compaction changed original part anchor rows/IDs"
    );
    let layout_after = layout_inventory(&pool, &schema, &input.runs).await?;
    if input.finite_manifest {
        ensure!(
            layout_after["dense_steps"].as_i64().unwrap_or(0) > 0
                && layout_after["packed_parts"].as_i64().unwrap_or(0) > 0
                && layout_after["packed_blocks"].as_i64().unwrap_or(0) > 0,
            "finite mover did not publish dense metadata and packed directories"
        );
    }
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
            && repeated.raw_archive_parts == 0
            && repeated.raw_archive_parts_sealed == 0
            && repeated.compact_directory_records == 0,
        "mover reentry was not row-idempotent"
    );
    let repeated_events = fingerprints::events(&pool, &schema, &input.runs, input.events).await?;
    ensure!(
        repeated_events == after_events,
        "mover reentry changes original events"
    );
    let repeated_anchors = fingerprints::archive_anchors(&pool, &schema, &input.runs).await?;
    ensure!(
        repeated_anchors == after_anchors,
        "reentry changed original anchors"
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
    measurement::compact_isolated_layout(
        &latest_empty_pool,
        &latest_empty_schema,
        &copy.domain,
        &Default::default(),
    )
    .await?;
    let latest_empty =
        measurement::snapshot(&latest_empty_pool, &latest_empty_schema, &copy.domain).await?;
    *phase = "source_unchanged_audit";
    let source_counts_after = sample_counts(&pool, "public", input).await?;
    ensure!(
        source_counts_after == source_counts,
        "public source changed during replay"
    );
    let source_events_after =
        fingerprints::events(&pool, "public", &input.runs, input.events).await?;
    ensure!(
        source_events_after == source_events,
        "public source event originals changed during replay"
    );
    let source_anchors_after = fingerprints::archive_anchors(&pool, "public", &input.runs).await?;
    ensure!(
        source_anchors_after == source_anchors,
        "public original anchors changed during replay"
    );
    let compact_ab = measurement::compare(
        &before_compact,
        &prefix_empty,
        &compact_equivalent,
        &latest_empty,
    )?;
    if input.prefix_version != CURRENT_PREFIX {
        ensure!(
            compact_ab["closure_saved_bytes"]
                .as_i64()
                .context("compact savings missing")?
                > 0,
            "matched compact-layout closure gain must be positive"
        );
    }
    let receipt = json!({"already_compacted":input.prefix_version == CURRENT_PREFIX,"sample_prefix_version":input.prefix_version,"finite_manifest":input.finite_manifest,"physical_layout_inventory":{"before_mover":layout_before,"after_mover":layout_after},"compact_layout_ab":compact_ab,"status":"passed","candidate_sha":input.candidate_sha,
        "source_receipt_sha256":input.receipt_hash,"source_costs_sha256":input.costs_hash,
        "run_allowlist_sha256":hash(serde_json::to_string(&input.runs)?.as_bytes()),"source_counts":source_counts,"copied_counts":copied_counts,
        "old_receipt_core_row_values_bytes":input.old_core_row_value_bytes,
        "copy":copy,"mover":moved,"reentry":repeated,"lossless_original_events":after_events,"lossless_readers":after_readers,"lossless_archive_anchors":after_anchors,"retained_business_rows":retained_after,"formal_upgrade_trace_queue_transition":queue_transition,"retained_stage_audit":"retained-stage-audit.json",
        "physical":{"prefix_empty":prefix_empty,"latest_empty":latest_empty,"old_gross":before,"before_compact_layout":before_compact,"formal_upgrade_before_mover":upgraded,"live_after_compaction_allocated":live_after,"isolated_compact_layout_equivalent":compact_equivalent},
        "notes":["Source public tables were selected only; all writes, migrations and VACUUM FULL targeted guarded isolated schemas.",
            "Legacy core-only costs are diagnostic context. Full finite closure includes client steps, conversation/recovery/Outbox/usage/billing, directory/header/item/ref/ownership/index/TOAST costs.",
            "Gross relation totals include empty relation/index overhead. Task domain and finite FK support/default costs are separate; shared canonical parents are counted once in the isolated closure, not attributed as incremental source growth.",
            "Raw as-inserted and post-mover MVCC allocation are diagnostic. A/B uses compact before and after with separate own-empty compact baselines; current-writer samples report workload cost. No live source file reclaim, CPU/latency/WAL/replica/backup savings claim.",
            "Parsed JSON values are compared as Value hashes; this alone does not establish original numeric-token precision. Native body Strings and every raw frame byte/kind/time/sequence are hashed exactly. IDs/order/cursors/filter/focus/source-reference identities are included. No original payloads are exported."]});
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
#[ignore = "Requires explicit finite manifest or legacy 62m receipt/cost/output environment and an isolated PostgreSQL replay"]
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
