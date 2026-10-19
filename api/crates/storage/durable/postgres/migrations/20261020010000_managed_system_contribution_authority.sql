-- System authentication contributions have no managed_service declaration.
-- Match domain::managed_installation_scope without widening mixed/invalid targets.
create or replace function validate_managed_authority_scope() returns trigger language plpgsql as $$
begin
    if new.workspace_id <> '00000000-0000-0000-0000-000000000000'::uuid then
        perform 1 from workspaces where id = new.workspace_id for key share;
        if not found then raise foreign_key_violation using message = 'managed authority workspace does not exist'; end if;
    elsif not exists (
        select 1 from extension_installations where id = new.installation_id
            and contract_version = '1flowbase.extension-bus/v1'
            and coalesce(metadata_json #>> '{managed_service,scope}' = 'system'
                or metadata_json -> 'binding_targets' = '["system"]'::jsonb, false)
    ) then
        raise check_violation using message = 'managed system service declaration required';
    end if;
    return new;
end $$;
