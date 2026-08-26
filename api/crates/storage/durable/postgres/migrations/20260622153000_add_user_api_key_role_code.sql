alter table api_keys
    add column if not exists role_code text null;

create index if not exists api_keys_user_role_code_idx
    on api_keys (creator_user_id, tenant_id, scope_id, role_code)
    where key_kind = 'user_api_key';
