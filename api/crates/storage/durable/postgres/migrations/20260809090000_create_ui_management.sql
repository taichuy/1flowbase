create table ui_code_templates (
    id uuid primary key,
    scope_id uuid not null default '00000000-0000-0000-0000-000000000000'::uuid,
    provider_code text not null,
    contribution_code text not null,
    name text not null,
    archived_at timestamptz,
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint ui_code_templates_system_scope_check
        check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid),
    constraint ui_code_templates_provider_code_check check (length(trim(provider_code)) > 0),
    constraint ui_code_templates_contribution_code_check check (length(trim(contribution_code)) > 0),
    constraint ui_code_templates_name_check check (length(trim(name)) > 0)
);

create unique index ui_code_templates_active_name_idx
    on ui_code_templates (scope_id, provider_code, contribution_code, lower(name))
    where archived_at is null;

create table ui_code_template_revisions (
    id uuid primary key,
    template_id uuid not null references ui_code_templates(id) on delete cascade,
    revision integer not null,
    source text not null,
    language text not null,
    is_latest boolean not null default false,
    is_published boolean not null default false,
    created_by uuid not null,
    created_at timestamptz not null default now(),
    constraint ui_code_template_revisions_revision_check check (revision > 0),
    constraint ui_code_template_revisions_source_check
        check (length(trim(source)) > 0 and length(source) <= 262144),
    constraint ui_code_template_revisions_language_check check (language in ('jsx', 'tsx')),
    constraint ui_code_template_revisions_number_unique unique (template_id, revision)
);

create unique index ui_code_template_revisions_latest_idx
    on ui_code_template_revisions (template_id) where is_latest;
create unique index ui_code_template_revisions_published_idx
    on ui_code_template_revisions (template_id) where is_published;

create table ui_code_template_defaults (
    scope_id uuid not null default '00000000-0000-0000-0000-000000000000'::uuid,
    provider_code text not null,
    contribution_code text not null,
    template_id uuid not null references ui_code_templates(id) on delete cascade,
    updated_by uuid not null,
    updated_at timestamptz not null default now(),
    primary key (scope_id, provider_code, contribution_code),
    constraint ui_code_template_defaults_system_scope_check
        check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid)
);

create table ui_component_overrides (
    id uuid primary key,
    scope_id uuid not null default '00000000-0000-0000-0000-000000000000'::uuid,
    provider_code text not null,
    contribution_code text not null,
    module_source text not null,
    export_name text not null,
    state text not null default 'inherit',
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint ui_component_overrides_system_scope_check
        check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid),
    constraint ui_component_overrides_locator_check check (
        length(trim(provider_code)) > 0
        and length(trim(contribution_code)) > 0
        and length(trim(module_source)) > 0
        and length(trim(export_name)) > 0
    ),
    constraint ui_component_overrides_state_check check (state in ('inherit', 'published', 'hidden')),
    constraint ui_component_overrides_locator_unique
        unique (scope_id, provider_code, contribution_code, module_source, export_name)
);

create table ui_component_contract_revisions (
    id uuid primary key,
    component_override_id uuid not null references ui_component_overrides(id) on delete cascade,
    revision integer not null,
    contract jsonb not null,
    is_latest boolean not null default false,
    is_published boolean not null default false,
    created_by uuid not null,
    created_at timestamptz not null default now(),
    constraint ui_component_contract_revisions_revision_check check (revision > 0),
    constraint ui_component_contract_revisions_contract_check check (jsonb_typeof(contract) = 'object'),
    constraint ui_component_contract_revisions_number_unique
        unique (component_override_id, revision)
);

create unique index ui_component_contract_revisions_latest_idx
    on ui_component_contract_revisions (component_override_id) where is_latest;
create unique index ui_component_contract_revisions_published_idx
    on ui_component_contract_revisions (component_override_id) where is_published;
