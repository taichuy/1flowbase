alter table tenants
    add column status text not null default 'active' check (status in ('active', 'suspended')),
    add column cell_id text not null default 'default' check (btrim(cell_id) <> ''),
    add constraint tenants_root_always_active_chk check (not is_root or status = 'active');

alter table workspace_memberships
    add column status text not null default 'active' check (status in ('active', 'disabled'));

alter table users
    add column managed_by_tenant_id uuid references tenants(id) on delete set null;
