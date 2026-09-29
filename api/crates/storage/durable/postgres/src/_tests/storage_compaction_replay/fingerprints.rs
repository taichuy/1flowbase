use super::{copy::Table, ident, qualified, run_array};
use anyhow::{ensure, Context, Result};
use control_plane_contracts::ports::{
    ClientTrajectoryStep, OrchestrationRuntimeRepository, ProviderTrajectoryRepository,
    TrajectorySelection,
};
use futures_util::TryStreamExt;
use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, BTreeSet};
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

#[derive(Default)]
struct Recorder {
    digest: Sha256,
    entries: u64,
}
impl Recorder {
    fn add(&mut self, bytes: &[u8]) {
        self.digest.update((bytes.len() as u64).to_le_bytes());
        self.digest.update(bytes);
        self.entries += 1;
    }
    fn json<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let value = serde_json::to_value(value)?;
        self.add(&serde_json::to_vec(&SortedValue(&value))?);
        Ok(())
    }
    fn finish(self) -> Signature {
        Signature {
            entries: self.entries,
            sha256: format!("{:x}", self.digest.finalize()),
        }
    }
}

// JSON object member order is not a Value contract. Sort keys recursively while
// preserving every array position, number and string. Native body Strings and
// binary frames are additionally hashed directly, never parsed as JSON values.
struct SortedValue<'a>(&'a Value);
impl Serialize for SortedValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(map) => {
                let sorted: BTreeMap<_, _> = map
                    .iter()
                    .map(|(key, value)| (key, SortedValue(value)))
                    .collect();
                sorted.serialize(serializer)
            }
            Value::Array(values) => values
                .iter()
                .map(SortedValue)
                .collect::<Vec<_>>()
                .serialize(serializer),
            value => value.serialize(serializer),
        }
    }
}

#[derive(Serialize, PartialEq, Eq)]
pub(super) struct Signature {
    pub entries: u64,
    pub sha256: String,
}

#[derive(Serialize, PartialEq, Eq)]
pub(super) struct Events {
    pub ordered_event_ids_headers_original_values: Signature,
    pub exact_native_body_strings: Signature,
}

pub(super) async fn events(pool: &PgPool, schema: &str, runs: &[Uuid]) -> Result<Events> {
    // Qualifying the resolver does not qualify its internal SQL. Bind the whole
    // read-only transaction to the source/destination schema, then restore the
    // pool connection's isolated search_path automatically at transaction end.
    let mut tx = pool.begin().await?;
    sqlx::query("set transaction read only")
        .execute(&mut *tx)
        .await?;
    sqlx::query(&format!("set local search_path to {}", ident(schema)))
        .execute(&mut *tx)
        .await?;
    let sql=format!("select to_jsonb(e)-array['payload','raw_json_payloads','observation_body_content_id','observation_body_manifest_id']::text[] headers,{}.runtime_event_original_payload(e.payload,e.raw_json_payloads,e.flow_run_id) original from {} e where e.flow_run_id=any({}) order by e.flow_run_id,e.sequence,e.id",ident(schema),qualified(schema,"runtime_events"),run_array(runs));
    let mut rows = sqlx::query(&sql).fetch(&mut *tx);
    let mut originals = Recorder::default();
    let mut native = Recorder::default();
    while let Some(row) = rows.try_next().await? {
        let headers: Value = row.try_get("headers")?;
        let original: Value = row.try_get("original")?;
        originals.json(&json!({"headers":headers,"original":original}))?;
        if original["source"] == "ai_native" && original["kind"] == "model_call" {
            let body = original["body"]
                .as_str()
                .context("Native input body is not an exact String")?;
            native.json(&headers)?;
            native.add(body.as_bytes());
        }
    }
    drop(rows);
    tx.commit().await?;
    let signature = originals.finish();
    ensure!(
        signature.entries == super::EVENTS as u64,
        "complete original event count differs"
    );
    Ok(Events {
        ordered_event_ids_headers_original_values: signature,
        exact_native_body_strings: native.finish(),
    })
}

