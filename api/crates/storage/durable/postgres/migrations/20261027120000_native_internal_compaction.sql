-- Internal compaction owns durable originals and candidate checkpoints independently
-- of event/projection retention. Bodies stay in the canonical lossless JSON archive.
create table runtime_internal_compaction_contents (
    scope_id uuid not null,
    application_id uuid not null,
    session_key text not null,
    content_id uuid not null,
    content_kind text not null check (content_kind in ('source', 'checkpoint')),
    created_at timestamptz not null default now(),
    primary key (scope_id, application_id, session_key, content_id, content_kind),
    foreign key (application_id, scope_id) references applications(id, scope_id) on delete cascade,
    foreign key (content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id) on delete cascade
);
create index runtime_internal_compaction_content_owner
    on runtime_internal_compaction_contents(content_id);

create table runtime_internal_compaction_heads (
    scope_id uuid not null,
    application_id uuid not null,
    session_key text not null,
    source_content_id uuid not null,
    source_hashes text[] not null,
    prefix_hash text not null,
    checkpoint_content_id uuid,
    updated_at timestamptz not null default now(),
    primary key (scope_id, application_id, session_key),
    foreign key (application_id, scope_id) references applications(id, scope_id) on delete cascade,
    foreign key (source_content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id),
    foreign key (checkpoint_content_id, scope_id, application_id)
        references runtime_canonical_contents(id, scope_id, application_id)
);
create index runtime_internal_compaction_source_owner
    on runtime_internal_compaction_heads(source_content_id);
create index runtime_internal_compaction_checkpoint_owner
    on runtime_internal_compaction_heads(checkpoint_content_id)
    where checkpoint_content_id is not null;

-- Preserve existing reclamation semantics, adding only the new durable owners.
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
      and not exists(select 1 from runtime_legacy_shadow_rows s where s.canonical_content_id=c.id)
      and not exists(select 1 from runtime_internal_compaction_contents a where a.content_id=c.id)
      and not exists(select 1 from runtime_internal_compaction_heads h where h.source_content_id=c.id or h.checkpoint_content_id=c.id);
    return null;
end;
$$;
