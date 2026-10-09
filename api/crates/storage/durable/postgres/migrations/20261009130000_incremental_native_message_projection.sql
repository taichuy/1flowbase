-- Additive metadata only. Retained requests/events remain the fact owners;
-- canonical bodies continue to live in the existing message projection.
alter table flow_runs add column message_projection_revision bigint not null default 0;
alter table application_run_conversation_message_items add column source_occurrence_sequence bigint;
create index application_run_message_owner_occurrence on application_run_conversation_message_items(flow_run_id,source_occurrence_sequence) where source_occurrence_sequence is not null;
create table application_run_native_projection_progress (
    flow_run_id uuid primary key references flow_runs(id) on delete cascade,
    projection_version integer not null,
    output_sequence bigint not null,
    result_count bigint not null check(result_count>=0),
    source_revision bigint not null,
    invalidated boolean not null default false,
    conflicting_source_keys text[] not null default array[]::text[],
    last_read_context_count bigint not null default 0,
    last_read_event_count bigint not null default 0
);

-- Repository transaction entrances take this lock BEFORE source rows,
-- trace queue, summaries, tasks or progress, after application KEY SHARE.
-- A late projection-only lock cannot
-- establish ordering after a source trigger has already acquired a queue row.
create function application_run_lock_native_projection(source_run uuid) returns uuid
language plpgsql as $$
declare authorized_application uuid; conversation_uuid uuid; locked_uuid uuid;
begin
    -- Application deletion owns FOR UPDATE before conversations. New run
    -- insertion already takes this application FK KEY SHARE before binding.
    select a.id into authorized_application from applications a
        join flow_runs f on f.application_id=a.id where f.id=source_run
        for key share of a;
    if authorized_application is null then return null; end if;
    select (f.log_context->>'log_conversation_id')::uuid into conversation_uuid
        from flow_runs f where f.id=source_run;
    if conversation_uuid is null then return null; end if;
    select c.id into locked_uuid from application_conversations c
        join flow_runs f on f.id=source_run and f.application_id=c.application_id
            and f.api_key_id is not distinct from c.api_key_id
            and coalesce(f.external_user,'')=coalesce(c.external_user,'')
        where c.id=conversation_uuid
        for no key update of c;
    if locked_uuid is null then raise exception 'native log conversation authorization mismatch'; end if;
    return locked_uuid;
end;
$$;

create function invalidate_native_message_projection(source_run uuid, application uuid, conversation uuid)
returns void language sql as $$
    update application_run_native_projection_progress set invalidated=true
    where flow_run_id=source_run or flow_run_id in
        (select run_id from application_run_log_conversation_runs(application,conversation))
$$;

create function revise_flow_message_projection() returns trigger language plpgsql as $$
declare
    old_results jsonb;
    new_results jsonb;
    append_only boolean;
    successor uuid;
    sticky_keys text[];
begin
    if tg_op='DELETE' then
        -- Preserve observed conflict metadata when the body-owning run is
        -- removed. This carries KEYS only, never another copy of any body.
        select array_agg(distinct key) into sticky_keys from (
            select source_item_key as key from application_run_conversation_message_items
                where flow_run_id=old.id and native_message->>'_log_conflicting'='true'
            union select unnest(conflicting_source_keys) from application_run_native_projection_progress where flow_run_id=old.id
        ) observed;
        select run_id into successor from application_run_log_conversation_runs(old.application_id,(old.log_context->>'log_conversation_id')::uuid) where run_id<>old.id order by run_id limit 1;
        if successor is not null and coalesce(cardinality(sticky_keys),0)>0 then
            insert into application_run_native_projection_progress(flow_run_id,projection_version,output_sequence,result_count,source_revision,invalidated,conflicting_source_keys)
                values(successor,0,'-9223372036854775808'::bigint,0,0,true,sticky_keys)
            on conflict(flow_run_id) do update set invalidated=true,conflicting_source_keys=(
                select array_agg(distinct key) from unnest(application_run_native_projection_progress.conflicting_source_keys||excluded.conflicting_source_keys) key);
        end if;
        perform invalidate_native_message_projection(old.id,old.application_id,(old.log_context->>'log_conversation_id')::uuid);
        return old;
    end if;
    if (new.status,new.updated_at,new.finished_at,new.log_context,new.input_payload,new.output_payload,new.error_payload,new.raw_json_payloads,new.application_id,new.api_key_id,new.external_user)
       is distinct from
       (old.status,old.updated_at,old.finished_at,old.log_context,old.input_payload,old.output_payload,old.error_payload,old.raw_json_payloads,old.application_id,old.api_key_id,old.external_user) then
        new.message_projection_revision=old.message_projection_revision+1;
    end if;
    if old.log_context is not null and
       (new.log_context,new.raw_json_payloads->'log_context',new.api_key_id,new.external_user,new.application_id)
       is distinct from
       (old.log_context,old.raw_json_payloads->'log_context',old.api_key_id,old.external_user,old.application_id) then
        old_results=coalesce(old.log_context->'tool_results','[]'::jsonb);
        new_results=coalesce(new.log_context->'tool_results','[]'::jsonb);
        -- Searchable JSONB may collapse NUL/escape distinctions. Any change to
        -- an original sidecar takes the conservative complete rebuild path.
        append_only=(new.log_context-'tool_results')=(old.log_context-'tool_results')
            and new.raw_json_payloads->'log_context' is not distinct from old.raw_json_payloads->'log_context'
            and new.api_key_id is not distinct from old.api_key_id
            and new.external_user is not distinct from old.external_user
            and new.application_id=old.application_id
            and jsonb_array_length(new_results)>=jsonb_array_length(old_results)
            and not exists(select 1 from jsonb_array_elements(old_results) with ordinality r(item,n)
                where item is distinct from new_results->((n-1)::integer));
        if append_only is not true then
            perform invalidate_native_message_projection(old.id,old.application_id,(old.log_context->>'log_conversation_id')::uuid);
            perform invalidate_native_message_projection(new.id,new.application_id,(new.log_context->>'log_conversation_id')::uuid);
        end if;
    end if;
    return new;
