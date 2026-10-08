-- Keep bulk reclamation atomic without retaining one advisory lock per content body.
-- Agent Logs has an application ingest/delete gate and can share one aggregate lock.
-- Native reference owners retain their per-content lock and established lock order.
create or replace function reclaim_observation_body() returns trigger language plpgsql as $$
declare candidate uuid := (to_jsonb(OLD)->>TG_ARGV[0])::uuid; application uuid; hash text; application_kind text;
begin
    if candidate is null or not exists(select 1 from runtime_observation_body_ownership where content_id=candidate) then return null; end if;
    select c.application_id,c.content_hash,a.application_type into application,hash,application_kind
      from runtime_canonical_contents c left join applications a on a.id=c.application_id where c.id=candidate;
    if not found then return null; end if;
    perform pg_advisory_xact_lock(hashtextextended(
      case when application_kind='agent_logs' then 'canonical-runtime-application:'||application::text
      else 'canonical-runtime:'||application::text||':'||hash end,0));
    perform 1 from runtime_canonical_contents where id=candidate for update;
    delete from runtime_canonical_contents c where c.id=candidate
      and not exists(select 1 from runtime_events e where e.observation_body_content_id=c.id)
      and not exists(select 1 from client_trajectory_sections s where s.content_id=c.id)
      and not exists(select 1 from runtime_context_projections p where p.actual_content_id=c.id)
      and not exists(select 1 from flow_run_recovery_history h where h.recovery_content_id=c.id)
      and not exists(select 1 from runtime_legacy_shadow_rows s where s.canonical_content_id=c.id);
    return null;
end;
$$;

-- FK actions probe by the referenced key alone; app/flow-prefixed indexes cannot serve
-- those probes. Index the cascade and final-reference lookup edges, not a guessed row limit.
create index application_log_receipt_record_owner on application_log_upload_receipts(record_id);
create index application_log_receipt_step_owner on application_log_upload_receipts(step_id);
create index client_trajectory_capture_record_owner on client_trajectory_captures(record_id) where record_id is not null;
create index application_run_message_record_owner on application_run_conversation_message_items(record_id) where record_id is not null;
create index client_trajectory_step_request_owner on client_trajectory_steps(request_id);
create index client_trajectory_section_request_owner on client_trajectory_sections(request_id);
create index client_trajectory_section_step_owner on client_trajectory_sections(step_id);
create index runtime_context_actual_content_owner on runtime_context_projections(actual_content_id) where actual_content_id is not null;
create index flow_recovery_content_owner on flow_run_recovery_history(recovery_content_id) where recovery_content_id is not null;
create index runtime_legacy_canonical_content_owner on runtime_legacy_shadow_rows(canonical_content_id) where canonical_content_id is not null;
