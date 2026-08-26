use super::*;

mod codec;
#[path = "../../../../application_runtime_archive/document.rs"]
mod document;
#[path = "../../../../application_runtime_archive/query.rs"]
mod query;
#[path = "../../../../application_runtime_archive/restore/mod.rs"]
mod restore;
pub(crate) mod routes;
#[path = "../../../../application_runtime_archive/upload_job.rs"]
mod upload_job;

const RUN_ARCHIVE_VERSION: i32 = 1;
const APPLICATION_RUN_ARCHIVE_SEMANTICS: &str = "application_run_archive_v1";

#[cfg(not(test))]
use codec::parse_run_archive_v1;
#[cfg(test)]
pub(in crate::routes::applications_group::application_runtime) use codec::{
    build_archive_from_trace_exports, parse_run_archive_v1,
};
use codec::{
    ensure_sha256_value, finalize_run_archive_v1_entries,
    normalize_run_archive_trace_tree_projection_status, normalize_sha256,
    run_archive_v1_entries_content_sha256, sha256_bytes,
};
use document::{application_run_archive_filename, build_run_archive_v1_document};
pub(super) use query::load_node_run_error_payloads;
use restore::restore_run_archive_v1;
use upload_job::{
    cleanup_run_archive_upload_chunks, create_run_archive_import_job, expected_archive_chunk_count,
    header_value, load_run_archive_import_job, load_run_archive_upload_session,
    load_upload_session_archive_bytes, mark_run_archive_import_job_failed,
    mark_run_archive_import_job_processing, mark_run_archive_import_job_succeeded,
    mark_upload_session_completed, persist_run_archive_chunk, persist_run_archive_upload_session,
    to_import_job_response, to_upload_session_response, CreateRunArchiveImportJobInput,
    PersistRunArchiveChunkInput, RUN_ARCHIVE_UPLOAD_MAX_BYTES, RUN_ARCHIVE_UPLOAD_MAX_CHUNKS,
    RUN_ARCHIVE_UPLOAD_MAX_CHUNK_BYTES,
};

pub(crate) use routes::{
    complete_run_archive_upload_session, create_run_archive_upload_session,
    export_application_run_archive, export_application_runs_archive, get_run_archive_import_job,
    upload_run_archive_chunk,
};
