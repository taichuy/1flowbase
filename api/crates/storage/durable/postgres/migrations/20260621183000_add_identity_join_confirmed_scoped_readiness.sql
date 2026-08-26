alter table api_key_data_model_permissions add column if not exists id uuid;
alter table api_key_data_model_permissions add column if not exists scope_id uuid;
alter table api_key_data_model_permissions add column if not exists created_by uuid;
alter table api_key_data_model_permissions add column if not exists updated_by uuid;

do $$
begin
    if exists (
        select 1
          from api_key_data_model_permissions permissions
          join api_keys keys on keys.id = permissions.api_key_id
          join model_definitions models on models.id = permissions.data_model_id
         where keys.scope_id <> models.scope_id
    ) then
        raise exception 'api_key_data_model_permissions contains cross-scope data model grants';
    end if;
end $$;

update api_key_data_model_permissions permissions
   set id = coalesce(
           permissions.id,
           md5(
               '1flowbase.api_key_data_model_permission:'
               || permissions.api_key_id::text
               || ':'
               || permissions.data_model_id::text
           )::uuid
       ),
       scope_id = keys.scope_id,
       created_by = coalesce(permissions.created_by, keys.creator_user_id),
       updated_by = coalesce(permissions.updated_by, keys.creator_user_id)
  from api_keys keys
 where permissions.api_key_id = keys.id
   and (
       permissions.id is null
       or permissions.scope_id is null
       or permissions.created_by is null
       or permissions.updated_by is null
   );
alter table api_key_data_model_permissions alter column id set not null;
alter table api_key_data_model_permissions alter column scope_id set not null;
create index if not exists api_key_data_model_permissions_scope_created_id_idx
    on api_key_data_model_permissions (scope_id, created_at, id);

alter table application_api_mappings add column if not exists id uuid;
alter table application_api_mappings add column if not exists scope_id uuid;
alter table application_api_mappings add column if not exists created_at timestamptz;
alter table application_api_mappings add column if not exists created_by uuid;
update application_api_mappings mappings
   set id = coalesce(
           mappings.id,
           md5('1flowbase.application_api_mapping:' || mappings.application_id::text)::uuid
       ),
       scope_id = applications.scope_id,
       created_at = coalesce(mappings.created_at, mappings.updated_at),
       created_by = coalesce(mappings.created_by, mappings.updated_by)
  from applications
 where mappings.application_id = applications.id
   and (
       mappings.id is null
       or mappings.scope_id is null
       or mappings.created_at is null
       or mappings.created_by is null
   );
alter table application_api_mappings alter column id set not null;
alter table application_api_mappings alter column scope_id set not null;
alter table application_api_mappings alter column created_at set default now();
alter table application_api_mappings alter column created_at set not null;
create index if not exists application_api_mappings_scope_created_id_idx
    on application_api_mappings (scope_id, created_at, id);

alter table application_environment_variables add column if not exists id uuid;
alter table application_environment_variables add column if not exists scope_id uuid;
update application_environment_variables variables
   set id = coalesce(
           variables.id,
           md5(
               '1flowbase.application_environment_variable:'
               || variables.application_id::text
               || ':'
               || variables.name
           )::uuid
       ),
       scope_id = applications.scope_id
  from applications
 where variables.application_id = applications.id
   and (variables.id is null or variables.scope_id is null);
alter table application_environment_variables alter column id set not null;
alter table application_environment_variables alter column scope_id set not null;
create index if not exists application_environment_variables_scope_created_id_idx
    on application_environment_variables (scope_id, created_at, id);

alter table application_tag_bindings add column if not exists id uuid;
alter table application_tag_bindings add column if not exists scope_id uuid;
alter table application_tag_bindings add column if not exists updated_at timestamptz;
alter table application_tag_bindings add column if not exists updated_by uuid;

do $$
begin
    if exists (
        select 1
          from application_tag_bindings bindings
          join applications applications on applications.id = bindings.application_id
          join application_tags tags on tags.id = bindings.tag_id
         where applications.workspace_id <> tags.workspace_id
    ) then
        raise exception 'application_tag_bindings contains cross-workspace tag bindings';
    end if;
end $$;

update application_tag_bindings bindings
   set id = coalesce(
           bindings.id,
           md5(
               '1flowbase.application_tag_binding:'
               || bindings.application_id::text
               || ':'
               || bindings.tag_id::text
           )::uuid
       ),
       scope_id = applications.scope_id,
       updated_at = coalesce(bindings.updated_at, bindings.created_at),
       updated_by = coalesce(bindings.updated_by, bindings.created_by)
  from applications
 where bindings.application_id = applications.id
   and (
       bindings.id is null
       or bindings.scope_id is null
       or bindings.updated_at is null
       or bindings.updated_by is null
   );
alter table application_tag_bindings alter column id set not null;
alter table application_tag_bindings alter column scope_id set not null;
alter table application_tag_bindings alter column updated_at set default now();
alter table application_tag_bindings alter column updated_at set not null;
create index if not exists application_tag_bindings_scope_created_id_idx
    on application_tag_bindings (scope_id, created_at, id);

alter table frontstage_page_schemas add column if not exists id uuid;
alter table frontstage_page_schemas add column if not exists scope_id uuid;
alter table frontstage_page_schemas add column if not exists created_by uuid;
alter table frontstage_page_schemas add column if not exists updated_by uuid;