pub(super) async fn retained(
    pool: &PgPool,
    schema: &str,
    tables: &[Table],
) -> Result<BTreeMap<String, Signature>> {
    let mut result = BTreeMap::new();
    for table in tables {
        if matches!(
            table.name.as_str(),
            "runtime_events"
                | "client_trajectory_steps"
                | "client_trajectory_sections"
                | "client_trajectory_archive_parts"
                | "runtime_canonical_contents"
                | "runtime_observation_body_ownership"
        ) {
            continue;
        }
        let columns = table
            .columns
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "select to_jsonb(v) value from (select {columns} from {}) v",
            qualified(schema, &table.name)
        );
        let mut stream = sqlx::query(&sql).fetch(pool);
        let mut hashes = Vec::new();
        while let Some(row) = stream.try_next().await? {
            let value: Value = row.try_get("value")?;
            hashes.push(Sha256::digest(serde_json::to_vec(&SortedValue(&value))?).to_vec());
        }
        // Tables without a primary key still compare as a full row multiset;
        // physical tuple order and VACUUM placement have no semantic meaning.
        hashes.sort();
        let mut recorder = Recorder::default();
        for value in hashes {
            recorder.add(&value);
        }
        result.insert(table.name.clone(), recorder.finish());
    }
    Ok(result)
}

#[derive(Serialize, PartialEq, Eq)]
pub(super) struct Readers {
    pub frames: u64,
    pub client_step_pages: Signature,
    pub node_filtered_pages: Signature,
    pub section_values_ids_and_cursors: Signature,
    pub raw_section_values_ids_and_cursors: Signature,
    pub request_focus_pages: Signature,
    pub frame_bytes_kind_time_sequence: Signature,
    pub source_directory_references: Signature,
}

async fn pages(
    store: &PgControlPlaneStore,
    flow: Uuid,
    node: Option<Uuid>,
    recorder: &mut Recorder,
) -> Result<Vec<ClientTrajectoryStep>> {
    let mut cursor = None;
    let mut steps = Vec::new();
    let mut seen = BTreeSet::new();
    loop {
        let page = store.client_trajectory_page(flow, node, cursor, 31).await?;
        recorder.json(&json!({"flow":flow,"node":node,"input_cursor":cursor,"page":page}))?;
        for step in &page.items {
            ensure!(
                seen.insert(step.id),
                "client cursor duplicated a step identity"
            );
        }
        steps.extend(page.items);
        match page.next_cursor {
            Some(next) => {
                ensure!(next > cursor.unwrap_or(0), "client cursor did not advance");
                cursor = Some(next);
            }
            None => break,
        }
    }
    Ok(steps)
}

async fn section(
    store: &PgControlPlaneStore,
    flow: Uuid,
    step: Uuid,
    name: &str,
    recorder: &mut Recorder,
) -> Result<()> {
    let mut cursor = None;
    loop {
        let page = store
            .client_trajectory_section(flow, None, step, name, cursor, 17)
            .await?;
        recorder.json(
            &json!({"flow":flow,"step":step,"section":name,"input_cursor":cursor,"page":page}),
        )?;
        let next = page.as_ref().and_then(|page| page.next_cursor);
        match next {
            Some(next) => {
                ensure!(next > cursor.unwrap_or(0), "section cursor did not advance");
                cursor = Some(next);
            }
            None => break,
        }
    }
    Ok(())
}

