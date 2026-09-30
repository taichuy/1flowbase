-- New writes archive transition I/O once; old inline and observation references remain readable.
create or replace function runtime_event_original_payload(projection jsonb, originals jsonb, owner_run uuid)
returns json language plpgsql stable as $$
declare
    original json := runtime_original_json(projection, originals, 'payload');
    reference jsonb := projection->'_observation_body_ref';
    client_reference jsonb := projection->'_client_archive_ref';
    body json; fact json;
    node_reference jsonb := projection->'_node_payload_ref';
begin
    if projection ? '_context_occurrences' then
        original := (select json_object_agg(key,value) from json_each(original) where key<>'_context_occurrences');
    end if;
    if node_reference is not null then
        if node_reference->>'version' is distinct from '1' then
            raise exception 'unknown node payload reference version';
        end if;
        select runtime_original_json(c.content,c.raw_json_payloads,'content') into body
          from runtime_canonical_contents c
          join flow_runs f on f.application_id=c.application_id and f.scope_id=c.scope_id
          where c.id=(node_reference->>'content_id')::uuid
            and c.application_id=(node_reference->>'application_id')::uuid and f.id=owner_run;
        if not found then raise exception 'node payload owner unavailable'; end if;
        return (select json_object_agg(k,v) from (
            select key k,value v from json_each(original) where key<>'_node_payload_ref'
            union all select key,(value #>> '{}')::json from jsonb_each(body::jsonb)
        ) fields);
    end if;
    if projection ? '_client_semantic_ref' then
        return client_trajectory_event_original_payload(projection,originals,owner_run);
    end if;
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
    if reference is null then return original; end if;
    if reference ? 'manifest_id' then
        if not exists(select 1 from runtime_native_snapshot_manifests m
          where m.id=(reference->>'manifest_id')::uuid and m.application_id=(reference->>'application_id')::uuid) then
            raise exception 'native snapshot application unavailable';
        end if;
        body := runtime_native_snapshot_body((reference->>'manifest_id')::uuid,owner_run);
    else
        select runtime_original_json(c.content,c.raw_json_payloads,'content') into body
          from runtime_canonical_contents c
          join flow_runs f on f.application_id=c.application_id and f.scope_id=c.scope_id
          where c.id=(reference->>'content_id')::uuid
            and c.application_id=(reference->>'application_id')::uuid and f.id=owner_run;
        if not found then raise exception 'observation body owner unavailable'; end if;
    end if;
    return (select json_object_agg(k,v) from (
        select key k,value v from json_each(original) where key not in ('_observation_body_ref','body')
        union all select 'body',body
    ) fields);
end;
$$;
