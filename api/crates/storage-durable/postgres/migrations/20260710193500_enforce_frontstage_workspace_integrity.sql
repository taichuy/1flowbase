do $$
declare
  cross_workspace_parent_count bigint;
  cross_workspace_block_code_count bigint;
begin
  select count(*)
  into cross_workspace_parent_count
  from frontstage_pages child
  join frontstage_pages parent on parent.id = child.parent_id
  where child.parent_id is not null
    and child.workspace_id <> parent.workspace_id;

  select count(*)
  into cross_workspace_block_code_count
  from frontstage_block_codes block_code
  join frontstage_pages page on page.id = block_code.page_id
  where block_code.workspace_id <> page.workspace_id;

  if cross_workspace_parent_count > 0 or cross_workspace_block_code_count > 0 then
    raise exception
      'frontstage workspace integrity migration rejected dirty data: parent rows %, block code rows %',
      cross_workspace_parent_count,
      cross_workspace_block_code_count;
  end if;
end $$;

alter table frontstage_pages
  drop constraint frontstage_pages_parent_id_fkey;

alter table frontstage_pages
  add constraint frontstage_pages_workspace_parent_fkey
  foreign key (workspace_id, parent_id)
  references frontstage_pages (workspace_id, id)
  on delete cascade;

alter table frontstage_block_codes
  drop constraint frontstage_block_codes_page_id_fkey;

alter table frontstage_block_codes
  add constraint frontstage_block_codes_workspace_page_fkey
  foreign key (workspace_id, page_id)
  references frontstage_pages (workspace_id, id)
  on delete cascade;
