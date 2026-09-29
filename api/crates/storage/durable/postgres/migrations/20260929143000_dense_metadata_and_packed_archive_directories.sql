-- Preserve all archive anchors/PK/FK/ACK identities, compress their CAD1 bodies in blocks.
alter table client_trajectory_archive_blocks
    drop constraint client_trajectory_archive_blocks_codec_version_check,
    add constraint client_trajectory_archive_blocks_codec_version_check check(codec_version in (1,2));
alter table client_trajectory_archive_parts drop constraint client_archive_codec_columns;
alter table client_trajectory_archive_parts add constraint client_archive_codec_columns check(
    (codec_version=0 and frame_directory is null and raw_byte_length is null and raw_checksum is null
        and block_id is null and block_offset is null)
    or (codec_version=1 and frame_directory is not null and raw_byte_length is not null and raw_checksum is not null
        and block_id is null and block_offset is null)
    or (codec_version in (2,3) and frame_directory is not null and raw_byte_length is not null and raw_checksum is not null
        and block_id is not null and block_offset>=0 and octet_length(bytes)=0)
);

alter table client_trajectory_steps add column metadata_layout smallint not null default 0;
alter table client_trajectory_steps add constraint client_step_dense_shape check(
    metadata_layout=0 or (metadata_layout=1 and metadata_compact and semantic_metadata_restored
        and jsonb_typeof(metadata)='object' and metadata ?& array['category','origin','available_sections'])
);

-- A versioned sparse object only omits exact known defaults/equalities. Preserve
-- original JSON text (NUL and numeric tokens) and append only absent fields.
create function client_trajectory_step_dense_metadata(metadata jsonb, originals jsonb,
    step_id uuid, request uuid, owner_run uuid, owner_node uuid, compact boolean,
    observed text, layout smallint)
returns json language plpgsql immutable as $$
declare original json; defaults json; field record; body text; suffix text := '';
    original_name json; original_preview json;
begin
    original := client_trajectory_step_original_metadata(metadata,originals,step_id,request,owner_run,owner_node,compact);
    if layout=0 then return original; end if;
    if layout<>1 or not compact then raise exception 'client step metadata layout invalid'; end if;
    original_name := coalesce(original->'name',original->'category');
    original_preview := coalesce(original->'preview',original_name);
    defaults := json_build_object('namespace',null,'parameters_preview',null,'call_id',null,
        'item_id',null,'response_id',null,'turn_id',null,'related_step_id',null,
        'sequence',0,'protocol','responses','transport','http','status','submitted',
        'created_at',runtime_original_json(to_jsonb(observed),originals,'observed_at'),
        'parent_id',request,'name',original_name,'preview',original_preview,'result_preview',original_preview);
    for field in select * from json_each(defaults) loop
        if original->field.key is null then
            suffix := suffix || ',' || to_json(field.key)::text || ':' || field.value::text;
        end if;
    end loop;
    body := btrim(original::text);
    return (left(body,length(body)-1) || suffix || '}')::json;
end $$;

create or replace function client_trajectory_step_storage_body(step_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_steps; original json;
begin
    select * into directory from client_trajectory_steps where id=step_id and flow_run_id=owner_run;
    if not found then raise exception 'client step owner unavailable'; end if;
    if directory.semantic_metadata_restored then
        return client_trajectory_step_dense_metadata(directory.metadata,directory.raw_json_payloads,
            directory.id,directory.request_id,directory.flow_run_id,directory.node_run_id,
            directory.metadata_compact,directory.observed_at,directory.metadata_layout);
    end if;
    select runtime_original_json(e.payload,e.raw_json_payloads,'payload') into original
    from runtime_events e where e.id=directory.event_id and e.flow_run_id=owner_run;
    if not found then raise exception 'client step legacy anchor unavailable'; end if;
    return original;
end $$;

-- Legacy projection writers publish complete, non-dense metadata.
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
            raw_json_payloads='{}',semantic_metadata_restored=false,metadata_compact=false,metadata_layout=0
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
