use control_plane::run_archive::RunArchiveService;
use control_plane_contracts::ports::{
    RestoreArchivedCallbackTaskInput, RestoreArchivedCheckpointInput, RestoreArchivedFlowRunInput,
    RestoreArchivedNodeRunInput, RestoreArchivedRunEventInput, RestoreRunArchiveEntryInput,
    RunArchiveRepository,
};

use super::*;

pub(crate) async fn restore_run_archive_v1(
    store: storage_durable_postgres::MainDurableStore,
    application: &domain::ApplicationRecord,
    actor: domain::ActorContext,
    job_id: Uuid,
    archive: RunArchiveV1Response,
    trace_rebuilder: Arc<dyn crate::runtime_ports::TraceProjectionRebuildPort>,
) -> Result<(), ApiError> {
    mark_run_archive_import_job_processing(&store, job_id).await?;
    let entries = archive
        .entries
        .into_iter()
        .map(to_restore_entry_input)
        .collect();
    let run_mappings = RunArchiveService::new(store.clone())
        .restore_run_archive_v1(
            store.for_actor(actor.clone()),
            application,
            &actor,
            job_id,
            archive.archive_version,
            entries,
        )
        .await?;
    let imported_run_mappings = run_mappings.clone();
    mark_run_archive_import_job_succeeded(&store, job_id, run_mappings).await?;
    let projection_warnings = rebuild_imported_run_trace_projections(
        &store,
        application.id,
        &imported_run_mappings,
        trace_rebuilder.as_ref(),
    )
    .await;
    if !projection_warnings.is_empty() {
        <_ as RunArchiveRepository>::update_run_archive_import_job_projection_warnings(
            &store,
            job_id,
            &imported_run_mappings,
            projection_warnings,
        )
        .await?;
    }
    Ok(())
}

async fn rebuild_imported_run_trace_projections(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: Uuid,
    run_mappings: &[(String, Uuid)],
    trace_rebuilder: &dyn crate::runtime_ports::TraceProjectionRebuildPort,
) -> Vec<serde_json::Value> {
    let mut warnings = Vec::new();
    for (source_run_id, target_run_id) in run_mappings {
        if let Err(error) = trace_rebuilder
            .rebuild(store, application_id, *target_run_id)
            .await
        {
            warnings.push(serde_json::json!({
                "code": "trace_projection_rebuild_failed",
                "source_run_id": source_run_id,
                "target_run_id": target_run_id.to_string(),
                "message": error.to_string()
            }));
        }
    }
    warnings
}

fn to_restore_entry_input(entry: RunArchiveV1EntryResponse) -> RestoreRunArchiveEntryInput {
    RestoreRunArchiveEntryInput {
        source_run_id: entry.source_run_id,
        flow_run: RestoreArchivedFlowRunInput {
            run_mode: entry.flow_run.run_mode,
            target_node_id: entry.flow_run.target_node_id,
            title: entry.flow_run.title,
            status: entry.flow_run.status,
            input_payload: entry.flow_run.input_payload,
            output_payload: entry.flow_run.output_payload,
            error_payload: entry.flow_run.error_payload,
            started_at: entry.flow_run.started_at,
            finished_at: entry.flow_run.finished_at,
            created_at: entry.flow_run.created_at,
        },
        flow_run_fact: entry.flow_run_fact,
        compiled_plan: entry.compiled_plan,
        node_runs: entry
            .node_runs
            .into_iter()
            .map(|node| RestoreArchivedNodeRunInput {
                id: node.id,
                node_id: node.node_id,
                node_type: node.node_type,
                node_alias: node.node_alias,
                status: node.status,
                input_payload: node.input_payload,
                output_payload: node.output_payload,
                error_payload: node.error_payload,
                metrics_payload: node.metrics_payload,
                debug_payload: node.debug_payload,
                started_at: node.started_at,
                finished_at: node.finished_at,
            })
            .collect(),
        checkpoints: entry
            .checkpoints
            .into_iter()
            .map(|checkpoint| RestoreArchivedCheckpointInput {
                id: checkpoint.id,
                node_run_id: checkpoint.node_run_id,
                status: checkpoint.status,
                reason: checkpoint.reason,
                locator_payload: checkpoint.locator_payload,
                variable_snapshot: checkpoint.variable_snapshot,
                external_ref_payload: checkpoint.external_ref_payload,
                created_at: checkpoint.created_at,
            })
            .collect(),
        callback_tasks: entry
            .callback_tasks
            .into_iter()
            .map(|task| RestoreArchivedCallbackTaskInput {
                id: task.id,
                node_run_id: task.node_run_id,
                callback_kind: task.callback_kind,
                status: task.status,
                request_payload: task.request_payload,
                response_payload: task.response_payload,
                external_ref_payload: task.external_ref_payload,
                created_at: task.created_at,
                completed_at: task.completed_at,
            })
            .collect(),
        events: entry
            .events
            .into_iter()
            .map(|event| RestoreArchivedRunEventInput {
                id: event.id,
                node_run_id: event.node_run_id,
                sequence: event.sequence,
                event_type: event.event_type,
                payload: event.payload,
                created_at: event.created_at,
            })
            .collect(),
        runtime_spans: entry.runtime_spans,
        runtime_events: entry.runtime_events,
        runtime_items: entry.runtime_items,
        context_projections: entry.context_projections,
        canonical_contents: entry.canonical_contents,
        invocation_context_bindings: entry.invocation_context_bindings,
        recovery_history: entry.recovery_history,
        resume_claims: entry.resume_claims,
        usage_ledger: entry.usage_ledger,
        model_failover_attempts: entry.model_failover_attempts,
        capability_invocations: entry.capability_invocations,
    }
}
