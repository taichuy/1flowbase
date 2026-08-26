alter table file_storages add column if not exists scope_id uuid;
update file_storages
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid;
alter table file_storages alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table file_storages alter column scope_id set not null;
alter table file_storages add constraint file_storages_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists file_storages_scope_created_id_idx
    on file_storages (scope_id, created_at, id);

alter table frontend_block_catalog add column if not exists scope_id uuid;
alter table frontend_block_catalog add column if not exists created_by uuid;
alter table frontend_block_catalog add column if not exists updated_by uuid;
update frontend_block_catalog
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid;
alter table frontend_block_catalog alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table frontend_block_catalog alter column scope_id set not null;
alter table frontend_block_catalog add constraint frontend_block_catalog_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists frontend_block_catalog_scope_created_id_idx
    on frontend_block_catalog (scope_id, created_at, id);

alter table host_extension_migrations add column if not exists scope_id uuid;
alter table host_extension_migrations add column if not exists created_by uuid;
alter table host_extension_migrations add column if not exists updated_by uuid;
alter table host_extension_migrations add column if not exists created_at timestamptz;
alter table host_extension_migrations add column if not exists updated_at timestamptz;
update host_extension_migrations
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid,
       created_at = coalesce(created_at, applied_at),
       updated_at = coalesce(updated_at, applied_at)
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid
    or created_at is null
    or updated_at is null;
alter table host_extension_migrations alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table host_extension_migrations alter column scope_id set not null;
alter table host_extension_migrations alter column created_at set default now();
alter table host_extension_migrations alter column created_at set not null;
alter table host_extension_migrations alter column updated_at set default now();
alter table host_extension_migrations alter column updated_at set not null;
alter table host_extension_migrations add constraint host_extension_migrations_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists host_extension_migrations_scope_created_id_idx
    on host_extension_migrations (scope_id, created_at, id);

alter table host_infrastructure_provider_configs add column if not exists scope_id uuid;
alter table host_infrastructure_provider_configs add column if not exists created_by uuid;
update host_infrastructure_provider_configs
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid,
       created_by = coalesce(created_by, updated_by)
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid
    or created_by is null;
alter table host_infrastructure_provider_configs alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table host_infrastructure_provider_configs alter column scope_id set not null;
alter table host_infrastructure_provider_configs add constraint host_infrastructure_provider_configs_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists host_infrastructure_provider_configs_scope_created_id_idx
    on host_infrastructure_provider_configs (scope_id, created_at, id);

alter table js_dependency_registry add column if not exists scope_id uuid;
alter table js_dependency_registry add column if not exists created_by uuid;
alter table js_dependency_registry add column if not exists updated_by uuid;
update js_dependency_registry
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid;
alter table js_dependency_registry alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table js_dependency_registry alter column scope_id set not null;
alter table js_dependency_registry add constraint js_dependency_registry_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists js_dependency_registry_scope_created_id_idx
    on js_dependency_registry (scope_id, created_at, id);

alter table node_contribution_registry add column if not exists scope_id uuid;
alter table node_contribution_registry add column if not exists created_by uuid;
alter table node_contribution_registry add column if not exists updated_by uuid;
update node_contribution_registry
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid;
alter table node_contribution_registry alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table node_contribution_registry alter column scope_id set not null;
alter table node_contribution_registry add constraint node_contribution_registry_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists node_contribution_registry_scope_created_id_idx
    on node_contribution_registry (scope_id, created_at, id);

alter table permission_definitions add column if not exists scope_id uuid;
update permission_definitions
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid;
alter table permission_definitions alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table permission_definitions alter column scope_id set not null;
alter table permission_definitions add constraint permission_definitions_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists permission_definitions_scope_created_id_idx
    on permission_definitions (scope_id, created_at, id);

alter table system_default_upgrade_runs add column if not exists scope_id uuid;
alter table system_default_upgrade_runs add column if not exists created_by uuid;
alter table system_default_upgrade_runs add column if not exists updated_by uuid;
update system_default_upgrade_runs
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid,
       created_by = coalesce(created_by, requested_by),
       updated_by = coalesce(updated_by, requested_by)
 where scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid
    or created_by is null
    or updated_by is null;
alter table system_default_upgrade_runs alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table system_default_upgrade_runs alter column scope_id set not null;
alter table system_default_upgrade_runs add constraint system_default_upgrade_runs_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists system_default_upgrade_runs_scope_created_id_idx
    on system_default_upgrade_runs (scope_id, created_at, id);

alter table system_default_upgrade_items add column if not exists scope_id uuid;
alter table system_default_upgrade_items add column if not exists created_by uuid;
alter table system_default_upgrade_items add column if not exists updated_by uuid;
update system_default_upgrade_items items
   set scope_id = '00000000-0000-0000-0000-000000000000'::uuid,
       created_by = coalesce(items.created_by, runs.requested_by),
       updated_by = coalesce(items.updated_by, runs.requested_by)
  from system_default_upgrade_runs runs
 where items.run_id = runs.id
   and (
       items.scope_id is distinct from '00000000-0000-0000-0000-000000000000'::uuid
       or items.created_by is null
       or items.updated_by is null
   );
alter table system_default_upgrade_items alter column scope_id set default '00000000-0000-0000-0000-000000000000'::uuid;
alter table system_default_upgrade_items alter column scope_id set not null;
alter table system_default_upgrade_items add constraint system_default_upgrade_items_system_scope_id_check
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists system_default_upgrade_items_scope_created_id_idx
    on system_default_upgrade_items (scope_id, created_at, id);
