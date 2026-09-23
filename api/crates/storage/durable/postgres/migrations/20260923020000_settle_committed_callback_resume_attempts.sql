-- Older executions could commit a callback claim after the ingress owner vanished,
-- leaving the public attempt parked even though the next callback was durable.
update flow_run_callback_resume_attempts a
   set status = 'succeeded',
       error_payload = null,
       completed_at = coalesce(a.completed_at, c.completed_at, now()),
       updated_at = now(),
       raw_json_payloads = a.raw_json_payloads - 'error_payload'
  from flow_run_resume_claims c
  join flow_run_callback_tasks task on task.id = c.callback_task_id
 where a.callback_task_id = c.callback_task_id
   and a.flow_run_id = c.flow_run_id
   and c.status = 'succeeded'
   and task.status = 'completed'
   and a.status in ('processing', 'received');