do $$
begin
    if exists (
        select 1
          from frontstage_page_schemas schemas
          join frontstage_pages pages on pages.id = schemas.page_id
         where schemas.workspace_id <> pages.workspace_id
    ) then
        raise exception 'frontstage_page_schemas contains cross-workspace page schemas';
    end if;
end $$;

update frontstage_page_schemas schemas
   set id = coalesce(
           schemas.id,
           md5('1flowbase.frontstage_page_schema:' || schemas.page_id::text)::uuid
       ),
       scope_id = pages.scope_id,
       created_by = coalesce(schemas.created_by, pages.created_by),
       updated_by = coalesce(schemas.updated_by, pages.updated_by, pages.created_by)
  from frontstage_pages pages
 where schemas.page_id = pages.id
   and (
       schemas.id is null
       or schemas.scope_id is null
       or schemas.created_by is null
       or schemas.updated_by is null
   );
alter table frontstage_page_schemas alter column id set not null;
alter table frontstage_page_schemas alter column scope_id set not null;
create index if not exists frontstage_page_schemas_scope_created_id_idx
    on frontstage_page_schemas (scope_id, created_at, id);

alter table main_source_defaults add column if not exists id uuid;
alter table main_source_defaults add column if not exists scope_id uuid generated always as (workspace_id) stored;
alter table main_source_defaults add column if not exists created_by uuid;
update main_source_defaults
   set id = coalesce(
           id,
           md5('1flowbase.main_source_default:' || workspace_id::text)::uuid
       ),
       created_by = coalesce(created_by, updated_by)
 where id is null or created_by is null;
alter table main_source_defaults alter column id set not null;
alter table main_source_defaults alter column scope_id set not null;
create index if not exists main_source_defaults_scope_created_id_idx
    on main_source_defaults (scope_id, created_at, id);

alter table model_provider_instance_secrets add column if not exists id uuid;
alter table model_provider_instance_secrets add column if not exists scope_id uuid;
alter table model_provider_instance_secrets add column if not exists created_at timestamptz;
alter table model_provider_instance_secrets add column if not exists created_by uuid;
alter table model_provider_instance_secrets add column if not exists updated_by uuid;
update model_provider_instance_secrets secrets
   set id = coalesce(
           secrets.id,
           md5(
               '1flowbase.model_provider_instance_secret:'
               || secrets.provider_instance_id::text
           )::uuid
       ),
       scope_id = instances.scope_id,
       created_at = coalesce(secrets.created_at, secrets.updated_at),
       created_by = coalesce(secrets.created_by, instances.created_by),
       updated_by = coalesce(secrets.updated_by, instances.updated_by)
  from model_provider_instances instances
 where secrets.provider_instance_id = instances.id
   and (
       secrets.id is null
       or secrets.scope_id is null
       or secrets.created_at is null
       or secrets.created_by is null
       or secrets.updated_by is null
   );
alter table model_provider_instance_secrets alter column id set not null;
alter table model_provider_instance_secrets alter column scope_id set not null;
alter table model_provider_instance_secrets alter column created_at set default now();
alter table model_provider_instance_secrets alter column created_at set not null;
create index if not exists model_provider_instance_secrets_scope_created_id_idx
    on model_provider_instance_secrets (scope_id, created_at, id);

alter table model_provider_main_instances add column if not exists id uuid;
alter table model_provider_main_instances add column if not exists scope_id uuid generated always as (workspace_id) stored;
update model_provider_main_instances
   set id = coalesce(
           id,
           md5(
               '1flowbase.model_provider_main_instance:'
               || workspace_id::text
               || ':'
               || provider_code
           )::uuid
       )
 where id is null;
alter table model_provider_main_instances alter column id set not null;
alter table model_provider_main_instances alter column scope_id set not null;
create index if not exists model_provider_main_instances_scope_created_id_idx
    on model_provider_main_instances (scope_id, created_at, id);

alter table provider_instance_model_catalog_cache add column if not exists id uuid;
alter table provider_instance_model_catalog_cache add column if not exists scope_id uuid;
alter table provider_instance_model_catalog_cache add column if not exists created_at timestamptz;
alter table provider_instance_model_catalog_cache add column if not exists created_by uuid;
alter table provider_instance_model_catalog_cache add column if not exists updated_by uuid;
update provider_instance_model_catalog_cache caches
   set id = coalesce(
           caches.id,
           md5(
               '1flowbase.provider_instance_model_catalog_cache:'
               || caches.provider_instance_id::text
           )::uuid
       ),
       scope_id = instances.scope_id,
       created_at = coalesce(caches.created_at, caches.updated_at),
       created_by = coalesce(caches.created_by, instances.created_by),
       updated_by = coalesce(caches.updated_by, instances.updated_by)
  from model_provider_instances instances
 where caches.provider_instance_id = instances.id
   and (
       caches.id is null
       or caches.scope_id is null
       or caches.created_at is null
       or caches.created_by is null
       or caches.updated_by is null
   );
alter table provider_instance_model_catalog_cache alter column id set not null;
alter table provider_instance_model_catalog_cache alter column scope_id set not null;
alter table provider_instance_model_catalog_cache alter column created_at set default now();
alter table provider_instance_model_catalog_cache alter column created_at set not null;
create index if not exists provider_instance_model_catalog_cache_scope_created_id_idx
    on provider_instance_model_catalog_cache (scope_id, created_at, id);
