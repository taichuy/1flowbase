alter table authenticators
  add column if not exists public_ui_block text not null default '';

update authenticators
set options = jsonb_set(
  case when jsonb_typeof(options) = 'object' then options else '{}'::jsonb end,
  '{extension_config}',
  case
    when jsonb_typeof(options -> 'extension_config') = 'object'
      then (options -> 'extension_config') || '{"self_registration_enabled": false}'::jsonb
    else '{"self_registration_enabled": false}'::jsonb
  end,
  true
)
where auth_type = 'password-local'
  and not coalesce(options -> 'extension_config', '{}'::jsonb) ? 'self_registration_enabled';

update authenticators
set options = jsonb_set(
  options,
  '{config_form_schema}',
  coalesce(options -> 'config_form_schema', '[]'::jsonb)
    || '[{"key":"self_registration_enabled","label":"Allow self registration","type":"boolean","control":"switch"}]'::jsonb,
  true
)
where auth_type = 'password-local'
  and jsonb_typeof(coalesce(options -> 'config_form_schema', '[]'::jsonb)) = 'array'
  and not exists (
    select 1
    from jsonb_array_elements(coalesce(options -> 'config_form_schema', '[]'::jsonb)) field
    where field ->> 'key' = 'self_registration_enabled'
  );

update authenticators
set options = jsonb_set(
  options,
  '{config_form_schema}',
  coalesce(options -> 'config_form_schema', '[]'::jsonb)
    || '[{"key":"public_ui_block","label":"Public authentication block","type":"string","control":"textarea","required":true}]'::jsonb,
  true
)
where auth_type = 'password-local'
  and jsonb_typeof(coalesce(options -> 'config_form_schema', '[]'::jsonb)) = 'array'
  and not exists (
    select 1
    from jsonb_array_elements(coalesce(options -> 'config_form_schema', '[]'::jsonb)) field
    where field ->> 'key' = 'public_ui_block'
  );
