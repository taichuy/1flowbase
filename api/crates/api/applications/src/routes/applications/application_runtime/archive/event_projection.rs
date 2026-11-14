//! JSON archive business view, selected before HTTP encoding. Restore-only raw
//! rows carry compiled plans, environment snapshots and opaque content bindings;
//! they are deliberately excluded from this typed event contract.
use super::*;

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(super) struct ArchiveDocument {
    archive_version: i32,
    exported_at: String,
    manifest: RunArchiveV1ManifestResponse,
    source: RunArchiveV1SourceResponse,
    entries: Vec<ArchiveEntry>,
    content_digest: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct ArchiveEntry {
    source_run_id: String,
    content_digest: String,
    flow_run: FlowRunResponse,
    node_runs: Vec<NodeRunResponse>,
    checkpoints: Vec<CheckpointResponse>,
    callback_tasks: Vec<CallbackTaskResponse>,
    events: Vec<RunEventResponse>,
    export_warnings: Vec<ApplicationRunTraceExportWarningResponse>,
}

impl ArchiveDocument {
    pub(super) fn from_archive(archive: &RunArchiveV1Response) -> Self {
        Self {
            archive_version: archive.archive_version,
            exported_at: archive.exported_at.clone(),
            manifest: archive.manifest.clone(),
            source: archive.source.clone(),
            entries: archive
                .entries
                .iter()
                .map(|entry| ArchiveEntry {
                    source_run_id: entry.source_run_id.clone(),
                    content_digest: entry.content_digest.clone(),
                    flow_run: entry.flow_run.clone(),
                    node_runs: entry.node_runs.clone(),
                    checkpoints: entry.checkpoints.clone(),
                    callback_tasks: entry.callback_tasks.clone(),
                    events: entry.events.clone(),
                    export_warnings: entry.export_warnings.clone(),
                })
                .collect(),
            content_digest: archive.content_digest.clone(),
        }
    }
}
