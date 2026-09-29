-- Snapshot occurrence identity stays in runtime_events. Only exact immutable
-- Native input strings are shared, within the existing application/scope owner.
create table runtime_native_snapshot_items (
    id uuid primary key,
    scope_id uuid not null,
    application_id uuid not null,
    content_hash text not null check(content_hash ~ '^sha256:[0-9a-f]{64}$'),
    body text not null,
    byte_size bigint not null check(byte_size=octet_length(body)),
    unique(application_id,content_hash),
    foreign key(application_id,scope_id) references applications(id,scope_id) on delete cascade
);
create table runtime_native_snapshot_manifests (
    id uuid primary key,
    scope_id uuid not null,
    application_id uuid not null,
    content_hash text not null check(content_hash ~ '^sha256:[0-9a-f]{64}$'),
    byte_size bigint not null check(byte_size>=0),
    layout jsonb not null check(jsonb_typeof(layout)='array'),
    unique(application_id,content_hash),
    foreign key(application_id,scope_id) references applications(id,scope_id) on delete cascade
);
create table runtime_native_snapshot_references (
    manifest_id uuid not null references runtime_native_snapshot_manifests(id) on delete cascade,
    item_id uuid not null references runtime_native_snapshot_items(id),
    primary key(manifest_id,item_id)
);
create index runtime_native_snapshot_item_owners on runtime_native_snapshot_references(item_id);
create function reject_native_snapshot_update() returns trigger language plpgsql as $$
begin raise exception 'native snapshot content is immutable'; end;
$$;
create trigger native_snapshot_item_immutable before update on runtime_native_snapshot_items
    for each row execute function reject_native_snapshot_update();
create trigger native_snapshot_manifest_immutable before update on runtime_native_snapshot_manifests
    for each row execute function reject_native_snapshot_update();
create trigger native_snapshot_reference_immutable before update on runtime_native_snapshot_references
    for each row execute function reject_native_snapshot_update();
alter table runtime_events add column observation_body_manifest_id uuid
    references runtime_native_snapshot_manifests(id);
create index runtime_event_native_manifest_owner on runtime_events(observation_body_manifest_id)
    where observation_body_manifest_id is not null;
alter table runtime_events add constraint observation_body_single_owner
    check(observation_body_content_id is null or observation_body_manifest_id is null);

create function runtime_native_snapshot_body(manifest uuid,owner_run uuid) returns json
language plpgsql stable as $$
declare m runtime_native_snapshot_manifests; total bigint; valid bigint; restored text;
begin
    select s.* into m from runtime_native_snapshot_manifests s join flow_runs f
      on f.scope_id=s.scope_id and f.application_id=s.application_id
      where s.id=manifest and f.id=owner_run;
    if not found then raise exception 'native snapshot owner unavailable'; end if;
    select count(*),count(piece),string_agg(piece,'' order by position) into total,valid,restored
    from (
      select p.position,case
        when jsonb_array_length(p.part)=2 and p.part->>0='literal' and jsonb_typeof(p.part->1)='string' then p.part->>1
        when jsonb_array_length(p.part)=2 and p.part->>0='item' then i.body
      end piece
      from jsonb_array_elements(m.layout) with ordinality p(part,position)
      left join runtime_native_snapshot_references r on r.manifest_id=m.id
        and r.item_id=case when p.part->>0='item' then (p.part->>1)::uuid end
      left join runtime_native_snapshot_items i on i.id=r.item_id
        and i.scope_id=m.scope_id and i.application_id=m.application_id
    ) pieces;
    if total=0 or total<>valid or octet_length(restored)<>m.byte_size
       or 'sha256:'||encode(sha256(convert_to(restored,'UTF8')),'hex')<>m.content_hash then
      raise exception 'native snapshot integrity mismatch';
    end if;
    return to_json(restored);
end;
$$;

-- Existing raw legacy locators stay version0. Client semantic anchors and Native
-- manifests add branches; ordinary/exceptional old original readers stay intact.
create or replace function runtime_event_original_payload(projection jsonb, originals jsonb, owner_run uuid)
returns json language plpgsql stable as $$
declare
    original json := runtime_original_json(projection, originals, 'payload');
    reference jsonb := projection->'_observation_body_ref';
    client_reference jsonb := projection->'_client_archive_ref';
    body json; fact json;
begin
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

create function reclaim_native_snapshot() returns trigger language plpgsql as $$
declare candidate uuid := OLD.observation_body_manifest_id; application uuid;
begin
    if candidate is null then return null; end if;
    select application_id into application from runtime_native_snapshot_manifests where id=candidate;
    if not found then return null; end if;
    perform pg_advisory_xact_lock(hashtextextended('native-snapshots:'||application::text,0));
    delete from runtime_native_snapshot_manifests m where m.id=candidate
      and not exists(select 1 from runtime_events e where e.observation_body_manifest_id=m.id);
    return null;
end;
$$;
create constraint trigger reclaim_deleted_native_snapshot after delete on runtime_events
    deferrable initially deferred for each row execute function reclaim_native_snapshot();
create constraint trigger reclaim_replaced_native_snapshot after update on runtime_events
    deferrable initially deferred for each row execute function reclaim_native_snapshot();
create function reclaim_native_snapshot_item() returns trigger language plpgsql as $$
declare application uuid;
begin
    select application_id into application from runtime_native_snapshot_items where id=OLD.item_id;
    if not found then return null; end if;
    perform pg_advisory_xact_lock(hashtextextended('native-snapshots:'||application::text,0));
    delete from runtime_native_snapshot_items i where i.id=OLD.item_id
      and not exists(select 1 from runtime_native_snapshot_references r where r.item_id=i.id);
    return null;
end;
$$;
create constraint trigger reclaim_deleted_native_snapshot_item after delete on runtime_native_snapshot_references
    deferrable initially deferred for each row execute function reclaim_native_snapshot_item();
-- Switching a retained observation can retire its old canonical body, under the
-- original ownership and complete reference guards, without changing that body.
create constraint trigger reclaim_replaced_observation_body after update on runtime_events
    deferrable initially deferred for each row execute function reclaim_observation_body('observation_body_content_id');
