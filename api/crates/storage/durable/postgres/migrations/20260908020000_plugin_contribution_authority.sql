-- No grant backfill: old installations acquire no rights from this migration.
create table plugin_contribution_authorization_revisions (
    installation_id uuid not null references extension_installations(id) on delete cascade,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    revision bigint not null default 0 check (revision >= 0),
    updated_at timestamptz not null default now(),
    primary key (installation_id, workspace_id)
);

create table plugin_contribution_authorizations (
    id uuid primary key,
    installation_id uuid not null,
    workspace_id uuid not null,
    contribution_id text not null check (length(trim(contribution_id)) > 0),
    point_id text not null check (length(trim(point_id)) > 0),
    permission text not null check (length(trim(permission)) > 0),
    resource_scope jsonb not null check (coalesce((
        resource_scope = '{"kind":"workspace"}'::jsonb
        or (resource_scope->>'kind' = 'owned_collection'
            and resource_scope->>'collection_code' ~ '^[a-z][a-z0-9_]*$'
            and resource_scope - 'kind' - 'collection_code' = '{}'::jsonb)
    ), false)),
    permission_contract_id text not null check (length(trim(permission_contract_id)) > 0),
    permission_contract_version text not null check (length(trim(permission_contract_version)) > 0),
    status text not null check (status in ('active', 'revoked')),
    granted_by uuid not null references users(id),
    revoked_by uuid references users(id),
    granted_at timestamptz not null default now(),
    revoked_at timestamptz,
    revision bigint not null check (revision > 0),
    foreign key (installation_id, workspace_id)
        references plugin_contribution_authorization_revisions(installation_id, workspace_id) on delete cascade,
    unique (installation_id, workspace_id, contribution_id, point_id, permission, resource_scope, permission_contract_id, permission_contract_version),
    check ((status = 'active' and revoked_by is null and revoked_at is null)
        or (status = 'revoked' and revoked_by is not null and revoked_at is not null))
);
create index plugin_contribution_authorizations_scope_idx
    on plugin_contribution_authorizations(workspace_id, installation_id, contribution_id, status);
create index plugin_contribution_authorizations_point_idx
    on plugin_contribution_authorizations(point_id, permission, status);
