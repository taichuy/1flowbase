alter table tenants
    add column status text not null default 'active' check (status in ('active', 'suspended')),
    add column cell_id text not null default 'default' check (btrim(cell_id) <> ''),
    add constraint tenants_root_always_active_chk check (not is_root or status = 'active');

create index tenants_status_cell_id on tenants (status, cell_id);

alter table workspace_memberships
    add column status text not null default 'active' check (status in ('active', 'disabled'));

create index workspace_memberships_status on workspace_memberships (status);
create index workspace_memberships_workspace_status on workspace_memberships (workspace_id, status);

alter table users
    add column managed_by_tenant_id uuid references tenants(id) on delete set null;

create index users_managed_by_tenant_id on users (managed_by_tenant_id);
