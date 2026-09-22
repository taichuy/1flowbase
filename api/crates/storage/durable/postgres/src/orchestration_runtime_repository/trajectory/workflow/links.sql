select coalesce(jsonb_agg(to_jsonb(l) order by l.relation,l.request_id),'[]'::jsonb) from (
 select 'trigger'::text relation,c.flow_run_id,c.request_id,null::text response_id
 from client_trajectory_captures c join flow_runs f on f.id=c.flow_run_id
 where f.application_id=$1 and c.flow_run_id=$2 and c.request_id::text=$3
 union
 select 'context',c.flow_run_id,c.request_id,c.response_id
 from client_trajectory_captures c join flow_runs f on f.id=c.flow_run_id
 where f.application_id=$1 and c.flow_run_id::text=$4 and c.response_id=$5
 and (f.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=f.import_job_id and j.status='succeeded'))
 union
 select distinct 'context',c.flow_run_id,c.request_id,$5
 from client_trajectory_steps c join flow_runs f on f.id=c.flow_run_id
 where f.application_id=$1 and c.flow_run_id::text=$4 and c.metadata->>'response_id'=$5 and c.metadata->>'origin'='emitted'
 and (f.import_job_id is null or exists(select 1 from run_archive_import_jobs j where j.id=f.import_job_id and j.status='succeeded'))
) l
