with recursive selected as (
 select f.id,coalesce(s.log_task_run_id,f.id) task_id
 from flow_runs f left join application_run_log_summaries s
 on s.flow_run_id=f.id and s.application_id=f.application_id
 where f.application_id=$1 and f.id=$2
 and (f.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=f.import_job_id and j.status='succeeded'))
), task_tree as (
 select t.id,t.member_run_ids,t.parent_task_run_id,array[t.id] path,0 depth
 from application_run_log_tasks t join selected s on s.task_id=t.id
 join flow_runs a on a.id=t.id and a.application_id=$1
 where t.application_id=$1 and (a.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=a.import_job_id and j.status='succeeded'))
 union all
 select t.id,t.member_run_ids,t.parent_task_run_id,p.path||t.id,p.depth+1
 from application_run_log_tasks t join task_tree p on t.parent_task_run_id=p.id
 join flow_runs a on a.id=t.id and a.application_id=$1
 where t.application_id=$1 and not t.id=any(p.path)
 and (a.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=a.import_job_id and j.status='succeeded'))
), members as (
 select t.id task_run_id,t.parent_task_run_id,m.id flow_run_id,t.depth
 from task_tree t cross join lateral unnest(t.member_run_ids) m(id)
 union all
 select s.id,null::uuid,s.id,0 from selected s where not exists(select 1 from task_tree)
), scope_runs as (
 select distinct f.id,f.run_mode,m.task_run_id,m.parent_task_run_id,m.depth,
 (f.id<>m.task_run_id or s.call_kind='compact') as is_round
 from members m join flow_runs f on f.id=m.flow_run_id and f.application_id=$1
 left join application_run_log_summaries s on s.flow_run_id=f.id and s.application_id=$1
 where f.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=f.import_job_id and j.status='succeeded')
)
