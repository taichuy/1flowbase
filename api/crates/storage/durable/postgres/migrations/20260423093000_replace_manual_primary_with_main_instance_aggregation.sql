create table model_provider_main_instances (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    provider_code text not null,
    auto_include_new_instances boolean not null default true,
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (workspace_id, provider_code)
);

alter table model_provider_instances
    add column included_in_main boolean not null default true;

insert into model_provider_main_instances (
    workspace_id,
    provider_code,
    auto_include_new_instances,
    created_by,
    updated_by
)
select distinct on (workspace_id, provider_code)
    workspace_id,
    provider_code,
    true,
    created_by,
    updated_by
from model_provider_instances
order by workspace_id, provider_code, created_at asc, id asc;

update model_provider_instances
set included_in_main = true
where included_in_main is distinct from true;
