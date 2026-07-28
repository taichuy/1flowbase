create table i18n_catalog_releases (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  schema_version text not null
    check (schema_version = '1flowbase.i18n-catalog-seed/v1'),
  catalog_version text not null check (btrim(catalog_version) <> ''),
  source_locale text not null check (source_locale = 'en_US'),
  locales text[] not null check (cardinality(locales) > 0 and locales @> array['en_US']::text[]),
  modules text[] not null check (cardinality(modules) > 0),
  generated_at timestamptz not null,
  semantic_sha256 text not null
    check (semantic_sha256 ~ '^sha256:[0-9a-f]{64}$'),
  imported_at timestamptz not null default now(),
  unique (workspace_id, catalog_version),
  unique (workspace_id, id)
);

create function reject_i18n_catalog_release_update()
returns trigger
language plpgsql
as $$
begin
  raise exception 'i18n catalog releases are immutable';
end;
$$;

create trigger i18n_catalog_releases_immutable
before update on i18n_catalog_releases
for each row execute function reject_i18n_catalog_release_update();

create table i18n_catalog_release_files (
  release_id uuid not null references i18n_catalog_releases(id) on delete cascade,
  module text not null check (module ~ '^@[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+){2,}$'),
  locale text not null check (locale ~ '^[a-z]{2}_[A-Z]{2}$'),
  path text not null check (btrim(path) <> ''),
  sha256 text not null check (sha256 ~ '^sha256:[0-9a-f]{64}$'),
  primary key (release_id, module, locale, path)
);

create table i18n_catalog_release_messages (
  release_id uuid not null references i18n_catalog_releases(id) on delete cascade,
  module text not null check (module ~ '^@[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+){2,}$'),
  msgid text not null check (msgid <> ''),
  primary key (release_id, module, msgid)
);

create table i18n_catalog_release_translations (
  release_id uuid not null,
  module text not null,
  msgid text not null,
  locale text not null check (locale ~ '^[a-z]{2}_[A-Z]{2}$' and locale <> 'en_US'),
  translation text not null,
  primary key (release_id, module, msgid, locale),
  foreign key (release_id, module, msgid)
    references i18n_catalog_release_messages(release_id, module, msgid)
    on delete cascade
);

create table workspace_i18n_catalog_states (
  workspace_id uuid primary key references workspaces(id) on delete cascade,
  active_release_id uuid,
  revision bigint not null default 0 check (revision >= 0),
  updated_at timestamptz not null default now(),
  foreign key (workspace_id, active_release_id)
    references i18n_catalog_releases(workspace_id, id)
);

create function enforce_workspace_i18n_catalog_revision()
returns trigger
language plpgsql
as $$
begin
  if new.revision < old.revision or new.revision > old.revision + 1 then
    raise exception 'workspace i18n catalog revision must be monotonic and advance one step';
  end if;
  if new.active_release_id is distinct from old.active_release_id
     and new.revision <> old.revision + 1 then
    raise exception 'active i18n catalog release changes must advance the revision';
  end if;
  return new;
end;
$$;

create trigger workspace_i18n_catalog_revision_guard
before update on workspace_i18n_catalog_states
for each row execute function enforce_workspace_i18n_catalog_revision();

create table workspace_i18n_catalog_overrides (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  module text not null check (module ~ '^@[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+){2,}$'),
  msgid text not null check (msgid <> ''),
  locale text not null check (locale ~ '^[a-z]{2}_[A-Z]{2}$' and locale <> 'en_US'),
  translation text not null,
  updated_at timestamptz not null default now(),
  primary key (workspace_id, module, msgid, locale)
);

create table workspace_i18n_catalog_custom_translations (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  module text not null check (module ~ '^@[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+){2,}$'),
  msgid text not null check (msgid <> ''),
  locale text not null check (locale ~ '^[a-z]{2}_[A-Z]{2}$' and locale <> 'en_US'),
  translation text not null,
  updated_at timestamptz not null default now(),
  primary key (workspace_id, module, msgid, locale)
);

create table workspace_i18n_catalog_obsolete_messages (
  workspace_id uuid not null references workspaces(id) on delete cascade,
  module text not null check (module ~ '^@[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+){2,}$'),
  msgid text not null check (msgid <> ''),
  obsolete_since_release_id uuid not null,
  marked_at timestamptz not null default now(),
  primary key (workspace_id, module, msgid),
  foreign key (workspace_id, obsolete_since_release_id)
    references i18n_catalog_releases(workspace_id, id)
);

create index i18n_catalog_release_translations_locale_idx
  on i18n_catalog_release_translations (release_id, locale);
