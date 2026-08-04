use sha2::{Digest, Sha256};

use super::*;

fn extract_archive_from_zip(bytes: &[u8]) -> Result<RunArchiveV1Response, ApiError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_not_valid_zip"))?;

    if let Some(content) = read_zip_file(&mut zip, "archive.json")? {
        return parse_run_archive_json(&content);
    }
    if let Some(archive) = extract_archive_from_trace_export_zip(&mut zip)? {
        return Ok(archive);
    }

    let mut root_json_names = Vec::new();
    for index in 0..zip.len() {
        let file = zip
            .by_index(index)
            .map_err(|_| ControlPlaneError::InvalidInput("archive_zip_read_error"))?;
        let file_name = file.name().to_string();
        if file_name.ends_with(".json") && !file_name.contains('/') {
            root_json_names.push(file_name);
        }
    }

    root_json_names.sort_by_key(|name| name == "manifest.json");
    for file_name in root_json_names {
        let Some(content) = read_zip_file(&mut zip, &file_name)? else {
            continue;
        };
        if let Ok(archive) = parse_run_archive_json(&content) {
            return Ok(archive);
        }
    }

    Err(ControlPlaneError::InvalidInput("archive_json_not_found_in_zip").into())
}

fn read_zip_file(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    use std::io::Read;

    let mut file = match zip.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(_) => return Err(ControlPlaneError::InvalidInput("archive_zip_read_error").into()),
    };
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_zip_read_error"))?;
    Ok(Some(content))
}

fn parse_run_archive_json(content: &[u8]) -> Result<RunArchiveV1Response, ApiError> {
    serde_json::from_slice(content)
        .map_err(|_| ControlPlaneError::InvalidInput("archive_json_invalid").into())
}

fn extract_archive_from_trace_export_zip(
    zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
) -> Result<Option<RunArchiveV1Response>, ApiError> {
    let Some(manifest_content) = read_zip_file(zip, "manifest.json")? else {
        return Ok(None);
    };
    let manifest: ApplicationRunSelectedExportManifestResponse =
        match serde_json::from_slice(&manifest_content) {
            Ok(manifest) => manifest,
            Err(_) => return Ok(None),
        };
    if manifest.export_version != APPLICATION_RUN_TRACE_EXPORT_VERSION
        || manifest.run_count == 0
        || manifest.entries.len() != manifest.run_count
    {
        return Err(ControlPlaneError::InvalidInput("archive_trace_manifest").into());
    }

    let mut documents = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let content = read_zip_file(zip, &entry.filename)?.ok_or(
            ControlPlaneError::InvalidInput("archive_trace_entry_missing"),
        )?;
        let document: ApplicationRunTraceExportResponse = serde_json::from_slice(&content)
            .map_err(|_| ControlPlaneError::InvalidInput("archive_trace_entry_json"))?;
        if document.export_version != APPLICATION_RUN_TRACE_EXPORT_VERSION
            || document.flow_run.id != entry.run_id
        {
            return Err(ControlPlaneError::InvalidInput("archive_trace_entry").into());
        }
        documents.push(document);
    }

    Ok(Some(build_archive_from_trace_exports(manifest, documents)?))
}

pub(in crate::routes::applications_group::application_runtime) fn build_archive_from_trace_exports(
    manifest: ApplicationRunSelectedExportManifestResponse,
    documents: Vec<ApplicationRunTraceExportResponse>,
) -> Result<RunArchiveV1Response, ApiError> {
    let first_document = documents
        .first()
        .ok_or(ControlPlaneError::InvalidInput("archive_trace_entries"))?;
    let exported_by_user_id = if first_document.run.principal.kind == "user" {
        first_document.run.principal.id.clone().unwrap_or_default()
    } else {
        String::new()
    };
    let source = RunArchiveV1SourceResponse {
        source_kind: "application_run_trace_export_zip".to_string(),
        workspace_id: "unknown".to_string(),
        application_id: manifest.application_id.clone(),
        application_type: first_document.run.application_type.clone(),
        application_name: "imported application runs".to_string(),
        exported_by_user_id,
        exported_at: manifest.exported_at.clone(),
        archive_builder: "api-server.application-runtime.trace-export-zip-import-v1".to_string(),
    };
    let mut entries = documents
        .into_iter()
        .map(trace_export_to_archive_entry)
        .collect::<Result<Vec<_>, ApiError>>()?;
    let manifest_entries = finalize_run_archive_v1_entries(&mut entries)?;
    let content_sha256 = run_archive_v1_entries_content_sha256(&entries)?;
    let archive_manifest = RunArchiveV1ManifestResponse {
        archive_version: RUN_ARCHIVE_VERSION,
        archive_semantics: APPLICATION_RUN_ARCHIVE_SEMANTICS.to_string(),
        exported_at: manifest.exported_at.clone(),
        source_workspace_id: "unknown".to_string(),
        source_application_id: manifest.application_id,
        run_count: entries.len(),
        selected_run_ids: manifest.selected_run_ids,
        entries: manifest_entries,
        content_sha256: content_sha256.clone(),
        checksum: content_sha256.clone(),
    };

    Ok(RunArchiveV1Response {
        archive_version: RUN_ARCHIVE_VERSION,
        exported_at: manifest.exported_at,
        manifest: archive_manifest,
        source,
        entries,
        content_digest: content_sha256,
    })
}

