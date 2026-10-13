-- Fork provenance describes where a manual conversation started, not delegation.
-- Keep the source run and summary parent facts intact; only the task read model
-- distinguishes a manual fork from an explicitly declared child thread.
create function project_application_log_manual_fork_root() returns trigger
language plpgsql as $$
begin
 if new.source_kind='native' and exists (
  select 1 from flow_runs f
  where f.id=new.id and f.application_id=new.application_id and f.scope_id=new.scope_id
    and nullif(btrim(f.log_context->>'forked_from_thread_id'),'') is not null
    and nullif(btrim(f.log_context->>'parent_thread_id'),'') is null
    and nullif(btrim(f.log_context->>'subagent_kind'),'') is null
 ) then
  new.parent_task_run_id:=null;
 end if;
 return new;
end $$;

-- PostgreSQL orders same-event triggers by name. Run after the native owner
-- canonicalizes source run parents, and before the generated is_root is stored.
create trigger application_log_project_manual_fork_root
 before insert or update on application_run_log_tasks
 for each row execute function project_application_log_manual_fork_root();

-- Forward-only repair: unrelated native tasks and imported records are untouched.
update application_run_log_tasks t set parent_task_run_id=null
from flow_runs f
where t.source_kind='native' and t.parent_task_run_id is not null
  and f.id=t.id and f.application_id=t.application_id and f.scope_id=t.scope_id
  and nullif(btrim(f.log_context->>'forked_from_thread_id'),'') is not null
  and nullif(btrim(f.log_context->>'parent_thread_id'),'') is null
  and nullif(btrim(f.log_context->>'subagent_kind'),'') is null;
