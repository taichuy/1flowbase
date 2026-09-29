-- Occurrence identity and immutable section content have independent lifetimes.
-- Retain historical runtime event anchors; only new direct directory rows omit them.
alter table client_trajectory_steps
    alter column event_id drop not null,
    add column raw_json_payloads jsonb not null default '{}'::jsonb,
    add column observed_at text,
    add column semantic_metadata_restored boolean not null default false;
alter table client_trajectory_sections drop constraint client_trajectory_sections_pkey;
alter table client_trajectory_sections
    alter column event_id drop not null,
    add column id uuid,
    add column content_id uuid references runtime_canonical_contents(id),
    add column content_path text[] not null default '{}',
    add column body_kind text not null default 'legacy'
        check (body_kind in ('legacy','content','timing')),
    add column observed_at text,
    add column raw_json_payloads jsonb not null default '{}'::jsonb,
    add column semantic_legacy_retained boolean not null default false,
    add column value_hash text,
    add column value_byte_size bigint check (value_byte_size >= 0);
update client_trajectory_sections set id=event_id;
alter table client_trajectory_sections alter column id set not null;
alter table client_trajectory_sections add primary key(id);
create unique index client_trajectory_section_legacy_event on client_trajectory_sections(event_id)
    where event_id is not null;
create index client_trajectory_section_content on client_trajectory_sections(content_id)
    where content_id is not null;
alter table client_trajectory_sections add constraint client_trajectory_section_body_shape check (
    (body_kind='legacy' and event_id is not null and content_id is null)
    or (body_kind='content' and content_id is not null and observed_at is not null
        and value_hash is not null and value_byte_size is not null)
    or (body_kind='timing' and section='timing' and content_id is null
        and observed_at is not null and value_hash is not null and value_byte_size is not null)
);
-- Paths use protocol field names, never arbitrary original JSON keys. Bodies with
-- NUL keys remain valid immutable content and are restored as complete JSON in Rust.
alter table client_trajectory_sections add constraint client_trajectory_section_content_path check (
    cardinality(content_path)=0 or
    (cardinality(content_path)=1 and content_path[1] in ('arguments','input','output','content','summary'))
);

-- The trigger continues to accept old-format importers without recreating new events.
create or replace function project_client_trajectory() returns trigger language plpgsql as $$
begin
    if new.event_type <> 'client_protocol_trajectory' then return new; end if;
    if new.payload->'fact'->>'kind' = 'integrity' then
        insert into client_trajectory_captures(request_id,event_id,flow_run_id,node_run_id,event_sequence,status,dropped_count,persist_failed_count)
        values ((new.payload->>'request_id')::uuid,new.id,new.flow_run_id,new.node_run_id,new.sequence,
            new.payload->'fact'->>'status',(new.payload->'fact'->>'dropped_count')::bigint,
            (new.payload->'fact'->>'persist_failed_count')::bigint)
        on conflict(request_id) do update set event_id=excluded.event_id,event_sequence=excluded.event_sequence,status=excluded.status,
            dropped_count=excluded.dropped_count,persist_failed_count=excluded.persist_failed_count
        where client_trajectory_captures.flow_run_id=excluded.flow_run_id
            and client_trajectory_captures.node_run_id is not distinct from excluded.node_run_id
            and client_trajectory_captures.event_sequence < excluded.event_sequence;
    elsif new.payload->'fact'->>'kind' = 'step' then
        insert into client_trajectory_steps(id,event_id,request_id,flow_run_id,node_run_id,event_sequence,metadata,raw_json_payloads,semantic_metadata_restored)
        values ((new.payload->'fact'->'step'->>'id')::uuid,new.id,(new.payload->>'request_id')::uuid,
            new.flow_run_id,new.node_run_id,new.sequence,new.payload->'fact'->'step','{}',false)
        on conflict(id) do update set event_id=excluded.event_id,metadata=excluded.metadata,
            raw_json_payloads='{}',semantic_metadata_restored=false
        where client_trajectory_steps.request_id=excluded.request_id
            and client_trajectory_steps.flow_run_id=excluded.flow_run_id;
    elsif new.payload->'fact'->>'kind' = 'section' then
        insert into client_trajectory_sections(id,event_id,request_id,step_id,flow_run_id,node_run_id,section,event_sequence)
        values (new.id,new.id,(new.payload->>'request_id')::uuid,(new.payload->'fact'->>'step_id')::uuid,
            new.flow_run_id,new.node_run_id,new.payload->'fact'->>'section',new.sequence);
    end if;
    return new;
end;
$$;

alter table flow_runs add column runtime_event_sequence_high_water bigint not null default 0
    check (runtime_event_sequence_high_water >= 0);
update flow_runs f set runtime_event_sequence_high_water=greatest(
    coalesce((select max(sequence) from runtime_events e where e.flow_run_id=f.id),0),
    coalesce((select max(event_sequence) from client_trajectory_steps s where s.flow_run_id=f.id),0),
    coalesce((select max(event_sequence) from client_trajectory_sections s where s.flow_run_id=f.id),0)
);
-- Batch callers reserve their first sequence and add offsets. One statement-level
-- update records the entire reservation after each chunk, without changing IDs.
create function maintain_runtime_event_sequence_high_water() returns trigger language plpgsql as $$
begin
    update flow_runs f set runtime_event_sequence_high_water=greatest(f.runtime_event_sequence_high_water,b.last_sequence)
    from (select flow_run_id,max(sequence) last_sequence from appended_runtime_events group by flow_run_id) b
    where f.id=b.flow_run_id;
    return null;
