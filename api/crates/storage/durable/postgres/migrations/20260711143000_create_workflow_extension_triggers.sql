create table if not exists workflow_extension_triggers (
    id uuid primary key,
    application_id uuid not null references applications(id) on delete cascade,
    scope_id uuid not null,
    subpath text not null,
    http_method text not null,
    response_mode text not null,
    parameter_mapping jsonb not null default '[]'::jsonb,
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint workflow_extension_triggers_subpath_nonempty_chk check (btrim(subpath) <> ''),
    constraint workflow_extension_triggers_http_method_chk check (
        http_method in ('GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS')
    ),
    constraint workflow_extension_triggers_response_mode_chk check (
        response_mode in ('sync', 'async')
    )
);

create unique index if not exists workflow_extension_triggers_application_id_uidx
    on workflow_extension_triggers (application_id);

create unique index if not exists workflow_extension_triggers_subpath_uidx
    on workflow_extension_triggers (subpath);

create index if not exists workflow_extension_triggers_scope_updated_id_idx
    on workflow_extension_triggers (scope_id, updated_at, id);

insert into workflow_extension_triggers (
    id,
    application_id,
    scope_id,
    subpath,
    http_method,
    response_mode,
    parameter_mapping,
    created_by,
    updated_by,
    created_at,
    updated_at
)
select
    gen_random_uuid(),
    mapping.application_id,
    application.scope_id,
    mapping.mapping_config #>> '{extension,slug}',
    mapping.mapping_config #>> '{extension,method}',
    mapping.mapping_config #>> '{extension,response_mode}',
    coalesce(mapping.mapping_config #> '{extension,parameters}', '[]'::jsonb),
    mapping.updated_by,
    mapping.updated_by,
    mapping.updated_at,
    mapping.updated_at
from application_api_mappings mapping
join applications application on application.id = mapping.application_id
where mapping.mapping_config ? 'extension'
  and nullif(mapping.mapping_config #>> '{extension,slug}', '') is not null
on conflict (application_id) do nothing;
