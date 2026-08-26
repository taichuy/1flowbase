alter table frontstage_pages
  add column content_presentation text not null default 'single';

alter table frontstage_pages
  add constraint frontstage_pages_content_presentation_check
  check (content_presentation in ('single', 'tabs'));

alter table frontstage_pages
  add constraint frontstage_groups_require_single_content_presentation
  check (kind = 'page' or content_presentation = 'single');

update frontstage_pages pages
set content_presentation = 'tabs'
where pages.kind = 'page'
  and exists (
    select 1
    from frontstage_page_tabs tabs
    where tabs.workspace_id = pages.workspace_id
      and tabs.page_id = pages.id
    offset 1
  );

set constraints all immediate;

alter table frontstage_page_tabs
  add column route_segment text;

update frontstage_page_tabs
set route_segment = 'tab-' || replace(id::text, '-', '')
where not is_default;

set constraints all immediate;

alter table frontstage_page_tabs
  add constraint frontstage_page_tabs_route_segment_shape_check
  check (
    route_segment is null
    or route_segment ~ '^[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?$'
  );

alter table frontstage_page_tabs
  add constraint frontstage_page_tabs_default_route_segment_check
  check (
    (is_default and route_segment is null)
    or (not is_default and route_segment is not null)
  );

create unique index frontstage_page_tabs_workspace_page_route_segment_uidx
  on frontstage_page_tabs (workspace_id, page_id, route_segment)
  where route_segment is not null;

do $$
begin
  if exists (
    select 1
    from frontstage_page_schemas schemas
    where jsonb_typeof(schemas.schema_payload) <> 'object'
      or jsonb_typeof(schemas.root_payload) <> 'object'
  ) then
    raise exception
      'frontstage tab document migration rejected non-object legacy payloads';
  end if;

  if exists (
    select 1
    from frontstage_page_schemas schemas
    where jsonb_typeof(schemas.schema_payload -> 'blocks') = 'array'
      and jsonb_typeof(schemas.root_payload -> 'blocks') = 'array'
      and schemas.schema_payload -> 'blocks' <> schemas.root_payload -> 'blocks'
  ) then
    raise exception
      'frontstage tab document migration rejected divergent schema and root blocks';
  end if;
end $$;

alter table frontstage_page_schemas
  add column document_payload jsonb;

update frontstage_page_schemas schemas
set document_payload = case
  when jsonb_typeof(schemas.root_payload -> 'blocks') = 'array'
    then jsonb_set(schemas.schema_payload, '{blocks}', schemas.root_payload -> 'blocks', true)
  else schemas.schema_payload
end;

alter table frontstage_page_schemas
  alter column document_payload set not null;

create or replace function enforce_frontstage_page_tab_invariant(target_workspace_id uuid, target_page_id uuid)
returns void
language plpgsql
as $$
declare
  page_kind text;
  page_content_presentation text;
  tab_count bigint;
  default_count bigint;
begin
  select kind, content_presentation
  into page_kind, page_content_presentation
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
  if page_content_presentation = 'single' and tab_count <> 1 then
    raise exception 'frontstage single page must keep only its default tab';
  end if;
end $$;

drop trigger frontstage_pages_require_tab on frontstage_pages;

create constraint trigger frontstage_pages_require_tab
after insert or update of kind, content_presentation on frontstage_pages
deferrable initially deferred
for each row execute function enforce_frontstage_page_tab_invariant_from_page();
