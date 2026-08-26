alter table plugin_installations add column if not exists scope_id uuid
    default '00000000-0000-0000-0000-000000000000'::uuid;
alter table plugin_installations add column if not exists updated_by uuid;
update plugin_installations
   set scope_id = coalesce(scope_id, '00000000-0000-0000-0000-000000000000'::uuid),
       updated_by = coalesce(updated_by, created_by);
alter table plugin_installations alter column scope_id set not null;
alter table plugin_installations
    add constraint plugin_installations_system_scope_chk
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists plugin_installations_scope_created_id_idx
    on plugin_installations (scope_id, created_at, id);

alter table plugin_worker_leases add column if not exists scope_id uuid
    default '00000000-0000-0000-0000-000000000000'::uuid;
alter table plugin_worker_leases add column if not exists created_by uuid;
alter table plugin_worker_leases add column if not exists updated_by uuid;
update plugin_worker_leases
   set scope_id = coalesce(scope_id, '00000000-0000-0000-0000-000000000000'::uuid);
alter table plugin_worker_leases alter column scope_id set not null;
alter table plugin_worker_leases
    add constraint plugin_worker_leases_system_scope_chk
    check (scope_id = '00000000-0000-0000-0000-000000000000'::uuid);
create index if not exists plugin_worker_leases_scope_created_id_idx
    on plugin_worker_leases (scope_id, created_at, id);

alter table plugin_tasks add column if not exists scope_kind text;
alter table plugin_tasks add column if not exists scope_id uuid;
alter table plugin_tasks add column if not exists updated_by uuid;

do $$
begin
    if exists (
        select 1
          from plugin_tasks
         where task_kind in ('assign', 'unassign')
           and workspace_id is null
    ) then
        raise exception 'plugin_tasks contains workspace tasks without workspace_id';
    end if;
end $$;

update plugin_tasks
   set scope_kind = case
           when task_kind in ('assign', 'unassign') then 'workspace'
           else 'system'
       end,
       scope_id = case
           when task_kind in ('assign', 'unassign') then workspace_id
           else '00000000-0000-0000-0000-000000000000'::uuid
       end,
       updated_by = coalesce(updated_by, created_by)
 where scope_id is null
    or scope_kind is null
    or updated_by is null;
alter table plugin_tasks alter column scope_kind set not null;
alter table plugin_tasks alter column scope_id set not null;
alter table plugin_tasks
    add constraint plugin_tasks_scope_kind_check
    check (scope_kind in ('system', 'workspace'));
alter table plugin_tasks
    add constraint plugin_tasks_scope_owner_check
    check (
        (
            task_kind in ('assign', 'unassign')
            and scope_kind = 'workspace'
            and workspace_id is not null
            and scope_id = workspace_id
        )
        or (
            task_kind in ('install', 'upgrade', 'uninstall', 'enable', 'disable', 'switch_version')
            and scope_kind = 'system'
            and scope_id = '00000000-0000-0000-0000-000000000000'::uuid
        )
    );
create index if not exists plugin_tasks_scope_created_id_idx
    on plugin_tasks (scope_id, created_at, id);
