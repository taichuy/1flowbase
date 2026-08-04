-- AC-001/AC-002: identify only runtime rows created by the unified lifecycle migration.
-- The original installation, assignment, artifact-instance identities and artifact bytes remain
-- unchanged; consumers must still validate the durable plugin identity and raw fingerprint.
update extension_installations
set receipt = receipt || jsonb_build_object(
        'legacy_manifest_compatibility', 'missing_publisher_namespace_v1'
    )
where category = 'runtime-extensions'
  and plugin_id is not null
  and receipt ->> 'migration' = 'unified_extension_installation_lifecycle'
  and nullif(receipt ->> 'legacy_plugin_installation_id', '') is not null
  and coalesce(receipt ->> 'legacy_manifest_compatibility', '') <> 'missing_publisher_namespace_v1';
