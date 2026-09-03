do $$
begin
  if exists (
    select 1
    from authenticators
    where id = '00000000-0000-0000-0000-000000000001'::uuid
      and auth_type <> 'password-local'
  ) then
    raise exception 'reserved password-local connection id is occupied by another authentication type';
  end if;

  if exists (
    select 1
    from user_auth_identities identities
    join authenticators entry on entry.id = identities.authenticator_id
    where entry.auth_type = 'password-local'
    group by identities.subject_type, lower(identities.subject_value)
    having count(distinct identities.user_id) > 1
  ) then
    raise exception 'cannot merge password-local identity namespaces with conflicting users';
  end if;
end $$;

create table authentication_connections (
  id uuid primary key,
  auth_type text not null,
  is_builtin boolean not null default false,
  config jsonb not null default '{}'::jsonb,
  created_by uuid,
  created_at timestamptz not null default now(),
  updated_by uuid,
  updated_at timestamptz not null default now()
);

insert into authentication_connections (
  id, auth_type, is_builtin, config, created_by, created_at, updated_by, updated_at
)
select
  '00000000-0000-0000-0000-000000000001'::uuid,
  'password-local',
  true,
  '{}'::jsonb,
  entry.created_by,
  coalesce(entry.created_at, now()),
  entry.updated_by,
  coalesce(entry.updated_at, now())
from (values (true)) seed(present)
left join authenticators entry
  on entry.id = '00000000-0000-0000-0000-000000000001'::uuid
 and entry.auth_type = 'password-local';

insert into authentication_connections (
  id, auth_type, is_builtin, config, created_by, created_at, updated_by, updated_at
)
select
  id,
  auth_type,
  is_builtin,
  coalesce(options -> 'extension_config', '{}'::jsonb),
  created_by,
  created_at,
  updated_by,
  updated_at
from authenticators
where auth_type <> 'password-local';

alter table user_auth_identities
  drop constraint user_auth_identities_authenticator_id_fkey;

alter table authenticators rename to login_entries;

alter table login_entries add column connection_id uuid;

update login_entries
set connection_id = case
  when auth_type = 'password-local'
    then '00000000-0000-0000-0000-000000000001'::uuid
  else id
end;

alter table login_entries alter column connection_id set not null;

alter table login_entries
  add constraint login_entries_connection_id_fkey
  foreign key (connection_id)
  references authentication_connections(id)
  on delete restrict;

alter table login_entries drop column auth_type;

drop index user_auth_identities_subject_uidx;

alter table user_auth_identities rename column authenticator_id to connection_id;

alter table user_auth_identities
  add column issuer text,
  add column realm text,
  add column profile jsonb not null default '{}'::jsonb,
  add column verified boolean not null default true;

update user_auth_identities identities
set connection_id = '00000000-0000-0000-0000-000000000001'::uuid
from login_entries entry
where entry.id = identities.connection_id
  and entry.connection_id = '00000000-0000-0000-0000-000000000001'::uuid;

with duplicate_local_identities as (
  select id,
         row_number() over (
           partition by user_id, subject_type, lower(subject_value)
           order by id
         ) as duplicate_rank
  from user_auth_identities
  where connection_id = '00000000-0000-0000-0000-000000000001'::uuid
)
delete from user_auth_identities identities
using duplicate_local_identities duplicate
where identities.id = duplicate.id
  and duplicate.duplicate_rank > 1;

alter table user_auth_identities
  add constraint user_auth_identities_connection_id_fkey
  foreign key (connection_id)
  references authentication_connections(id)
  on delete restrict;

create unique index user_auth_identities_local_subject_uidx
  on user_auth_identities (subject_type, lower(subject_value))
  where connection_id = '00000000-0000-0000-0000-000000000001'::uuid;

create unique index user_auth_identities_external_subject_uidx
  on user_auth_identities (
    connection_id,
    coalesce(issuer, ''),
    coalesce(realm, ''),
    subject_type,
    subject_value
  )
  where connection_id <> '00000000-0000-0000-0000-000000000001'::uuid;

do $$
begin
  if exists (
    select 1
    from role_console_operation_policies legacy
    join role_console_operation_policies current
      on current.role_id = legacy.role_id
     and current.operation_id = replace(
       legacy.operation_id,
       'auth_center.authenticators.',
       'auth_center.login_entries.'
     )
    where legacy.operation_id like 'auth_center.authenticators.%'
  ) then
    raise exception 'cannot rename auth center operation policies with duplicate login-entry policies';
  end if;
end $$;

update role_console_operation_policies
set operation_id = replace(
      operation_id,
      'auth_center.authenticators.',
      'auth_center.login_entries.'
    ),
    updated_at = now()
where operation_id like 'auth_center.authenticators.%';
