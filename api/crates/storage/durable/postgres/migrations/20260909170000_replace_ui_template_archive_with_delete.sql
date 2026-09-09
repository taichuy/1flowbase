-- All existing templates, including archived ones, remain available for explicit deletion.
-- Preserve the name uniqueness contract without silently renaming or discarding user content.
do $$
begin
    if exists (
        select 1 from ui_code_templates
        group by scope_id, provider_code, contribution_code, lower(name)
        having count(*) > 1
    ) then
        raise exception 'cannot restore archived UI templates with duplicate names; resolve the names before retrying migration';
    end if;
end $$;

drop index ui_code_templates_active_name_idx;
alter table ui_code_templates drop column archived_at;
create unique index ui_code_templates_name_idx
    on ui_code_templates (scope_id, provider_code, contribution_code, lower(name));

-- A custom archive grant is not authorization to permanently delete content.
-- Full groups retain their full profile; custom groups must grant the new delete operation.
delete from role_console_operation_policies
where operation_id = 'ui_management.templates.archive';
