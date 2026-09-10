alter table ui_code_templates
    add column owner_plugin_code text,
    add column owner_feature_id text,
    add column applied_plugin_version text,
    add constraint ui_code_templates_plugin_ownership_complete check (
        (owner_plugin_code is null and owner_feature_id is null and applied_plugin_version is null)
        or (owner_plugin_code is not null and owner_feature_id is not null and applied_plugin_version is not null)
    );
create unique index ui_code_templates_plugin_owner on ui_code_templates(scope_id, owner_plugin_code, contribution_code)
    where owner_plugin_code is not null;

-- Verified package defaults are retained before enable; storing them does not apply them.
create table plugin_settings_template_defaults (
    installation_id uuid not null references extension_installations(id) on delete cascade,
    contribution_code text not null,
    feature_id text not null,
    source text not null check (octet_length(source) between 1 and 262144),
    language text not null check (language in ('jsx','tsx')),
    primary key (installation_id, contribution_code)
);
create table plugin_settings_template_applications (
    scope_id uuid not null,
    category text not null,
    organization text not null,
    artifact_id text not null,
    application_generation bigint not null,
    installation_id uuid not null,
    applied_at timestamptz not null default now(),
    primary key(scope_id,category,organization,artifact_id,application_generation),
    unique(scope_id,category,organization,artifact_id,application_generation,installation_id),
    foreign key(scope_id,category,organization,artifact_id,application_generation,installation_id)
      references native_plugin_application_requests(scope_id,category,organization,artifact_id,application_generation,installation_id) on delete restrict
);
alter table ui_code_templates
    add column applied_installation_id uuid,
    add column applied_application_generation bigint,
    add column owner_category text,
    add column owner_organization text,
    add column owner_artifact_id text,
    add constraint ui_code_templates_native_application_fk
      foreign key(scope_id,owner_category,owner_organization,owner_artifact_id,applied_application_generation,applied_installation_id)
      references plugin_settings_template_applications(scope_id,category,organization,artifact_id,application_generation,installation_id)
      deferrable initially deferred,
    add constraint ui_code_templates_native_application_complete check (
      (owner_plugin_code is null and applied_installation_id is null and applied_application_generation is null and owner_category is null and owner_organization is null and owner_artifact_id is null)
      or (owner_plugin_code is not null and applied_installation_id is not null and applied_application_generation is not null and owner_category is not null and owner_organization is not null and owner_artifact_id is not null)
    );
