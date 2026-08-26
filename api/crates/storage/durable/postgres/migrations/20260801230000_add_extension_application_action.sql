alter table extension_installations
    add column application_action text not null default 'none';

update extension_installations
set application_action = case category
    when 'agent-flow' then 'import_agent_flow'
    when 'mcp' then 'import_mcp'
    when 'i18n' then 'activate_i18n'
    when 'runtime-extensions' then 'configure_model_provider'
    else 'none'
end;

alter table extension_installations
    add constraint extension_installations_application_action_check check (
        application_action in (
            'none', 'import_agent_flow', 'import_mcp',
            'activate_i18n', 'configure_model_provider'
        )
    );
