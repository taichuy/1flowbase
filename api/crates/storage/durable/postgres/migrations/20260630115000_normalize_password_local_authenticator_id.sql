do $$
declare
  fixed_authenticator_id uuid := '00000000-0000-0000-0000-000000000001'::uuid;
begin
  insert into authenticators (
    id,
    auth_type,
    title,
    enabled,
    is_builtin,
    sort_order,
    options,
    created_by,
    created_at,
    updated_by,
    updated_at
  )
  select
    fixed_authenticator_id,
    auth_type,
    title,
    enabled,
    true,
    sort_order,
    options,
    created_by,
    created_at,
    updated_by,
    now()
  from authenticators
  where id <> fixed_authenticator_id
    and auth_type = 'password-local'
    and is_builtin = true
  order by updated_at desc nulls last, created_at desc nulls last, id
  limit 1
  on conflict (id) do update
    set auth_type = excluded.auth_type,
        title = excluded.title,
        enabled = excluded.enabled,
        is_builtin = true,
        sort_order = excluded.sort_order,
        options = excluded.options,
        updated_by = excluded.updated_by,
        updated_at = now();

  update user_auth_identities
  set authenticator_id = fixed_authenticator_id
  where authenticator_id in (
    select id
    from authenticators
    where id <> fixed_authenticator_id
      and auth_type = 'password-local'
      and is_builtin = true
  );

  delete from authenticators
  where id <> fixed_authenticator_id
    and auth_type = 'password-local'
    and is_builtin = true;
end $$;

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
