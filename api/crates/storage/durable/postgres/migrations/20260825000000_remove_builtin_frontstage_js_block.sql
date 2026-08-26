-- The built-in Native React example block and its Tailwind authoring toolchain were removed.
-- Delete only the exact system-owned installation previously created by Boot Core.
-- Installation-owned catalog, assignment, artifact, projection, and retained asset rows cascade.
delete from extension_installations
where category = 'capability-plugins'
  and organization = '1flowbase'
  and artifact_id = '1flowbase'
  and artifact_version = '1.0.0'
  and plugin_id = '1flowbase@1.0.0'
  and source_kind = 'builtin';
