alter table flow_runs
    add column flow_schema_version text not null default '',
    add column document_hash text not null default '';

create index flow_runs_debug_snapshot_document_idx
    on flow_runs (
        application_id,
        created_by,
        flow_draft_id,
        debug_session_id,
        flow_schema_version,
        document_hash,
        started_at desc,
        id desc
    );
