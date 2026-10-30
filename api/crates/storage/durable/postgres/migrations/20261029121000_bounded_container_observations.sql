-- Pending payloads are cleared only after every idempotent observation write.
create table flow_container_observations (
    id uuid primary key,
    sequence bigint generated always as identity unique,
    flow_run_id uuid not null,
    execution_id uuid not null,
    observation_key text not null,
    trace_payload jsonb,
    raw_json_payloads jsonb not null default '{}'::jsonb,
    completed_at timestamptz,
    unique(flow_run_id,execution_id,observation_key),
    foreign key(flow_run_id,execution_id) references flow_container_executions(flow_run_id,execution_id) on delete cascade
);
create index flow_container_observations_pending_idx on flow_container_observations(flow_run_id,sequence) where completed_at is null;
alter table flow_container_executions add column last_observation_sequence bigint not null default 0;
