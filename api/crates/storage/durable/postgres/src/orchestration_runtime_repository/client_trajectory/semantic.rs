//! Narrow client occurrences; all content sharing is proved on restored Values.
use super::*;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgRow;

pub(super) fn step_metadata(body: Value, restored: bool) -> Result<Value> {
    if restored {
        return Ok(body);
    }
    body.get("fact")
        .and_then(|fact| fact.get("step"))
        .cloned()
        .ok_or_else(|| anyhow!("client legacy step metadata unavailable"))
}

fn contains_nul(value: &Value) -> bool {
    match value {
        Value::String(value) => value.contains('\0'),
        Value::Array(items) => items.iter().any(contains_nul),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| key.contains('\0') || contains_nul(value)),
        _ => false,
    }
}

pub(super) async fn write_step(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &AppendClientTrajectoryInput,
    step: &ClientTrajectoryStep,
    sequence: i64,
) -> Result<()> {
    let metadata = serde_json::to_value(step)?;
    let (projection, mut originals) = lossless_json_columns("metadata", &metadata);
    let (observed_at, at_originals) =
        lossless_json_columns("observed_at", &json!(input.observed_at));
    originals
        .as_object_mut()
        .expect("originals object")
        .extend(at_originals.as_object().expect("originals object").clone());
    sqlx::query(r#"
        insert into client_trajectory_steps(id,request_id,flow_run_id,node_run_id,event_sequence,metadata,
            raw_json_payloads,observed_at,semantic_metadata_restored)
        values($1,$2,$3,$4,$5,$6,$7,($8::jsonb->>0),true)
        on conflict(id) do update set metadata=excluded.metadata,raw_json_payloads=excluded.raw_json_payloads,
            observed_at=excluded.observed_at,semantic_metadata_restored=true
        where client_trajectory_steps.request_id=excluded.request_id
            and client_trajectory_steps.flow_run_id=excluded.flow_run_id
            and client_trajectory_steps.node_run_id is not distinct from excluded.node_run_id
    "#).bind(step.id).bind(input.request_id).bind(input.flow_run_id).bind(input.node_run_id)
        .bind(sequence).bind(projection).bind(originals)
        .bind(json!([observed_at])).execute(&mut **tx).await?;
    Ok(())
}

fn value_identity(value: &Value) -> Result<(String, i64)> {
    let mut bytes = Vec::new();
    write_canonical_runtime_json(value, &mut bytes)?;
    Ok((
        format!("sha256:{:x}", Sha256::digest(&bytes)),
        i64::try_from(bytes.len())?,
    ))
}

fn locate<'a>(body: &'a Value, path: &[String]) -> Result<&'a Value> {
    // Limit locators to the actual protocol fields emitted by classification.
    // Other shapes simply own a canonical body with the empty path.
    anyhow::ensure!(
        path.is_empty()
            || path.len() == 1
                && ["arguments", "input", "output", "content", "summary"]
                    .contains(&path[0].as_str()),
        "client section locator invalid"
    );
    if path.is_empty() {
        Ok(body)
    } else {
        body.get(&path[0])
            .ok_or_else(|| anyhow!("client section locator unavailable"))
    }
}

