alter table extension_installations add constraint extension_installations_native_target_identity
    unique (scope_id, category, organization, artifact_id, id);

create table native_plugin_application_requests (
    scope_id uuid not null,
    category text not null check (category = 'host-extensions'),
    organization text not null,
    artifact_id text not null,
    application_generation bigint not null check (application_generation > 0),
    installation_id uuid not null,
    created_by uuid not null references users(id) on delete restrict,
    created_at timestamptz not null default now(),
    primary key (scope_id, category, organization, artifact_id, application_generation),
    unique (scope_id, category, organization, artifact_id, application_generation, installation_id),
    foreign key (scope_id, category, organization, artifact_id, installation_id)
        references extension_installations(scope_id, category, organization, artifact_id, id) on delete restrict,
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid)
);

create table native_plugin_targets (
    scope_id uuid not null,
    category text not null check (category = 'host-extensions'),
    organization text not null,
    artifact_id text not null,
    installation_id uuid not null,
    selection_revision bigint not null check (selection_revision > 0),
    enabled boolean not null,
    application_generation bigint not null check (application_generation > 0),
    created_by uuid not null references users(id) on delete restrict,
    updated_by uuid not null references users(id) on delete restrict,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (scope_id, category, organization, artifact_id),
    foreign key (scope_id, category, organization, artifact_id, installation_id)
        references extension_installations(scope_id, category, organization, artifact_id, id) on delete restrict,
    foreign key (scope_id, category, organization, artifact_id, application_generation, installation_id)
        references native_plugin_application_requests(scope_id, category, organization, artifact_id, application_generation, installation_id) on delete restrict,
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid)
);

-- Only an unambiguous legacy enable is evidence of selection. Conflicting families remain
-- untouched and are reported by startup; an administrator resolves them by exact enable.
insert into native_plugin_application_requests
    (scope_id, category, organization, artifact_id, application_generation, installation_id, created_by)
select scope_id, category, organization, artifact_id, 1, id, created_by
from (
    select i.*, count(*) over (partition by scope_id, category, organization, artifact_id) as candidates
    from extension_installations i
    where category = 'host-extensions' and plugin_id is not null
      and desired_state in ('pending_restart', 'active_requested')
) legacy where candidates = 1;
insert into native_plugin_targets
    (scope_id, category, organization, artifact_id, installation_id, selection_revision, enabled,
     application_generation, created_by, updated_by)
select scope_id, category, organization, artifact_id, installation_id, 1, true,
       application_generation, created_by, created_by
from native_plugin_application_requests;
