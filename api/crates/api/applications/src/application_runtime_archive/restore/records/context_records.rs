use super::*;

pub(in super::super) async fn insert_canonical_contents_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    application_id: Uuid,
    id_maps: &mut ArchiveRestoreIdMaps,
    contents: &[serde_json::Value],
) -> Result<(), ApiError> {
    for content in contents {
        let source_id = archive_uuid(content, "id", "canonical_content_id")?;
        let proposed_id = *id_maps
            .canonical_contents
            .get(&source_id)
            .ok_or(ControlPlaneError::InvalidInput("canonical_content_id"))?;
        let content_hash =
            archive_required_string(content, "content_hash", "canonical_content_hash")?;
        let payload = content
            .get("content")
            .cloned()
            .ok_or(ControlPlaneError::InvalidInput("canonical_content"))?;
        let byte_size = archive_json_i64(content, "byte_size").ok_or(
            ControlPlaneError::InvalidInput("canonical_content_byte_size"),
        )?;
        let inserted = sqlx::query_scalar::<_, Uuid>(
            r#"
            insert into runtime_canonical_contents (
                id, scope_id, application_id, content_hash, content, byte_size, created_at
            ) values ($1, $2, $3, $4, $5, $6, $7)
            on conflict (application_id, content_hash) do nothing
            returning id
            "#,
        )
        .bind(proposed_id)
        .bind(scope_id)
        .bind(application_id)
        .bind(&content_hash)
        .bind(&payload)
        .bind(byte_size)
        .bind(parse_archive_time(&archive_required_string(
            content,
            "created_at",
            "canonical_content_created_at",
        )?)?)
        .fetch_optional(&mut **tx)
        .await?;
        let target_id = match inserted {
            Some(id) => id,
            None => {
                let (id, stored_payload, stored_byte_size) =
                    sqlx::query_as::<_, (Uuid, serde_json::Value, i64)>(
                        "select id, content, byte_size from runtime_canonical_contents where application_id = $1 and content_hash = $2",
                    )
                    .bind(application_id)
                    .bind(&content_hash)
                    .fetch_one(&mut **tx)
                    .await?;
                if stored_payload != payload || stored_byte_size != byte_size {
                    return Err(
                        ControlPlaneError::Conflict("canonical_content_hash_collision").into(),
                    );
                }
                id
            }
        };
        id_maps.canonical_contents.insert(source_id, target_id);
        insert_import_mapping(
            tx,
            job_id,
            "canonical_content",
            &source_id.to_string(),
            target_id,
        )
        .await?;
    }
    Ok(())
}

