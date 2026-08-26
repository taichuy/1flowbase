alter table application_run_log_summaries
    add column if not exists scope_id uuid;

update application_run_log_summaries summaries
set scope_id = applications.workspace_id
from applications
where summaries.application_id = applications.id
  and summaries.scope_id is null;

alter table application_run_log_summaries
    alter column scope_id set not null;

alter table application_run_log_summaries
    drop constraint if exists application_run_log_summaries_status_check;

alter table application_run_log_summaries
    add constraint application_run_log_summaries_status_check
    check (status in (
        'queued',
        'running',
        'waiting_callback',
        'waiting_human',
        'paused',
        'succeeded',
        'failed',
        'cancelled'
    ));

alter table application_run_log_summaries
    add column if not exists id uuid generated always as (flow_run_id) stored;

create index if not exists application_run_log_summaries_scope_created_idx
    on application_run_log_summaries (scope_id, created_at desc, flow_run_id desc);

create index if not exists application_run_log_summaries_scope_application_idx
    on application_run_log_summaries (scope_id, application_id, created_at desc, flow_run_id desc);

alter table node_runs
    add column if not exists scope_id uuid;

update node_runs
set scope_id = applications.workspace_id
from flow_runs
join applications on applications.id = flow_runs.application_id
where node_runs.flow_run_id = flow_runs.id
  and node_runs.scope_id is null;

alter table node_runs
    alter column scope_id set not null;

alter table node_runs
    add column if not exists node_run_id uuid generated always as (id) stored;

alter table node_runs
    add column if not exists created_at timestamptz;

update node_runs
set created_at = started_at
where created_at is null;

alter table node_runs
    alter column created_at set default now(),
    alter column created_at set not null;

create index if not exists node_runs_scope_flow_created_idx
    on node_runs (scope_id, flow_run_id, created_at desc, id desc);

create index if not exists node_runs_scope_node_created_idx
    on node_runs (scope_id, node_run_id, created_at desc, id desc);

alter table flow_run_events
    add column if not exists scope_id uuid;

update flow_run_events
set scope_id = applications.workspace_id
from flow_runs
join applications on applications.id = flow_runs.application_id
where flow_run_events.flow_run_id = flow_runs.id
  and flow_run_events.scope_id is null;

alter table flow_run_events
    alter column scope_id set not null;

create index if not exists flow_run_events_scope_flow_sequence_idx
    on flow_run_events (scope_id, flow_run_id, sequence asc, id asc);

create index if not exists flow_run_events_scope_node_sequence_idx
    on flow_run_events (scope_id, node_run_id, sequence asc, id asc);

alter table flow_run_checkpoints
    add column if not exists scope_id uuid;

update flow_run_checkpoints
set scope_id = applications.workspace_id
from flow_runs
join applications on applications.id = flow_runs.application_id
where flow_run_checkpoints.flow_run_id = flow_runs.id
  and flow_run_checkpoints.scope_id is null;

alter table flow_run_checkpoints
    alter column scope_id set not null;

create index if not exists flow_run_checkpoints_scope_flow_created_idx
    on flow_run_checkpoints (scope_id, flow_run_id, created_at desc, id desc);

create index if not exists flow_run_checkpoints_scope_node_created_idx
    on flow_run_checkpoints (scope_id, node_run_id, created_at desc, id desc);

alter table flow_run_callback_tasks
    add column if not exists scope_id uuid;

update flow_run_callback_tasks
set scope_id = applications.workspace_id
from flow_runs
join applications on applications.id = flow_runs.application_id
where flow_run_callback_tasks.flow_run_id = flow_runs.id
  and flow_run_callback_tasks.scope_id is null;

alter table flow_run_callback_tasks
    alter column scope_id set not null;

create index if not exists flow_run_callback_tasks_scope_flow_created_idx
    on flow_run_callback_tasks (scope_id, flow_run_id, created_at desc, id desc);

create index if not exists flow_run_callback_tasks_scope_node_created_idx
    on flow_run_callback_tasks (scope_id, node_run_id, created_at desc, id desc);

create table if not exists application_conversations (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    external_conversation_id text not null,
    external_user text,
    api_key_id uuid references api_keys(id) on delete set null,
    title text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (application_id, api_key_id, external_user, external_conversation_id)
);

