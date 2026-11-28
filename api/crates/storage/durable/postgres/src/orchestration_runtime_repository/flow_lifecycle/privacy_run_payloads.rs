// Content fields, not event types, define the legacy event rewrite. Run start,
// node observations, previews and resume events can all duplicate user input.
const PRIVACY_EVENT_CONTENT_FIELDS: &[&str] = &[
    "input_payload",
    "output_payload",
    "error_payload",
    "debug_payload",
    "response_payload",
    "query",
    "prompt",
    "resolved_inputs",
    "rendered_templates",
    "node_output",
];

impl PgControlPlaneStore {
    pub(crate) async fn rewrite_privacy_run_payloads(
        &self,
        workspace_id: Uuid,
        flow_run_id: Uuid,
        mask: &control_plane_contracts::ports::PrivacyRunPayloadMask<'_>,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        lock_application_run_native_projection(&mut tx, flow_run_id).await?;
        let row = sqlx::query(
            "select application_id, title, runtime_original_json(input_payload, raw_json_payloads, 'input_payload') input_payload
             from flow_runs where id=$1 and scope_id=$2 for no key update",
        )
        .bind(flow_run_id)
        .bind(workspace_id)
        .fetch_one(&mut *tx)
        .await?;
        let application_id: Uuid = row.try_get("application_id")?;
        let original_title: String = row.try_get("title")?;
        let original_input: Value = row.try_get("input_payload")?;
        let mut input = original_input.clone();
        mask(&mut input);
        let title = mask_privacy_run_title(original_title.clone(), &original_input, &input, mask);
        let mut changed = title != original_title || input != original_input;
        if changed {
            sqlx::query(
                "update flow_runs set input_payload=($2::jsonb->0), title=$3,
                   raw_json_payloads=(raw_json_payloads-'input_payload') || jsonb_strip_nulls(jsonb_build_object('input_payload', $2::jsonb->1))
                 where id=$1",
            )
            .bind(flow_run_id)
            .bind(lossless_json_parameter(&input))
            .bind(title)
            .execute(&mut *tx)
            .await?;
        }

        // `node_runs` is a directory; its payloads and lossless sidecars live
        // in node_run_details. Existing completed non-LLM nodes can echo input
        // into their output/developer snapshots; mask those copies as well.
        let nodes = sqlx::query_as::<_, (Uuid, String, Value)>(
            "select d.node_run_id, d.section, runtime_original_json(d.payload, d.raw_json_payloads, d.section)
             from node_runs n join node_run_details d on d.node_run_id=n.id
             where n.flow_run_id=$1 and d.section in ('input_payload','debug_payload','output_payload','error_payload') and d.payload is not null
             order by d.node_run_id,d.section for update of d",
        )
        .bind(flow_run_id)
        .fetch_all(&mut *tx)
        .await?;
        for (id, section, original) in nodes {
            let mut payload = original.clone();
            mask(&mut payload);
            if payload == original {
                continue;
            }
            sqlx::query(
                "update node_run_details set payload=($3::jsonb->0),
                   raw_json_payloads=(raw_json_payloads-$2) || jsonb_strip_nulls(jsonb_build_object($2::text,$3::jsonb->1))
                 where node_run_id=$1 and section=$2",
            )
            .bind(id)
            .bind(section)
            .bind(lossless_json_parameter(&payload))
            .execute(&mut *tx)
            .await?;
            changed = true;
        }

        // Read the lossless payload, then replace both representations in the
        // same transaction. Keep event identities and envelope metadata intact.
        let legacy_events = sqlx::query_as::<_, (Uuid, Value)>(
            "select id, runtime_original_json(payload,raw_json_payloads,'payload') from flow_run_events
             where flow_run_id=$1 and payload ?| $2::text[] order by sequence for update",
        )
        .bind(flow_run_id)
        .bind(PRIVACY_EVENT_CONTENT_FIELDS)
        .fetch_all(&mut *tx)
        .await?;
        for (id, original) in legacy_events {
            let Some(payload) = masked_payload_fields(original, mask, PRIVACY_EVENT_CONTENT_FIELDS) else {
                continue;
            };
            sqlx::query(
                "update flow_run_events set payload=($3::jsonb->0),
                   raw_json_payloads=(raw_json_payloads-'payload') || jsonb_strip_nulls(jsonb_build_object('payload',$3::jsonb->1))
                 where flow_run_id=$1 and id=$2",
            ).bind(flow_run_id).bind(id).bind(lossless_json_parameter(&payload)).execute(&mut *tx).await?;
        }
        let runtime_events = sqlx::query_as::<_, (Uuid, String, Value, Option<Uuid>)>(
            "select id, event_type, runtime_event_original_payload(payload,raw_json_payloads,flow_run_id), observation_body_content_id from runtime_events
             where flow_run_id=$1 and event_type in ('node_started','node_finished','node_preview_started','node_preview_completed','node_preview_failed') order by sequence for update",
        ).bind(flow_run_id).fetch_all(&mut *tx).await?;
        let mut replaced_contents = Vec::new();
        for (id, event_type, original, old_content) in runtime_events {
            let Some(payload) = masked_payload_fields(original, mask, PRIVACY_EVENT_CONTENT_FIELDS) else {
                continue;
            };
            let (payload, content, manifest) =
                if matches!(event_type.as_str(), "node_started" | "node_finished") {
                    observation_bodies::archive_node_event_payload(&mut tx, flow_run_id, &payload)
                        .await?
                } else {
                    (payload, None, None)
                };
            sqlx::query(
                "update runtime_events set payload=($3::jsonb->0),
                   raw_json_payloads=(raw_json_payloads-'payload') || jsonb_strip_nulls(jsonb_build_object('payload',$3::jsonb->1)),
                   observation_body_content_id=$4, observation_body_manifest_id=$5
                 where flow_run_id=$1 and id=$2",
            ).bind(flow_run_id).bind(id).bind(lossless_json_parameter(&payload)).bind(content).bind(manifest).execute(&mut *tx).await?;
            if let Some(old) = old_content.filter(|old| Some(*old) != content) {
                replaced_contents.push(old);
            }
        }
        if !replaced_contents.is_empty() {
            // Only reclaim observation-owned bodies. Identical content may still
            // belong to another run or consumer; use the normal retention checks.
            let owned: Vec<Uuid> = sqlx::query_scalar(
                "select c.id from runtime_canonical_contents c join runtime_observation_body_ownership o on o.content_id=c.id
                 where c.application_id=$1 and c.id=any($2) order by c.content_hash",
            ).bind(application_id).bind(&replaced_contents).fetch_all(&mut *tx).await?;
            sqlx::query(
                "select pg_advisory_xact_lock(hashtextextended(
                   case when a.application_type='agent_logs' then 'canonical-runtime-application:'||c.application_id::text
                   else 'canonical-runtime:'||c.application_id::text||':'||c.content_hash end,0))
                 from runtime_canonical_contents c join applications a on a.id=c.application_id
                 where c.id=any($1) order by c.content_hash",
            ).bind(&owned).execute(&mut *tx).await?;
            sqlx::query(include_str!(
                "../agent_logs/deletion_jobs/reclaim_contents.sql"
            ))
            .bind(application_id)
            .bind(&owned)
            .execute(&mut *tx)
            .await?;
        }

        // A warm trace can already contain a materialized copy of node I/O.
        // Its writer takes the same flow row lock. Keep projection references,
        // counters and locators intact; rewrite only the copied content fields.
        let traces = sqlx::query_as::<_, (Uuid, Value)>(
            "select trace_node_id, runtime_original_json(payload,raw_json_payloads,'payload')
             from application_run_trace_node_contents where flow_run_id=$1 for update",
        )
        .bind(flow_run_id)
        .fetch_all(&mut *tx)
        .await?;
        for (id, original) in traces {
            let Some(payload) = masked_payload_fields(
                original,
                mask,
                &[
                    "input_payload",
                    "output_payload",
                    "error_payload",
                    "debug_payload",
                ],
            ) else {
                continue;
            };
            sqlx::query(
                "update application_run_trace_node_contents set payload=($3::jsonb->0),
                   raw_json_payloads=(raw_json_payloads-'payload') || jsonb_strip_nulls(jsonb_build_object('payload',$3::jsonb->1))
                 where flow_run_id=$1 and trace_node_id=$2",
            ).bind(flow_run_id).bind(id).bind(lossless_json_parameter(&payload)).execute(&mut *tx).await?;
        }

        if changed {
            // Assistant conversation directory rows carry identity/timestamps only.
            // Log summaries, tasks and run-conversation items ARE stored copies.
            Self::refresh_completed_output_projection(&mut tx, flow_run_id).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn masked_payload_fields(
    original: Value,
    mask: &control_plane_contracts::ports::PrivacyRunPayloadMask<'_>,
    fields: &[&str],
) -> Option<Value> {
    let mut payload = original.clone();
    for &key in fields {
        if let Some(value) = payload.get_mut(key) {
            mask(value);
        }
    }
    (payload != original).then_some(payload)
}

fn mask_privacy_run_title(
    title: String,
    original: &Value,
    masked: &Value,
    mask: &control_plane_contracts::ports::PrivacyRunPayloadMask<'_>,
) -> String {
    use control_plane_contracts::application_public_runtime::embedded_assistant_user_message;
    use control_plane_contracts::flow_run_title::{
        build_flow_run_title, derive_flow_run_title_from_input_payload, FLOW_RUN_TITLE_MAX_CHARS,
    };

    // Projection limits may already have truncated a match. Always derive a
    // generated title from the masked original, never from a truncated secret.
    if let (Some(before), Some(after)) = (
        embedded_assistant_user_message(original),
        embedded_assistant_user_message(masked),
    ) {
        if title == build_flow_run_title(None, before.content()) {
            return build_flow_run_title(None, after.content());
        }
    }
    if derive_flow_run_title_from_input_payload(original).as_deref() == Some(&title)
        || title.chars().count() >= FLOW_RUN_TITLE_MAX_CHARS
    {
        return build_flow_run_title(
            derive_flow_run_title_from_input_payload(masked).as_deref(),
            "",
        );
    }
    let mut value = Value::String(title);
    mask(&mut value);
    build_flow_run_title(value.as_str(), "")
}
