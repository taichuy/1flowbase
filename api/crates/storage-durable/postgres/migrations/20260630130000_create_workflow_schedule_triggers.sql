create table if not exists workflow_schedule_triggers (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    scope_id uuid not null,
    enabled boolean not null default false,
    cron text not null,
    timezone text not null,
    input_payload jsonb not null default '{}'::jsonb,
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint workflow_schedule_triggers_cron_nonempty_chk check (btrim(cron) <> ''),
    constraint workflow_schedule_triggers_timezone_nonempty_chk check (btrim(timezone) <> '')
);

create unique index if not exists workflow_schedule_triggers_application_id_uidx
    on workflow_schedule_triggers (application_id);

create index if not exists workflow_schedule_triggers_scope_updated_id_idx
    on workflow_schedule_triggers (scope_id, updated_at, id);

create index if not exists workflow_schedule_triggers_scope_created_id_idx
    on workflow_schedule_triggers (scope_id, created_at, id);
