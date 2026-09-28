-- Node directory rows contain identity/status only. Every payload section has
-- its own physical row, including its lossless original JSON sidecar.
create table node_run_details (
    node_run_id uuid not null references node_runs(id) on delete cascade,
    section text not null,
    payload jsonb,
    raw_json_payloads jsonb not null default '{}'::jsonb,
    primary key (node_run_id, section),
    check (section not in ('input_payload','output_payload','metrics_payload','debug_payload') or payload is not null)
);

insert into node_run_details (node_run_id,section,payload,raw_json_payloads)
select id,'input_payload',input_payload,case when raw_json_payloads ? 'input_payload' then jsonb_build_object('input_payload',raw_json_payloads->'input_payload') else '{}'::jsonb end from node_runs;
insert into node_run_details (node_run_id,section,payload,raw_json_payloads)
select id,'output_payload',output_payload,case when raw_json_payloads ? 'output_payload' then jsonb_build_object('output_payload',raw_json_payloads->'output_payload') else '{}'::jsonb end from node_runs;
insert into node_run_details (node_run_id,section,payload,raw_json_payloads)
select id,'error_payload',error_payload,case when raw_json_payloads ? 'error_payload' then jsonb_build_object('error_payload',raw_json_payloads->'error_payload') else '{}'::jsonb end from node_runs;
insert into node_run_details (node_run_id,section,payload,raw_json_payloads)
select id,'metrics_payload',metrics_payload,case when raw_json_payloads ? 'metrics_payload' then jsonb_build_object('metrics_payload',raw_json_payloads->'metrics_payload') else '{}'::jsonb end from node_runs;
insert into node_run_details (node_run_id,section,payload,raw_json_payloads)
select id,'debug_payload',debug_payload,case when raw_json_payloads ? 'debug_payload' then jsonb_build_object('debug_payload',raw_json_payloads->'debug_payload') else '{}'::jsonb end from node_runs;
insert into node_run_details (node_run_id,section,raw_json_payloads)
select id,'_sidecars',raw_json_payloads - 'input_payload' - 'output_payload' - 'error_payload' - 'metrics_payload' - 'debug_payload' from node_runs where (raw_json_payloads - 'input_payload' - 'output_payload' - 'error_payload' - 'metrics_payload' - 'debug_payload') <> '{}'::jsonb;

alter table node_runs
    drop column input_payload,
    drop column output_payload,
    drop column error_payload,
    drop column metrics_payload,
    drop column debug_payload,
    drop column raw_json_payloads;

-- Internal adapter view: preserves full operational reads and atomic SQL
-- write/RETURNING behavior without putting bodies back into directory rows.
create view node_run_records as
select n.*,
    coalesce(d0.payload,'{}'::jsonb) as input_payload,
    coalesce(d1.payload,'{}'::jsonb) as output_payload,
    d2.payload as error_payload,
    coalesce(d3.payload,'{}'::jsonb) as metrics_payload,
    coalesce(d4.payload,'{}'::jsonb) as debug_payload,
    coalesce(d0.raw_json_payloads,'{}'::jsonb) || coalesce(d1.raw_json_payloads,'{}'::jsonb) || coalesce(d2.raw_json_payloads,'{}'::jsonb) || coalesce(d3.raw_json_payloads,'{}'::jsonb) || coalesce(d4.raw_json_payloads,'{}'::jsonb) || coalesce(d5.raw_json_payloads,'{}'::jsonb) as raw_json_payloads
from node_runs n
left join node_run_details d0 on d0.node_run_id=n.id and d0.section='input_payload'
left join node_run_details d1 on d1.node_run_id=n.id and d1.section='output_payload'
left join node_run_details d2 on d2.node_run_id=n.id and d2.section='error_payload'
left join node_run_details d3 on d3.node_run_id=n.id and d3.section='metrics_payload'
left join node_run_details d4 on d4.node_run_id=n.id and d4.section='debug_payload'
left join node_run_details d5 on d5.node_run_id=n.id and d5.section='_sidecars'
;
alter view node_run_records alter column input_payload set default '{}'::jsonb;
alter view node_run_records alter column output_payload set default '{}'::jsonb;
alter view node_run_records alter column metrics_payload set default '{}'::jsonb;
alter view node_run_records alter column debug_payload set default '{}'::jsonb;
alter view node_run_records alter column raw_json_payloads set default '{}'::jsonb;
alter view node_run_records alter column started_at set default now();
alter view node_run_records alter column created_at set default now();

create function write_node_run_record() returns trigger language plpgsql as $$
declare
    metadata node_runs%rowtype;
    field text;
    next_payload jsonb;
    next_raw jsonb;
    previous_payload jsonb;
