alter table data_source_secrets add column if not exists id uuid;
alter table data_source_secrets add column if not exists scope_id uuid;
alter table data_source_secrets add column if not exists created_at timestamptz;
alter table data_source_secrets add column if not exists created_by uuid;
alter table data_source_secrets add column if not exists updated_by uuid;
update data_source_secrets secrets
   set id = coalesce(
           secrets.id,
           md5('1flowbase.data_source_secret:' || secrets.data_source_instance_id::text)::uuid
       ),
       scope_id = instances.scope_id,
       created_at = coalesce(secrets.created_at, secrets.updated_at),
       created_by = coalesce(secrets.created_by, instances.created_by),
       updated_by = coalesce(secrets.updated_by, instances.created_by)
  from data_source_instances instances
 where secrets.data_source_instance_id = instances.id
   and (
       secrets.id is null
       or secrets.scope_id is null
       or secrets.created_at is null
       or secrets.created_by is null
       or secrets.updated_by is null
   );
alter table data_source_secrets alter column id set not null;
alter table data_source_secrets alter column scope_id set not null;
alter table data_source_secrets alter column created_at set default now();
alter table data_source_secrets alter column created_at set not null;
create index if not exists data_source_secrets_scope_created_id_idx
    on data_source_secrets (scope_id, created_at, id);

alter table data_source_catalog_caches add column if not exists id uuid;
alter table data_source_catalog_caches add column if not exists scope_id uuid;
alter table data_source_catalog_caches add column if not exists created_at timestamptz;
alter table data_source_catalog_caches add column if not exists created_by uuid;
alter table data_source_catalog_caches add column if not exists updated_by uuid;
update data_source_catalog_caches caches
   set id = coalesce(
           caches.id,
           md5('1flowbase.data_source_catalog_cache:' || caches.data_source_instance_id::text)::uuid
       ),
       scope_id = instances.scope_id,
       created_at = coalesce(caches.created_at, caches.updated_at),
       created_by = coalesce(caches.created_by, instances.created_by),
       updated_by = coalesce(caches.updated_by, instances.created_by)
  from data_source_instances instances
 where caches.data_source_instance_id = instances.id
   and (
       caches.id is null
       or caches.scope_id is null
       or caches.created_at is null
       or caches.created_by is null
       or caches.updated_by is null
   );
alter table data_source_catalog_caches alter column id set not null;
alter table data_source_catalog_caches alter column scope_id set not null;
alter table data_source_catalog_caches alter column created_at set default now();
alter table data_source_catalog_caches alter column created_at set not null;
create index if not exists data_source_catalog_caches_scope_created_id_idx
    on data_source_catalog_caches (scope_id, created_at, id);
