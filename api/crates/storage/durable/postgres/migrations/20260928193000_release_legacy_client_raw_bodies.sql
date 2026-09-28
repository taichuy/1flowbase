-- Legacy raw event IDs remain metadata anchors. Their copied values are now
-- resolved from the request archive in the same bound run. Request-local cursors
-- and runtime event / Outbox sequences are unchanged.
create or replace function runtime_event_original_payload(projection jsonb, originals jsonb, owner_run uuid)
returns json language plpgsql stable as $$
declare
    original json := runtime_original_json(projection, originals, 'payload');
    reference jsonb := projection->'_observation_body_ref';
    client_reference jsonb := projection->'_client_archive_ref';
    body json;
    fact json;
begin
    if client_reference is not null then
        select convert_from(substring(p.bytes from (p.frames->0->>'offset')::integer+1
                       for (p.frames->0->>'length')::integer),'UTF8')::json into body
          from client_trajectory_archive_parts p
          join client_trajectory_archive_heads h on h.request_id=p.request_id
          join client_trajectory_captures c on c.request_id=p.request_id
          where p.part_id=(client_reference->>'part_id')::uuid
            and p.request_id=(client_reference->>'request_id')::uuid
            and h.flow_run_id=owner_run and c.flow_run_id=owner_run
            and jsonb_array_length(p.frames)=1 and p.frames->0->>'format'='legacy_json';
        if not found then raise exception 'client archive body owner unavailable'; end if;
        select json_object_agg(k,v) into fact from (
            select key k,value v from json_each(original->'fact') where key<>'value'
            union all select 'value',body
        ) fields;
        return (select json_object_agg(k,v) from (
            select key k,value v from json_each(original) where key not in ('_client_archive_ref','fact')
            union all select 'fact',fact
        ) fields);
    end if;
    -- Keep the observation resolver from 192000 unchanged, including inline
    -- lossless originals and same-application/scope ownership checks.
    if reference is null then return original; end if;
    select runtime_original_json(c.content,c.raw_json_payloads,'content') into body
      from runtime_canonical_contents c
      join flow_runs f on f.application_id=c.application_id and f.scope_id=c.scope_id
      where c.id=(reference->>'content_id')::uuid
        and c.application_id=(reference->>'application_id')::uuid
        and f.id=owner_run;
    if not found then raise exception 'observation body owner unavailable'; end if;
    return (select json_object_agg(k,v) from (
        select key k,value v from json_each(original)
            where key not in ('_observation_body_ref','body')
        union all select 'body',body
    ) fields);
end;
$$;

do $$
declare
    candidate record;
    original json;
    stripped_fact json;
    stripped json;
begin
    for candidate in
        select e.id,e.flow_run_id,e.sequence,e.payload,e.raw_json_payloads,p.request_id,p.frames,p.bytes
          from runtime_events e join client_trajectory_archive_parts p on p.part_id=e.id
          join client_trajectory_archive_heads h on h.request_id=p.request_id and h.flow_run_id=e.flow_run_id
          join client_trajectory_captures c on c.request_id=p.request_id and c.flow_run_id=e.flow_run_id
          where e.event_type='client_protocol_trajectory'
            and e.payload->'fact'->>'kind'='section' and e.payload->'fact'->>'section'='raw'
            and e.payload->>'request_id'=p.request_id::text
            and e.payload->'_client_archive_ref' is null
            and p.first_sequence=e.sequence and p.last_sequence=e.sequence
            and jsonb_array_length(p.frames)=1
            and p.frames->0->>'format'='legacy_json'
            and (p.frames->0->>'sequence')::bigint=e.sequence
            and (p.frames->0->>'offset')::bigint=0
            and (p.frames->0->>'length')::bigint=octet_length(p.bytes)
    loop
        original := runtime_original_json(candidate.payload,candidate.raw_json_payloads,'payload');
        -- PostgreSQL text keys cannot represent U+0000. Preserve such originals
        -- exactly inline. Conservatively retain any NUL-bearing original rather
        -- than risking loss while lexing json_each; its existing archive copy
        -- remains the raw-section source. This is a lossless storage exception.
        if position('\u0000' in original::text)>0 then continue; end if;
        if candidate.bytes<>convert_to((original->'fact'->'value')::text,'UTF8') then continue; end if;
        select coalesce(json_object_agg(key,value),'{}'::json) into stripped_fact
          from json_each(original->'fact') where key<>'value';
        select json_object_agg(k,v) into stripped from (
            select key k,value v from json_each(original) where key<>'fact'
            union all select 'fact',stripped_fact
        ) fields;
        update runtime_events set
            payload=jsonb_set(candidate.payload #- '{fact,value}', '{_client_archive_ref}',
                jsonb_build_object('part_id',candidate.id,'request_id',candidate.request_id)),
            raw_json_payloads=jsonb_set(candidate.raw_json_payloads-'payload','{payload}',to_jsonb(stripped::text))
          where id=candidate.id and flow_run_id=candidate.flow_run_id;
    end loop;
end;
$$;