fn trace_export_to_archive_entry(
    document: ApplicationRunTraceExportResponse,
) -> Result<RunArchiveV1EntryResponse, ApiError> {
    let source_run_id = document.flow_run.id.clone();
    let flow_run_fact = trace_export_flow_run_fact(&document.flow_run)?;
    let mut trace_tree = serde_json::to_value(document.trace_tree)?;
    normalize_run_archive_trace_tree_projection_status(&mut trace_tree);

    Ok(RunArchiveV1EntryResponse {
        source_run_id,
        content_digest: String::new(),
        flow_run: document.flow_run,
        flow_run_fact,
        compiled_plan: None,
        node_runs: document.node_runs,
        checkpoints: document.checkpoints,
        callback_tasks: document.callback_tasks,
        events: document.events,
        runtime_spans: Vec::new(),
        runtime_events: Vec::new(),
        runtime_items: Vec::new(),
        context_projections: Vec::new(),
        usage_ledger: Vec::new(),
        model_failover_attempts: Vec::new(),
        capability_invocations: Vec::new(),
        trace_tree,
        export_warnings: document.export_warnings,
    })
}

pub(super) fn normalize_run_archive_trace_tree_projection_status(
    trace_tree: &mut serde_json::Value,
) {
    let Some(projection_status) = trace_tree
        .get_mut("projection_status")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };

    projection_status.insert("last_attempt_at".to_string(), serde_json::Value::Null);
    projection_status.insert("last_success_at".to_string(), serde_json::Value::Null);
}

pub(super) fn finalize_run_archive_v1_entries(
    entries: &mut [RunArchiveV1EntryResponse],
) -> Result<Vec<RunArchiveV1ManifestEntryResponse>, ApiError> {
    for entry in entries.iter_mut() {
        entry.content_digest = run_archive_v1_entry_content_sha256(entry)?;
    }

    Ok(entries
        .iter()
        .map(|entry| RunArchiveV1ManifestEntryResponse {
            source_run_id: entry.source_run_id.clone(),
            content_sha256: entry.content_digest.clone(),
            content_digest: entry.content_digest.clone(),
        })
        .collect())
}

pub(super) fn run_archive_v1_entries_content_sha256(
    entries: &[RunArchiveV1EntryResponse],
) -> Result<String, ApiError> {
    let payload = entries
        .iter()
        .map(run_archive_v1_entry_digest_payload)
        .collect::<Vec<_>>();
    Ok(sha256_bytes(&serde_json::to_vec(&payload)?))
}

fn run_archive_v1_entry_content_sha256(
    entry: &RunArchiveV1EntryResponse,
) -> Result<String, ApiError> {
    Ok(sha256_bytes(&serde_json::to_vec(
        &run_archive_v1_entry_digest_payload(entry),
    )?))
}

fn run_archive_v1_entry_digest_payload(entry: &RunArchiveV1EntryResponse) -> serde_json::Value {
    let mut value = serde_json::json!({
        "source_run_id": &entry.source_run_id,
        "flow_run": &entry.flow_run,
        "flow_run_fact": &entry.flow_run_fact,
        "compiled_plan": &entry.compiled_plan,
        "node_runs": &entry.node_runs,
        "checkpoints": &entry.checkpoints,
        "callback_tasks": &entry.callback_tasks,
        "events": &entry.events,
        "runtime_spans": &entry.runtime_spans,
        "runtime_events": &entry.runtime_events,
        "runtime_items": &entry.runtime_items,
        "context_projections": &entry.context_projections,
        "usage_ledger": &entry.usage_ledger,
        "model_failover_attempts": &entry.model_failover_attempts,
        "capability_invocations": &entry.capability_invocations,
    });
    remove_run_archive_digest_volatile_fields(&mut value);
    value
}