begin
    if TG_OP='INSERT' then
        insert into node_runs (id,scope_id,flow_run_id,node_id,node_type,node_alias,status,
            started_at,finished_at,created_at,created_by,updated_by)
        values (NEW.id,NEW.scope_id,NEW.flow_run_id,NEW.node_id,NEW.node_type,NEW.node_alias,NEW.status,
            NEW.started_at,NEW.finished_at,NEW.created_at,NEW.created_by,NEW.updated_by)
        returning * into metadata;
    else
        -- A payload-only update must not overwrite concurrently changed
        -- metadata with the view's earlier snapshot. The base row owns the lock
        -- and the existing trace refresh trigger still executes exactly once.
        if NEW.id is distinct from OLD.id then raise exception 'node run identity is immutable'; end if;
        update node_runs set
            scope_id=case when NEW.scope_id is distinct from OLD.scope_id then NEW.scope_id else node_runs.scope_id end,
            flow_run_id=case when NEW.flow_run_id is distinct from OLD.flow_run_id then NEW.flow_run_id else node_runs.flow_run_id end,
            node_id=case when NEW.node_id is distinct from OLD.node_id then NEW.node_id else node_runs.node_id end,
            node_type=case when NEW.node_type is distinct from OLD.node_type then NEW.node_type else node_runs.node_type end,
            node_alias=case when NEW.node_alias is distinct from OLD.node_alias then NEW.node_alias else node_runs.node_alias end,
            status=case when NEW.status is distinct from OLD.status then NEW.status else node_runs.status end,
            started_at=case when NEW.started_at is distinct from OLD.started_at then NEW.started_at else node_runs.started_at end,
            finished_at=case when NEW.finished_at is distinct from OLD.finished_at then NEW.finished_at else node_runs.finished_at end,
            created_at=case when NEW.created_at is distinct from OLD.created_at then NEW.created_at else node_runs.created_at end,
            created_by=case when NEW.created_by is distinct from OLD.created_by then NEW.created_by else node_runs.created_by end,
            updated_by=case when NEW.updated_by is distinct from OLD.updated_by then NEW.updated_by else node_runs.updated_by end
        where id=OLD.id returning * into metadata;
        if not found then return null; end if;
    end if;
    NEW.node_run_id := metadata.node_run_id;
    NEW.scope_id := metadata.scope_id;
    NEW.flow_run_id := metadata.flow_run_id;
    NEW.node_id := metadata.node_id;
    NEW.node_type := metadata.node_type;
    NEW.node_alias := metadata.node_alias;
    NEW.status := metadata.status;
    NEW.started_at := metadata.started_at;
    NEW.finished_at := metadata.finished_at;
    NEW.created_at := metadata.created_at;
    NEW.created_by := metadata.created_by;
    NEW.updated_by := metadata.updated_by;
    foreach field in array ARRAY['input_payload','output_payload','error_payload','metrics_payload','debug_payload'] loop
        next_payload := case field
            when 'input_payload' then NEW.input_payload
            when 'output_payload' then NEW.output_payload
            when 'error_payload' then NEW.error_payload
            when 'metrics_payload' then NEW.metrics_payload
            when 'debug_payload' then NEW.debug_payload end;
        next_raw := case when NEW.raw_json_payloads ? field
            then jsonb_build_object(field,NEW.raw_json_payloads->field) else '{}'::jsonb end;
        previous_payload := case field
            when 'input_payload' then OLD.input_payload
            when 'output_payload' then OLD.output_payload
            when 'error_payload' then OLD.error_payload
            when 'metrics_payload' then OLD.metrics_payload
            when 'debug_payload' then OLD.debug_payload end;
        if TG_OP='INSERT' or next_payload is distinct from previous_payload
            or NEW.raw_json_payloads->field is distinct from OLD.raw_json_payloads->field then
            insert into node_run_details(node_run_id,section,payload,raw_json_payloads)
            values(NEW.id,field,next_payload,next_raw)
            on conflict(node_run_id,section) do update set payload=excluded.payload,raw_json_payloads=excluded.raw_json_payloads;
        end if;
    end loop;
    next_raw := NEW.raw_json_payloads - 'input_payload' - 'output_payload' - 'error_payload' - 'metrics_payload' - 'debug_payload';
    if next_raw <> '{}'::jsonb then
        insert into node_run_details(node_run_id,section,raw_json_payloads) values(NEW.id,'_sidecars',next_raw)
        on conflict(node_run_id,section) do update set raw_json_payloads=excluded.raw_json_payloads;
    else
        delete from node_run_details where node_run_id=NEW.id and section='_sidecars';
    end if;
    return NEW;
end;
$$;
create trigger node_run_records_write instead of insert or update on node_run_records
for each row execute function write_node_run_record();
