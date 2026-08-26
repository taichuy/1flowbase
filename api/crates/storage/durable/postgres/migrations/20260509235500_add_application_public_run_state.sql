alter table flow_runs drop constraint flow_runs_run_mode_check;

alter table flow_runs
    add constraint flow_runs_run_mode_check
    check (run_mode in ('debug_node_preview', 'debug_flow_run', 'published_api_run'));

alter table flow_runs
    add column api_key_id uuid,
    add column publication_version_id uuid,
    add column external_user text,
    add column external_conversation_id text,
    add column external_trace_id text,
    add column compatibility_mode text,
    add column idempotency_key text;

create table application_public_conversations (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    api_key_id uuid not null references api_keys(id) on delete cascade,
    external_user text not null,
    external_conversation_id text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (application_id, api_key_id, external_user, external_conversation_id)
);
