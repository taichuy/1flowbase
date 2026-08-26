alter table flow_runs
    add column debug_session_id text not null default '';

create index flow_runs_debug_snapshot_idx
    on flow_runs (application_id, created_by, flow_draft_id, debug_session_id, started_at desc, id desc);
