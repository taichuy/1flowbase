use super::*;
mod native_body;
use control_plane_contracts::ports::{
    ApplicationRunPayloadSection, ProviderTrajectoryBody, ProviderTrajectoryEvidence,
    ProviderTrajectoryPage, ProviderTrajectoryRepository, ProviderTrajectoryStep,
    ProviderTrajectoryView,
};

#[async_trait]
impl ProviderTrajectoryRepository for PgControlPlaneStore {
    async fn provider_run_trajectory_page(
        &self,
        flow_run_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ProviderTrajectoryPage> {
        self.trajectory_page_for_scope(flow_run_id, None, cursor, limit)
            .await
    }
    async fn application_run_payload(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
        section: ApplicationRunPayloadSection,
    ) -> Result<Option<Value>> {
        let sql = match section {
            ApplicationRunPayloadSection::InputPayload => "select runtime_original_json(input_payload,raw_json_payloads,'input_payload') from flow_runs where application_id=$1 and id=$2 and (import_job_id is null or exists (select 1 from run_archive_import_jobs j where j.id=flow_runs.import_job_id and j.status='succeeded'))",
            ApplicationRunPayloadSection::OutputPayload => "select runtime_original_json(output_payload,raw_json_payloads,'output_payload') from flow_runs where application_id=$1 and id=$2 and (import_job_id is null or exists (select 1 from run_archive_import_jobs j where j.id=flow_runs.import_job_id and j.status='succeeded'))",
        };
        Ok(sqlx::query_scalar(sql)
            .bind(application_id)
            .bind(flow_run_id)
            .fetch_optional(self.pool())
            .await?)
    }
    async fn provider_trajectory_page(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ProviderTrajectoryPage> {
        self.trajectory_page_for_scope(flow_run_id, Some(node_run_id), cursor, limit)
            .await
    }

    async fn provider_trajectory_body(
        &self,
        flow_run_id: Uuid,
        node_run_id: Uuid,
        event_id: Uuid,
        cursor: Option<i64>,
        limit: i64,
        view: ProviderTrajectoryView,
    ) -> Result<Option<ProviderTrajectoryBody>> {
        // Resolve only the scoped index. Semantic details never read raw evidence.
        let scope = sqlx::query(
            r#"
            select invocation_id,provider_attempt_index,event_sequence,metadata,body_event_id,
                coalesce(metadata->>'source','supplier_protocol') as source,false as raw
            from provider_semantic_trajectory_steps
            where flow_run_id=$1 and node_run_id=$2 and event_id=$3
            union all
            select metadata->>'invocation_id',(metadata->>'provider_attempt_index')::bigint,
                event_sequence,metadata,event_id,'supplier_protocol',true
            from provider_protocol_trajectory_events
            where flow_run_id=$1 and node_run_id=$2 and event_id=$3
                and event_type='provider_protocol_observation' and $4
            limit 1
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(event_id)
        .bind(view == ProviderTrajectoryView::Protocol)
        .fetch_optional(self.pool())
        .await?;
        let Some(scope) = scope else { return Ok(None) };
        let source: String = scope.get("source");
        let native = source == "ai_native";
        let metadata: Value = scope.get("metadata");
        if view == ProviderTrajectoryView::Semantic {
            let sequence: i64 = scope.get("event_sequence");
            let items = if cursor.is_some_and(|cursor| cursor >= sequence) {
                vec![]
            } else {
                let body = if native {
                    let payload: Value = sqlx::query_scalar(
                        "select runtime_original_json(payload,raw_json_payloads,'payload') from runtime_events where id=$1 and flow_run_id=$2 and node_run_id=$3"
                    ).bind(scope.get::<Uuid,_>("body_event_id")).bind(flow_run_id).bind(node_run_id)
                        .fetch_one(self.pool()).await?;
                    self.native_trajectory_body(flow_run_id, node_run_id, &payload)
                        .await?
                } else {
                    let mut historical = metadata.clone();
                    historical["source"] = Value::String(source.clone());
                    serde_json::to_string(&historical)?
                };
                vec![ProviderTrajectoryEvidence {
                    event_id,
                    sequence,
                    body,
                    encoding: "utf8".into(),
                }]
            };
            return Ok(Some(ProviderTrajectoryBody {
                event_id,
                source,
                evidence_scope: "step".into(),
                items,
                next_cursor: None,
            }));
        }
        let raw: bool = scope.get("raw");
        let first = if native {
            None
        } else {
            metadata
                .get(if raw {
                    "sequence"
                } else {
                    "raw_sequence_start"
                })
                .and_then(Value::as_i64)
        };
        let last = if native {
            None
        } else {
            metadata
                .get(if raw { "sequence" } else { "raw_sequence_end" })
                .and_then(Value::as_i64)
        };
        let limit = limit.clamp(1, 16);
        let rows = sqlx::query(
            r#"
            select p.event_id,(p.metadata->>'sequence')::bigint as sequence,
                runtime_original_json(e.payload,e.raw_json_payloads,'payload') as payload
            from provider_protocol_trajectory_events p join runtime_events e on e.id=p.event_id
            where p.flow_run_id=$1 and p.node_run_id=$2
                and p.event_type='provider_protocol_observation'
                and p.metadata->>'invocation_id'=$3
                and (p.metadata->>'provider_attempt_index')::bigint=$4
                and ($5::bigint is null or (p.metadata->>'sequence')::bigint >= $5)
                and ($6::bigint is null or (p.metadata->>'sequence')::bigint <= $6)
                and (p.metadata->>'sequence')::bigint > $7
            order by (p.metadata->>'sequence')::bigint limit $8
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(scope.get::<String, _>("invocation_id"))
        .bind(scope.get::<i64, _>("provider_attempt_index"))
        .bind(if native {
            None
        } else {
            Some(first.unwrap_or(1))
        })
        .bind(if native {
            None
        } else {
            Some(last.unwrap_or(0))
        })
        .bind(cursor.unwrap_or(0))
        .bind(limit + 1)
        .fetch_all(self.pool())
        .await?;
        let has_more = rows.len() > limit as usize;
        let items = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let payload: Value = row.get("payload");
                Ok(ProviderTrajectoryEvidence {
                    event_id: row.get("event_id"),
                    sequence: row.get("sequence"),
                    body: payload
                        .get("body")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("protocol observation body missing"))?
                        .into(),
                    encoding: payload
                        .get("encoding")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("protocol observation encoding missing"))?
                        .into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next_cursor = if has_more {
            items.last().map(|item| item.sequence)
        } else {
            None
        };
        Ok(Some(ProviderTrajectoryBody {
            event_id,
            source: "supplier_protocol".into(),
            evidence_scope: if native { "invocation" } else { "step" }.into(),
            items,
            next_cursor,
        }))
    }
}

impl PgControlPlaneStore {
    async fn trajectory_page_for_scope(
        &self,
        flow_run_id: Uuid,
        node_run_id: Option<Uuid>,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ProviderTrajectoryPage> {
        let limit = limit.clamp(1, 100);
        let counts = sqlx::query(include_str!("integrity.sql"))
            .bind(flow_run_id)
            .bind(node_run_id)
            .fetch_one(self.pool())
            .await?;
        let rows = sqlx::query(
            r#"
            select event_id,event_sequence,'provider_semantic_step' as event_type,
                metadata || jsonb_build_object('source',coalesce(metadata->>'source','supplier_protocol'),'flow_run_id',flow_run_id,'node_run_id',node_run_id) as metadata,created_at
            from provider_semantic_trajectory_steps
            where flow_run_id=$1 and ($2::uuid is null or node_run_id=$2) and event_sequence > $3
            order by event_sequence asc limit $4
        "#,
        )
        .bind(flow_run_id)
        .bind(node_run_id)
        .bind(cursor.unwrap_or(0))
        .bind(limit + 1)
        .fetch_all(self.pool())
        .await?;
        let has_more = rows.len() > limit as usize;
        let items: Vec<ProviderTrajectoryStep> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ProviderTrajectoryStep {
                    event_id: row.get("event_id"),
                    event_sequence: row.get("event_sequence"),
                    event_type: row.get("event_type"),
                    metadata: row.get("metadata"),
                    created_at: row
                        .get::<OffsetDateTime, _>("created_at")
                        .format(&time::format_description::well_known::Rfc3339)?,
                })
            })
            .collect::<Result<_>>()?;
        let next_cursor = if has_more {
            items.last().map(|item| item.event_sequence)
        } else {
            None
        };
        Ok(ProviderTrajectoryPage {
            items,
            next_cursor,
            observation_count: counts.get("observation_count"),
            persist_failed_count: counts.get("persist_failed_count"),
            integrity: counts.get("integrity"),
            protocol_integrity: counts.get("protocol_integrity"),
            protocol_persist_failed_count: counts.get("protocol_persist_failed_count"),
        })
    }
}
