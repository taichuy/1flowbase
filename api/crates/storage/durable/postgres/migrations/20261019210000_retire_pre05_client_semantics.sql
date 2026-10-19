-- Client trajectory semantic layouts predating 0.5 are no longer a supported
-- read/write format. Keep the original rows for explicit archival/export; never
-- delete or silently reinterpret them when applying the current schema.
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
    elsif new.payload->'fact'->>'kind' in ('step','section') then
        raise exception 'legacy client trajectory event format is unsupported; use the current trajectory append interface'
            using errcode='22023';
    end if;
    return new;
end;
$$;

create or replace function client_trajectory_step_storage_body(step_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_steps;
begin
    select * into directory from client_trajectory_steps where id=step_id and flow_run_id=owner_run;
    if not found then raise exception 'client step owner unavailable'; end if;
    if not directory.semantic_metadata_restored then
        raise exception 'legacy client trajectory step format is unsupported' using errcode='22023';
    end if;
    return client_trajectory_step_dense_metadata(directory.metadata,directory.raw_json_payloads,
        directory.id,directory.request_id,directory.flow_run_id,directory.node_run_id,
        directory.metadata_compact,directory.observed_at,directory.metadata_layout);
end $$;

create or replace function client_trajectory_section_original_value(section_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_sections; body json;
begin
    select * into directory from client_trajectory_sections where id=section_id and flow_run_id=owner_run;
    if not found then raise exception 'client section owner unavailable'; end if;
    if directory.body_kind='legacy' then
        raise exception 'legacy client trajectory section format is unsupported' using errcode='22023';
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

create or replace function application_run_log_task_last_client_text(members uuid[]) returns text
language sql stable as $$
    select body.content
    from unnest(members) with ordinality member(run_id,position)
    join client_trajectory_steps s on s.flow_run_id=member.run_id
        and s.metadata->>'category'='assistant' and s.metadata->>'origin'='emitted'
    join client_trajectory_sections p on p.flow_run_id=s.flow_run_id and p.step_id=s.id
        and p.section='result' and p.body_kind='content'
    join runtime_canonical_contents c on c.id=p.content_id
        and c.application_id=(select application_id from flow_runs where id=p.flow_run_id)
        and c.scope_id=(select scope_id from flow_runs where id=p.flow_run_id)
    cross join lateral (select case when cardinality(p.content_path)=0 then c.content else c.content#>p.content_path end as value) raw
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
