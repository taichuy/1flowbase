do $$
declare
  group_owned_tab_count bigint;
  group_owned_block_code_count bigint;
begin
  select count(*)
  into group_owned_tab_count
  from frontstage_page_tabs tab
  join frontstage_pages owner
    on owner.workspace_id = tab.workspace_id
   and owner.id = tab.page_id
  where owner.kind <> 'page';

  select count(*)
  into group_owned_block_code_count
  from frontstage_block_codes block_code
  join frontstage_pages owner
    on owner.workspace_id = block_code.workspace_id
   and owner.id = block_code.page_id
  where owner.kind <> 'page';

  if group_owned_tab_count > 0 or group_owned_block_code_count > 0 then
    raise exception
      'frontstage page owner kind migration rejected dirty data: tab rows %, block code rows %',
      group_owned_tab_count,
      group_owned_block_code_count;
  end if;
end $$;

create function enforce_frontstage_tab_page_owner()
returns trigger
language plpgsql
as $$
declare
  owner_kind text;
  owner_row_exists boolean;
begin
  select owner.kind
  into owner_kind
  from frontstage_page_tabs tab
  join frontstage_pages owner
    on owner.workspace_id = tab.workspace_id
   and owner.id = tab.page_id
  where tab.id = new.id
  for share of owner;

  owner_row_exists := found;

  if owner_row_exists and owner_kind is distinct from 'page' then
    raise exception 'frontstage_page_tab_owner_must_be_page';
  end if;

  return null;
end $$;

create function enforce_frontstage_block_code_page_owner()
returns trigger
language plpgsql
as $$
declare
  owner_kind text;
  owner_row_exists boolean;
begin
  select owner.kind
  into owner_kind
  from frontstage_block_codes block_code
  join frontstage_pages owner
    on owner.workspace_id = block_code.workspace_id
   and owner.id = block_code.page_id
  where block_code.id = new.id
  for share of owner;

  owner_row_exists := found;

  if owner_row_exists and owner_kind is distinct from 'page' then
    raise exception 'frontstage_block_code_owner_must_be_page';
  end if;

  return null;
end $$;

create function enforce_frontstage_page_owner_rows()
returns trigger
language plpgsql
as $$
declare
  current_kind text;
begin
  select kind
  into current_kind
  from frontstage_pages
  where id = new.id;

  if found
    and current_kind <> 'page'
    and (
      exists (
        select 1
        from frontstage_page_tabs tab
        where tab.workspace_id = new.workspace_id
          and tab.page_id = new.id
      )
      or exists (
        select 1
        from frontstage_block_codes block_code
        where block_code.workspace_id = new.workspace_id
          and block_code.page_id = new.id
      )
    )
  then
    raise exception 'frontstage_page_owner_rows_require_page_kind';
  end if;

  return null;
end $$;

create constraint trigger frontstage_page_tabs_require_page_owner
after insert or update of workspace_id, page_id on frontstage_page_tabs
deferrable initially deferred
for each row execute function enforce_frontstage_tab_page_owner();

create constraint trigger frontstage_block_codes_require_page_owner
after insert or update of workspace_id, page_id on frontstage_block_codes
deferrable initially deferred
for each row execute function enforce_frontstage_block_code_page_owner();

create constraint trigger frontstage_pages_owner_rows_require_page_kind
after update of kind on frontstage_pages
deferrable initially deferred
for each row execute function enforce_frontstage_page_owner_rows();