create index if not exists application_conversations_scope_created_idx
    on application_conversations (scope_id, created_at desc, id desc);

create index if not exists application_conversations_application_idx
    on application_conversations (application_id, created_at desc, id desc);

create index if not exists application_conversations_external_id_idx
    on application_conversations (external_conversation_id, created_at desc, id desc);

create index if not exists application_conversations_external_user_idx
    on application_conversations (external_user, created_at desc, id desc);

create index if not exists application_conversations_api_key_idx
    on application_conversations (api_key_id, created_at desc, id desc);

insert into application_conversations (
    id,
    scope_id,
    application_id,
    api_key_id,
    external_user,
    external_conversation_id,
    created_at,
    updated_at
)
select
    legacy.id,
    applications.workspace_id,
    legacy.application_id,
    legacy.api_key_id,
    legacy.external_user,
    legacy.external_conversation_id,
    legacy.created_at,
    legacy.updated_at
from application_public_conversations legacy
join applications on applications.id = legacy.application_id
on conflict (application_id, api_key_id, external_user, external_conversation_id)
do update set updated_at = excluded.updated_at;

create table if not exists application_conversation_messages (
    id uuid primary key,
    scope_id uuid not null references workspaces(id) on delete cascade,
    conversation_id uuid not null references application_conversations(id) on delete cascade,
    application_id uuid not null references applications(id) on delete cascade,
    flow_run_id uuid references flow_runs(id) on delete set null,
    node_run_id uuid references node_runs(id) on delete set null,
    role text not null,
    content text not null,
    sequence bigint not null default 0,
    status text not null default 'succeeded',
    started_at timestamptz,
    finished_at timestamptz,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists application_conversation_messages_flow_sequence_unique_idx
    on application_conversation_messages (conversation_id, flow_run_id, sequence);

create index if not exists application_conversation_messages_conversation_sequence_idx
    on application_conversation_messages (conversation_id, sequence asc, created_at asc, id asc);

create index if not exists application_conversation_messages_scope_created_idx
    on application_conversation_messages (scope_id, created_at desc, id desc);

create index if not exists application_conversation_messages_flow_idx
    on application_conversation_messages (flow_run_id, created_at asc, id asc);

create index if not exists application_conversation_messages_node_idx
    on application_conversation_messages (node_run_id, created_at asc, id asc);

create index if not exists application_conversation_messages_role_idx
    on application_conversation_messages (role, created_at asc, id asc);

create temporary table builtin_runtime_read_models (
    id uuid primary key,
    code text not null,
    title text not null,
    physical_table_name text not null
) on commit drop;

insert into builtin_runtime_read_models (id, code, title, physical_table_name)
values
    ('00000000-0532-4000-8000-000000000001', 'application_run_log_summaries', 'Application run log summaries', 'application_run_log_summaries'),
    ('00000000-0533-4000-8000-000000000001', 'application_conversations', 'Application conversations', 'application_conversations'),
    ('00000000-0533-4000-8000-000000000002', 'application_conversation_messages', 'Application conversation messages', 'application_conversation_messages'),
    ('00000000-0534-4000-8000-000000000001', 'node_runs', 'Node runs', 'node_runs'),
    ('00000000-0534-4000-8000-000000000002', 'flow_run_events', 'Flow run events', 'flow_run_events'),
    ('00000000-0534-4000-8000-000000000003', 'flow_run_checkpoints', 'Flow run checkpoints', 'flow_run_checkpoints'),
    ('00000000-0534-4000-8000-000000000004', 'flow_run_callback_tasks', 'Flow run callback tasks', 'flow_run_callback_tasks');

insert into model_definitions (
    id,
    scope_kind,
    scope_id,
    data_source_instance_id,
    source_kind,
    external_resource_key,
    external_table_id,
    external_capability_snapshot,
    code,
    title,
    physical_table_name,
    acl_namespace,
    audit_namespace,
    availability_status,
    status,
    api_exposure_status,
    owner_kind,
    owner_id,
    is_protected,
    created_by,
    updated_by
)
select
    models.id,
    'system',
    '00000000-0000-0000-0000-000000000000'::uuid,
    null,
    'main_source',
    null,
    null,
    null,
    models.code,
    models.title,
    models.physical_table_name,
    'state_model.' || models.code,
    'audit.state_model.' || models.code,
    'available',
    'published',
    'published_not_exposed',
    'core',
    null,
    true,
    null,
    null
from builtin_runtime_read_models models
where not exists (
    select 1
    from model_definitions existing
    where existing.data_source_instance_id is null
      and existing.code = models.code
);

update model_definitions definitions
set
    scope_kind = 'system',
    scope_id = '00000000-0000-0000-0000-000000000000'::uuid,
    source_kind = 'main_source',
    external_resource_key = null,
    external_table_id = null,
    external_capability_snapshot = null,
    title = models.title,
    physical_table_name = models.physical_table_name,
    acl_namespace = 'state_model.' || models.code,
    audit_namespace = 'audit.state_model.' || models.code,
    availability_status = 'available',
    status = 'published',
    api_exposure_status = 'published_not_exposed',
    owner_kind = 'core',
    owner_id = null,
    is_protected = true,
    updated_at = now()
from builtin_runtime_read_models models
where definitions.data_source_instance_id is null
  and definitions.code = models.code;

create temporary table builtin_runtime_read_model_fields (
    model_code text not null,
    field_id uuid not null,
    code text not null,
    title text not null,
    physical_column_name text not null,
    field_kind text not null,
    is_required boolean not null,
    is_unique boolean not null,
    sort_order integer not null
) on commit drop;

insert into builtin_runtime_read_model_fields (
    model_code,
    field_id,
    code,
    title,
    physical_column_name,
    field_kind,
    is_required,
    is_unique,
    sort_order
)
values
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000001', 'id', 'ID', 'id', 'string', true, true, 0),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000002', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', true, true, 1),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000003', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 2),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000004', 'application_id', 'Application ID', 'application_id', 'many_to_one', true, false, 3),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000005', 'run_mode', 'Run mode', 'run_mode', 'string', true, false, 4),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000006', 'status', 'Status', 'status', 'string', true, false, 5),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000007', 'target_node_id', 'Target node ID', 'target_node_id', 'string', false, false, 6),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000008', 'title', 'Title', 'title', 'string', true, false, 7),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000009', 'external_user', 'External user', 'external_user', 'string', false, false, 8),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000010', 'authorized_account', 'Authorized account', 'authorized_account', 'string', false, false, 9),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000011', 'api_key_id', 'API key ID', 'api_key_id', 'many_to_one', false, false, 10),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000012', 'api_key_name_snapshot', 'API key name snapshot', 'api_key_name_snapshot', 'string', false, false, 11),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000013', 'publication_version_id', 'Publication version ID', 'publication_version_id', 'many_to_one', false, false, 12),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000014', 'external_conversation_id', 'External conversation ID', 'external_conversation_id', 'string', false, false, 13),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000015', 'external_trace_id', 'External trace ID', 'external_trace_id', 'string', false, false, 14),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000016', 'compatibility_mode', 'Compatibility mode', 'compatibility_mode', 'string', false, false, 15),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000017', 'idempotency_key', 'Idempotency key', 'idempotency_key', 'string', false, false, 16),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000018', 'total_tokens', 'Total tokens', 'total_tokens', 'number', false, false, 17),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000019', 'unique_node_count', 'Unique node count', 'unique_node_count', 'number', true, false, 18),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000020', 'tool_callback_count', 'Tool callback count', 'tool_callback_count', 'number', true, false, 19),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000021', 'started_at', 'Started at', 'started_at', 'datetime', true, false, 20),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000022', 'finished_at', 'Finished at', 'finished_at', 'datetime', false, false, 21),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000023', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 22),
    ('application_run_log_summaries', '00000000-1532-4000-8000-000000000024', 'updated_at', 'Updated at', 'updated_at', 'datetime', true, false, 23),
    ('application_conversations', '00000000-1533-4000-8000-000000000001', 'id', 'ID', 'id', 'string', true, true, 0),
    ('application_conversations', '00000000-1533-4000-8000-000000000002', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('application_conversations', '00000000-1533-4000-8000-000000000003', 'application_id', 'Application ID', 'application_id', 'many_to_one', true, false, 2),
    ('application_conversations', '00000000-1533-4000-8000-000000000004', 'external_conversation_id', 'External conversation ID', 'external_conversation_id', 'string', true, false, 3),
    ('application_conversations', '00000000-1533-4000-8000-000000000005', 'external_user', 'External user', 'external_user', 'string', false, false, 4),
    ('application_conversations', '00000000-1533-4000-8000-000000000006', 'api_key_id', 'API key ID', 'api_key_id', 'many_to_one', false, false, 5),
    ('application_conversations', '00000000-1533-4000-8000-000000000007', 'title', 'Title', 'title', 'string', false, false, 6),
    ('application_conversations', '00000000-1533-4000-8000-000000000008', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 7),
    ('application_conversations', '00000000-1533-4000-8000-000000000009', 'updated_at', 'Updated at', 'updated_at', 'datetime', true, false, 8),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000101', 'id', 'ID', 'id', 'string', true, true, 0),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000102', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000103', 'conversation_id', 'Conversation ID', 'conversation_id', 'many_to_one', true, false, 2),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000104', 'application_id', 'Application ID', 'application_id', 'many_to_one', true, false, 3),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000105', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', false, false, 4),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000106', 'node_run_id', 'Node run ID', 'node_run_id', 'many_to_one', false, false, 5),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000107', 'role', 'Role', 'role', 'string', true, false, 6),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000108', 'content', 'Content', 'content', 'text', true, false, 7),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000109', 'sequence', 'Sequence', 'sequence', 'number', true, false, 8),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000110', 'status', 'Status', 'status', 'string', true, false, 9),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000111', 'started_at', 'Started at', 'started_at', 'datetime', false, false, 10),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000112', 'finished_at', 'Finished at', 'finished_at', 'datetime', false, false, 11),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000113', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 12),
    ('application_conversation_messages', '00000000-1533-4000-8000-000000000114', 'updated_at', 'Updated at', 'updated_at', 'datetime', true, false, 13),
    ('node_runs', '00000000-1534-4000-8000-000000000001', 'id', 'ID', 'id', 'string', true, true, 0),
    ('node_runs', '00000000-1534-4000-8000-000000000002', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('node_runs', '00000000-1534-4000-8000-000000000003', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', true, false, 2),
    ('node_runs', '00000000-1534-4000-8000-000000000004', 'node_run_id', 'Node run ID', 'node_run_id', 'many_to_one', true, false, 3),
    ('node_runs', '00000000-1534-4000-8000-000000000005', 'node_id', 'Node ID', 'node_id', 'string', true, false, 4),
    ('node_runs', '00000000-1534-4000-8000-000000000006', 'node_type', 'Node type', 'node_type', 'string', true, false, 5),
    ('node_runs', '00000000-1534-4000-8000-000000000007', 'node_alias', 'Node alias', 'node_alias', 'string', true, false, 6),
    ('node_runs', '00000000-1534-4000-8000-000000000008', 'status', 'Status', 'status', 'string', true, false, 7),
    ('node_runs', '00000000-1534-4000-8000-000000000009', 'started_at', 'Started at', 'started_at', 'datetime', true, false, 8),
    ('node_runs', '00000000-1534-4000-8000-000000000010', 'finished_at', 'Finished at', 'finished_at', 'datetime', false, false, 9),
    ('node_runs', '00000000-1534-4000-8000-000000000011', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 10),
    ('flow_run_events', '00000000-1534-4000-8000-000000000101', 'id', 'ID', 'id', 'string', true, true, 0),
    ('flow_run_events', '00000000-1534-4000-8000-000000000102', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('flow_run_events', '00000000-1534-4000-8000-000000000103', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', true, false, 2),
    ('flow_run_events', '00000000-1534-4000-8000-000000000104', 'node_run_id', 'Node run ID', 'node_run_id', 'many_to_one', false, false, 3),
    ('flow_run_events', '00000000-1534-4000-8000-000000000105', 'sequence', 'Sequence', 'sequence', 'number', true, false, 4),
    ('flow_run_events', '00000000-1534-4000-8000-000000000106', 'event_type', 'Event type', 'event_type', 'string', true, false, 5),
    ('flow_run_events', '00000000-1534-4000-8000-000000000107', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 6),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000201', 'id', 'ID', 'id', 'string', true, true, 0),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000202', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000203', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', true, false, 2),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000204', 'node_run_id', 'Node run ID', 'node_run_id', 'many_to_one', false, false, 3),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000205', 'status', 'Status', 'status', 'string', true, false, 4),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000206', 'reason', 'Reason', 'reason', 'text', true, false, 5),
    ('flow_run_checkpoints', '00000000-1534-4000-8000-000000000207', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 6),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000301', 'id', 'ID', 'id', 'string', true, true, 0),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000302', 'scope_id', 'Scope ID', 'scope_id', 'many_to_one', true, false, 1),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000303', 'flow_run_id', 'Flow run ID', 'flow_run_id', 'many_to_one', true, false, 2),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000304', 'node_run_id', 'Node run ID', 'node_run_id', 'many_to_one', true, false, 3),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000305', 'callback_kind', 'Callback kind', 'callback_kind', 'string', true, false, 4),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000306', 'status', 'Status', 'status', 'string', true, false, 5),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000307', 'created_at', 'Created at', 'created_at', 'datetime', true, false, 6),
    ('flow_run_callback_tasks', '00000000-1534-4000-8000-000000000308', 'completed_at', 'Completed at', 'completed_at', 'datetime', false, false, 7);

