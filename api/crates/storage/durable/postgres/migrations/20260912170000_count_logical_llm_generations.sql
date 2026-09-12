-- #2036 AC-009: a resumed run can contain multiple logical LLM generations.
-- Provider attempts belong to a generation and must not increase this count.
-- DROP EXPRESSION retains existing values for historical runs without facts.
alter table application_run_log_summaries
    alter column invocation_count drop expression;
alter table application_run_log_summaries
    alter column invocation_count set not null,
    alter column invocation_count set default 1;

with generations as (
    select flow_run_id, count(*)::bigint as invocation_count
    from runtime_spans
    where kind = 'llm_turn'
    group by flow_run_id
)
update application_run_log_summaries s
set invocation_count = case when s.call_kind = 'compact' then 0 else g.invocation_count end
from generations g
where s.flow_run_id = g.flow_run_id;

-- Only the count projection changes; retained token, cost and message facts stay intact.
update application_run_log_tasks t
set invocation_count = (
    select sum(s.invocation_count)::bigint
    from application_run_log_summaries s
    where s.flow_run_id = any(t.member_run_ids)
);
