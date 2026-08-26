alter table plugin_assignments
    add column provider_code text;

update plugin_assignments pa
set provider_code = pi.provider_code
from plugin_installations pi
where pi.id = pa.installation_id;

with ranked as (
    select
        pa.id,
        row_number() over (
            partition by pa.workspace_id, pa.provider_code
            order by pa.created_at desc, pa.id desc
        ) as rn
    from plugin_assignments pa
)
delete from plugin_assignments pa
using ranked
where ranked.id = pa.id
  and ranked.rn > 1;

alter table plugin_assignments
    alter column provider_code set not null;

alter table plugin_assignments
    drop constraint plugin_assignments_installation_id_workspace_id_key;

alter table plugin_assignments
    add constraint plugin_assignments_workspace_provider_code_key
        unique (workspace_id, provider_code);

drop index if exists plugin_assignments_workspace_idx;
create index plugin_assignments_workspace_provider_idx
    on plugin_assignments (workspace_id, provider_code, created_at desc, id desc);

alter table plugin_tasks
    drop constraint plugin_tasks_task_kind_check;

alter table plugin_tasks
    add constraint plugin_tasks_task_kind_check check (
        task_kind in (
            'install',
            'upgrade',
            'uninstall',
            'enable',
            'disable',
            'assign',
            'unassign',
            'switch_version'
        )
    );
