create temporary table builtin_system_tables_for_contract (
    code text primary key,
    physical_table_name text not null
) on commit drop;

insert into builtin_system_tables_for_contract (code, physical_table_name)
values
    ('attachments', 'attachments'),
    ('users', 'users'),
    ('roles', 'roles');

update model_definitions definitions
set physical_table_name = tables.physical_table_name,
    owner_kind = 'core',
    owner_id = null,
    is_protected = true,
    availability_status = 'available',
    status = 'published',
    updated_at = now()
from builtin_system_tables_for_contract tables
where definitions.scope_kind = 'system'
  and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
  and definitions.data_source_instance_id is null
  and definitions.source_kind = 'main_source'
  and definitions.code = tables.code;

create temporary table builtin_system_table_field_contracts (
    model_code text not null,
    code text not null,
    title text not null,
    physical_column_name text not null,
    field_kind text not null,
    is_system boolean not null,
    is_writable boolean not null,
    is_required boolean not null,
    is_unique boolean not null,
    sort_order integer not null,
    primary key (model_code, code)
) on commit drop;

insert into builtin_system_table_field_contracts (
    model_code, code, title, physical_column_name, field_kind,
    is_system, is_writable, is_required, is_unique, sort_order
)
values
    ('attachments', 'id', 'id', 'id', 'string', true, false, true, true, 0),
    ('attachments', 'scope_id', 'scope_id', 'scope_id', 'many_to_one', true, false, true, false, 1),
    ('attachments', 'created_by', 'created_by', 'created_by', 'string', true, false, true, false, 2),
    ('attachments', 'updated_by', 'updated_by', 'updated_by', 'string', true, false, true, false, 3),
    ('attachments', 'created_at', 'created_at', 'created_at', 'datetime', true, false, true, false, 4),
    ('attachments', 'updated_at', 'updated_at', 'updated_at', 'datetime', true, false, true, false, 5),
    ('attachments', 'title', '标题', 'title', 'string', false, true, false, false, 6),
    ('attachments', 'filename', '文件名', 'filename', 'string', false, true, true, false, 7),
    ('attachments', 'extname', '扩展名', 'extname', 'string', false, true, false, false, 8),
    ('attachments', 'size', '大小', 'size', 'number', false, true, true, false, 9),
    ('attachments', 'mimetype', 'MIME 类型', 'mimetype', 'string', false, true, true, false, 10),
    ('attachments', 'path', '存储路径', 'path', 'string', false, true, true, false, 11),
    ('attachments', 'meta', '元数据', 'meta', 'json', false, true, true, false, 12),
    ('attachments', 'url', '缓存地址', 'url', 'string', false, true, false, false, 13),
    ('attachments', 'storage_id', '存储器 ID', 'storage_id', 'string', false, true, true, false, 14),

    ('users', 'id', 'id', 'id', 'string', true, false, true, true, 0),
    ('users', 'created_by', 'created_by', 'created_by', 'string', true, false, true, false, 1),
    ('users', 'updated_by', 'updated_by', 'updated_by', 'string', true, false, true, false, 2),
    ('users', 'created_at', 'created_at', 'created_at', 'datetime', true, false, true, false, 3),
    ('users', 'updated_at', 'updated_at', 'updated_at', 'datetime', true, false, true, false, 4),
    ('users', 'account', '账号', 'account', 'string', true, false, true, true, 5),
    ('users', 'email', '邮箱', 'email', 'string', true, false, true, true, 6),
    ('users', 'phone', '手机号', 'phone', 'string', true, false, false, true, 7),
    ('users', 'name', '姓名', 'name', 'string', true, false, true, false, 8),
    ('users', 'nickname', '昵称', 'nickname', 'string', true, false, true, false, 9),
    ('users', 'avatar_url', '头像', 'avatar_url', 'string', true, false, false, false, 10),
    ('users', 'introduction', '简介', 'introduction', 'text', true, false, true, false, 11),
    ('users', 'preferred_locale', '偏好语言', 'preferred_locale', 'string', true, false, false, false, 12),
    ('users', 'meta', '元数据', 'meta', 'json', true, false, true, false, 13),
    ('users', 'default_display_role', '默认展示角色', 'default_display_role', 'string', true, false, false, false, 14),
    ('users', 'email_login_enabled', '邮箱登录', 'email_login_enabled', 'boolean', true, false, true, false, 15),
    ('users', 'phone_login_enabled', '手机登录', 'phone_login_enabled', 'boolean', true, false, true, false, 16),
    ('users', 'status', '状态', 'status', 'string', true, false, true, false, 17),

    ('roles', 'id', 'id', 'id', 'string', true, false, true, true, 0),
    ('roles', 'created_by', 'created_by', 'created_by', 'string', true, false, true, false, 1),
    ('roles', 'updated_by', 'updated_by', 'updated_by', 'string', true, false, true, false, 2),
    ('roles', 'created_at', 'created_at', 'created_at', 'datetime', true, false, true, false, 3),
    ('roles', 'updated_at', 'updated_at', 'updated_at', 'datetime', true, false, true, false, 4),
    ('roles', 'scope_id', '作用域 ID', 'scope_id', 'many_to_one', true, false, true, false, 5),
    ('roles', 'scope_kind', '作用域', 'scope_kind', 'string', true, false, true, false, 6),
    ('roles', 'workspace_id', '工作区 ID', 'workspace_id', 'many_to_one', true, false, false, false, 7),
    ('roles', 'code', '角色标识', 'code', 'string', true, false, true, true, 8),
    ('roles', 'name', '角色名称', 'name', 'string', true, false, true, false, 9),
    ('roles', 'introduction', '简介', 'introduction', 'text', true, false, true, false, 10),
    ('roles', 'is_builtin', '内置角色', 'is_builtin', 'boolean', true, false, true, false, 11),
    ('roles', 'is_editable', '可编辑', 'is_editable', 'boolean', true, false, true, false, 12),
    ('roles', 'auto_grant_new_permissions', '自动授予新权限', 'auto_grant_new_permissions', 'boolean', true, false, true, false, 13),
    ('roles', 'is_default_member_role', '默认成员角色', 'is_default_member_role', 'boolean', true, false, true, false, 14),
    ('roles', 'system_kind', '系统角色类型', 'system_kind', 'string', true, false, false, false, 15);