fn remove_run_archive_digest_volatile_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                remove_run_archive_digest_volatile_fields(item);
            }
        }
        serde_json::Value::Object(object) => {
            object.remove("updated_at");
            for item in object.values_mut() {
                remove_run_archive_digest_volatile_fields(item);
            }
        }
        _ => {}
    }
}

fn trace_export_flow_run_fact(flow_run: &FlowRunResponse) -> Result<serde_json::Value, ApiError> {
    let mut value = serde_json::to_value(flow_run)?;
    let object = value
        .as_object_mut()
        .ok_or(ControlPlaneError::InvalidInput("archive_trace_flow_run"))?;
    if let Some(external_user) = flow_run.expand_id.as_ref() {
        object.insert(
            "external_user".to_string(),
            serde_json::Value::String(external_user.clone()),
        );
    }
    object.insert(
        "document_hash".to_string(),
        serde_json::Value::String("imported-trace-export".to_string()),
    );
    Ok(value)
}

pub(in crate::routes::applications_group::application_runtime) fn parse_run_archive_v1(
    bytes: &[u8],
) -> Result<RunArchiveV1Response, ApiError> {
    // Try to parse as JSON first (single file archive)
    let archive: RunArchiveV1Response = match serde_json::from_slice(bytes) {
        Ok(archive) => archive,
        Err(_) => {
            // If JSON parsing fails, try to extract from ZIP
            extract_archive_from_zip(bytes)?
        }
    };
    if archive.archive_version != RUN_ARCHIVE_VERSION
        || archive.manifest.archive_version != RUN_ARCHIVE_VERSION
        || archive.manifest.archive_semantics != APPLICATION_RUN_ARCHIVE_SEMANTICS
    {
        return Err(ControlPlaneError::InvalidInput("archive_version").into());
    }
    let content_sha256 = run_archive_v1_entries_content_sha256(&archive.entries)?;
    if normalize_sha256(&content_sha256) != normalize_sha256(&archive.manifest.content_sha256) {
        return Err(ControlPlaneError::InvalidInput("archive_content_sha256").into());
    }
    if normalize_sha256(&archive.manifest.checksum)
        != normalize_sha256(&archive.manifest.content_sha256)
        || normalize_sha256(&archive.content_digest)
            != normalize_sha256(&archive.manifest.content_sha256)
    {
        return Err(ControlPlaneError::InvalidInput("archive_checksum").into());
    }
    if archive.entries.is_empty()
        || archive.entries.len() != archive.manifest.run_count
        || archive.entries.len() != archive.manifest.entries.len()
    {
        return Err(ControlPlaneError::InvalidInput("archive_entries").into());
    }
    for (entry, manifest_entry) in archive.entries.iter().zip(&archive.manifest.entries) {
        if entry.source_run_id != manifest_entry.source_run_id {
            return Err(ControlPlaneError::InvalidInput("archive_entries").into());
        }
        let entry_content_digest = run_archive_v1_entry_content_sha256(entry)?;
        if normalize_sha256(&entry_content_digest) != normalize_sha256(&entry.content_digest) {
            return Err(ControlPlaneError::InvalidInput("archive_entry_digest").into());
        }
        if normalize_sha256(&entry_content_digest)
            != normalize_sha256(&manifest_entry.content_sha256)
        {
            return Err(ControlPlaneError::InvalidInput("archive_entry_sha256").into());
        }
        if normalize_sha256(&entry.content_digest)
            != normalize_sha256(&manifest_entry.content_digest)
        {
            return Err(ControlPlaneError::InvalidInput("archive_entry_digest").into());
        }
    }

    Ok(archive)
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn normalize_sha256(value: &str) -> String {
    value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase()
}

pub(super) fn ensure_sha256_value(value: &str, field: &'static str) -> Result<(), ApiError> {
    let normalized = normalize_sha256(value);
    if normalized.len() == 64 && normalized.chars().all(|value| value.is_ascii_hexdigit()) {
        return Ok(());
    }

    Err(ControlPlaneError::InvalidInput(field).into())
}