pub(super) async fn readers(store: &PgControlPlaneStore, runs: &[Uuid]) -> Result<Readers> {
    let mut step_pages = Recorder::default();
    let mut node_pages = Recorder::default();
    let mut sections = Recorder::default();
    let mut raw_sections = Recorder::default();
    let mut focus = Recorder::default();
    let mut frames = Recorder::default();
    let mut references = Recorder::default();
    let mut frame_count = 0;
    for flow in runs {
        let steps = pages(store, *flow, None, &mut step_pages).await?;
        let expected_ids: Vec<Uuid> = sqlx::query_scalar(
            "select id from client_trajectory_steps where flow_run_id=$1 order by id",
        )
        .bind(flow)
        .fetch_all(store.pool())
        .await?;
        let mut actual_ids: Vec<_> = steps.iter().map(|s| s.id).collect();
        actual_ids.sort();
        ensure!(
            actual_ids == expected_ids,
            "client page did not enumerate every original step identity"
        );
        let nodes: Vec<Uuid> =
            sqlx::query_scalar("select id from node_runs where flow_run_id=$1 order by id")
                .bind(flow)
                .fetch_all(store.pool())
                .await?;
        for node in nodes {
            pages(store, *flow, Some(node), &mut node_pages).await?;
        }
        for step in &steps {
            let mut names = step.available_sections.clone();
            names.sort();
            names.dedup();
            for name in names {
                if name != "raw" {
                    section(store, *flow, step.id, &name, &mut sections).await?;
                }
            }
        }
        let requests:Vec<Uuid>=sqlx::query_scalar("select request_id from client_trajectory_archive_heads where flow_run_id=$1 order by request_id")
            .bind(flow).fetch_all(store.pool()).await?;
        for request in requests {
            let mut cursor = 0;
            loop {
                let page = store
                    .read_client_trajectory_archive(request, cursor, 128)
                    .await?;
                if page.is_empty() {
                    break;
                }
                for frame in page {
                    ensure!(frame.sequence > cursor, "raw frame order/cursor invalid");
                    frames.json(&json!({"flow":flow,"request":request,"sequence":frame.sequence,"kind":frame.kind,"observed_at":frame.observed_at}))?;
                    frames.add(&frame.bytes);
                    frame_count += 1;
                    cursor = frame.sequence;
                }
            }
            // Raw is capture-scoped and shared by each Step. Hash the actual
            // public raw section once per request, rather than replaying the
            // same 85k frames for every repeated semantic history Step.
            let root = steps
                .iter()
                .find(|s| s.id == request)
                .or_else(|| steps.iter().find(|s| s.request_id == request))
                .context("archive request has no public step")?;
            section(store, *flow, root.id, "raw", &mut raw_sections).await?;
            let selected = store
                .client_trajectory_filtered_page(
                    *flow,
                    None,
                    None,
                    3,
                    TrajectorySelection {
                        request_id: Some(request),
                        target_id: Some(root.id),
                    },
                )
                .await?;
            ensure!(
                selected.items.first().map(|s| s.id) == Some(root.id),
                "request focus locator did not resolve original identity"
            );
            focus.json(&json!({"request":request,"target":root.id,"page":selected}))?;
        }
    }
    // Occurrence/source identities stay independently addressable even when
    // their payload storage switches from event bodies to immutable locators.
    for sql in [
        "select jsonb_build_object('id',id,'event_id',event_id,'request_id',request_id,'flow_run_id',flow_run_id,'node_run_id',node_run_id,'event_sequence',event_sequence) value from client_trajectory_steps where flow_run_id=any($1) order by flow_run_id,event_sequence,id",
        "select jsonb_build_object('id',id,'event_id',event_id,'request_id',request_id,'step_id',step_id,'flow_run_id',flow_run_id,'node_run_id',node_run_id,'section',section,'event_sequence',event_sequence) value from client_trajectory_sections where flow_run_id=any($1) order by flow_run_id,event_sequence,id",
        "select to_jsonb(c) value from client_trajectory_captures c where flow_run_id=any($1) order by flow_run_id,request_id",
        "select to_jsonb(h) value from client_trajectory_archive_heads h where flow_run_id=any($1) order by flow_run_id,request_id",
        "select to_jsonb(l) value from client_trajectory_node_links l where flow_run_id=any($1) order by flow_run_id,request_id,node_run_id",
    ] {
        let mut rows=sqlx::query(sql).bind(runs).fetch(store.pool());
        while let Some(row)=rows.try_next().await? { references.json(&row.try_get::<Value,_>("value")?)?; }
    }
    Ok(Readers {
        frames: frame_count,
        client_step_pages: step_pages.finish(),
        node_filtered_pages: node_pages.finish(),
        section_values_ids_and_cursors: sections.finish(),
        raw_section_values_ids_and_cursors: raw_sections.finish(),
        request_focus_pages: focus.finish(),
        frame_bytes_kind_time_sequence: frames.finish(),
        source_directory_references: references.finish(),
    })
}
