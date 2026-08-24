-- WP-D2 intentionally replaces the unpublished override/revision contract without data migration.
drop table if exists ui_component_contract_revisions;
drop table if exists ui_component_overrides;

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
    constraint ui_component_records_identity_unique unique (scope_id, component_code)
);

create index ui_component_records_origin_group_idx
    on ui_component_records (scope_id, origin, source, "group", component_code);