end;
$$;
-- Allocators and internal revision increments must never enter the body
-- comparison above. UPDATE OF filters on the statement's actual SET columns.
create trigger flow_message_projection_revision before update of
    status,updated_at,finished_at,log_context,input_payload,output_payload,error_payload,
    raw_json_payloads,application_id,api_key_id,external_user
    or delete on flow_runs
for each row execute function revise_flow_message_projection();

create function revise_event_message_projection() returns trigger language plpgsql as $$
declare source_run uuid; source_application uuid; conversation uuid;
begin
    if tg_op='INSERT' and new.event_type<>'provider_output_item_done' then return new; end if;
    if tg_op='DELETE' and old.event_type<>'provider_output_item_done' then return old; end if;
    if tg_op='UPDATE' and new.event_type<>'provider_output_item_done' and old.event_type<>'provider_output_item_done' then return new; end if;
    if tg_op<>'INSERT' then
        source_run=old.flow_run_id;
        select application_id,(log_context->>'log_conversation_id')::uuid into source_application,conversation from flow_runs where id=source_run;
        perform invalidate_native_message_projection(source_run,source_application,conversation);
        update flow_runs set message_projection_revision=message_projection_revision+1 where id=source_run;
    end if;
    if tg_op<>'DELETE' then
        source_run=new.flow_run_id;
        select application_id,(log_context->>'log_conversation_id')::uuid into source_application,conversation from flow_runs where id=source_run;
        -- Inserts below an already consumed boundary are non-append changes.
        if exists(select 1 from application_run_native_projection_progress where flow_run_id=source_run and output_sequence>=new.sequence) then
            perform invalidate_native_message_projection(source_run,source_application,conversation);
        end if;
        update flow_runs set message_projection_revision=message_projection_revision+1 where id=source_run;
        return new;
    end if;
    return old;
end;
$$;
create trigger native_message_event_revision after insert or update or delete on runtime_events
for each row execute function revise_event_message_projection();

-- A run-local counter avoids hashing every historical body on each freshness
-- probe. Node rows matter to the legacy assistant-message derivation too.
create function revise_node_message_projection() returns trigger language plpgsql as $$
begin
    if tg_op<>'INSERT' then
        update flow_runs set message_projection_revision=message_projection_revision+1 where id=old.flow_run_id;
    end if;
    if tg_op<>'DELETE' then
        update flow_runs set message_projection_revision=message_projection_revision+1 where id=new.flow_run_id;
        return new;
    end if;
    return old;
end;
$$;
create trigger node_message_projection_revision after insert or update or delete on node_runs
for each row execute function revise_node_message_projection();
create function revise_node_detail_message_projection() returns trigger language plpgsql as $$
begin
    if tg_op<>'INSERT' then
        update flow_runs f set message_projection_revision=f.message_projection_revision+1
            where f.id=(select n.flow_run_id from node_runs n where n.id=old.node_run_id);
    end if;
    if tg_op<>'DELETE' then
        -- A moved detail changes both retained owners, even if its body is equal.
        update flow_runs f set message_projection_revision=f.message_projection_revision+1
            where f.id=(select n.flow_run_id from node_runs n where n.id=new.node_run_id);
        return new;
    end if;
    return old;
end;
$$;
create trigger node_detail_message_projection_revision after insert or update or delete on node_run_details
for each row execute function revise_node_detail_message_projection();
create or replace function application_run_message_projection_watermark(run_id uuid) returns text
language sql stable as $$
    select message_projection_revision::text from flow_runs where id=$1
$$;
