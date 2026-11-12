-- Keep command settings and custom event targets in one canonical channel collection.
-- Ambiguous mixed generations must be resolved explicitly rather than overwrite commands.
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM communication_bindings
    WHERE config_json ? 'commands'
      AND (config_json ? 'command_settings' OR config_json ? 'event_commands')
  ) THEN
    RAISE EXCEPTION 'communication binding contains mixed command configuration generations';
  END IF;
END $$;

UPDATE communication_bindings AS binding
SET config_json = (binding.config_json - 'command_settings' - 'event_commands')
  || jsonb_build_object('commands',
    COALESCE((
      SELECT jsonb_agg(entry.value || '{"kind":"registered"}'::jsonb ORDER BY entry.ordinality)
      FROM jsonb_array_elements(COALESCE(binding.config_json->'command_settings', '[]'::jsonb))
        WITH ORDINALITY AS entry(value, ordinality)
    ), '[]'::jsonb)
    || COALESCE((
      SELECT jsonb_agg(entry.value || '{"kind":"event"}'::jsonb ORDER BY entry.ordinality)
      FROM jsonb_array_elements(COALESCE(binding.config_json->'event_commands', '[]'::jsonb))
        WITH ORDINALITY AS entry(value, ordinality)
    ), '[]'::jsonb)),
    revision = binding.revision + 1
WHERE NOT (binding.config_json ? 'commands');
