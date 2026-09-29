-- One lifecycle owner covers every durable reference to shared canonical data.
create or replace function reclaim_observation_body() returns trigger language plpgsql as $$
declare candidate uuid := (to_jsonb(OLD)->>TG_ARGV[0])::uuid; application uuid; hash text;
begin
    if candidate is null or not exists(select 1 from runtime_observation_body_ownership where content_id=candidate) then return null; end if;
    select application_id,content_hash into application,hash from runtime_canonical_contents where id=candidate;
    if not found then return null; end if;
    perform pg_advisory_xact_lock(hashtextextended('canonical-runtime:'||application::text||':'||hash,0));
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
create constraint trigger reclaim_client_section_body after delete on client_trajectory_sections
    deferrable initially deferred for each row execute function reclaim_observation_body('content_id');
create constraint trigger reclaim_replaced_client_section_body after update on client_trajectory_sections
    deferrable initially deferred for each row execute function reclaim_observation_body('content_id');
