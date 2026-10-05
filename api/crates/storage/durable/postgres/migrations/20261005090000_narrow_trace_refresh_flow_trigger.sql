-- Sequence allocation bumps runtime_event_sequence_high_water once per runtime
-- event. That counter is not a projection source, so it must not requeue a full
-- trace rebuild. Every other column stays a refresh source; add new flow_runs
-- columns here unless they are pure counters like the high-water.
drop trigger if exists trace_refresh_flow on flow_runs;
create trigger trace_refresh_flow_insert after insert on flow_runs
for each row execute function enqueue_application_run_trace_refresh();
create trigger trace_refresh_flow after update of
    application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, target_node_id,
    status, input_payload, output_payload, error_payload, created_by, started_at,
    finished_at, created_at, debug_session_id, flow_schema_version, document_hash,
    api_key_id, publication_version_id, external_user, external_conversation_id,
    external_trace_id, compatibility_mode, idempotency_key, updated_at, title, scope_id,
    updated_by, import_job_id, import_source_run_id, assistant_conversation_id,
    log_context, raw_json_payloads
on flow_runs for each row execute function enqueue_application_run_trace_refresh();