insert into model_fields (
    id,
    data_model_id,
    code,
    title,
    physical_column_name,
    external_field_key,
    field_kind,
    is_system,
    is_writable,
    is_required,
    is_unique,
    default_value,
    display_interface,
    display_options,
    relation_target_model_id,
    relation_options,
    sort_order,
    availability_status,
    created_by,
    updated_by
)
select
    fields.field_id,
    definitions.id,
    fields.code,
    fields.title,
    fields.physical_column_name,
    null,
    fields.field_kind,
    true,
    false,
    fields.is_required,
    fields.is_unique,
    null,
    null,
    '{}'::jsonb,
    null,
    '{}'::jsonb,
    fields.sort_order,
    'available',
    null,
    null
from builtin_runtime_read_model_fields fields
join model_definitions definitions
  on definitions.data_source_instance_id is null
 and definitions.code = fields.model_code
where not exists (
    select 1
    from model_fields existing
    where existing.data_model_id = definitions.id
      and existing.code = fields.code
);

update model_fields target
set
    title = fields.title,
    physical_column_name = fields.physical_column_name,
    external_field_key = null,
    field_kind = fields.field_kind,
    is_system = true,
    is_writable = false,
    is_required = fields.is_required,
    is_unique = fields.is_unique,
    default_value = null,
    display_interface = null,
    display_options = '{}'::jsonb,
    relation_target_model_id = null,
    relation_options = '{}'::jsonb,
    sort_order = fields.sort_order,
    availability_status = 'available',
    updated_at = now()
