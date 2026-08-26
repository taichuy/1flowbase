do $$
begin
  if exists (
    select 1
    from i18n_catalog_release_translations
    group by release_id, msgid, locale
    having count(distinct translation) > 1
  ) then
    raise exception 'i18n catalog migration conflict: release key and locale have different translations across modules';
  end if;

  if exists (
    select 1
    from i18n_catalog_release_files
    group by release_id, locale, path
    having count(distinct sha256) > 1
  ) then
    raise exception 'i18n catalog migration conflict: release file identity has different digests across modules';
  end if;

  if exists (
    select 1
    from workspace_i18n_catalog_overrides
    group by workspace_id, msgid, locale
    having count(distinct translation) > 1
  ) then
    raise exception 'i18n catalog migration conflict: workspace override key and locale have different translations across modules';
  end if;

  if exists (
    select 1
    from workspace_i18n_catalog_custom_translations
    group by workspace_id, msgid, locale
    having count(distinct translation) > 1
  ) then
    raise exception 'i18n catalog migration conflict: custom key and locale have different translations across modules';
  end if;

  if exists (
    select 1
    from workspace_i18n_catalog_obsolete_messages
    group by workspace_id, msgid
    having count(distinct obsolete_since_release_id) > 1
  ) then
    raise exception 'i18n catalog migration conflict: obsolete key has different release markers across modules';
  end if;
end;
$$;

drop trigger i18n_catalog_releases_immutable on i18n_catalog_releases;
alter table i18n_catalog_releases
  drop constraint i18n_catalog_releases_schema_version_check;
update i18n_catalog_releases
set schema_version = '1flowbase.i18n-catalog-seed/v2';
alter table i18n_catalog_releases
  add constraint i18n_catalog_releases_schema_version_check
    check (schema_version = '1flowbase.i18n-catalog-seed/v2'),
  drop column modules;
create trigger i18n_catalog_releases_immutable
before update on i18n_catalog_releases
for each row execute function reject_i18n_catalog_release_update();

alter table i18n_catalog_release_files rename to i18n_catalog_release_files_legacy;
alter table i18n_catalog_release_messages rename to i18n_catalog_release_messages_legacy;
alter table i18n_catalog_release_translations rename to i18n_catalog_release_translations_legacy;
alter table workspace_i18n_catalog_overrides rename to workspace_i18n_catalog_overrides_legacy;
alter table workspace_i18n_catalog_custom_translations
  rename to workspace_i18n_catalog_custom_translations_legacy;
alter table workspace_i18n_catalog_obsolete_messages
  rename to workspace_i18n_catalog_obsolete_messages_legacy;

create table i18n_catalog_release_files (
  release_id uuid not null references i18n_catalog_releases(id) on delete cascade,
  locale text not null check (locale ~ '^[a-z]{2,3}(_[A-Z][A-Za-z]{1,7})?$'),
  path text not null check (btrim(path) <> ''),
  sha256 text not null check (sha256 ~ '^sha256:[0-9a-f]{64}$'),
  primary key (release_id, locale, path)
);

create table i18n_catalog_release_messages (
  release_id uuid not null references i18n_catalog_releases(id) on delete cascade,
  key text not null check (btrim(key) <> ''),
  primary key (release_id, key)
);

create table i18n_catalog_release_translations (
  release_id uuid not null,
  key text not null,
  locale text not null check (locale ~ '^[a-z]{2,3}(_[A-Z][A-Za-z]{1,7})?$'),
  translation text not null,
  primary key (release_id, key, locale),
  foreign key (release_id, key)
    references i18n_catalog_release_messages(release_id, key)
    on delete cascade
);

create table workspace_i18n_catalog_overrides (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  key text not null check (btrim(key) <> ''),
  locale text not null check (locale ~ '^[a-z]{2,3}(_[A-Z][A-Za-z]{1,7})?$'),
  translation text not null,
  updated_at timestamptz not null default now(),
  primary key (workspace_id, key, locale)
);

create table workspace_i18n_catalog_custom_translations (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  key text not null check (btrim(key) <> ''),
  locale text not null check (locale ~ '^[a-z]{2,3}(_[A-Z][A-Za-z]{1,7})?$'),
  translation text not null,
  updated_at timestamptz not null default now(),
  primary key (workspace_id, key, locale)
);

create table workspace_i18n_catalog_obsolete_messages (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  key text not null check (btrim(key) <> ''),
  obsolete_since_release_id uuid not null,
  marked_at timestamptz not null default now(),
  primary key (workspace_id, key),
  foreign key (workspace_id, obsolete_since_release_id)
    references i18n_catalog_releases(workspace_id, id)
);

insert into i18n_catalog_release_files (release_id, locale, path, sha256)
select release_id, locale, path, min(sha256)
from i18n_catalog_release_files_legacy
group by release_id, locale, path;

insert into i18n_catalog_release_messages (release_id, key)
select distinct release_id, msgid
from i18n_catalog_release_messages_legacy;

insert into i18n_catalog_release_translations (release_id, key, locale, translation)
select release_id, key, locale, min(translation)
from (
  select release_id, msgid as key, locale, translation
  from i18n_catalog_release_translations_legacy
  union all
  select release_id, msgid as key, 'en_US'::text as locale, msgid as translation
  from i18n_catalog_release_messages_legacy
) translations
group by release_id, key, locale;

insert into workspace_i18n_catalog_overrides (
  workspace_id, key, locale, translation, updated_at
)
select workspace_id, msgid, locale, min(translation), max(updated_at)
from workspace_i18n_catalog_overrides_legacy
group by workspace_id, msgid, locale;

insert into workspace_i18n_catalog_custom_translations (
  workspace_id, key, locale, translation, updated_at
)
select workspace_id, msgid, locale, min(translation), max(updated_at)
from workspace_i18n_catalog_custom_translations_legacy
group by workspace_id, msgid, locale;

insert into workspace_i18n_catalog_obsolete_messages (
  workspace_id, key, obsolete_since_release_id, marked_at
)
select workspace_id, msgid, (array_agg(obsolete_since_release_id))[1], max(marked_at)
from workspace_i18n_catalog_obsolete_messages_legacy
group by workspace_id, msgid;

drop table i18n_catalog_release_translations_legacy;
drop table i18n_catalog_release_messages_legacy;
drop table i18n_catalog_release_files_legacy;
drop table workspace_i18n_catalog_overrides_legacy;
drop table workspace_i18n_catalog_custom_translations_legacy;
drop table workspace_i18n_catalog_obsolete_messages_legacy;

create index i18n_catalog_release_translations_locale_idx
  on i18n_catalog_release_translations (release_id, locale);
