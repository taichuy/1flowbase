create table assistant_conversations (
    conversation_id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    created_by uuid not null references users(id) on delete cascade,
    seed_legacy_flow_run_id uuid references flow_runs(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

alter table assistant_conversations
    add constraint assistant_conversations_application_conversation_key
    unique (application_id, conversation_id);

create index assistant_conversations_scope_user_application_updated_idx
    on assistant_conversations (scope_id, created_by, application_id, updated_at desc, conversation_id desc);

alter table flow_runs
    add column if not exists assistant_conversation_id uuid;

alter table flow_runs
    add constraint flow_runs_assistant_conversation_application_fk
    foreign key (application_id, assistant_conversation_id)
    references assistant_conversations(application_id, conversation_id)
    on delete restrict;

create index flow_runs_assistant_conversation_started_idx
    on flow_runs (assistant_conversation_id, started_at asc, id asc)
    where assistant_conversation_id is not null;
