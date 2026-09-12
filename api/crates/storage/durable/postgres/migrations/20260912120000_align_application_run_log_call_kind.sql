-- #2034: call_kind is owned by the AI Native operation; task/conversation scope
-- is defined once in the database and shared by every original log read path.

-- One definition of "which native operation did this run execute". Used by the
-- projection writer and by the backfill below.
create function application_run_log_call_kind(run_id uuid) returns text
language sql stable as $$
    select case
        when f.log_context->>'call_kind' in ('generate','compact','count_tokens') then f.log_context->>'call_kind'
        else coalesce((
            -- Mirrors domain::AiNativeOperation::call_kind: a local summary is a compaction.
            select case when count(distinct k.call_kind)=1 then min(k.call_kind) end
            from jsonb_each(case when jsonb_typeof(f.input_payload)='object' then f.input_payload else '{}'::jsonb end) o
            cross join lateral (select case
                when o.value->'operation'->>'kind'='generate' and o.value->'operation'->>'profile'='local_summary' then 'compact'
                when o.value->'operation'->>'kind' in ('generate','compact','count_tokens') then o.value->'operation'->>'kind' end) k(call_kind)
            where jsonb_typeof(o.value)='object' and o.value ? 'operation' and k.call_kind is not null
        ), 'generate') end
    from flow_runs f where f.id=$1
$$;

alter table application_run_log_summaries
    add column call_kind text not null default 'generate'
        check (call_kind in ('generate','compact','count_tokens'));
update application_run_log_summaries s set call_kind=application_run_log_call_kind(s.flow_run_id);

-- The constant placeholder is replaced by per-run contributions derived from
-- call_kind; task rows sum them. No writer maintains these values.
alter table application_run_log_summaries drop column invocation_count;
alter table application_run_log_summaries
    add column invocation_count bigint generated always as (case when call_kind='generate' then 1 else 0 end) stored,
    add column compaction_count bigint generated always as (case when call_kind='compact' then 1 else 0 end) stored;

-- Task scope: every original call explicitly bound to the same task anchor,
-- within the same authorization domain as the anchor run.
create function application_run_log_task_runs(application uuid, anchor uuid) returns table(run_id uuid)
language sql stable as $$
    select sibling.flow_run_id
    from application_run_log_summaries a
    join application_run_log_summaries sibling on sibling.application_id=a.application_id
        and sibling.scope_id=a.scope_id
        and sibling.api_key_id is not distinct from a.api_key_id
        and coalesce(sibling.external_user,'')=coalesce(a.external_user,'')
        and (sibling.flow_run_id=a.flow_run_id
            or (a.log_task_run_id is not null and sibling.log_task_run_id=a.log_task_run_id))
    where a.application_id=$1 and a.flow_run_id=$2
$$;

-- Conversation scope: every run bound to the same log conversation within the
-- conversation's own authorization domain. Public conversations without a
-- client thread never qualify.
create function application_run_log_conversation_runs(application uuid, conversation uuid) returns table(run_id uuid)
language sql stable as $$
    select f.id
    from application_conversations c
    join flow_runs f on f.application_id=c.application_id
        and f.api_key_id is not distinct from c.api_key_id
        and coalesce(f.external_user,'')=coalesce(c.external_user,'')
        and f.log_context->>'log_conversation_id'=c.id::text
    where c.id=$2 and c.application_id=$1 and c.client_thread_id is not null
$$;

insert into model_fields (
    id,data_model_id,scope_id,code,title,physical_column_name,field_kind,
    is_system,is_writable,is_required,is_unique,display_options,relation_options,
    sort_order,availability_status
)
select md5(d.id::text || ':' || f.code)::uuid,d.id,d.scope_id,f.code,f.code,f.code,f.kind,
    true,false,true,false,'{}'::jsonb,'{}'::jsonb,f.ordinal,'available'
from model_definitions d cross join (values
    ('call_kind','string',45),('compaction_count','number',46)
) f(code,kind,ordinal)
where d.code='application_run_log_summaries' and d.data_source_instance_id is null
    and d.owner_kind='core'
    and not exists(select 1 from model_fields e where e.data_model_id=d.id and e.code=f.code);