end $$;
create trigger runtime_event_sequence_high_water after insert on runtime_events
    referencing new table as appended_runtime_events for each statement
    execute function maintain_runtime_event_sequence_high_water();

create function client_trajectory_step_storage_body(step_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_steps; original json;
begin
    select * into directory from client_trajectory_steps where id=step_id and flow_run_id=owner_run;
    if not found then raise exception 'client step owner unavailable'; end if;
    if directory.semantic_metadata_restored then
        return runtime_original_json(directory.metadata,directory.raw_json_payloads,'metadata');
    end if;
    select runtime_original_json(e.payload,e.raw_json_payloads,'payload') into original
    from runtime_events e where e.id=directory.event_id and e.flow_run_id=owner_run;
    if not found then raise exception 'client step legacy anchor unavailable'; end if;
    -- Return the entire old payload. Rust takes fact.step after decoding; JSON
    -- extraction in PostgreSQL can reject NUL anywhere in a historical original.
    return original;
end $$;

create function client_trajectory_section_original_value(section_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_sections; original json; body json;
begin
    select * into directory from client_trajectory_sections where id=section_id and flow_run_id=owner_run;
    if not found then raise exception 'client section owner unavailable'; end if;
    if directory.body_kind='legacy' then
        select runtime_original_json(e.payload,e.raw_json_payloads,'payload') into original
        from runtime_events e where e.id=directory.event_id and e.flow_run_id=owner_run;
        if not found then raise exception 'client section legacy anchor unavailable'; end if;
        return original #> '{fact,value}';
    elsif directory.body_kind='timing' then
        return json_build_object('observed_at',runtime_original_json(to_jsonb(directory.observed_at),directory.raw_json_payloads,'observed_at'));
    end if;
    select runtime_original_json(c.content,c.raw_json_payloads,'content') into body
    from runtime_canonical_contents c join flow_runs f on f.application_id=c.application_id and f.scope_id=c.scope_id
    where c.id=directory.content_id and f.id=owner_run;
    if not found then raise exception 'client section content owner unavailable'; end if;
    if cardinality(directory.content_path)=0 then return body; end if;
    body := body #> directory.content_path;
    if body is null then raise exception 'client section locator unavailable'; end if;
    return body;
end $$;

-- Historical event IDs remain valid and keep their complete original DTO wrapper.
-- Only a Rust-verified value is replaced by this adapter-owned locator.
create function client_trajectory_event_original_payload(projection jsonb,originals jsonb,owner_run uuid)
returns json language plpgsql stable as $$
declare original json := runtime_original_json(projection,originals,'payload'); section_value json; fact json;
begin
    if not projection ? '_client_semantic_ref' then return original; end if;
    section_value := client_trajectory_section_original_value((projection#>>'{_client_semantic_ref,section_id}')::uuid,owner_run);
    fact := (select json_object_agg(key,field) from (
        select f.key,f.value as field from json_each(original->'fact') f where f.key<>'value'
        union all select 'value',section_value
    ) fields);
    return (select json_object_agg(key,field) from (
        select f.key,f.value as field from json_each(original) f where f.key not in ('fact','_client_semantic_ref')
        union all select 'fact',fact
    ) fields);
end $$;

-- Text is a PostgreSQL query projection. Use safe projections for its text-only
-- extraction, while section readers/equality checks restore full originals in Rust.
create or replace function application_run_log_task_last_client_text(members uuid[]) returns text
language sql stable as $$
    select body.content
    from unnest(members) with ordinality member(run_id,position)
    join client_trajectory_steps s on s.flow_run_id=member.run_id
        and s.metadata->>'category'='assistant' and s.metadata->>'origin'='emitted'
    join client_trajectory_sections p on p.flow_run_id=s.flow_run_id and p.step_id=s.id and p.section='result'
    left join runtime_events e on e.id=p.event_id and p.body_kind='legacy'
    left join runtime_canonical_contents c on c.id=p.content_id
        and c.application_id=(select application_id from flow_runs where id=p.flow_run_id)
        and c.scope_id=(select scope_id from flow_runs where id=p.flow_run_id)
    cross join lateral (select case p.body_kind
        when 'legacy' then e.payload#>'{fact,value}'
        when 'content' then case when cardinality(p.content_path)=0 then c.content else c.content#>p.content_path end
        end as value) raw
    cross join lateral (select case jsonb_typeof(raw.value)
        when 'string' then raw.value#>>'{}'
        when 'array' then (select string_agg(case part->>'type'
            when 'output_text' then part->>'text' when 'text' then part->>'text'
            when 'refusal' then part->>'refusal' end,'' order by position)
            from jsonb_array_elements(raw.value) with ordinality item(part,position))
        end as content) body
    where nullif(btrim(body.content),'') is not null
    order by member.position desc,p.event_sequence desc limit 1
$$;
