-- One mutable recovery head, not one full context copy per transition.
create table flow_container_checkpoints (
    flow_run_id uuid primary key references flow_runs(id) on delete cascade,
    checkpoint jsonb not null,
    raw_json_payloads jsonb not null default '{}'::jsonb,
    revision bigint not null default 1,
    native_recovery_execution_id uuid,
    native_recovery_started boolean not null default false,
    updated_at timestamptz not null default now()
);

-- A successor run may preserve the same logical execution identity while owning
-- its own node record. Within a run each identity has exactly one node receipt.
create table flow_container_executions (
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    execution_id uuid not null,
    node_id text not null,
    parent_execution_id uuid,
    round_path jsonb not null,
    node_run_id uuid unique references node_runs(id) on delete cascade,
    primary key (flow_run_id, execution_id)
);
create index flow_container_execution_parent_idx
    on flow_container_executions(flow_run_id, parent_execution_id);

create table flow_container_rounds (
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    execution_id uuid not null,
    container_execution_id uuid not null,
    round_index bigint not null check (round_index >= 0),
    deadline_unix_ms bigint not null,
    started_at timestamptz not null default now(),
    finished_at timestamptz,
    status text not null check (status in ('running','waiting','succeeded','failed','cancelled')),
    error_payload jsonb,
    output_payload jsonb,
    variables jsonb,
    raw_json_payloads jsonb not null default '{}'::jsonb,
    primary key (flow_run_id, execution_id),
    unique (flow_run_id, container_execution_id, round_index)
);

-- Condition evaluation after a successful round is a distinct control fact;
-- it never rewrites the already committed successful round.
create table flow_container_control_failures (
    flow_run_id uuid not null,
    execution_id uuid not null,
    after_round_index bigint not null check (after_round_index >= 0),
    error_payload jsonb not null,
    raw_json_payloads jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    primary key(flow_run_id,execution_id,after_round_index),
    foreign key(flow_run_id,execution_id) references flow_container_executions(flow_run_id,execution_id) on delete cascade
);
