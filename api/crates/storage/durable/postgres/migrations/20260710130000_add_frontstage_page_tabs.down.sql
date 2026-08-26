do $$
begin
  if exists (
    select 1
    from frontstage_pages
    where placement <> 'sidebar'
  ) then
    raise exception 'frontstage rollback cannot downgrade non-sidebar navigation placement';
  end if;

  if exists (
    select 1
    from frontstage_pages pages
    left join frontstage_page_tabs tabs
      on tabs.workspace_id = pages.workspace_id
     and tabs.page_id = pages.id
    where pages.kind = 'page'
    group by pages.workspace_id, pages.id
    having count(tabs.id) <> 1
  ) then
    raise exception 'frontstage rollback cannot downgrade: each page must have exactly one tab; multi-tab or tabless page detected';
  end if;

  if exists (
    select 1
    from frontstage_page_tabs tabs
    join frontstage_pages pages
      on pages.workspace_id = tabs.workspace_id
     and pages.id = tabs.page_id
    where pages.kind <> 'page'
  ) then
    raise exception 'frontstage rollback cannot downgrade tabs attached to non-page navigation nodes';
  end if;

  if exists (
    select 1
    from frontstage_page_tabs
    where title is distinct from 'Default'
       or rank <> 'a'
       or not is_default
  ) then
    raise exception 'frontstage rollback cannot downgrade customized single-tab metadata';
  end if;

  if exists (
    select 1
    from frontstage_page_schemas schemas
    left join frontstage_page_tabs tabs
      on tabs.workspace_id = schemas.workspace_id
     and tabs.id = schemas.tab_id
    where tabs.id is null
  ) then
    raise exception 'frontstage rollback cannot downgrade unreachable page document';
  end if;

  if exists (
    select 1
    from frontstage_page_tabs tabs
    join frontstage_page_schemas schemas
      on schemas.workspace_id = tabs.workspace_id
     and schemas.tab_id = tabs.id
    where tabs.document_root_uid <> schemas.root_uid
  ) then
    raise exception 'frontstage rollback cannot downgrade mismatched tab and document roots';
  end if;
end $$;

alter table frontstage_pages add column schema_root_uid text;

drop trigger frontstage_page_tabs_preserve_invariant on frontstage_page_tabs;
drop trigger frontstage_pages_require_tab on frontstage_pages;
drop function enforce_frontstage_page_tab_invariant_from_tab();
drop function enforce_frontstage_page_tab_invariant_from_page();
drop function enforce_frontstage_page_tab_invariant(uuid, uuid);

update frontstage_pages pages
set schema_root_uid = tabs.document_root_uid
from frontstage_page_tabs tabs
where tabs.workspace_id = pages.workspace_id
  and tabs.page_id = pages.id;

do $$
begin
  if exists (
    select 1 from frontstage_pages
    where kind = 'page' and schema_root_uid is null
  ) then
    raise exception 'frontstage rollback cannot restore a page schema root';
  end if;
end $$;

alter table frontstage_pages
  add constraint frontstage_pages_check check (
    (kind = 'group' and schema_root_uid is null) or
    (kind = 'page' and schema_root_uid is not null)
  );

alter table frontstage_page_schemas add column page_id uuid;

update frontstage_page_schemas schemas
set page_id = tabs.page_id
from frontstage_page_tabs tabs
where tabs.workspace_id = schemas.workspace_id
  and tabs.id = schemas.tab_id;

do $$
begin
  if exists (select 1 from frontstage_page_schemas where page_id is null) then
    raise exception 'frontstage rollback cannot restore a page document association';
  end if;
end $$;

alter table frontstage_page_schemas alter column page_id set not null;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_pkey;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_workspace_tab_uidx;
alter table frontstage_page_schemas drop constraint frontstage_page_schemas_workspace_tab_fkey;
alter table frontstage_page_schemas drop column tab_id;
alter table frontstage_page_schemas add primary key (page_id);
alter table frontstage_page_schemas add unique (workspace_id, page_id);
alter table frontstage_page_schemas
  add foreign key (page_id) references frontstage_pages(id) on delete cascade;

drop table frontstage_page_tabs;
alter table frontstage_pages drop constraint frontstage_pages_placement_check;
alter table frontstage_pages drop column placement;
