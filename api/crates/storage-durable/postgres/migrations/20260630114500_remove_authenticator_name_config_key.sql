update authenticators
set options = jsonb_set(
    coalesce(options, '{}'::jsonb),
    '{config_form_schema}',
    coalesce(
      (
        select jsonb_agg(item.value order by item.ordinality)
        from jsonb_array_elements(
          coalesce(options -> 'config_form_schema', '[]'::jsonb)
        ) with ordinality as item(value, ordinality)
        where item.value ->> 'key' <> 'name'
      ),
      '[]'::jsonb
    ),
    true
  ),
  updated_at = now()
where id = '00000000-0000-0000-0000-000000000001'
  and jsonb_typeof(coalesce(options -> 'config_form_schema', '[]'::jsonb)) = 'array'
  and exists (
    select 1
    from jsonb_array_elements(options -> 'config_form_schema') item
    where item ->> 'key' = 'name'
  );
