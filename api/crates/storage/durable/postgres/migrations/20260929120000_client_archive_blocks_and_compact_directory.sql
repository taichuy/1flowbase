-- Physical backing changes only: original part identities and durable receipts remain.
create table client_trajectory_archive_blocks (
    block_id uuid primary key,
    request_id uuid not null references client_trajectory_archive_heads(request_id) on delete cascade,
    codec_version smallint not null check(codec_version=1),
    bytes bytea not null,
    raw_byte_length bigint not null check(raw_byte_length>=0),
    raw_checksum bytea not null check(octet_length(raw_checksum)=32),
    unique(request_id,block_id)
);
alter table client_trajectory_archive_parts
    add column block_id uuid,
    add column block_offset bigint,
    add constraint client_archive_block_owner foreign key(request_id,block_id)
        references client_trajectory_archive_blocks(request_id,block_id),
    drop constraint client_archive_codec_columns,
    add constraint client_archive_codec_columns check(
        (codec_version=0 and frame_directory is null and raw_byte_length is null and raw_checksum is null
            and block_id is null and block_offset is null)
        or (codec_version=1 and frame_directory is not null and raw_byte_length is not null and raw_checksum is not null
            and block_id is null and block_offset is null)
        or (codec_version=2 and frame_directory is not null and raw_byte_length is not null and raw_checksum is not null
            and block_id is not null and block_offset>=0 and octet_length(bytes)=0)
    );
-- Avoid PostgreSQL's CHECK-NULL acceptance for partially specified locators.
alter table client_trajectory_archive_parts add constraint client_archive_locator_shape check(
    (block_id is null and block_offset is null) or (block_id is not null and block_offset is not null)
);
create index client_archive_block_references on client_trajectory_archive_parts(block_id) where block_id is not null;

alter table client_trajectory_steps add column metadata_compact boolean not null default false;
alter table client_trajectory_steps add constraint client_step_compact_shape check(
    not metadata_compact or (semantic_metadata_restored and not metadata ?| array['id','request_id','flow_run_id','node_run_id'])
);
alter table client_trajectory_sections add column value_digest bytea
    check(value_digest is null or octet_length(value_digest)=32);
alter table client_trajectory_sections drop constraint client_trajectory_section_body_shape;
alter table client_trajectory_sections add constraint client_trajectory_section_body_shape check (
    (body_kind='legacy' and event_id is not null and content_id is null)
    or (body_kind='content' and content_id is not null and observed_at is not null
        and (value_hash is not null or value_digest is not null) and value_byte_size is not null)
    or (body_kind='timing' and section='timing' and content_id is null and observed_at is not null
        and (value_hash is not null or value_digest is not null) and value_byte_size is not null)
);

-- Keep JSON tokens untouched, including escaped NUL and numbers. Only the four
-- verified typed identities omitted by this layout are appended to the object.
create function client_trajectory_step_original_metadata(metadata jsonb, originals jsonb,
    step_id uuid, request uuid, owner_run uuid, owner_node uuid, compact boolean)
returns json language plpgsql immutable as $$
declare original json := runtime_original_json(metadata,originals,'metadata'); body text;
begin
    if not compact then return original; end if;
    body := btrim(original::text);
    if left(body,1)<>'{' or right(body,1)<>'}' then raise exception 'compact client metadata object unavailable'; end if;
    return (left(body,length(body)-1) || case when length(btrim(substr(body,2,length(body)-2)))=0 then '' else ',' end
        || '"id":' || to_json(step_id)::text || ',"request_id":' || to_json(request)::text
        || ',"flow_run_id":' || to_json(owner_run)::text || ',"node_run_id":' || coalesce(to_json(owner_node)::text,'null') || '}')::json;
end $$;
create or replace function client_trajectory_step_storage_body(step_id uuid,owner_run uuid)
returns json language plpgsql stable as $$
declare directory client_trajectory_steps; original json;
begin
    select * into directory from client_trajectory_steps where id=step_id and flow_run_id=owner_run;
    if not found then raise exception 'client step owner unavailable'; end if;
    if directory.semantic_metadata_restored then
        return client_trajectory_step_original_metadata(directory.metadata,directory.raw_json_payloads,
            directory.id,directory.request_id,directory.flow_run_id,directory.node_run_id,directory.metadata_compact);
    end if;
    select runtime_original_json(e.payload,e.raw_json_payloads,'payload') into original
    from runtime_events e where e.id=directory.event_id and e.flow_run_id=owner_run;
    if not found then raise exception 'client step legacy anchor unavailable'; end if;
    return original;
end $$;
-- The unique constraint already provides the same ordered (flow_run_id,sequence) access.
drop index runtime_events_flow_sequence_idx;

-- Old-format importers reset the new version marker on identity-preserving updates.
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
            raw_json_payloads='{}',semantic_metadata_restored=false,metadata_compact=false
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

