update authenticators
set options = coalesce(options, '{}'::jsonb)
  || jsonb_strip_nulls(jsonb_build_object(
    'description',
      case
        when coalesce(options, '{}'::jsonb) ? 'description' then null
        else to_jsonb('Local password authentication'::text)
      end,
    'config_form_schema',
      case
        when coalesce(options, '{}'::jsonb) ? 'config_form_schema' then null
        else '[
          {
            "key": "name",
            "label": "Authenticator identifier",
            "type": "string",
            "read_only": true,
            "required": true,
            "pattern": "^[A-Za-z0-9_]+$"
          },
          {
            "key": "title",
            "label": "Authenticator title",
            "type": "string",
            "required": true
          },
          {
            "key": "description",
            "label": "Description",
            "type": "string",
            "control": "textarea",
            "read_only": false,
            "required": false
          },
          {
            "key": "enabled",
            "label": "Enabled",
            "type": "boolean",
            "control": "switch"
          }
        ]'::jsonb
      end,
    'extension_config',
      case
        when coalesce(options, '{}'::jsonb) ? 'extension_config' then null
        else '{}'::jsonb
      end
  )),
  updated_at = now()
where name = 'password-local';
