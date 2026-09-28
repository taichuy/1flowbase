-- Native/supplier observation identity remains an operational lookup anchor.
-- The exact body is immutable content owned by the same application; it is not
-- copied into each indexed event row. Client frames have their own request archive.
alter table runtime_events add column observation_body_content_id uuid
    references runtime_canonical_contents(id);
create index runtime_event_observation_body_owner
    on runtime_events(observation_body_content_id)
    where observation_body_content_id is not null;

create function runtime_event_original_payload(projection jsonb, originals jsonb, owner_run uuid)
returns json language plpgsql stable as $$
declare
    original json := runtime_original_json(projection, originals, 'payload');
    reference jsonb := projection->'_observation_body_ref';
    body json;
begin
    -- @field-contract-compat source=runtime_events.payload.body alias=inline_observation_body remove_by=2026-10-31
    -- Older observations remain lossless until the explicit historical mover has
    -- validated their owners and native/client/protocol references.
    if reference is null then return original; end if;
    select runtime_original_json(c.content,c.raw_json_payloads,'content') into body
      from runtime_canonical_contents c
      join flow_runs f on f.application_id=c.application_id and f.scope_id=c.scope_id
      where c.id=(reference->>'content_id')::uuid
        and c.application_id=(reference->>'application_id')::uuid
        and f.id=owner_run;
    if not found then raise exception 'observation body owner unavailable'; end if;
    -- JSON (not JSONB) preserves original NUL-containing strings and field names.
    return (select json_object_agg(k,v) from (
        select key k,value v from json_each(original)
            where key not in ('_observation_body_ref','body')
        union all select 'body',body
    ) fields);
end;
$$;

-- Only content first created by observation archiving is reclaimed automatically.
-- Pre-existing generic canonical content keeps its original owner's lifecycle.
create table runtime_observation_body_ownership (
    content_id uuid primary key references runtime_canonical_contents(id) on delete cascade
);
create function reclaim_observation_body() returns trigger language plpgsql as $$
declare
    candidate uuid := (to_jsonb(OLD)->>TG_ARGV[0])::uuid;
    application uuid;
    hash text;
begin
    if candidate is null or not exists(select 1 from runtime_observation_body_ownership where content_id=candidate) then return null; end if;
    select application_id,content_hash into application,hash from runtime_canonical_contents where id=candidate;
    if not found then return null; end if;
    -- Match content insertion's serialization; row locking also orders FK writers.
    perform pg_advisory_xact_lock(hashtextextended('canonical-runtime:'||application::text||':'||hash,0));
    perform 1 from runtime_canonical_contents where id=candidate for update;
    delete from runtime_canonical_contents c where c.id=candidate
      and not exists(select 1 from runtime_events e where e.observation_body_content_id=c.id)
      and not exists(select 1 from runtime_context_projections p where p.actual_content_id=c.id)
      and not exists(select 1 from flow_run_recovery_history h where h.recovery_content_id=c.id)
      and not exists(select 1 from runtime_legacy_shadow_rows s where s.canonical_content_id=c.id);
    return null;
end;
$$;
-- Deferred until all run/app cascades have removed their durable references.
create constraint trigger reclaim_runtime_event_observation_body after delete on runtime_events
    deferrable initially deferred for each row execute function reclaim_observation_body('observation_body_content_id');
create constraint trigger reclaim_context_observation_body after delete on runtime_context_projections
    deferrable initially deferred for each row execute function reclaim_observation_body('actual_content_id');
create constraint trigger reclaim_recovery_observation_body after delete on flow_run_recovery_history
    deferrable initially deferred for each row execute function reclaim_observation_body('recovery_content_id');
create constraint trigger reclaim_shadow_observation_body after delete on runtime_legacy_shadow_rows
    deferrable initially deferred for each row execute function reclaim_observation_body('canonical_content_id');
