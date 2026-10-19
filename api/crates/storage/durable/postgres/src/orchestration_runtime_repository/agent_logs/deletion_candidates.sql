select t.id as record_id, coalesce(m.run_ids, '{}'::uuid[]) as native_run_ids
from application_run_log_tasks t
join applications a on a.id=t.application_id and a.scope_id=t.scope_id
left join lateral (
    select array_agg(f.id order by f.id) as run_ids,
           bool_and(f.status in ('succeeded','failed','cancelled','incomplete')
                    and f.finished_at is not null) as terminal,
           bool_or(exists(select 1 from assistant_conversations c
                          where c.seed_legacy_flow_run_id=f.id)) as assistant_seed
    from flow_runs f
    where t.source_kind='native'
      and f.application_id=t.application_id and f.scope_id=t.scope_id
      and ((f.id=t.native_run_id and f.log_context->>'log_task_run_id' is null)
           or f.log_context->>'log_task_run_id'=t.id::text)
) m on t.source_kind='native'
where t.application_id=$1 and t.scope_id=$2
  and ($3::timestamptz is null or t.started_at >= $3)
  and ($4::timestamptz is null or t.started_at < $4)
  and ($5::timestamptz is null or t.ingested_at < $5)
  and ((a.application_type='agent_logs' and t.source_kind='imported')
       or (a.application_type='agent_flow' and t.source_kind='native'
           and m.terminal and not m.assistant_seed))
