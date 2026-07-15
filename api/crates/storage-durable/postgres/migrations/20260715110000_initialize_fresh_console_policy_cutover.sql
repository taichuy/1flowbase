alter table role_console_policy_migration_cutover_state
  drop constraint role_console_policy_migration_cutover_state_shape_check;

alter table role_console_policy_migration_cutover_state
  add constraint role_console_policy_migration_cutover_state_shape_check check (
    (marker = 'legacy'
      and run_id is null
      and catalog_fingerprint is null
      and mapping_fingerprint is null)
    or (marker = 'fenced'
      and run_id is not null
      and catalog_fingerprint = btrim(catalog_fingerprint)
      and catalog_fingerprint <> ''
      and mapping_fingerprint = btrim(mapping_fingerprint)
      and mapping_fingerprint <> '')
    or (marker = 'console_policy'
      and (
        (run_id is null
          and catalog_fingerprint is null
          and mapping_fingerprint is null)
        or (run_id is not null
          and catalog_fingerprint = btrim(catalog_fingerprint)
          and catalog_fingerprint <> ''
          and mapping_fingerprint = btrim(mapping_fingerprint)
          and mapping_fingerprint <> '')
      ))
  );

update role_console_policy_migration_cutover_state
set marker = 'console_policy', updated_at = now()
where singleton
  and marker = 'legacy'
  and not exists (select 1 from roles);
