-- Raw capture cursors retain historical runtime event_sequence values. New frames
-- continue above the last archived cursor within the request, independently of
-- runtime_events / Outbox sequences. There is no public access before scope binding.
create table client_trajectory_archive_heads (
    request_id uuid primary key,
    transport text not null,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    persisted_through bigint not null default 0
);
create table client_trajectory_archive_parts (
    part_id uuid primary key,
    request_id uuid not null references client_trajectory_archive_heads(request_id) on delete cascade,
    first_sequence bigint not null,
    last_sequence bigint not null,
    frames jsonb not null,
    bytes bytea not null,
    unique(request_id, first_sequence),
    check(last_sequence >= first_sequence)
);
create index client_trajectory_archive_page on client_trajectory_archive_parts(request_id,last_sequence);
-- Historical frames keep exact JSON values as well as decoded bytes, including
-- literal NUL encoded by the lossless JSON envelope. Preserve event IDs as anchors.
insert into client_trajectory_archive_heads(request_id,transport,flow_run_id,persisted_through)
select s.request_id,coalesce(max(st.metadata->>'transport'),'http'),s.flow_run_id,max(s.event_sequence)
from client_trajectory_sections s left join client_trajectory_steps st on st.id=s.request_id
where s.section='raw' group by s.request_id,s.flow_run_id;
insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes)
select s.event_id,s.request_id,s.event_sequence,s.event_sequence,
    jsonb_build_array(jsonb_build_object('sequence',s.event_sequence,'format',v.format,
        'kind',coalesce(e.payload->'fact'->'value'->>'frame_kind','response_json'),
        'observed_at',e.payload->>'observed_at','offset',0,
        'length',octet_length(v.bytes))),v.bytes
from client_trajectory_sections s join runtime_events e on e.id=s.event_id
cross join lateral (select runtime_original_json(e.payload,e.raw_json_payloads,'payload') as original) o
cross join lateral (
    -- PostgreSQL JSON extraction lexes strings and rejects NUL even when its
    -- result type is JSON. The CASE branch must avoid extraction entirely.
    -- Rust restores fact.value from the full serialized payload for these rows.
    select case when position('\u0000' in o.original::text)>0
        then 'legacy_payload_json' else 'legacy_json' end as format,
        case when position('\u0000' in o.original::text)>0
        then convert_to(o.original::text,'UTF8')
        else convert_to((o.original->'fact'->'value')::text,'UTF8') end as bytes
) v
where s.section='raw';
-- Legacy event bodies stay until all direct event export readers are switched to
-- the archive resolver. The section reader uses only parts after this migration.
delete from client_trajectory_sections where section='raw';
