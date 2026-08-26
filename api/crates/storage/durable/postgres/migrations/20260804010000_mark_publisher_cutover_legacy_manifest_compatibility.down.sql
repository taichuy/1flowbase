-- Roll back only the compatibility marker owned by this migration.
update extension_installations
set receipt = receipt - 'legacy_manifest_compatibility'
where category = 'runtime-extensions'
  and plugin_id is not null
  and receipt ->> 'migration' = 'unified_extension_installation_lifecycle'
  and nullif(receipt ->> 'legacy_plugin_installation_id', '') is not null
  and receipt ->> 'legacy_manifest_compatibility' = 'missing_publisher_namespace_v1';
