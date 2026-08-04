use super::*;

fn required_json_field<T>(value: &serde_json::Value, field: &'static str) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let field_value = value
        .get(field)
        .cloned()
        .ok_or(ControlPlaneError::Conflict(field))?;
    Ok(serde_json::from_value(field_value)?)
}
pub(super) async fn build_run_archive_v1_document(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    application: &domain::ApplicationRecord,
    run_ids: Vec<Uuid>,
    exported_at: OffsetDateTime,
) -> Result<RunArchiveV1Response, ApiError> {
    let mut entries = Vec::with_capacity(run_ids.len());
    for run_id in &run_ids {
        let entry = build_run_archive_v1_entry(
            state.clone(),
            workspace_id,
            application,
            application.id,
            *run_id,
            exported_at,
        )
        .await?;
        entries.push(entry);
    }
    let manifest_entries = finalize_run_archive_v1_entries(&mut entries)?;
    let content_sha256 = run_archive_v1_entries_content_sha256(&entries)?;
    let exported_at_text = application_logs::format_time(exported_at);
    let selected_run_ids = run_ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    let manifest = RunArchiveV1ManifestResponse {
        archive_version: RUN_ARCHIVE_VERSION,
        archive_semantics: APPLICATION_RUN_ARCHIVE_SEMANTICS.to_string(),
        exported_at: exported_at_text.clone(),
        source_workspace_id: application.workspace_id.to_string(),
        source_application_id: application.id.to_string(),
        run_count: entries.len(),
        selected_run_ids,
        entries: manifest_entries,
        content_sha256: content_sha256.clone(),
        checksum: content_sha256.clone(),
    };
    let source = RunArchiveV1SourceResponse {
        source_kind: "application_run".to_string(),
        workspace_id: application.workspace_id.to_string(),
        application_id: application.id.to_string(),
        application_type: application.application_type.as_str().to_string(),
        application_name: application.name.clone(),
        exported_by_user_id: actor_user_id.to_string(),
        exported_at: exported_at_text.clone(),
        archive_builder: "api-server.application-runtime.run-archive-v1".to_string(),
    };

    Ok(RunArchiveV1Response {
        archive_version: RUN_ARCHIVE_VERSION,
        exported_at: exported_at_text,
        manifest,
        source,
        entries,
        content_digest: content_sha256,
    })
}

async fn build_run_archive_v1_entry(
    state: Arc<ApiState>,
    workspace_id: Uuid,
    application: &domain::ApplicationRecord,
    application_id: Uuid,
    run_id: Uuid,
    exported_at: OffsetDateTime,
) -> Result<RunArchiveV1EntryResponse, ApiError> {
    let export_document = build_application_run_trace_export_document(
        state.clone(),
        workspace_id,
        application,
        application_id,
        run_id,
        exported_at,
    )
    .await?;
    let export_value = export_document.value.clone();
    let flow_run = required_json_field(&export_value, "flow_run")?;
    let node_runs = required_json_field(&export_value, "node_runs")?;
    let checkpoints = required_json_field(&export_value, "checkpoints")?;
    let callback_tasks = required_json_field(&export_value, "callback_tasks")?;
    let events = required_json_field(&export_value, "events")?;
    let mut trace_tree =
        export_value
            .get("trace_tree")
            .cloned()
            .ok_or(ControlPlaneError::Conflict(
                "application_run_archive_trace_tree",
            ))?;
    normalize_run_archive_trace_tree_projection_status(&mut trace_tree);
    let export_warnings = required_json_field(&export_value, "export_warnings")?;
    let detail = <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_detail(
        &state.store,
        application_id,
        run_id,
    )
    .await?
    .ok_or(ControlPlaneError::NotFound("flow_run"))?;
    let compiled_plan = match detail.flow_run.compiled_plan_id {
        Some(compiled_plan_id) => {
            <MainDurableStore as OrchestrationRuntimeRepository>::get_compiled_plan(
                &state.store,
                compiled_plan_id,
            )
            .await?
            .map(serde_json::to_value)
            .transpose()?
        }
        None => None,
    };
    let runtime_spans = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_spans(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let runtime_events = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_events(
            &state.store,
            run_id,
            0,
        )
        .await?,
    )?;
    let runtime_items = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_runtime_items(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let context_projections = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_context_projections(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let usage_ledger = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_usage_ledger(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let model_failover_attempts = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_model_failover_attempt_ledger(
            &state.store,
            run_id,
        )
        .await?,
    )?;
    let capability_invocations = records_to_json_values(
        <MainDurableStore as OrchestrationRuntimeRepository>::list_capability_invocations(
            &state.store,
            run_id,
        )
        .await?,
    )?;

    Ok(RunArchiveV1EntryResponse {
        source_run_id: run_id.to_string(),
        content_digest: String::new(),
        flow_run,
        flow_run_fact: serde_json::to_value(&detail.flow_run)?,
        compiled_plan,
        node_runs,
        checkpoints,
        callback_tasks,
        events,
        runtime_spans,
        runtime_events,
        runtime_items,
        context_projections,
        usage_ledger,
        model_failover_attempts,
        capability_invocations,
        trace_tree,
        export_warnings,
    })
}

fn records_to_json_values<T: Serialize>(
    records: Vec<T>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    records
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApiError::from)
}

pub(super) fn application_run_archive_filename(
    application_name: &str,
    exported_at: &str,
    run_count: usize,
) -> String {
    let timestamp = exported_at
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!(
        "1flowbase-run-archive-{}-{}-{}runs.json",
        safe_filename_segment(application_name),
        timestamp,
        run_count
    )
}
