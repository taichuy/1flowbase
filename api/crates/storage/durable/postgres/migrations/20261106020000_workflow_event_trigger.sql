-- Workflow events remain distinct from extension and schedule triggers.
ALTER TABLE applications DROP CONSTRAINT applications_workflow_trigger_type_check;
ALTER TABLE applications ADD CONSTRAINT applications_workflow_trigger_type_check CHECK (
    (application_type = 'workflow' AND workflow_trigger_type IN ('extension','schedule','event'))
    OR (application_type <> 'workflow' AND workflow_trigger_type IS NULL)
);
ALTER TABLE flow_runs DROP CONSTRAINT flow_runs_run_mode_check;
ALTER TABLE flow_runs ADD CONSTRAINT flow_runs_run_mode_check CHECK (
    run_mode IN ('debug_node_preview','debug_flow_run','published_api_run','workflow_http_run','workflow_schedule_run','assistant_execution','workflow_event_run')
);
CREATE UNIQUE INDEX flow_runs_workflow_event_idempotency_key_unique
ON flow_runs (application_id, idempotency_key)
WHERE run_mode = 'workflow_event_run' AND idempotency_key IS NOT NULL;
