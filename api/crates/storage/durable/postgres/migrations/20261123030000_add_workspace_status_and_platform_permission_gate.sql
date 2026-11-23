alter table console_permission_catalog_groups
  add column root_workspace_only boolean not null default false;

alter table workspaces
  add column status text not null default 'active' check (status in ('active', 'suspended'));

create index workspaces_status on workspaces (status);
