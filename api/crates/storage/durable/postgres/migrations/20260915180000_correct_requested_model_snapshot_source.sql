-- Preserve the external caller's selected model when an older snapshot used
-- the internal sys fallback instead. List queries still read task fields only.
update application_run_log_summaries summaries
set requested_model_id = runs.input_payload #>> '{node-start,model}'
from flow_runs runs
where runs.id = summaries.flow_run_id
  and runs.input_payload #>> '{node-start,model}' is not null
  and summaries.requested_model_id is distinct from runs.input_payload #>> '{node-start,model}';

update application_run_log_tasks tasks
set requested_model_id = summaries.requested_model_id
from application_run_log_summaries summaries
where summaries.flow_run_id = tasks.id
  and tasks.requested_model_id is distinct from summaries.requested_model_id;