from builtin_runtime_read_model_fields fields
join model_definitions definitions
  on definitions.data_source_instance_id is null
 and definitions.code = fields.model_code
where target.data_model_id = definitions.id
  and target.code = fields.code;

with model_scope_grants as (
    select
        'system'::text as scope_kind,
        '00000000-0000-0000-0000-000000000000'::uuid as scope_id,
        definitions.id as data_model_id,
        'system_all'::text as permission_profile
    from model_definitions definitions
    join builtin_runtime_read_models models on models.code = definitions.code
    where definitions.data_source_instance_id is null
    union all
    select
        'workspace',
        workspaces.id,
        definitions.id,
        'scope_all'
    from workspaces
    cross join model_definitions definitions
    join builtin_runtime_read_models models on models.code = definitions.code
    where definitions.data_source_instance_id is null
), hashed_model_scope_grants as (
    select
        (
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 1, 8) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 9, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 13, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 17, 4) || '-' ||
            substr(md5(scope_kind || ':' || scope_id::text || ':' || data_model_id::text), 21, 12)
        )::uuid as id,
        scope_kind,
        scope_id,
        data_model_id,
        permission_profile
    from model_scope_grants
)
insert into scope_data_model_grants (
    id,
    scope_kind,
    scope_id,
    data_model_id,
    enabled,
    permission_profile,
    created_by
)
select
    id,
    scope_kind,
    scope_id,
    data_model_id,
    true,
    permission_profile,
    null
from hashed_model_scope_grants
on conflict (scope_kind, scope_id, data_model_id)
do update set
    enabled = true,
    permission_profile = excluded.permission_profile,
    updated_at = now();
