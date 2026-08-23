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

create table ui_component_records (
    id uuid primary key,
    scope_id uuid not null default '00000000-0000-0000-0000-000000000000'::uuid,
    component_code text not null,
    name text not null,
    description text not null,
    import_code text not null,
    source_code text not null,
    origin text not null,
    source text not null,
    "group" text not null,
    upstream_identity text not null,
    upstream_version text not null,
    version text not null,
    keywords text[] not null default '{}',
    catalog_updated_at timestamptz,
    source_locator text,
    source_checksum text,
    created_by uuid not null,
    updated_by uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint ui_component_records_system_scope_check
        check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid),
    constraint ui_component_records_component_code_check
        check (component_code ~ '^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$'),
    constraint ui_component_records_required_text_check check (
        length(trim(name)) > 0
        and length(trim(description)) > 0
        and length(trim(import_code)) > 0
        and length(trim(source_code)) > 0
        and length(trim(upstream_identity)) > 0
        and length(trim(upstream_version)) > 0
    ),
    constraint ui_component_records_origin_check check (origin in ('official', 'custom')),
    constraint ui_component_records_source_check
        check (source ~ '^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$'),
    constraint ui_component_records_group_check
        check ("group" ~ '^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$'),
    constraint ui_component_records_version_check check (version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'),
    constraint ui_component_records_source_checksum_check check (
        source_checksum is null or source_checksum ~ '^sha256:[a-f0-9]{64}$'
    ),
    constraint ui_component_records_identity_unique unique (scope_id, component_code)
);

create index ui_component_records_origin_group_idx
    on ui_component_records (scope_id, origin, source, "group", component_code);
