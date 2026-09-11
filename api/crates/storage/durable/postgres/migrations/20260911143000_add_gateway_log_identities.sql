-- Log-only bindings: never participate in model history or provider cursor ownership.
create table gateway_log_conversations (
    id uuid primary key,
    scope_id uuid not null,
    application_id uuid not null references applications(id) on delete cascade,
    api_key_id uuid not null,
    external_user text not null,
    protocol text not null,
    thread_id text not null,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    unique (application_id, api_key_id, external_user, protocol, thread_id)
);
create index gateway_log_conversations_page on gateway_log_conversations(scope_id, application_id, updated_at desc, id desc);
create table gateway_log_turns (
    id uuid primary key,
    conversation_id uuid not null references gateway_log_conversations(id) on delete cascade,
    client_turn_id text not null,
    parent_task_id uuid references gateway_log_turns(id) on delete set null,
    parent_conversation_id uuid references gateway_log_conversations(id) on delete set null,
    relation_status text not null default 'no_parent_declared',
    created_at timestamptz not null,
    updated_at timestamptz not null,
    unique (conversation_id, client_turn_id)
);
create index gateway_log_turns_page on gateway_log_turns(conversation_id, created_at, id);
create table gateway_log_invocations (
    flow_run_id uuid primary key references flow_runs(id) on delete cascade,
    scope_id uuid not null,
    application_id uuid not null references applications(id) on delete cascade,
    api_key_id uuid not null,
    conversation_id uuid references gateway_log_conversations(id) on delete no action,
    turn_id uuid references gateway_log_turns(id) on delete no action,
    caused_by_run_id uuid references flow_runs(id) on delete set null,
    identity_status text not null,
    context jsonb not null,
    projection_version integer not null default 1,
    created_at timestamptz not null,
    check (flow_run_id is distinct from caused_by_run_id)
);
create index gateway_log_invocations_conversation on gateway_log_invocations(conversation_id, created_at, flow_run_id);
create index gateway_log_invocations_turn on gateway_log_invocations(turn_id, created_at, flow_run_id);
create index gateway_log_invocations_scope on gateway_log_invocations(scope_id, application_id, flow_run_id);
create index gateway_log_output_facts on flow_run_events(flow_run_id, sequence) where event_type = 'provider_output_item_done';
create index gateway_log_attempts on model_provider_request_logs(flow_run_id, started_at, attempt_id);
-- Rebuildable, identity-based projections over retained input and formal events.
create table gateway_log_output_items (
    owner_id uuid not null,
    item_key text not null,
    conversation_id uuid references gateway_log_conversations(id) on delete cascade,
    turn_id uuid references gateway_log_turns(id) on delete cascade,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    sequence bigint not null,
    observed_at timestamptz not null,
    item jsonb not null,
    conflicting boolean not null default false,
    primary key(owner_id,item_key)
);
create index gateway_log_output_run on gateway_log_output_items(flow_run_id,sequence);
create index gateway_log_output_turn on gateway_log_output_items(turn_id);
create table gateway_log_tool_results (
    conversation_id uuid not null references gateway_log_conversations(id) on delete cascade,
    call_id text not null,
    flow_run_id uuid not null references flow_runs(id) on delete cascade,
    received_at timestamptz not null,
    result jsonb not null,
    conflicting boolean not null default false,
    primary key(conversation_id,call_id)
);
