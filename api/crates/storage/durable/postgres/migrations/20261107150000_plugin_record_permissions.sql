-- Caller workspace policies intentionally have no installation/version foreign key:
-- plugin upgrades and system execution mounts preserve stable publisher/plugin/resource identity.
create table plugin_record_permission_policies (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    publisher_namespace text not null check (length(publisher_namespace) between 1 and 255),
    plugin_code text not null check (length(plugin_code) between 1 and 255),
    resource_code text not null check (length(resource_code) between 1 and 255),
    record_id text not null check (length(record_id) between 1 and 1024),
    mode text not null check (mode in ('inherit', 'restricted')),
    revision bigint not null check (revision > 0),
    created_by uuid,
    updated_by uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (workspace_id, publisher_namespace, plugin_code, resource_code, record_id)
);
-- No grant is distinct from no restricted header: an empty restricted header denies access.
create table plugin_record_permission_grants (
    policy_id uuid not null references plugin_record_permission_policies(id) on delete cascade,
    role_id uuid not null references roles(id) on delete cascade,
    actions text[] not null check (array_position(actions, null) is null and not ('create'=any(actions))),
    primary key (policy_id, role_id)
);
create index plugin_record_permission_grants_role on plugin_record_permission_grants(role_id, policy_id);

-- The repository validates under the same transaction; this guard also prevents alternate
-- durable writers from introducing a cross-workspace role identity.
create function plugin_record_permission_check_role_scope() returns trigger language plpgsql as $$
begin
    if not exists (
        select 1 from plugin_record_permission_policies p join roles r on r.id = new.role_id
        where p.id = new.policy_id and r.scope_kind = 'workspace' and r.workspace_id = p.workspace_id
    ) then
        raise exception 'record permission role workspace mismatch' using errcode = '23514';
    end if;
    return new;
end $$;
create trigger plugin_record_permission_grant_scope before insert or update on plugin_record_permission_grants
for each row execute function plugin_record_permission_check_role_scope();

-- A bounded receipt freshness marker. Worker requests without receipts never read it.
create table plugin_record_permission_scope_revisions (
    workspace_id uuid not null references workspaces(id) on delete cascade,
    publisher_namespace text not null,
    plugin_code text not null,
    revision bigint not null default 1,
    primary key (workspace_id, publisher_namespace, plugin_code)
);
create function plugin_record_permission_bump_scope() returns trigger language plpgsql as $$
declare policy_row record;
begin
    if tg_table_name = 'plugin_record_permission_policies' then
        if tg_op = 'DELETE' then policy_row := old; else policy_row := new; end if;
    else
        select workspace_id,publisher_namespace,plugin_code into policy_row
        from plugin_record_permission_policies where id = case when tg_op = 'DELETE' then old.policy_id else new.policy_id end;
        if not found then return null; end if;
    end if;
    if exists(select 1 from workspaces where id=policy_row.workspace_id) then
        insert into plugin_record_permission_scope_revisions (workspace_id,publisher_namespace,plugin_code,revision)
        values (policy_row.workspace_id,policy_row.publisher_namespace,policy_row.plugin_code,1)
        on conflict (workspace_id,publisher_namespace,plugin_code) do update
        set revision=plugin_record_permission_scope_revisions.revision+1;
    end if;
    return null;
end $$;
create trigger plugin_record_permission_policy_revision after insert or update or delete on plugin_record_permission_policies
for each row execute function plugin_record_permission_bump_scope();
create trigger plugin_record_permission_grant_revision after insert or update or delete on plugin_record_permission_grants
for each row execute function plugin_record_permission_bump_scope();