pub(in super::super) async fn insert_context_projections_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    application_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    projections: &[serde_json::Value],
) -> Result<(), ApiError> {
    for projection in projections {
        let source_projection_id = archive_uuid(projection, "id", "context_projection_id")?;
        let target_projection_id = *id_maps
            .context_projections
            .get(&source_projection_id)
            .ok_or(ControlPlaneError::InvalidInput("context_projection_id"))?;
        let target_node_run_id = archive_optional_uuid(projection, "node_run_id")
            .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
        let target_llm_turn_span_id = archive_optional_uuid(projection, "llm_turn_span_id")
            .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
        let target_compaction_event_id = archive_optional_uuid(projection, "compaction_event_id")
            .and_then(|source_id| id_maps.runtime_events.get(&source_id).copied());
        let source_item_refs = remap_source_item_refs(
            projection
                .get("source_item_refs")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
            &id_maps.runtime_items,
        );
        let source_model_input_ref = archive_required_string(
            projection,
            "model_input_ref",
            "context_projection_model_input_ref",
        )?;
        let model_input_ref = source_model_input_ref
            .strip_prefix("runtime_canonical_content:")
            .and_then(|value| Uuid::parse_str(value).ok())
            .and_then(|source_id| id_maps.canonical_contents.get(&source_id))
            .map(|target_id| format!("runtime_canonical_content:{target_id}"))
            .unwrap_or(source_model_input_ref);
        sqlx::query(
            r#"
            insert into runtime_context_projections (
                id,
                scope_id,
                application_id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                projection_kind,
                merge_stage_ref,
                source_transcript_ref,
                source_item_refs,
                compaction_event_id,
                summary_version,
                model_input_ref,
                model_input_hash,
                compacted_summary_ref,
                previous_projection_id,
                token_estimate,
                provider_continuation_metadata,
                context_sequence,
                transition_kind,
                transition_actor,
                declared_compaction_provenance,
                actual_content_id,
                created_at
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                $12, $13, $14, $15, null, $16, $17, $18, $19, $20, $21, $22, $23
            )
            "#,
        )
        .bind(target_projection_id)
        .bind(scope_id)
        .bind(application_id)
        .bind(target_run_id)
        .bind(target_node_run_id)
        .bind(target_llm_turn_span_id)
        .bind(archive_required_string(
            projection,
            "projection_kind",
            "context_projection_kind",
        )?)
        .bind(archive_json_string(projection, "merge_stage_ref"))
        .bind(archive_json_string(projection, "source_transcript_ref"))
        .bind(source_item_refs)
        .bind(target_compaction_event_id)
        .bind(archive_json_string(projection, "summary_version"))
        .bind(model_input_ref)
        .bind(archive_required_string(
            projection,
            "model_input_hash",
            "context_projection_model_input_hash",
        )?)
        .bind(archive_json_string(projection, "compacted_summary_ref"))
        .bind(archive_json_i64(projection, "token_estimate"))
        .bind(archive_json_value_or_object(
            projection,
            "provider_continuation_metadata",
        ))
        .bind(archive_json_i64(projection, "context_sequence"))
        .bind(archive_json_string(projection, "transition_kind"))
        .bind(archive_json_string(projection, "transition_actor"))
        .bind(
            projection
                .get("declared_compaction_provenance")
                .filter(|value| !value.is_null())
                .cloned(),
        )
        .bind(
            archive_optional_uuid(projection, "actual_content_id")
                .and_then(|source_id| id_maps.canonical_contents.get(&source_id).copied()),
        )
        .bind(parse_archive_time(&archive_required_string(
            projection,
            "created_at",
            "context_projection_created_at",
        )?)?)
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "context_projection",
            &source_projection_id.to_string(),
            target_projection_id,
        )
        .await?;
    }

    for projection in projections {
        let Some(source_previous_projection_id) =
            archive_optional_uuid(projection, "previous_projection_id")
        else {
            continue;
        };
        let source_projection_id = archive_uuid(projection, "id", "context_projection_id")?;
        let Some(target_projection_id) = id_maps.context_projections.get(&source_projection_id)
        else {
            continue;
        };
        let Some(target_previous_projection_id) = id_maps
            .context_projections
            .get(&source_previous_projection_id)
        else {
            continue;
        };
        sqlx::query(
            r#"
            update runtime_context_projections
            set previous_projection_id = $2
            where id = $1
            "#,
        )
        .bind(*target_projection_id)
        .bind(*target_previous_projection_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub(in super::super) async fn insert_invocation_context_bindings_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope_id: Uuid,
    application_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    bindings: &[serde_json::Value],
) -> Result<(), ApiError> {
    for binding in bindings {
        let source_span_id = archive_uuid(binding, "invocation_span_id", "invocation_span_id")?;
        let source_context_id = archive_uuid(binding, "context_version_id", "context_version_id")?;
        let target_span_id = *id_maps
            .runtime_spans
            .get(&source_span_id)
            .ok_or(ControlPlaneError::InvalidInput("invocation_span_id"))?;
        let target_context_id = *id_maps
            .context_projections
            .get(&source_context_id)
            .ok_or(ControlPlaneError::InvalidInput("context_version_id"))?;
        sqlx::query(
            r#"
            insert into runtime_invocation_context_bindings (
                invocation_span_id, scope_id, application_id, flow_run_id,
                context_version_id, created_at
            ) values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(target_span_id)
        .bind(scope_id)
        .bind(application_id)
        .bind(target_run_id)
        .bind(target_context_id)
        .bind(parse_archive_time(&archive_required_string(
            binding,
            "created_at",
            "invocation_context_binding_created_at",
        )?)?)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(in super::super) async fn insert_recovery_history_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    application_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    records: &[serde_json::Value],
) -> Result<(), ApiError> {
    for record in records {
        let source_id = archive_uuid(record, "id", "recovery_history_id")?;
        let target_id = Uuid::now_v7();
        let context_source_id = archive_uuid(record, "context_version_id", "context_version_id")?;
        let target_context_id = *id_maps
            .context_projections
            .get(&context_source_id)
            .ok_or(ControlPlaneError::InvalidInput("context_version_id"))?;
        let target_content_id = archive_optional_uuid(record, "recovery_content_id")
            .and_then(|id| id_maps.canonical_contents.get(&id).copied());
        let target_node_id = archive_optional_uuid(record, "node_run_id")
            .and_then(|id| id_maps.node_runs.get(&id).copied());
        sqlx::query(
            r#"
            insert into flow_run_recovery_history (
                id, scope_id, application_id, flow_run_id, node_run_id, sequence,
                state_code, node_sequence, iteration_index, attempt_index,
                resume_sequence, event_sequence, context_version_id,
                recovery_content_id, idempotency_key, created_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(target_id)
        .bind(scope_id)
        .bind(application_id)
        .bind(target_run_id)
        .bind(target_node_id)
        .bind(
            archive_json_i64(record, "sequence")
                .ok_or(ControlPlaneError::InvalidInput("recovery_sequence"))?,
        )
        .bind(archive_required_string(
            record,
            "state_code",
            "recovery_state_code",
        )?)
        .bind(archive_json_i64(record, "node_sequence").unwrap_or(0))
        .bind(archive_json_i64(record, "iteration_index").unwrap_or(0))
        .bind(i32::try_from(
            archive_json_i64(record, "attempt_index").unwrap_or(0),
        )?)
        .bind(archive_json_i64(record, "resume_sequence").unwrap_or(0))
        .bind(archive_json_i64(record, "event_sequence").unwrap_or(0))
        .bind(target_context_id)
        .bind(target_content_id)
        .bind(archive_required_string(
            record,
            "idempotency_key",
            "recovery_idempotency_key",
        )?)
        .bind(parse_archive_time(&archive_required_string(
            record,
            "created_at",
            "recovery_created_at",
        )?)?)
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "recovery_history",
            &source_id.to_string(),
            target_id,
        )
        .await?;
    }
    Ok(())
}
