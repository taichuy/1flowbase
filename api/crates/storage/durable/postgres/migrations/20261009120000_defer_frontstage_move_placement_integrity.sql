-- Keep the existing placement invariant, but permit a subtree move to validate
-- its final state at commit instead of rejecting intermediate row updates.
drop trigger frontstage_pages_placement_integrity_trigger on frontstage_pages;

create or replace function enforce_frontstage_page_placement_integrity()
returns trigger
language plpgsql
as $$
declare
  current_page frontstage_pages%rowtype;
  parent_placement text;
  parent_parent_id uuid;
begin
  select * into current_page
  from frontstage_pages
  where workspace_id = new.workspace_id and id = new.id;
  if not found then
    return null;
  end if;

  if current_page.parent_id is not null then
    select parent.placement, parent.parent_id
    into parent_placement, parent_parent_id
    from frontstage_pages parent
    where parent.workspace_id = current_page.workspace_id
      and parent.id = current_page.parent_id
    for update;
    if found and not (
      parent_placement = current_page.placement
      or (
        parent_placement = 'topbar'
        and parent_parent_id is null
        and current_page.placement = 'sidebar'
      )
    ) then
      raise exception 'frontstage_page_placement_mismatch'
        using constraint = 'frontstage_pages_parent_child_placement';
    end if;
  end if;

  if exists (
    select 1 from frontstage_pages child
    where child.workspace_id = current_page.workspace_id
      and child.parent_id = current_page.id
      and not (
        child.placement = current_page.placement
        or (
          current_page.placement = 'topbar'
          and current_page.parent_id is null
          and child.placement = 'sidebar'
        )
      )
  ) then
    raise exception 'frontstage_group_placement_requires_empty_group'
      using constraint = 'frontstage_pages_parent_child_placement';
  end if;
  return null;
end $$;

create constraint trigger frontstage_pages_placement_integrity_trigger
  after insert or update of workspace_id, parent_id, placement on frontstage_pages
  deferrable initially immediate
  for each row execute function enforce_frontstage_page_placement_integrity();