insert into model_fields (
    id,
    data_model_id,
    scope_id,
    code,
    title,
    physical_column_name,
    external_field_key,
    field_kind,
    is_system,
    is_writable,
    is_required,
    is_unique,
    default_value,
    display_interface,
    display_options,
    relation_target_model_id,
    relation_options,
    sort_order,
    availability_status
)
select
    (
        substr(md5(definitions.id::text || ':' || fields.code), 1, 8) || '-' ||
        substr(md5(definitions.id::text || ':' || fields.code), 9, 4) || '-' ||
        substr(md5(definitions.id::text || ':' || fields.code), 13, 4) || '-' ||
        substr(md5(definitions.id::text || ':' || fields.code), 17, 4) || '-' ||
        substr(md5(definitions.id::text || ':' || fields.code), 21, 12)
    )::uuid,
    definitions.id,
    definitions.scope_id,
    fields.code,
    fields.title,
    fields.physical_column_name,
    null,
    fields.field_kind,
    fields.is_system,
    fields.is_writable,
    fields.is_required,
    fields.is_unique,
    null,
    null,
    '{}'::jsonb,
    null,
    '{}'::jsonb,
    fields.sort_order,
    'available'
from model_definitions definitions
join builtin_system_table_field_contracts fields on fields.model_code = definitions.code
where definitions.scope_kind = 'system'
  and definitions.scope_id = '00000000-0000-0000-0000-000000000000'::uuid
  and definitions.data_source_instance_id is null
  and definitions.source_kind = 'main_source'
on conflict (data_model_id, code)
do update set
    scope_id = excluded.scope_id,
    physical_column_name = excluded.physical_column_name,
    external_field_key = null,
    field_kind = excluded.field_kind,
    is_system = excluded.is_system,
    is_writable = excluded.is_writable,
    is_required = excluded.is_required,
    is_unique = excluded.is_unique,
    default_value = null,
    relation_target_model_id = null,
    sort_order = excluded.sort_order,
    availability_status = 'available',
    updated_at = now();
