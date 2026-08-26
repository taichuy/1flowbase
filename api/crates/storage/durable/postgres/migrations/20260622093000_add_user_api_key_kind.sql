alter table api_keys
    drop constraint if exists api_keys_key_kind_check;

alter table api_keys
    add constraint api_keys_key_kind_check
    check (key_kind in ('data_model_api_key', 'application_api_key', 'user_api_key'));

alter table api_keys
    drop constraint if exists api_keys_application_key_application_required_check;

alter table api_keys
    add constraint api_keys_application_key_application_required_check
    check (
        (key_kind = 'application_api_key' and application_id is not null)
        or (key_kind <> 'application_api_key' and application_id is null)
    );

create index if not exists api_keys_user_scope_created_idx
    on api_keys (creator_user_id, tenant_id, scope_id, created_at desc, id desc)
    where key_kind = 'user_api_key';
