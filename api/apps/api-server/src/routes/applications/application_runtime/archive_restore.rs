struct ImportedRunFinalUpdate {
    target_run_id: Uuid,
    status: domain::FlowRunStatus,
    input_payload: serde_json::Value,
    output_payload: serde_json::Value,
    error_payload: Option<serde_json::Value>,
    finished_at: Option<OffsetDateTime>,
}

#[derive(Default)]
struct ArchiveRestoreIdMaps {
    node_runs: std::collections::HashMap<Uuid, Uuid>,
    runtime_spans: std::collections::HashMap<Uuid, Uuid>,
    runtime_events: std::collections::HashMap<Uuid, Uuid>,
    runtime_items: std::collections::HashMap<Uuid, Uuid>,
    usage_ledger: std::collections::HashMap<Uuid, Uuid>,
    model_failover_attempts: std::collections::HashMap<Uuid, Uuid>,
    context_projections: std::collections::HashMap<Uuid, Uuid>,
}

async fn restore_run_archive_v1(
    state: Arc<ApiState>,
    application: &domain::ApplicationRecord,
    actor_user_id: Uuid,
    job_id: Uuid,
    archive: RunArchiveV1Response,
) -> Result<(), ApiError> {
    mark_run_archive_import_job_processing(&state, job_id).await?;
    if archive.archive_version != RUN_ARCHIVE_VERSION {
        return Err(ControlPlaneError::InvalidInput("archive_version").into());
    }

    let editor_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(actor_user_id, application.id)
        .await?;
    let mut run_mappings = Vec::with_capacity(archive.entries.len());
    let mut final_run_updates = Vec::with_capacity(archive.entries.len());
    let mut tx = state.store.pool().begin().await?;

    for entry in archive.entries {
        let source_run_id = Uuid::parse_str(&entry.source_run_id)
            .map_err(|_| ControlPlaneError::InvalidInput("source_run_id"))?;
        let target_run_id = Uuid::now_v7();
        let source_flow_run = &entry.flow_run;
        let flow_schema_version = entry
            .compiled_plan
            .as_ref()
            .and_then(|value| value.get("schema_version"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("1flowbase.flow/v2");
        let document_hash = entry
            .flow_run_fact
            .get("document_hash")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("imported-run-archive");
        let compiled_plan_payload = entry
            .compiled_plan
            .as_ref()
            .and_then(|value| value.get("plan").cloned())
            .unwrap_or_else(|| serde_json::json!({}));

        let compiled_plan_id = match sqlx::query_scalar::<_, Uuid>(
            r#"
            select id
            from flow_compiled_plans
            where flow_draft_id = $1
            "#,
        )
        .bind(editor_state.draft.id)
        .fetch_optional(&mut *tx)
        .await?
        {
            Some(existing_id) => existing_id,
            None => {
                let compiled_plan_id = Uuid::now_v7();
                sqlx::query(
                    r#"
                    insert into flow_compiled_plans (
                        id,
                        flow_id,
                        flow_draft_id,
                        schema_version,
                        document_hash,
                        document_updated_at,
                        plan,
                        scope_id,
                        created_by,
                        updated_by
                    ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
                    "#,
                )
                .bind(compiled_plan_id)
                .bind(editor_state.flow.id)
                .bind(editor_state.draft.id)
                .bind(flow_schema_version)
                .bind(document_hash)
                .bind(editor_state.draft.updated_at)
                .bind(compiled_plan_payload)
                .bind(application.workspace_id)
                .bind(actor_user_id)
                .execute(&mut *tx)
                .await?;
                compiled_plan_id
            }
        };

        sqlx::query(
            r#"
            insert into flow_runs (
                id,
                application_id,
                flow_id,
                flow_draft_id,
                compiled_plan_id,
                debug_session_id,
                flow_schema_version,
                document_hash,
                run_mode,
                target_node_id,
                title,
                status,
                input_payload,
                output_payload,
                error_payload,
                created_by,
                api_key_id,
                publication_version_id,
                external_user,
                external_conversation_id,
                external_trace_id,
                compatibility_mode,
                idempotency_key,
                started_at,
                finished_at,
                created_at,
                updated_at,
                scope_id,
                import_job_id,
                import_source_run_id
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, 'running', $12, '{}'::jsonb, null, $13, null, null,
                $14, $15, $16, $17, null, $18, null, $19, $19, $20, $21, $22
            )
            "#,
        )
        .bind(target_run_id)
        .bind(application.id)
        .bind(editor_state.flow.id)
        .bind(editor_state.draft.id)
        .bind(compiled_plan_id)
        .bind(format!("imported:{job_id}:{source_run_id}"))
        .bind(flow_schema_version)
        .bind(document_hash)
        .bind(source_flow_run.run_mode.as_str())
        .bind(source_flow_run.target_node_id.as_deref())
        .bind(&source_flow_run.title)
        .bind(&source_flow_run.input_payload)
        .bind(actor_user_id)
        .bind(archive_json_string(&entry.flow_run_fact, "external_user"))
        .bind(archive_json_string(
            &entry.flow_run_fact,
            "external_conversation_id",
        ))
        .bind(archive_json_string(&entry.flow_run_fact, "external_trace_id"))
        .bind(archive_json_string(&entry.flow_run_fact, "compatibility_mode"))
        .bind(parse_archive_time(&source_flow_run.started_at)?)
        .bind(parse_archive_time(&source_flow_run.created_at)?)
        .bind(application.workspace_id)
        .bind(job_id)
        .bind(source_run_id.to_string())
        .execute(&mut *tx)
        .await?;

        let mut id_maps = ArchiveRestoreIdMaps {
            runtime_spans: preassign_archive_ids(&entry.runtime_spans, "runtime_span_id")?,
            runtime_events: preassign_archive_ids(&entry.runtime_events, "runtime_event_id")?,
            runtime_items: preassign_archive_ids(&entry.runtime_items, "runtime_item_id")?,
            usage_ledger: preassign_archive_ids(&entry.usage_ledger, "usage_ledger_id")?,
            model_failover_attempts: preassign_archive_ids(
                &entry.model_failover_attempts,
                "model_failover_attempt_id",
            )?,
            context_projections: preassign_archive_ids(
                &entry.context_projections,
                "context_projection_id",
            )?,
            ..Default::default()
        };
        for node in &entry.node_runs {
            let source_node_run_id = Uuid::parse_str(&node.id)
                .map_err(|_| ControlPlaneError::InvalidInput("node_run_id"))?;
            let target_node_run_id = Uuid::now_v7();
            id_maps
                .node_runs
                .insert(source_node_run_id, target_node_run_id);
            sqlx::query(
                r#"
                insert into node_runs (
                    id,
                    scope_id,
                    flow_run_id,
                    node_id,
                    node_type,
                    node_alias,
                    status,
                    input_payload,
                    output_payload,
                    error_payload,
                    metrics_payload,
                    debug_payload,
                    started_at,
                    finished_at
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                "#,
            )
            .bind(target_node_run_id)
            .bind(application.workspace_id)
            .bind(target_run_id)
            .bind(&node.node_id)
            .bind(&node.node_type)
            .bind(&node.node_alias)
            .bind(&node.status)
            .bind(&node.input_payload)
            .bind(&node.output_payload)
            .bind(&node.error_payload)
            .bind(&node.metrics_payload)
            .bind(&node.debug_payload)
            .bind(parse_archive_time(&node.started_at)?)
            .bind(parse_optional_archive_time(node.finished_at.as_deref())?)
            .execute(&mut *tx)
            .await?;
            insert_import_mapping(
                &mut tx,
                job_id,
                "node_run",
                &source_node_run_id.to_string(),
                target_node_run_id,
            )
            .await?;
        }

        for event in &entry.events {
            let source_event_id = Uuid::parse_str(&event.id)
                .map_err(|_| ControlPlaneError::InvalidInput("run_event_id"))?;
            let target_event_id = Uuid::now_v7();
            let target_node_run_id = event
                .node_run_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
            sqlx::query(
                r#"
                insert into flow_run_events (
                    id,
                    scope_id,
                    flow_run_id,
                    node_run_id,
                    sequence,
                    event_type,
                    payload,
                    created_at
                ) values ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(target_event_id)
            .bind(application.workspace_id)
            .bind(target_run_id)
            .bind(target_node_run_id)
            .bind(event.sequence)
            .bind(&event.event_type)
            .bind(&event.payload)
            .bind(parse_archive_time(&event.created_at)?)
            .execute(&mut *tx)
            .await?;
            insert_import_mapping(
                &mut tx,
                job_id,
                "run_event",
                &source_event_id.to_string(),
                target_event_id,
            )
            .await?;
        }

        for checkpoint in &entry.checkpoints {
            let source_checkpoint_id = Uuid::parse_str(&checkpoint.id)
                .map_err(|_| ControlPlaneError::InvalidInput("checkpoint_id"))?;
            let target_checkpoint_id = Uuid::now_v7();
            let target_node_run_id = checkpoint
                .node_run_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
            sqlx::query(
                r#"
                insert into flow_run_checkpoints (
                    id,
                    scope_id,
                    flow_run_id,
                    node_run_id,
                    status,
                    reason,
                    locator_payload,
                    variable_snapshot,
                    external_ref_payload,
                    created_at
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(target_checkpoint_id)
            .bind(application.workspace_id)
            .bind(target_run_id)
            .bind(target_node_run_id)
            .bind(&checkpoint.status)
            .bind(&checkpoint.reason)
            .bind(&checkpoint.locator_payload)
            .bind(&checkpoint.variable_snapshot)
            .bind(&checkpoint.external_ref_payload)
            .bind(parse_archive_time(&checkpoint.created_at)?)
            .execute(&mut *tx)
            .await?;
            insert_import_mapping(
                &mut tx,
                job_id,
                "checkpoint",
                &source_checkpoint_id.to_string(),
                target_checkpoint_id,
            )
            .await?;
        }

        for task in &entry.callback_tasks {
            let source_task_id = Uuid::parse_str(&task.id)
                .map_err(|_| ControlPlaneError::InvalidInput("callback_task_id"))?;
            let target_task_id = Uuid::now_v7();
            let source_node_id = Uuid::parse_str(&task.node_run_id)
                .map_err(|_| ControlPlaneError::InvalidInput("callback_task_node_run_id"))?;
            let target_node_run_id = id_maps
                .node_runs
                .get(&source_node_id)
                .copied()
                .ok_or(ControlPlaneError::InvalidInput("callback_task_node_run_id"))?;
            sqlx::query(
                r#"
                insert into flow_run_callback_tasks (
                    id,
                    scope_id,
                    flow_run_id,
                    node_run_id,
                    callback_kind,
                    status,
                    request_payload,
                    response_payload,
                    external_ref_payload,
                    created_at,
                    completed_at
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(target_task_id)
            .bind(application.workspace_id)
            .bind(target_run_id)
            .bind(target_node_run_id)
            .bind(&task.callback_kind)
            .bind(&task.status)
            .bind(&task.request_payload)
            .bind(&task.response_payload)
            .bind(&task.external_ref_payload)
            .bind(parse_archive_time(&task.created_at)?)
            .bind(parse_optional_archive_time(task.completed_at.as_deref())?)
            .execute(&mut *tx)
            .await?;
            insert_import_mapping(
                &mut tx,
                job_id,
                "callback_task",
                &source_task_id.to_string(),
                target_task_id,
            )
            .await?;
        }

        insert_runtime_spans_from_archive(
            &mut tx,
            job_id,
            application.workspace_id,
            target_run_id,
            &id_maps,
            &entry.runtime_spans,
        )
        .await?;
        insert_model_failover_attempts_from_archive(
            &mut tx,
            job_id,
            application.workspace_id,
            target_run_id,
            &id_maps,
            &entry.model_failover_attempts,
        )
        .await?;
        for usage in &entry.usage_ledger {
            insert_usage_ledger_from_archive(
                &mut tx,
                job_id,
                application.workspace_id,
                target_run_id,
                &id_maps,
                usage,
            )
            .await?;
        }
        link_model_failover_attempt_usage_from_archive(
            &mut tx,
            &id_maps,
            &entry.model_failover_attempts,
        )
        .await?;
        for runtime_event in &entry.runtime_events {
            insert_runtime_event_from_archive(
                &mut tx,
                job_id,
                application.workspace_id,
                target_run_id,
                &id_maps,
                runtime_event,
            )
            .await?;
        }
        insert_runtime_items_from_archive(
            &mut tx,
            job_id,
            application.workspace_id,
            target_run_id,
            &id_maps,
            &entry.runtime_items,
        )
        .await?;
        insert_context_projections_from_archive(
            &mut tx,
            job_id,
            application.workspace_id,
            target_run_id,
            &id_maps,
            &entry.context_projections,
        )
        .await?;
        insert_capability_invocations_from_archive(
            &mut tx,
            job_id,
            application.workspace_id,
            target_run_id,
            &id_maps,
            &entry.capability_invocations,
        )
        .await?;

        insert_import_mapping(
            &mut tx,
            job_id,
            "flow_run",
            &source_run_id.to_string(),
            target_run_id,
        )
        .await?;
        run_mappings.push((source_run_id.to_string(), target_run_id));
        final_run_updates.push(ImportedRunFinalUpdate {
            target_run_id,
            status: parse_flow_run_status(&source_flow_run.status)?,
            input_payload: source_flow_run.input_payload.clone(),
            output_payload: source_flow_run.output_payload.clone(),
            error_payload: source_flow_run.error_payload.clone(),
            finished_at: parse_optional_archive_time(source_flow_run.finished_at.as_deref())?,
        });
    }

    tx.commit().await?;
    for update in final_run_updates {
        <MainDurableStore as OrchestrationRuntimeRepository>::update_flow_run_payloads(
            &state.store,
            &UpdateFlowRunPayloadsInput {
                flow_run_id: update.target_run_id,
                input_payload: update.input_payload.clone(),
                output_payload: update.output_payload.clone(),
                error_payload: update.error_payload.clone(),
            },
        )
        .await?;
        <MainDurableStore as OrchestrationRuntimeRepository>::update_flow_run(
            &state.store,
            &UpdateFlowRunInput {
                flow_run_id: update.target_run_id,
                status: update.status,
                output_payload: update.output_payload,
                error_payload: update.error_payload,
                finished_at: update.finished_at,
            },
        )
        .await?;
    }
    let imported_run_mappings = run_mappings.clone();
    mark_run_archive_import_job_succeeded(&state, job_id, run_mappings).await?;
    let projection_warnings =
        rebuild_imported_run_trace_projections(&state, application.id, &imported_run_mappings)
            .await;
    if !projection_warnings.is_empty() {
        update_run_archive_import_job_projection_warnings(
            &state,
            job_id,
            &imported_run_mappings,
            projection_warnings,
        )
        .await?;
    }
    Ok(())
}

async fn rebuild_imported_run_trace_projections(
    state: &Arc<ApiState>,
    application_id: Uuid,
    run_mappings: &[(String, Uuid)],
) -> Vec<serde_json::Value> {
    let mut warnings = Vec::new();
    for (source_run_id, target_run_id) in run_mappings {
        match ensure_application_run_trace_projection_status(state, application_id, *target_run_id)
            .await
        {
            Ok(status) => {
                if status.status != domain::ApplicationRunTraceProjectionStatus::Succeeded {
                    warnings.push(serde_json::json!({
                        "code": "trace_projection_not_succeeded",
                        "source_run_id": source_run_id,
                        "target_run_id": target_run_id.to_string(),
                        "projection_status": status.status.as_str()
                    }));
                }
            }
            Err(error) => warnings.push(serde_json::json!({
                "code": "trace_projection_rebuild_failed",
                "source_run_id": source_run_id,
                "target_run_id": target_run_id.to_string(),
                "message": error.0.to_string()
            })),
        }
    }
    warnings
}

async fn update_run_archive_import_job_projection_warnings(
    state: &Arc<ApiState>,
    job_id: Uuid,
    run_mappings: &[(String, Uuid)],
    warnings: Vec<serde_json::Value>,
) -> Result<(), ApiError> {
    let result_payload = serde_json::json!({
        "source_to_target_run_ids": run_mappings
            .iter()
            .map(|(source_run_id, target_run_id)| serde_json::json!({
                "source_run_id": source_run_id,
                "target_run_id": target_run_id.to_string()
            }))
            .collect::<Vec<_>>(),
        "warnings": warnings
    });
    sqlx::query(
        r#"
        update run_archive_import_jobs
        set result_payload = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(job_id)
    .bind(result_payload)
    .execute(state.store.pool())
    .await?;
    Ok(())
}

async fn insert_runtime_spans_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    spans: &[serde_json::Value],
) -> Result<(), ApiError> {
    for span in spans {
        let source_span_id = archive_uuid(span, "id", "runtime_span_id")?;
        let target_span_id = *id_maps
            .runtime_spans
            .get(&source_span_id)
            .ok_or(ControlPlaneError::InvalidInput("runtime_span_id"))?;
        let target_node_run_id = archive_optional_uuid(span, "node_run_id")
            .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
        sqlx::query(
            r#"
            insert into runtime_spans (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                parent_span_id,
                kind,
                name,
                status,
                capability_id,
                input_ref,
                output_ref,
                error_payload,
                metadata,
                started_at,
                finished_at,
                created_at
            ) values ($1, $2, $3, $4, null, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $13)
            "#,
        )
        .bind(target_span_id)
        .bind(scope_id)
        .bind(target_run_id)
        .bind(target_node_run_id)
        .bind(archive_required_string(span, "kind", "runtime_span_kind")?)
        .bind(archive_required_string(span, "name", "runtime_span_name")?)
        .bind(archive_required_string(span, "status", "runtime_span_status")?)
        .bind(archive_json_string(span, "capability_id"))
        .bind(archive_json_string(span, "input_ref"))
        .bind(archive_json_string(span, "output_ref"))
        .bind(optional_archive_json_value(span, "error_payload"))
        .bind(archive_json_value_or_object(span, "metadata"))
        .bind(parse_archive_time(&archive_required_string(
            span,
            "started_at",
            "runtime_span_started_at",
        )?)?)
        .bind(parse_optional_archive_time(
            archive_json_string(span, "finished_at").as_deref(),
        )?)
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "runtime_span",
            &source_span_id.to_string(),
            target_span_id,
        )
        .await?;
    }

    for span in spans {
        let Some(source_parent_span_id) = archive_optional_uuid(span, "parent_span_id") else {
            continue;
        };
        let source_span_id = archive_uuid(span, "id", "runtime_span_id")?;
        let Some(target_parent_span_id) = id_maps.runtime_spans.get(&source_parent_span_id) else {
            continue;
        };
        let target_span_id = id_maps
            .runtime_spans
            .get(&source_span_id)
            .copied()
            .ok_or(ControlPlaneError::InvalidInput("runtime_span_id"))?;
        sqlx::query(
            r#"
            update runtime_spans
            set parent_span_id = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(target_span_id)
        .bind(*target_parent_span_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn insert_model_failover_attempts_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    attempts: &[serde_json::Value],
) -> Result<(), ApiError> {
    for attempt in attempts {
        let source_attempt_id =
            archive_uuid(attempt, "id", "model_failover_attempt_id")?;
        let target_attempt_id = *id_maps
            .model_failover_attempts
            .get(&source_attempt_id)
            .ok_or(ControlPlaneError::InvalidInput("model_failover_attempt_id"))?;
        let target_node_run_id = archive_optional_uuid(attempt, "node_run_id")
            .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
        let target_llm_turn_span_id = archive_optional_uuid(attempt, "llm_turn_span_id")
            .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
        let started_at = parse_archive_time(&archive_required_string(
            attempt,
            "started_at",
            "model_failover_attempt_started_at",
        )?)?;
        sqlx::query(
            r#"
            insert into model_failover_attempt_ledger (
                id,
                scope_id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref,
                created_at
            ) values (
                $1, $2, $3, $4, $5, null, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                null, null, $21, $13
            )
            "#,
        )
        .bind(target_attempt_id)
        .bind(scope_id)
        .bind(target_run_id)
        .bind(target_node_run_id)
        .bind(target_llm_turn_span_id)
        .bind(archive_json_i64(attempt, "attempt_index").unwrap_or(0) as i32)
        .bind(archive_optional_uuid(attempt, "provider_instance_id"))
        .bind(archive_required_string(
            attempt,
            "provider_code",
            "model_failover_attempt_provider_code",
        )?)
        .bind(archive_required_string(
            attempt,
            "upstream_model_id",
            "model_failover_attempt_upstream_model_id",
        )?)
        .bind(archive_required_string(
            attempt,
            "protocol",
            "model_failover_attempt_protocol",
        )?)
        .bind(archive_json_string(attempt, "request_ref"))
        .bind(archive_json_string(attempt, "request_hash"))
        .bind(started_at)
        .bind(parse_optional_archive_time(
            archive_json_string(attempt, "first_token_at").as_deref(),
        )?)
        .bind(parse_optional_archive_time(
            archive_json_string(attempt, "finished_at").as_deref(),
        )?)
        .bind(archive_required_string(
            attempt,
            "status",
            "model_failover_attempt_status",
        )?)
        .bind(archive_json_bool(attempt, "failed_after_first_token").unwrap_or(false))
        .bind(archive_json_string(attempt, "upstream_request_id"))
        .bind(archive_json_string(attempt, "error_code"))
        .bind(archive_json_string(attempt, "error_message_ref"))
        .bind(archive_json_string(attempt, "response_ref"))
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "model_failover_attempt",
            &source_attempt_id.to_string(),
            target_attempt_id,
        )
        .await?;
    }

    Ok(())
}

async fn insert_usage_ledger_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    usage: &serde_json::Value,
) -> Result<(), ApiError> {
    let source_usage_id = archive_uuid(usage, "id", "usage_ledger_id")?;
    let target_usage_id = *id_maps
        .usage_ledger
        .get(&source_usage_id)
        .ok_or(ControlPlaneError::InvalidInput("usage_ledger_id"))?;
    let target_node_run_id = usage
        .get("node_run_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
    let target_span_id = archive_optional_uuid(usage, "span_id")
        .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
    let target_failover_attempt_id = archive_optional_uuid(usage, "failover_attempt_id")
        .and_then(|source_id| id_maps.model_failover_attempts.get(&source_id).copied());
    sqlx::query(
        r#"
        insert into runtime_usage_ledger (
            id,
            scope_id,
            flow_run_id,
            node_run_id,
            span_id,
            failover_attempt_id,
            provider_instance_id,
            gateway_route_id,
            model_id,
            upstream_model_id,
            upstream_request_id,
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
            total_tokens,
            input_cache_hit_tokens,
            input_cache_miss_tokens,
            cache_read_tokens,
            cache_write_tokens,
            price_snapshot,
            cost_snapshot,
            usage_status,
            raw_usage,
            normalized_usage,
            created_at
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19,
            $20, $21, $22, $23, $24, $25, $26
        )
        "#,
    )
    .bind(target_usage_id)
    .bind(scope_id)
    .bind(target_run_id)
    .bind(target_node_run_id)
    .bind(target_span_id)
    .bind(target_failover_attempt_id)
    .bind(archive_optional_uuid(usage, "provider_instance_id"))
    .bind(archive_optional_uuid(usage, "gateway_route_id"))
    .bind(archive_json_string(usage, "model_id"))
    .bind(archive_json_string(usage, "upstream_model_id"))
    .bind(archive_json_string(usage, "upstream_request_id"))
    .bind(archive_json_i64(usage, "input_tokens"))
    .bind(archive_json_i64(usage, "cached_input_tokens"))
    .bind(archive_json_i64(usage, "output_tokens"))
    .bind(archive_json_i64(usage, "reasoning_output_tokens"))
    .bind(archive_json_i64(usage, "total_tokens"))
    .bind(archive_json_i64(usage, "input_cache_hit_tokens"))
    .bind(archive_json_i64(usage, "input_cache_miss_tokens"))
    .bind(archive_json_i64(usage, "cache_read_tokens"))
    .bind(archive_json_i64(usage, "cache_write_tokens"))
    .bind(usage.get("price_snapshot").cloned())
    .bind(usage.get("cost_snapshot").cloned())
    .bind(archive_json_string(usage, "usage_status").unwrap_or_else(|| "recorded".to_string()))
    .bind(
        usage
            .get("raw_usage")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .bind(
        usage
            .get("normalized_usage")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .bind(
        archive_json_string(usage, "created_at")
            .as_deref()
            .map(parse_archive_time)
            .transpose()?
            .unwrap_or_else(OffsetDateTime::now_utc),
    )
    .execute(&mut **tx)
    .await?;
    insert_import_mapping(
        tx,
        job_id,
        "usage_ledger",
        &source_usage_id.to_string(),
        target_usage_id,
    )
    .await?;
    Ok(())
}

async fn insert_runtime_event_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    event: &serde_json::Value,
) -> Result<(), ApiError> {
    let source_event_id = archive_uuid(event, "id", "runtime_event_id")?;
    let target_event_id = *id_maps
        .runtime_events
        .get(&source_event_id)
        .ok_or(ControlPlaneError::InvalidInput("runtime_event_id"))?;
    let target_node_run_id = event
        .get("node_run_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .and_then(|source_id| id_maps.node_runs.get(&source_id).copied());
    let target_span_id = archive_optional_uuid(event, "span_id")
        .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
    let target_parent_span_id = archive_optional_uuid(event, "parent_span_id")
        .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
    let target_item_id = archive_optional_uuid(event, "item_id")
        .and_then(|source_id| id_maps.runtime_items.get(&source_id).copied());
    sqlx::query(
        r#"
        insert into runtime_events (
            id,
            scope_id,
            flow_run_id,
            node_run_id,
            span_id,
            parent_span_id,
            sequence,
            event_type,
            layer,
            source,
            trust_level,
            item_id,
            ledger_ref,
            payload,
            visibility,
            durability,
            created_at
        ) values (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17
        )
        "#,
    )
    .bind(target_event_id)
    .bind(scope_id)
    .bind(target_run_id)
    .bind(target_node_run_id)
    .bind(target_span_id)
    .bind(target_parent_span_id)
    .bind(archive_json_i64(event, "sequence").unwrap_or(0))
    .bind(archive_json_string(event, "event_type").unwrap_or_else(|| "imported_event".to_string()))
    .bind(archive_json_string(event, "layer").unwrap_or_else(|| "diagnostic".to_string()))
    .bind(archive_json_string(event, "source").unwrap_or_else(|| "host".to_string()))
    .bind(archive_json_string(event, "trust_level").unwrap_or_else(|| "host_fact".to_string()))
    .bind(target_item_id)
    .bind(archive_json_string(event, "ledger_ref"))
    .bind(event.get("payload").cloned().unwrap_or_else(|| serde_json::json!({})))
    .bind(archive_json_string(event, "visibility").unwrap_or_else(|| "workspace".to_string()))
    .bind(archive_json_string(event, "durability").unwrap_or_else(|| "durable".to_string()))
    .bind(
        archive_json_string(event, "created_at")
            .as_deref()
            .map(parse_archive_time)
            .transpose()?
            .unwrap_or_else(OffsetDateTime::now_utc),
    )
    .execute(&mut **tx)
    .await?;
    insert_import_mapping(
        tx,
        job_id,
        "runtime_event",
        &source_event_id.to_string(),
        target_event_id,
    )
    .await?;
    Ok(())
}

async fn link_model_failover_attempt_usage_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id_maps: &ArchiveRestoreIdMaps,
    attempts: &[serde_json::Value],
) -> Result<(), ApiError> {
    for attempt in attempts {
        let Some(source_usage_ledger_id) = archive_optional_uuid(attempt, "usage_ledger_id") else {
            continue;
        };
        let source_attempt_id =
            archive_uuid(attempt, "id", "model_failover_attempt_id")?;
        let Some(target_attempt_id) = id_maps.model_failover_attempts.get(&source_attempt_id) else {
            continue;
        };
        let Some(target_usage_ledger_id) = id_maps.usage_ledger.get(&source_usage_ledger_id) else {
            continue;
        };
        sqlx::query(
            r#"
            update model_failover_attempt_ledger
            set usage_ledger_id = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(*target_attempt_id)
        .bind(*target_usage_ledger_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn insert_runtime_items_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    items: &[serde_json::Value],
) -> Result<(), ApiError> {
    for item in items {
        let source_item_id = archive_uuid(item, "id", "runtime_item_id")?;
        let target_item_id = *id_maps
            .runtime_items
            .get(&source_item_id)
            .ok_or(ControlPlaneError::InvalidInput("runtime_item_id"))?;
        let target_span_id = archive_optional_uuid(item, "span_id")
            .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
        let target_source_event_id = archive_optional_uuid(item, "source_event_id")
            .and_then(|source_id| id_maps.runtime_events.get(&source_id).copied());
        let target_usage_ledger_id = archive_optional_uuid(item, "usage_ledger_id")
            .and_then(|source_id| id_maps.usage_ledger.get(&source_id).copied());
        let created_at = parse_archive_time(&archive_required_string(
            item,
            "created_at",
            "runtime_item_created_at",
        )?)?;
        sqlx::query(
            r#"
            insert into runtime_items (
                id,
                scope_id,
                flow_run_id,
                span_id,
                kind,
                status,
                source_event_id,
                input_ref,
                output_ref,
                usage_ledger_id,
                trust_level,
                created_at,
                updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(target_item_id)
        .bind(scope_id)
        .bind(target_run_id)
        .bind(target_span_id)
        .bind(archive_required_string(item, "kind", "runtime_item_kind")?)
        .bind(archive_required_string(item, "status", "runtime_item_status")?)
        .bind(target_source_event_id)
        .bind(archive_json_string(item, "input_ref"))
        .bind(archive_json_string(item, "output_ref"))
        .bind(target_usage_ledger_id)
        .bind(archive_required_string(
            item,
            "trust_level",
            "runtime_item_trust_level",
        )?)
        .bind(created_at)
        .bind(
            archive_json_string(item, "updated_at")
                .as_deref()
                .map(parse_archive_time)
                .transpose()?
                .unwrap_or(created_at),
        )
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "runtime_item",
            &source_item_id.to_string(),
            target_item_id,
        )
        .await?;
    }

    Ok(())
}

async fn insert_context_projections_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
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
        sqlx::query(
            r#"
            insert into runtime_context_projections (
                id,
                scope_id,
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
                created_at
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, null, $15, $16, $17
            )
            "#,
        )
        .bind(target_projection_id)
        .bind(scope_id)
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
        .bind(archive_required_string(
            projection,
            "model_input_ref",
            "context_projection_model_input_ref",
        )?)
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
        let Some(target_previous_projection_id) =
            id_maps.context_projections.get(&source_previous_projection_id)
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

async fn insert_capability_invocations_from_archive(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    scope_id: Uuid,
    target_run_id: Uuid,
    id_maps: &ArchiveRestoreIdMaps,
    invocations: &[serde_json::Value],
) -> Result<(), ApiError> {
    for invocation in invocations {
        let source_invocation_id = archive_uuid(invocation, "id", "capability_invocation_id")?;
        let target_invocation_id = Uuid::now_v7();
        let target_span_id = archive_optional_uuid(invocation, "span_id")
            .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
        let target_requested_by_span_id = archive_optional_uuid(invocation, "requested_by_span_id")
            .and_then(|source_id| id_maps.runtime_spans.get(&source_id).copied());
        sqlx::query(
            r#"
            insert into capability_invocations (
                id,
                scope_id,
                flow_run_id,
                span_id,
                capability_id,
                requested_by_span_id,
                requester_kind,
                arguments_ref,
                authorization_status,
                authorization_reason,
                result_ref,
                normalized_result,
                started_at,
                finished_at,
                error_payload,
                created_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(target_invocation_id)
        .bind(scope_id)
        .bind(target_run_id)
        .bind(target_span_id)
        .bind(archive_required_string(
            invocation,
            "capability_id",
            "capability_invocation_capability_id",
        )?)
        .bind(target_requested_by_span_id)
        .bind(archive_required_string(
            invocation,
            "requester_kind",
            "capability_invocation_requester_kind",
        )?)
        .bind(archive_json_string(invocation, "arguments_ref"))
        .bind(archive_required_string(
            invocation,
            "authorization_status",
            "capability_invocation_authorization_status",
        )?)
        .bind(archive_json_string(invocation, "authorization_reason"))
        .bind(archive_json_string(invocation, "result_ref"))
        .bind(optional_archive_json_value(invocation, "normalized_result"))
        .bind(parse_optional_archive_time(
            archive_json_string(invocation, "started_at").as_deref(),
        )?)
        .bind(parse_optional_archive_time(
            archive_json_string(invocation, "finished_at").as_deref(),
        )?)
        .bind(optional_archive_json_value(invocation, "error_payload"))
        .bind(
            archive_json_string(invocation, "created_at")
                .as_deref()
                .map(parse_archive_time)
                .transpose()?
                .unwrap_or_else(OffsetDateTime::now_utc),
        )
        .execute(&mut **tx)
        .await?;
        insert_import_mapping(
            tx,
            job_id,
            "capability_invocation",
            &source_invocation_id.to_string(),
            target_invocation_id,
        )
        .await?;
    }

    Ok(())
}

async fn insert_import_mapping(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job_id: Uuid,
    entity_kind: &str,
    source_id: &str,
    target_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        insert into run_archive_import_mappings (
            id,
            scope_id,
            job_id,
            entity_kind,
            source_id,
            target_id,
            created_by,
            updated_by
        )
        select $1, jobs.scope_id, $2, $3, $4, $5, jobs.actor_user_id, jobs.actor_user_id
        from run_archive_import_jobs jobs
        where jobs.id = $2
        on conflict (job_id, entity_kind, source_id) do nothing
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(job_id)
    .bind(entity_kind)
    .bind(source_id)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn preassign_archive_ids(
    records: &[serde_json::Value],
    invalid_field: &'static str,
) -> Result<std::collections::HashMap<Uuid, Uuid>, ApiError> {
    let mut ids = std::collections::HashMap::with_capacity(records.len());
    for record in records {
        let source_id = archive_uuid(record, "id", invalid_field)?;
        if ids.insert(source_id, Uuid::now_v7()).is_some() {
            return Err(ControlPlaneError::InvalidInput(invalid_field).into());
        }
    }
    Ok(ids)
}

fn archive_uuid(
    value: &serde_json::Value,
    field: &str,
    invalid_field: &'static str,
) -> Result<Uuid, ApiError> {
    archive_json_string(value, field)
        .and_then(|value| Uuid::parse_str(&value).ok())
        .ok_or(ControlPlaneError::InvalidInput(invalid_field).into())
}

fn archive_optional_uuid(value: &serde_json::Value, field: &str) -> Option<Uuid> {
    archive_json_string(value, field).and_then(|value| Uuid::parse_str(&value).ok())
}

fn archive_required_string(
    value: &serde_json::Value,
    field: &str,
    invalid_field: &'static str,
) -> Result<String, ApiError> {
    archive_json_string(value, field).ok_or(ControlPlaneError::InvalidInput(invalid_field).into())
}

fn archive_json_string(value: &serde_json::Value, field: &str) -> Option<String> {
    match value.get(field)? {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        value => serde_json::from_value::<OffsetDateTime>(value.clone())
            .ok()
            .map(application_logs::format_time)
            .or_else(|| Some(value.to_string())),
    }
}

fn archive_json_i64(value: &serde_json::Value, field: &str) -> Option<i64> {
    value.get(field).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn archive_json_bool(value: &serde_json::Value, field: &str) -> Option<bool> {
    value.get(field).and_then(|value| {
        value
            .as_bool()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn optional_archive_json_value(
    value: &serde_json::Value,
    field: &str,
) -> Option<serde_json::Value> {
    match value.get(field)? {
        serde_json::Value::Null => None,
        field_value => Some(field_value.clone()),
    }
}

fn archive_json_value_or_object(value: &serde_json::Value, field: &str) -> serde_json::Value {
    value
        .get(field)
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

fn remap_source_item_refs(
    value: serde_json::Value,
    item_id_map: &std::collections::HashMap<Uuid, Uuid>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => Uuid::parse_str(&text)
            .ok()
            .and_then(|source_id| item_id_map.get(&source_id).copied())
            .map(|target_id| serde_json::Value::String(target_id.to_string()))
            .unwrap_or(serde_json::Value::String(text)),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(|item| remap_source_item_refs(item, item_id_map))
                .collect(),
        ),
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, remap_source_item_refs(value, item_id_map)))
                .collect(),
        ),
        other => other,
    }
}

fn parse_archive_time(value: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(ApiError::from)
}

fn parse_optional_archive_time(value: Option<&str>) -> Result<Option<OffsetDateTime>, ApiError> {
    value.map(parse_archive_time).transpose()
}

fn parse_flow_run_status(value: &str) -> Result<domain::FlowRunStatus, ApiError> {
    match value {
        "queued" => Ok(domain::FlowRunStatus::Queued),
        "running" => Ok(domain::FlowRunStatus::Running),
        "waiting_callback" => Ok(domain::FlowRunStatus::WaitingCallback),
        "waiting_human" => Ok(domain::FlowRunStatus::WaitingHuman),
        "paused" => Ok(domain::FlowRunStatus::Paused),
        "succeeded" => Ok(domain::FlowRunStatus::Succeeded),
        "incomplete" => Ok(domain::FlowRunStatus::Incomplete),
        "failed" => Ok(domain::FlowRunStatus::Failed),
        "cancelled" => Ok(domain::FlowRunStatus::Cancelled),
        _ => Err(ControlPlaneError::InvalidInput("flow_run_status").into()),
    }
}
