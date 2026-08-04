use control_plane::{
    flow::FlowService,
    ports::{UpdateFlowRunInput, UpdateFlowRunPayloadsInput},
};

use super::*;

struct ImportedRunFinalUpdate {
    target_run_id: Uuid,
    status: domain::FlowRunStatus,
    input_payload: serde_json::Value,
    output_payload: serde_json::Value,
    error_payload: Option<serde_json::Value>,
    finished_at: Option<OffsetDateTime>,
}

pub(crate) async fn restore_run_archive_v1(
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
        .bind(archive_json_string(
            &entry.flow_run_fact,
            "external_trace_id",
        ))
        .bind(archive_json_string(
            &entry.flow_run_fact,
            "compatibility_mode",
        ))
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
