#[derive(Debug)]
struct LegacyShadowCandidate {
    source_kind: control_plane_contracts::ports::LegacyRuntimeShadowSourceKind,
    source_table: String,
    source_column: String,
    source_row_id: Uuid,
    scope_id: Uuid,
    application_id: Uuid,
    flow_run_id: Uuid,
    run_status: String,
    payload: Value,
    locator_payload: Option<Value>,
    source_created_at: OffsetDateTime,
}

fn legacy_shadow_source_rank(
    source_kind: control_plane_contracts::ports::LegacyRuntimeShadowSourceKind,
) -> i32 {
    use control_plane_contracts::ports::LegacyRuntimeShadowSourceKind;
    match source_kind {
        LegacyRuntimeShadowSourceKind::CheckpointContext => 1,
        LegacyRuntimeShadowSourceKind::CallbackRequest => 2,
        LegacyRuntimeShadowSourceKind::CallbackResponse => 3,
        LegacyRuntimeShadowSourceKind::RunEventHistory => 4,
    }
}

fn parse_legacy_shadow_source_kind(
    value: &str,
) -> Result<control_plane_contracts::ports::LegacyRuntimeShadowSourceKind> {
    use control_plane_contracts::ports::LegacyRuntimeShadowSourceKind;
    match value {
        "checkpoint_context" => Ok(LegacyRuntimeShadowSourceKind::CheckpointContext),
        "callback_request" => Ok(LegacyRuntimeShadowSourceKind::CallbackRequest),
        "callback_response" => Ok(LegacyRuntimeShadowSourceKind::CallbackResponse),
        "run_event_history" => Ok(LegacyRuntimeShadowSourceKind::RunEventHistory),
        _ => Err(anyhow!(
            "unknown legacy runtime shadow source kind: {value}"
        )),
    }
}

fn legacy_run_classification(status: &str) -> control_plane_contracts::ports::LegacyRuntimeRunClassification {
    use control_plane_contracts::ports::LegacyRuntimeRunClassification;
    match status {
        "succeeded" | "incomplete" | "failed" | "cancelled" => {
            LegacyRuntimeRunClassification::Terminal
        }
        _ => LegacyRuntimeRunClassification::Pending,
    }
}

fn accumulate_legacy_shadow_statistics(
    statistics: &mut Vec<control_plane_contracts::ports::LegacyRuntimeShadowStatistics>,
    candidate: &LegacyShadowCandidate,
    source_bytes: u64,
    canonical_bytes: u64,
    outcome: &str,
) {
    let classification = legacy_run_classification(&candidate.run_status);
    let position = statistics.iter().position(|item| {
        item.source_kind == candidate.source_kind
            && item.source_table == candidate.source_table
            && item.source_column == candidate.source_column
            && item.application_id == candidate.application_id
            && item.flow_run_id == candidate.flow_run_id
            && item.run_classification == classification
    });
    let index = position.unwrap_or_else(|| {
        statistics.push(control_plane_contracts::ports::LegacyRuntimeShadowStatistics {
            source_kind: candidate.source_kind,
            source_table: candidate.source_table.clone(),
            source_column: candidate.source_column.clone(),
            application_id: candidate.application_id,
            flow_run_id: candidate.flow_run_id,
            run_classification: classification,
            scanned_rows: 0,
            shadowed_rows: 0,
            already_shadowed_rows: 0,
            difference_rows: 0,
            source_bytes: 0,
            canonical_bytes: 0,
        });
        statistics.len() - 1
    });
    let item = &mut statistics[index];
    item.scanned_rows += 1;
    item.source_bytes += source_bytes;
    item.canonical_bytes += canonical_bytes;
    match outcome {
        "shadowed" => item.shadowed_rows += 1,
        "already_shadowed" => item.already_shadowed_rows += 1,
        _ => item.difference_rows += 1,
    }
}

