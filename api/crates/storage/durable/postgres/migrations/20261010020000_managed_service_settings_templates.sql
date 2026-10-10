-- Keep the native target FK while admitting separately identified managed applications.
alter table plugin_settings_template_applications
    add column managed_service boolean not null default false,
    add column native_installation_id uuid;
update plugin_settings_template_applications set native_installation_id=installation_id;
do $$ declare old_fk text; begin
    select conname into old_fk from pg_constraint
    where conrelid='plugin_settings_template_applications'::regclass
      and confrelid='native_plugin_application_requests'::regclass and contype='f';
    execute format('alter table plugin_settings_template_applications drop constraint %I', old_fk);
end $$;
alter table plugin_settings_template_applications
    add constraint plugin_template_installation_fk foreign key(installation_id) references extension_installations(id) on delete restrict,
    add constraint plugin_template_native_request_fk
      foreign key(scope_id,category,organization,artifact_id,application_generation,native_installation_id)
      references native_plugin_application_requests(scope_id,category,organization,artifact_id,application_generation,installation_id) on delete restrict,
    add constraint plugin_template_application_kind check (
      (managed_service and native_installation_id is null and scope_id='00000000-0000-0000-0000-000000000000'::uuid)
      or (not managed_service and native_installation_id=installation_id and native_installation_id is not null));
create function set_plugin_template_native_installation() returns trigger language plpgsql as $$
begin
    if not new.managed_service then new.native_installation_id := new.installation_id; end if;
    return new;
end $$;
create trigger set_plugin_template_native_installation before insert on plugin_settings_template_applications
for each row execute function set_plugin_template_native_installation();
