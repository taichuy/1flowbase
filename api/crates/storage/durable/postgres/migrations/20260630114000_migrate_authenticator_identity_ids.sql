alter table user_auth_identities
  add column if not exists authenticator_id uuid;

do $$
begin
  if exists (
    select 1
    from information_schema.columns
    where table_schema = current_schema()
      and table_name = 'user_auth_identities'
      and column_name = 'authenticator_name'
  ) then
    execute '
      update user_auth_identities identities
      set authenticator_id = authenticators.id
      from authenticators
      where identities.authenticator_id is null
        and identities.authenticator_name = authenticators.name
    ';
  end if;
end $$;

do $$
begin
  if exists (
    select 1
    from user_auth_identities
    where authenticator_id is null
  ) then
    raise exception 'cannot migrate user_auth_identities.authenticator_name to authenticator_id';
  end if;
end $$;

alter table user_auth_identities
  drop constraint if exists user_auth_identities_authenticator_name_fkey;

drop index if exists user_auth_identities_subject_uidx;

alter table user_auth_identities
  alter column authenticator_id set not null;

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conrelid = 'user_auth_identities'::regclass
      and conname = 'user_auth_identities_authenticator_id_fkey'
  ) then
    alter table user_auth_identities
      add constraint user_auth_identities_authenticator_id_fkey
      foreign key (authenticator_id)
      references authenticators(id)
      on delete cascade;
  end if;
end $$;

create unique index if not exists user_auth_identities_subject_uidx
  on user_auth_identities (authenticator_id, subject_type, lower(subject_value));

alter table user_auth_identities
  drop column if exists authenticator_name;

alter table authenticators
  drop constraint if exists authenticators_name_key;

alter table authenticators
  drop column if exists name;
