-- Correct the operational view trigger without rewriting the deployed migration.
-- Both the directory row and view expose id; detail rows use node_run_id.
create or replace function write_node_run_record() returns trigger language plpgsql as $$
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
    NEW.id := metadata.id;
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
