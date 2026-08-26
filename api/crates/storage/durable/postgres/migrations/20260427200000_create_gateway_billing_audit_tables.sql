create table runtime_cost_ledger (
    id uuid primary key,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    usage_ledger_id uuid references runtime_usage_ledger(id) on delete set null,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    provider_instance_id uuid,
    provider_account_id uuid,
    gateway_route_id uuid,
    model_id text,
    upstream_model_id text,
    price_snapshot jsonb not null default '{}'::jsonb,
    raw_cost numeric,
    normalized_cost numeric,
    settlement_currency text,
    cost_source text not null,
    cost_status text not null,
    created_at timestamptz not null default now()
);

create table runtime_credit_ledger (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    user_id uuid references users(id) on delete set null,
    app_id uuid references applications(id) on delete set null,
    agent_id uuid,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    span_id uuid references runtime_spans(id) on delete set null,
    cost_ledger_id uuid references runtime_cost_ledger(id) on delete set null,
    transaction_type text not null,
    amount numeric not null,
    balance_after numeric,
    credit_unit text not null,
    reason text not null,
    idempotency_key text not null,
    status text not null,
    created_at timestamptz not null default now(),
    unique(workspace_id, idempotency_key)
);

create table billing_sessions (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    flow_run_id uuid references flow_runs(id) on delete cascade,
    client_request_id text,
    idempotency_key text not null,
    route_id uuid,
    provider_account_id uuid,
    status text not null,
    reserved_credit_ledger_id uuid references runtime_credit_ledger(id) on delete set null,
    settled_credit_ledger_id uuid references runtime_credit_ledger(id) on delete set null,
    refund_credit_ledger_id uuid references runtime_credit_ledger(id) on delete set null,
    metadata jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique(workspace_id, idempotency_key)
);

create table provider_account_pools (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    provider_code text not null,
    upstream_kind text not null,
    accounts jsonb not null default '[]'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);
