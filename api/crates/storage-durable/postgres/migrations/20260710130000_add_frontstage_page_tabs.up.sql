do $$
declare
  missing_schema_rows bigint;
  duplicate_schema_rows bigint;
  root_mismatches bigint;
begin
  with page_schema_counts as (
    select
      pages.id,
      pages.workspace_id,
      pages.schema_root_uid as page_root_uid,
      count(schemas.page_id) as schema_count,
      min(schemas.root_uid) as schema_root_uid
    from frontstage_pages pages
    left join frontstage_page_schemas schemas
      on schemas.workspace_id = pages.workspace_id
      and schemas.page_id = pages.id
    where pages.kind = 'page'
    group by pages.id, pages.workspace_id, pages.schema_root_uid
  )
  select
    count(*) filter (where schema_count = 0),
    count(*) filter (where schema_count > 1),
    count(*) filter (
      where schema_count = 1
        and page_root_uid is distinct from schema_root_uid
    )
  into missing_schema_rows, duplicate_schema_rows, root_mismatches
  from page_schema_counts;

  if missing_schema_rows > 0
    or duplicate_schema_rows > 0
    or root_mismatches > 0
  then
    raise exception
      'frontstage page tabs preflight rejected legacy data: missing schema rows %, duplicate schema rows %, root mismatches %',
      missing_schema_rows,
      duplicate_schema_rows,
      root_mismatches;
  end if;
end $$;

alter table frontstage_pages
  add column placement text not null default 'sidebar';

alter table frontstage_pages
  add constraint frontstage_pages_placement_check
  check (placement in ('topbar', 'sidebar'));

create table frontstage_page_tabs (
  id uuid primary key,
  workspace_id uuid not null references workspaces(id) on delete cascade,
  page_id uuid not null,
  title text,
  rank text not null default '',
  is_default boolean not null default false,
  document_root_uid text not null,
  created_by uuid,
  updated_by uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  foreign key (workspace_id, page_id)
    references frontstage_pages (workspace_id, id)
    on delete cascade,
  unique (workspace_id, id),
  unique (workspace_id, document_root_uid)
);

create index frontstage_page_tabs_workspace_page_rank_idx
  on frontstage_page_tabs (workspace_id, page_id, rank, id);

create unique index frontstage_page_tabs_one_default_per_page_uidx
  on frontstage_page_tabs (workspace_id, page_id)
  where is_default;

insert into frontstage_page_tabs (
  id,
  workspace_id,
  page_id,
  title,
  rank,
  is_default,
  document_root_uid,
  created_by,
  updated_by,
  created_at,
  updated_at
)
select
  md5('1flowbase.frontstage.default_tab:' || pages.id::text)::uuid,
  pages.workspace_id,
  pages.id,
  'Default',
  'a',
  true,
  pages.schema_root_uid,
  pages.created_by,
  coalesce(pages.updated_by, pages.created_by),
  pages.created_at,
  pages.updated_at
from frontstage_pages pages
where pages.kind = 'page';

alter table frontstage_page_schemas add column tab_id uuid;

update frontstage_page_schemas schemas
set tab_id = tabs.id
from frontstage_page_tabs tabs
where tabs.workspace_id = schemas.workspace_id
  and tabs.page_id = schemas.page_id
  and tabs.is_default;

do $$
begin
  if exists (select 1 from frontstage_page_schemas where tab_id is null) then
    raise exception 'frontstage page schema backfill produced an unreachable document';
  end if;
end $$;

alter table frontstage_page_schemas alter column tab_id set not null;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_pkey;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_workspace_id_page_id_key;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_page_id_fkey;
alter table frontstage_page_schemas drop column page_id;
alter table frontstage_page_schemas add primary key (tab_id);
alter table frontstage_page_schemas
  add constraint frontstage_page_schemas_workspace_tab_uidx unique (workspace_id, tab_id);
alter table frontstage_page_schemas
  add constraint frontstage_page_schemas_workspace_tab_fkey
  foreign key (workspace_id, tab_id)
  references frontstage_page_tabs (workspace_id, id)
  on delete cascade;

alter table frontstage_pages drop constraint frontstage_pages_check;
alter table frontstage_pages drop column schema_root_uid;

create function enforce_frontstage_page_tab_invariant(target_workspace_id uuid, target_page_id uuid)
returns void
language plpgsql
as $$
declare
  page_kind text;
  tab_count bigint;
  default_count bigint;
begin
  select kind into page_kind
  from frontstage_pages
  where workspace_id = target_workspace_id and id = target_page_id;

  if page_kind is null or page_kind <> 'page' then
    return;
  end if;

  select count(*), count(*) filter (where is_default)
  into tab_count, default_count
  from frontstage_page_tabs
  where workspace_id = target_workspace_id and page_id = target_page_id;

  if tab_count = 0 then
    raise exception 'frontstage page must keep at least one tab';
  end if;
  if default_count <> 1 then
    raise exception 'frontstage page must keep exactly one default tab';
  end if;
end $$;

create function enforce_frontstage_page_tab_invariant_from_page()
returns trigger
language plpgsql
as $$
begin
  perform enforce_frontstage_page_tab_invariant(new.workspace_id, new.id);
  return null;
end $$;

create function enforce_frontstage_page_tab_invariant_from_tab()
returns trigger
language plpgsql
as $$
begin
  if tg_op = 'DELETE' then
    perform enforce_frontstage_page_tab_invariant(old.workspace_id, old.page_id);
  elsif tg_op = 'UPDATE'
    and (old.workspace_id, old.page_id) is distinct from (new.workspace_id, new.page_id)
  then
    perform enforce_frontstage_page_tab_invariant(old.workspace_id, old.page_id);
    perform enforce_frontstage_page_tab_invariant(new.workspace_id, new.page_id);
  else
    perform enforce_frontstage_page_tab_invariant(new.workspace_id, new.page_id);
  end if;
  return null;
end $$;

create constraint trigger frontstage_pages_require_tab
after insert or update of kind on frontstage_pages
deferrable initially deferred
for each row execute function enforce_frontstage_page_tab_invariant_from_page();

create constraint trigger frontstage_page_tabs_preserve_invariant
after insert or update or delete on frontstage_page_tabs
deferrable initially deferred
for each row execute function enforce_frontstage_page_tab_invariant_from_tab();
