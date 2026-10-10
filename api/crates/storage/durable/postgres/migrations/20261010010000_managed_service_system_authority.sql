-- System services use the canonical system scope, never a synthetic business workspace.
alter table plugin_contribution_authorization_revisions
    drop constraint plugin_contribution_authorization_revisions_workspace_id_fkey;
alter table plugin_contribution_authorizations
    drop constraint plugin_contribution_authorizations_resource_scope_check;
alter table plugin_contribution_authorizations add constraint plugin_contribution_authorizations_resource_scope_check
    check (coalesce((resource_scope in ('{"kind":"workspace"}'::jsonb, '{"kind":"system"}'::jsonb)
        or (resource_scope->>'kind' = 'owned_collection'
            and resource_scope->>'collection_code' ~ '^[a-z][a-z0-9_]*$'
            and resource_scope - 'kind' - 'collection_code' = '{}'::jsonb)), false));

create function validate_managed_authority_scope() returns trigger language plpgsql as $$
begin
    if new.workspace_id <> '00000000-0000-0000-0000-000000000000'::uuid then
        perform 1 from workspaces where id = new.workspace_id for key share;
        if not found then raise foreign_key_violation using message = 'managed authority workspace does not exist'; end if;
    elsif not exists (select 1 from extension_installations where id = new.installation_id
        and contract_version = '1flowbase.extension-bus/v1' and metadata_json #>> '{managed_service,scope}' = 'system') then
        raise check_violation using message = 'managed system service declaration required';
    end if;
    return new;
end $$;
create trigger managed_authority_scope before insert or update of workspace_id, installation_id
    on plugin_contribution_authorization_revisions for each row execute function validate_managed_authority_scope();
create function cleanup_managed_authority_workspace() returns trigger language plpgsql as $$
begin
    delete from plugin_contribution_authorization_revisions where workspace_id = old.id;
    return old;
end $$;
create trigger managed_authority_workspace_deleted after delete on workspaces
    for each row execute function cleanup_managed_authority_workspace();