impl PgControlPlaneStore {
    async fn convert_legacy_runtime_shadow_batch(
        &self,
        input: &ConvertLegacyRuntimeShadowBatchInput,
    ) -> Result<ConvertLegacyRuntimeShadowBatchResult> {
        use control_plane_contracts::ports::{
            LegacyRuntimeShadowDifference, LegacyRuntimeShadowExecution,
            LegacyRuntimeShadowSourceKind,
        };

        let limit = input.limit.clamp(1, 1_000);
        let query_limit = i64::try_from(limit + 1)?;
        let after_rank = input
            .after
            .as_ref()
            .map(|cursor| legacy_shadow_source_rank(cursor.source_kind));
        let after_created_at = input.after.as_ref().map(|cursor| cursor.created_at);
        let after_row_id = input.after.as_ref().map(|cursor| cursor.source_row_id);
        let rows = sqlx::query(
            r#"
            with candidates as (
                select 1 as source_rank, 'checkpoint_context'::text as source_kind,
                       'flow_run_checkpoints'::text as source_table,
                       'variable_snapshot'::text as source_column,
                       checkpoints.id as source_row_id, runs.scope_id, runs.application_id,
                       runs.id as flow_run_id, runs.status as run_status,
                       checkpoints.variable_snapshot as payload,
                       checkpoints.locator_payload as locator_payload,
                       checkpoints.created_at as source_created_at
                  from flow_run_checkpoints checkpoints
                  join flow_runs runs on runs.id = checkpoints.flow_run_id
                union all
                select 2, 'callback_request', 'flow_run_callback_tasks', 'request_payload',
                       tasks.id, runs.scope_id, runs.application_id, runs.id, runs.status,
                       tasks.request_payload, null::jsonb, tasks.created_at
                  from flow_run_callback_tasks tasks
                  join flow_runs runs on runs.id = tasks.flow_run_id
                union all
                select 3, 'callback_response', 'flow_run_callback_tasks', 'response_payload',
                       tasks.id, runs.scope_id, runs.application_id, runs.id, runs.status,
                       tasks.response_payload, null::jsonb, tasks.created_at
                  from flow_run_callback_tasks tasks
                  join flow_runs runs on runs.id = tasks.flow_run_id
                 where tasks.response_payload is not null
                union all
                select 4, 'run_event_history', 'flow_run_events', 'payload',
                       events.id, runs.scope_id, runs.application_id, runs.id, runs.status,
                       events.payload, null::jsonb, events.created_at
                  from flow_run_events events
                  join flow_runs runs on runs.id = events.flow_run_id
            )
            select * from candidates
             where ($1::uuid is null or application_id = $1)
               and ($2::uuid is null or flow_run_id = $2)
               and (
                   $3::integer is null
                   or (source_rank, source_created_at, source_row_id) >
                      ($3, $4::timestamptz, $5::uuid)
               )
             order by source_rank, source_created_at, source_row_id
             limit $6
            "#,
        )
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .bind(after_rank)
        .bind(after_created_at)
        .bind(after_row_id)
        .bind(query_limit)
        .fetch_all(self.pool())
        .await?;

        let has_more = rows.len() > limit;
        let candidates = rows
            .into_iter()
            .take(limit)
            .map(|row| {
                Ok(LegacyShadowCandidate {
                    source_kind: parse_legacy_shadow_source_kind(
                        row.get::<String, _>("source_kind").as_str(),
                    )?,
                    source_table: row.get("source_table"),
                    source_column: row.get("source_column"),
                    source_row_id: row.get("source_row_id"),
                    scope_id: row.get("scope_id"),
                    application_id: row.get("application_id"),
                    flow_run_id: row.get("flow_run_id"),
                    run_status: row.get("run_status"),
                    payload: row.get("payload"),
                    locator_payload: row.get("locator_payload"),
                    source_created_at: row.get("source_created_at"),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let next =
            candidates.last().map(
                |candidate| control_plane_contracts::ports::LegacyRuntimeShadowCursor {
                    source_kind: candidate.source_kind,
                    created_at: candidate.source_created_at,
                    source_row_id: candidate.source_row_id,
                },
            );

        let mut tx = self.pool().begin().await?;
        let lock_budget_ms = input.lock_budget_ms.clamp(1, 1_000);
        sqlx::query("select set_config('lock_timeout', $1, true)")
            .bind(format!("{lock_budget_ms}ms"))
            .execute(&mut *tx)
            .await?;
        let batch_id = Uuid::now_v7();
        sqlx::query(
            r#"
                insert into runtime_legacy_shadow_batches (
                    id, execution_mode, status, requested_limit, lock_budget_ms, start_cursor
                ) values ($1, 'apply', 'running', $2, $3, $4)
                "#,
        )
        .bind(batch_id)
        .bind(i32::try_from(limit)?)
        .bind(i32::try_from(lock_budget_ms)?)
        .bind(input.after.as_ref().map(serde_json::to_value).transpose()?)
        .execute(&mut *tx)
        .await?;

        let mut statistics = Vec::new();
        let mut differences = Vec::new();
        for candidate in &candidates {
            let mut canonical = Vec::new();
            write_canonical_runtime_json(&candidate.payload, &mut canonical)?;
            let source_hash = format!("sha256:{:x}", Sha256::digest(&canonical));
            let source_bytes = u64::try_from(canonical.len())?;
            let lock_key = format!(
                "{}:{}:{}",
                candidate.source_table, candidate.source_column, candidate.source_row_id
            );
            let locked = sqlx::query_scalar::<_, bool>(
                "select pg_try_advisory_xact_lock(hashtextextended($1, 0))",
            )
            .bind(lock_key)
            .fetch_one(&mut *tx)
            .await?;
            if !locked {
                differences.push(LegacyRuntimeShadowDifference {
                    source_kind: candidate.source_kind,
                    source_table: candidate.source_table.clone(),
                    source_column: candidate.source_column.clone(),
                    source_row_id: candidate.source_row_id,
                    application_id: candidate.application_id,
                    flow_run_id: candidate.flow_run_id,
                    reason: "lock_budget_unavailable".to_string(),
                    source_bytes,
                });
                accumulate_legacy_shadow_statistics(
                    &mut statistics,
                    candidate,
                    source_bytes,
                    0,
                    "difference",
                );
                continue;
            }

            let existing = sqlx::query(
                r#"
                select rows.source_hash, contents.content, contents.byte_size
                  from runtime_legacy_shadow_rows rows
                  join runtime_canonical_contents contents on contents.id = rows.canonical_content_id
                 where rows.source_table = $1 and rows.source_column = $2
                   and rows.source_row_id = $3
                "#,
            )
            .bind(&candidate.source_table)
            .bind(&candidate.source_column)
            .bind(candidate.source_row_id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(existing) = existing {
                let stored_hash: String = existing.get("source_hash");
                let stored_content: Value = existing.get("content");
                let canonical_bytes = u64::try_from(existing.get::<i64, _>("byte_size"))?;
                if stored_hash == source_hash && stored_content == candidate.payload {
                    accumulate_legacy_shadow_statistics(
                        &mut statistics,
                        candidate,
                        source_bytes,
                        canonical_bytes,
                        "already_shadowed",
                    );
                } else {
                    differences.push(LegacyRuntimeShadowDifference {
                        source_kind: candidate.source_kind,
                        source_table: candidate.source_table.clone(),
                        source_column: candidate.source_column.clone(),
                        source_row_id: candidate.source_row_id,
                        application_id: candidate.application_id,
                        flow_run_id: candidate.flow_run_id,
                        reason: "source_changed_after_shadow".to_string(),
                        source_bytes,
                    });
                    accumulate_legacy_shadow_statistics(
                        &mut statistics,
                        candidate,
                        source_bytes,
                        canonical_bytes,
                        "difference",
                    );
                }
                continue;
            }

            let mut parent_context_version_id = None;
            let mut context_sequence = None;
            if candidate.source_kind == LegacyRuntimeShadowSourceKind::CheckpointContext {
                if candidate
                    .locator_payload
                    .as_ref()
                    .is_some_and(|locator| locator.get("context_version_id").is_some())
                {
                    differences.push(LegacyRuntimeShadowDifference {
                        source_kind: candidate.source_kind,
                        source_table: candidate.source_table.clone(),
                        source_column: candidate.source_column.clone(),
                        source_row_id: candidate.source_row_id,
                        application_id: candidate.application_id,
                        flow_run_id: candidate.flow_run_id,
                        reason: "checkpoint_context_ownership_not_legacy".to_string(),
                        source_bytes,
                    });
                    accumulate_legacy_shadow_statistics(
                        &mut statistics,
                        candidate,
                        source_bytes,
                        0,
                        "difference",
                    );
                    continue;
                }
                let has_non_shadow_projection = sqlx::query_scalar::<_, bool>(
                    r#"
                    select exists (
                        select 1 from runtime_context_projections projections
                         where projections.flow_run_id = $1
                           and not exists (
                               select 1 from runtime_legacy_shadow_rows shadow_rows
                                where shadow_rows.context_version_id = projections.id
                           )
                    )
                    "#,
                )
                .bind(candidate.flow_run_id)
                .fetch_one(&mut *tx)
                .await?;
                if has_non_shadow_projection {
                    differences.push(LegacyRuntimeShadowDifference {
                        source_kind: candidate.source_kind,
                        source_table: candidate.source_table.clone(),
                        source_column: candidate.source_column.clone(),
                        source_row_id: candidate.source_row_id,
                        application_id: candidate.application_id,
                        flow_run_id: candidate.flow_run_id,
                        reason: "mixed_context_lineage_ownership".to_string(),
                        source_bytes,
                    });
                    accumulate_legacy_shadow_statistics(
                        &mut statistics,
                        candidate,
                        source_bytes,
                        0,
                        "difference",
                    );
                    continue;
                }
                let previous_checkpoint_id = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    select checkpoints.id
                      from flow_run_checkpoints checkpoints
                     where checkpoints.flow_run_id = $1
                       and not (checkpoints.locator_payload ? 'context_version_id')
                       and (checkpoints.created_at, checkpoints.id) < ($2, $3)
                     order by checkpoints.created_at desc, checkpoints.id desc
                     limit 1
                    "#,
                )
                .bind(candidate.flow_run_id)
                .bind(candidate.source_created_at)
                .bind(candidate.source_row_id)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(previous_checkpoint_id) = previous_checkpoint_id {
                    parent_context_version_id = sqlx::query_scalar::<_, Uuid>(
                        r#"
                        select context_version_id from runtime_legacy_shadow_rows
                         where source_table = 'flow_run_checkpoints'
                           and source_column = 'variable_snapshot'
                           and source_row_id = $1
                        "#,
                    )
                    .bind(previous_checkpoint_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    if parent_context_version_id.is_none() {
                        differences.push(LegacyRuntimeShadowDifference {
                            source_kind: candidate.source_kind,
                            source_table: candidate.source_table.clone(),
                            source_column: candidate.source_column.clone(),
                            source_row_id: candidate.source_row_id,
                            application_id: candidate.application_id,
                            flow_run_id: candidate.flow_run_id,
                            reason: "previous_checkpoint_not_shadowed".to_string(),
                            source_bytes,
                        });
                        accumulate_legacy_shadow_statistics(
                            &mut statistics,
                            candidate,
                            source_bytes,
                            0,
                            "difference",
                        );
                        continue;
                    }
                }
                context_sequence = Some(
                    sqlx::query_scalar::<_, i64>(
                        r#"
                        select count(*) from flow_run_checkpoints checkpoints
                         where checkpoints.flow_run_id = $1
                           and not (checkpoints.locator_payload ? 'context_version_id')
                           and (checkpoints.created_at, checkpoints.id) < ($2, $3)
                        "#,
                    )
                    .bind(candidate.flow_run_id)
                    .bind(candidate.source_created_at)
                    .bind(candidate.source_row_id)
                    .fetch_one(&mut *tx)
                    .await?,
                );
            }

            let content_id = sqlx::query_scalar::<_, Uuid>(
                r#"
                insert into runtime_canonical_contents (
                    id, scope_id, application_id, content_hash, content, byte_size
                ) values ($1, $2, $3, $4, $5, $6)
                on conflict (application_id, content_hash) do nothing
                returning id
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(candidate.scope_id)
            .bind(candidate.application_id)
            .bind(&source_hash)
            .bind(&candidate.payload)
            .bind(i64::try_from(canonical.len())?)
            .fetch_optional(&mut *tx)
            .await?;
            let content_id = match content_id {
                Some(id) => id,
                None => sqlx::query_scalar::<_, Uuid>(
                    "select id from runtime_canonical_contents where application_id = $1 and content_hash = $2 and content = $3",
                )
                .bind(candidate.application_id)
                .bind(&source_hash)
                .bind(&candidate.payload)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| anyhow!("canonical runtime content hash collision"))?,
            };
            let context_version_id = if let Some(sequence) = context_sequence {
                let context_version_id = Uuid::now_v7();
                sqlx::query(
                    r#"
                    insert into runtime_context_projections (
                        id, flow_run_id, projection_kind, source_item_refs, model_input_ref,
                        model_input_hash, provider_continuation_metadata, previous_projection_id,
                        scope_id, application_id, context_sequence, transition_kind,
                        transition_actor, actual_content_id
                    ) values (
                        $1, $2, 'context_version',
                        jsonb_build_array(jsonb_build_object(
                            'source_table', 'flow_run_checkpoints',
                            'source_column', 'variable_snapshot',
                            'source_row_id', $3::text
                        )), 'runtime_canonical_content:' || $4::text, $5, '{}'::jsonb, $6,
                        $7, $8, $9, $10, 'host', $4
                    )
                    "#,
                )
                .bind(context_version_id)
                .bind(candidate.flow_run_id)
                .bind(candidate.source_row_id)
                .bind(content_id)
                .bind(&source_hash)
                .bind(parent_context_version_id)
                .bind(candidate.scope_id)
                .bind(candidate.application_id)
                .bind(sequence)
                .bind(if sequence == 0 { "initial" } else { "append" })
                .execute(&mut *tx)
                .await?;
                Some(context_version_id)
            } else {
                None
            };
            sqlx::query(
                r#"
                insert into runtime_legacy_shadow_rows (
                    id, batch_id, source_kind, source_table, source_column, source_row_id,
                    scope_id, application_id, flow_run_id, run_classification, source_hash,
                    source_byte_size, canonical_content_id, context_version_id
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(batch_id)
            .bind(candidate.source_kind.as_str())
            .bind(&candidate.source_table)
            .bind(&candidate.source_column)
            .bind(candidate.source_row_id)
            .bind(candidate.scope_id)
            .bind(candidate.application_id)
            .bind(candidate.flow_run_id)
            .bind(match legacy_run_classification(&candidate.run_status) {
                control_plane_contracts::ports::LegacyRuntimeRunClassification::Pending => "pending",
                control_plane_contracts::ports::LegacyRuntimeRunClassification::Terminal => "terminal",
            })
            .bind(&source_hash)
            .bind(i64::try_from(canonical.len())?)
            .bind(content_id)
            .bind(context_version_id)
            .execute(&mut *tx)
            .await?;
            accumulate_legacy_shadow_statistics(
                &mut statistics,
                candidate,
                source_bytes,
                source_bytes,
                "shadowed",
            );
        }

        let result = ConvertLegacyRuntimeShadowBatchResult {
            next,
            has_more,
            statistics,
            differences,
        };
        if input.execution == LegacyRuntimeShadowExecution::Preview {
            tx.rollback().await?;
        } else {
            sqlx::query(
                r#"
                update runtime_legacy_shadow_batches
                   set status = 'completed', next_cursor = $2, statistics = $3,
                       difference_count = $4, completed_at = now()
                 where id = $1
                "#,
            )
            .bind(batch_id)
            .bind(result.next.as_ref().map(serde_json::to_value).transpose()?)
            .bind(serde_json::to_value(&result.statistics)?)
            .bind(i64::try_from(result.differences.len())?)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(result)
    }

    async fn rollback_legacy_runtime_shadow(
        &self,
        input: &RollbackLegacyRuntimeShadowInput,
    ) -> Result<RollbackLegacyRuntimeShadowResult> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("legacy-shadow-rollback:{}", input.application_id))
            .execute(&mut *tx)
            .await?;
        let rows = sqlx::query(
            r#"
            delete from runtime_legacy_shadow_rows
             where application_id = $1 and ($2::uuid is null or flow_run_id = $2)
            returning canonical_content_id, context_version_id
            "#,
        )
        .bind(input.application_id)
        .bind(input.flow_run_id)
        .fetch_all(&mut *tx)
        .await?;
        let deleted_shadow_rows = u64::try_from(rows.len())?;
        let context_version_ids = rows
            .iter()
            .filter_map(|row| row.get::<Option<Uuid>, _>("context_version_id"))
            .collect::<Vec<_>>();
        let canonical_content_ids = rows
            .iter()
            .map(|row| row.get::<Uuid, _>("canonical_content_id"))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let deleted_context_versions = if context_version_ids.is_empty() {
            0
        } else {
            sqlx::query(
                r#"
                delete from runtime_context_projections
                 where id = any($1)
                "#,
            )
            .bind(&context_version_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };
        let deleted_canonical_contents = if canonical_content_ids.is_empty() {
            0
        } else {
            sqlx::query(
                r#"
                delete from runtime_canonical_contents contents
                 where contents.id = any($1)
                   and not exists (
                       select 1 from runtime_context_projections projections
                        where projections.actual_content_id = contents.id
                   )
                   and not exists (
                       select 1 from runtime_legacy_shadow_rows shadow_rows
                        where shadow_rows.canonical_content_id = contents.id
                   )
                "#,
            )
            .bind(&canonical_content_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };
        tx.commit().await?;
        Ok(RollbackLegacyRuntimeShadowResult {
            deleted_shadow_rows,
            deleted_context_versions,
            deleted_canonical_contents,
            retained_shared_canonical_contents: u64::try_from(canonical_content_ids.len())?
                .saturating_sub(deleted_canonical_contents),
        })
    }
}
