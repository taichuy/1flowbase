drop table if exists api_key_data_model_permissions;

delete from api_keys
where key_kind = 'data_model_api_key'
   or token_prefix like 'dmk_%';

alter table api_keys
    alter column key_kind drop default;

alter table api_keys
    drop constraint if exists api_keys_key_kind_check;

alter table api_keys
    add constraint api_keys_key_kind_check
    check (key_kind in ('application_api_key', 'user_api_key'));
