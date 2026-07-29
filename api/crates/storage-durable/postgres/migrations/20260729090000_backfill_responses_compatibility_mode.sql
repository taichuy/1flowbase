-- Only the server-owned Responses transport summary is strong enough to recover the
-- historical ingress mapping protocol. Client envelopes, Provider routes, model names,
-- and prompt text are deliberately not used as backfill evidence.
update flow_runs
set compatibility_mode = 'openai-responses-v1',
    updated_at = greatest(updated_at, now())
where compatibility_mode is null
  and import_job_id is null
  and input_payload #>> '{sys,public_provider_transport,protocol}' = 'openai_responses';

update application_run_log_summaries summaries
set compatibility_mode = runs.compatibility_mode,
    log_updated_at = greatest(summaries.log_updated_at, now())
from flow_runs runs
where summaries.flow_run_id = runs.id
  and summaries.compatibility_mode is null
  and runs.compatibility_mode = 'openai-responses-v1'
  and runs.import_job_id is null
  and runs.input_payload #>> '{sys,public_provider_transport,protocol}' = 'openai_responses';

-- compatibility_mode participates in the trace source watermark. Do not rewrite the
-- watermark as if the old projection had observed the recovered value; make it stale so
-- the existing projection owner can rebuild it from durable run facts.
update application_run_trace_projection_statuses statuses
set status = 'stale',
    updated_at = greatest(statuses.updated_at, now())
from flow_runs runs
where statuses.flow_run_id = runs.id
  and statuses.status <> 'stale'
  and runs.compatibility_mode = 'openai-responses-v1'
  and runs.import_job_id is null
  and runs.input_payload #>> '{sys,public_provider_transport,protocol}' = 'openai_responses';