async fn shared_overview_child(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &AppendClientTrajectoryInput,
    step_id: Uuid,
    section: &str,
    value: &Value,
    historical: bool,
) -> Result<Option<(Uuid, Vec<String>)>> {
    let keys: &[&str] = match section {
        "parameters" => &["arguments", "input"],
        "result" => &["output", "content", "summary"],
        _ => return Ok(None),
    };
    // Restore the entire overview before comparing its child. Never use SQL
    // JSONB equality or a hash as proof of the original section value.
    let rows = sqlx::query(r#"
        select p.content_id,p.content_path,runtime_original_json(c.content,c.raw_json_payloads,'content') as body
        from client_trajectory_sections p join flow_runs f on f.id=p.flow_run_id
        join runtime_canonical_contents c on c.id=p.content_id and c.scope_id=f.scope_id and c.application_id=f.application_id
        where p.flow_run_id=$1 and p.request_id=$2 and p.step_id=$3 and p.section='overview' and p.body_kind='content'
        order by p.event_sequence desc
    "#).bind(input.flow_run_id).bind(input.request_id).bind(step_id).fetch_all(&mut **tx).await?;
    for row in rows {
        let body: Value = row.get("body");
        // Historical event SQL reconstruction cannot extract from an overview
        // containing NUL elsewhere. Keep its ordinary section independently.
        if historical && contains_nul(&body) {
            continue;
        }
        let path: Vec<String> = row.get("content_path");
        // Overviews are full immutable bodies, never another child locator.
        anyhow::ensure!(path.is_empty(), "client overview locator invalid");
        for key in keys {
            if body.get(*key).is_some_and(|candidate| candidate == value) {
                return Ok(Some((row.get("content_id"), vec![(*key).to_owned()])));
            }
        }
    }
    Ok(None)
}

pub(super) async fn write_section(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    input: &AppendClientTrajectoryInput,
    step_id: Uuid,
    section: &str,
    value: &Value,
    sequence: i64,
    historical_id: Option<Uuid>,
) -> Result<Uuid> {
    let id = historical_id.unwrap_or_else(Uuid::now_v7);
    let (value_hash, value_byte_size) = value_identity(value)?;
    let mut content_id = None;
    let mut content_path = Vec::<String>::new();
    let body_kind = if section == "timing" && *value == json!({"observed_at":input.observed_at}) {
        "timing"
    } else {
        if let Some((id, path)) =
            shared_overview_child(tx, input, step_id, section, value, historical_id.is_some())
                .await?
        {
            content_id = Some(id);
            content_path = path;
        } else {
            let (scope_id, application_id) = sqlx::query_as::<_, (Uuid, Uuid)>(
                "select scope_id,application_id from flow_runs where id=$1",
            )
            .bind(input.flow_run_id)
            .fetch_one(&mut **tx)
            .await?;
            let (id, _, _, created) =
                put_canonical_runtime_content_with_creation(tx, scope_id, application_id, value)
                    .await?;
            if created {
                sqlx::query(
                    "insert into runtime_observation_body_ownership(content_id) values($1)",
                )
                .bind(id)
                .execute(&mut **tx)
                .await?;
            }
            content_id = Some(id);
        }
        "content"
    };
    let (observed_at, originals) = lossless_json_columns("observed_at", &json!(input.observed_at));
    sqlx::query(r#"
        insert into client_trajectory_sections(id,event_id,request_id,step_id,flow_run_id,node_run_id,section,event_sequence,
            content_id,content_path,body_kind,observed_at,raw_json_payloads,value_hash,value_byte_size)
        values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,($12::jsonb#>>'{}'),$13,$14,$15)
        on conflict(id) do update set content_id=excluded.content_id,content_path=excluded.content_path,
            body_kind=excluded.body_kind,observed_at=excluded.observed_at,raw_json_payloads=excluded.raw_json_payloads,
            value_hash=excluded.value_hash,value_byte_size=excluded.value_byte_size
        where client_trajectory_sections.event_id=excluded.event_id
            and client_trajectory_sections.flow_run_id=excluded.flow_run_id
            and client_trajectory_sections.request_id=excluded.request_id
            and client_trajectory_sections.step_id=excluded.step_id
            and client_trajectory_sections.section=excluded.section
            and client_trajectory_sections.event_sequence=excluded.event_sequence
    "#).bind(id).bind(historical_id).bind(input.request_id).bind(step_id).bind(input.flow_run_id).bind(input.node_run_id)
        .bind(section).bind(sequence).bind(content_id).bind(&content_path).bind(body_kind).bind(observed_at).bind(originals)
        .bind(value_hash).bind(value_byte_size).execute(&mut **tx).await?;
    Ok(id)
}

const SECTION_ROWS: &str = r#"
    select p.event_sequence,p.body_kind,p.content_path,p.value_hash,p.value_byte_size,
        runtime_original_json(to_jsonb(p.observed_at),p.raw_json_payloads,'observed_at') as observed_at,
        case when p.body_kind='legacy' then runtime_event_original_payload(e.payload,e.raw_json_payloads,e.flow_run_id) end as legacy_payload,
        case when p.body_kind='content' then runtime_original_json(c.content,c.raw_json_payloads,'content') end as body
    from client_trajectory_sections p join flow_runs f on f.id=p.flow_run_id
    left join runtime_events e on e.id=p.event_id and e.flow_run_id=p.flow_run_id and p.body_kind='legacy'
    left join runtime_canonical_contents c on c.id=p.content_id and c.scope_id=f.scope_id and c.application_id=f.application_id
    where p.flow_run_id=$1 and p.request_id=$2 and p.step_id=$3 and p.section=$4 and p.event_sequence>$5
    order by p.event_sequence limit $6
"#;

pub(super) async fn section_rows(
    pool: &sqlx::PgPool,
    flow_run_id: Uuid,
    request_id: Uuid,
    step_id: Uuid,
    section: &str,
    cursor: i64,
    limit: i64,
) -> Result<Vec<PgRow>> {
    Ok(sqlx::query(SECTION_ROWS)
        .bind(flow_run_id)
        .bind(request_id)
        .bind(step_id)
        .bind(section)
        .bind(cursor)
        .bind(limit)
        .fetch_all(pool)
        .await?)
}

pub(super) fn restore_section(row: &PgRow) -> Result<Value> {
    let body_kind: String = row.try_get("body_kind")?;
    let value = match body_kind.as_str() {
        "legacy" => {
            // @field-contract-compat source=runtime_events.payload.fact.value alias=legacy_client_section remove_by=2026-10-31
            // Remaining old events are actual historical anchors, read until verified migration.
            let payload: Value = row.try_get("legacy_payload")?;
            payload
                .get("fact")
                .and_then(|fact| fact.get("value"))
                .ok_or_else(|| anyhow!("client legacy section value unavailable"))?
                .clone()
        }
        "timing" => json!({"observed_at": row.try_get::<Value, _>("observed_at")?}),
        "content" => {
            let body: Value = row.try_get("body")?;
            let path: Vec<String> = row.try_get("content_path")?;
            locate(&body, &path)?.clone()
        }
        _ => return Err(anyhow!("client section body kind invalid")),
    };
    if body_kind != "legacy" {
        let expected_hash: String = row.try_get("value_hash")?;
        let expected_size: i64 = row.try_get("value_byte_size")?;
        let (hash, size) = value_identity(&value)?;
        anyhow::ensure!(
            hash == expected_hash && size == expected_size,
            "client section value integrity mismatch"
        );
    }
    Ok(value)
}

impl PgControlPlaneStore {
    /// One technical batch from an explicit run allowlist. Repeat until zero.
    /// The returned count includes reviewed NUL rows retained in the old layout.
    /// Integrity/scope failures roll back without modifying old anchors/bodies.
    pub(crate) async fn migrate_client_trajectory_semantic_history(
        &self,
        run_ids: &[Uuid],
        batch_size: i64,
    ) -> Result<u64> {
        anyhow::ensure!(
            batch_size > 0,
            "client semantic migration batch size invalid"
        );
        if run_ids.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        // Match all append writers' flow lock order, including terminal observations.
        let mut ordered_runs = run_ids.to_vec();
        ordered_runs.sort_unstable();
        ordered_runs.dedup();
        for run in &ordered_runs {
            let exists: Option<Uuid> =
                sqlx::query_scalar("select id from flow_runs where id=$1 for no key update")
                    .bind(run)
                    .fetch_optional(&mut *tx)
                    .await?;
            anyhow::ensure!(
                exists.is_some(),
                "client semantic migration run unavailable"
            );
        }
        let steps = sqlx::query(r#"
            select s.id,s.request_id,s.flow_run_id,s.node_run_id,e.event_type,e.flow_run_id as event_flow_run_id,
                e.node_run_id as event_node_run_id,runtime_original_json(e.payload,e.raw_json_payloads,'payload') as payload
            from client_trajectory_steps s join runtime_events e on e.id=s.event_id
            where s.flow_run_id=any($1) and not s.semantic_metadata_restored
            order by s.flow_run_id,s.event_sequence,s.id limit $2
        "#).bind(&ordered_runs).bind(batch_size).fetch_all(&mut *tx).await?;
        let mut migrated = u64::try_from(steps.len())?;
        for row in steps {
            let payload: Value = row.try_get("payload")?;
            let input: AppendClientTrajectoryInput = serde_json::from_value(payload.clone())?;
            validate_legacy_scope(&row, &input)?;
            let ClientTrajectoryFact::Step { step } = &input.fact else {
                return Err(anyhow!("client historical step fact mismatch"));
            };
            anyhow::ensure!(
                step.id == row.get::<Uuid, _>("id")
                    && step.request_id == input.request_id
                    && step.flow_run_id == input.flow_run_id
                    && step.node_run_id == input.node_run_id,
                "client historical step scope mismatch"
            );
            let (metadata, mut originals) =
                lossless_json_columns("metadata", &payload["fact"]["step"]);
            let (_, at_originals) = lossless_json_columns("observed_at", &json!(input.observed_at));
            originals
                .as_object_mut()
                .expect("originals object")
                .extend(at_originals.as_object().expect("originals object").clone());
            sqlx::query("update client_trajectory_steps set metadata=$2,raw_json_payloads=$3,observed_at=($4::jsonb->>0),semantic_metadata_restored=true where id=$1")
                .bind(step.id).bind(metadata).bind(originals).bind(lossless_text_parameter(&input.observed_at))
                .execute(&mut *tx).await?;
        }
        let remaining = batch_size - i64::try_from(migrated)?;
        let sections = sqlx::query(r#"
            select p.id,p.event_id,p.request_id,p.step_id,p.section,p.event_sequence,p.flow_run_id,p.node_run_id,e.sequence as anchor_sequence,
                e.event_type,e.flow_run_id as event_flow_run_id,e.node_run_id as event_node_run_id,
                runtime_original_json(e.payload,e.raw_json_payloads,'payload') as payload
            from client_trajectory_sections p join runtime_events e on e.id=p.event_id
            where p.flow_run_id=any($1) and p.body_kind='legacy' and not p.semantic_legacy_retained and p.section<>'raw'
            order by p.flow_run_id,p.event_sequence,p.id limit $2
        "#).bind(&ordered_runs).bind(remaining).fetch_all(&mut *tx).await?;
        migrated += u64::try_from(sections.len())?;
        for row in sections {
            let original: Value = row.try_get("payload")?;
            let input: AppendClientTrajectoryInput = serde_json::from_value(original.clone())?;
            validate_legacy_scope(&row, &input)?;
            if contains_nul(&original) {
                // Preserve unsupported historical originals and move past them.
                // This marker only controls migration; the legacy reader remains.
                sqlx::query("update client_trajectory_sections set semantic_legacy_retained=true where id=$1")
                    .bind(row.get::<Uuid,_>("id")).execute(&mut *tx).await?;
                continue;
            }
            let ClientTrajectoryFact::Section {
                step_id,
                section,
                value,
            } = &input.fact
            else {
                return Err(anyhow!("client historical section fact mismatch"));
            };
            anyhow::ensure!(
                *step_id == row.get::<Uuid, _>("step_id")
                    && *section == row.get::<String, _>("section"),
                "client historical section identity mismatch"
            );
            let valid: bool = sqlx::query_scalar("select exists(select 1 from client_trajectory_steps where id=$1 and request_id=$2 and flow_run_id=$3)")
                .bind(step_id).bind(input.request_id).bind(input.flow_run_id).fetch_one(&mut *tx).await?;
            anyhow::ensure!(valid, "client historical section step unavailable");
            let id: Uuid = row.get("id");
            anyhow::ensure!(
                id == row.get::<Uuid, _>("event_id")
                    && row.get::<i64, _>("event_sequence") == row.get::<i64, _>("anchor_sequence"),
                "client historical section anchor mismatch"
            );
            write_section(
                &mut tx,
                &input,
                *step_id,
                section,
                value,
                row.get("event_sequence"),
                Some(id),
            )
            .await?;
            let restored = sqlx::query(SECTION_ROWS)
                .bind(input.flow_run_id)
                .bind(input.request_id)
                .bind(step_id)
                .bind(section)
                .bind(row.get::<i64, _>("event_sequence") - 1)
                .bind(1_i64)
                .fetch_one(&mut *tx)
                .await?;
            anyhow::ensure!(
                restore_section(&restored)? == *value,
                "client historical section value mismatch"
            );
            let mut compact = original.clone();
            compact["fact"]
                .as_object_mut()
                .ok_or_else(|| anyhow!("client historical fact unavailable"))?
                .remove("value");
            compact
                .as_object_mut()
                .ok_or_else(|| anyhow!("client historical payload unavailable"))?
                .insert("_client_semantic_ref".into(), json!({"section_id":id}));
            let (projection, originals) = lossless_json_columns("payload", &compact);
            sqlx::query("update runtime_events set payload=$2,raw_json_payloads=$3 where id=$1")
                .bind(id)
                .bind(projection)
                .bind(originals)
                .execute(&mut *tx)
                .await?;
            let replayed: Value = sqlx::query_scalar("select client_trajectory_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
                .bind(id).fetch_one(&mut *tx).await?;
            anyhow::ensure!(
                replayed == original,
                "client historical event complete payload mismatch"
            );
        }
        tx.commit().await?;
        Ok(migrated)
    }
}

fn validate_legacy_scope(row: &PgRow, input: &AppendClientTrajectoryInput) -> Result<()> {
    anyhow::ensure!(
        row.get::<String, _>("event_type") == "client_protocol_trajectory"
            && input.request_id == row.get::<Uuid, _>("request_id")
            && input.flow_run_id == row.get::<Uuid, _>("flow_run_id")
            && input.flow_run_id == row.get::<Uuid, _>("event_flow_run_id")
            && input.node_run_id == row.get::<Option<Uuid>, _>("node_run_id")
            && input.node_run_id == row.get::<Option<Uuid>, _>("event_node_run_id"),
        "client historical event scope mismatch"
    );
    Ok(())
}
