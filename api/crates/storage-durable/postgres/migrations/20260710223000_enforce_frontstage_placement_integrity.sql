do $$
declare
  placement_mismatch_count bigint;
begin
  select count(*)
  into placement_mismatch_count
  from frontstage_pages child
  join frontstage_pages parent
    on parent.workspace_id = child.workspace_id
   and parent.id = child.parent_id
  where child.placement <> parent.placement;

  if placement_mismatch_count > 0 then
    raise exception
      'frontstage placement integrity migration rejected dirty data: mismatch rows %',
      placement_mismatch_count;
  end if;
end $$;

create function enforce_frontstage_page_placement_integrity()
returns trigger
language plpgsql
as $$
declare
  parent_placement text;
begin
  if new.parent_id is not null then
    select parent.placement
    into parent_placement
    from frontstage_pages parent
    where parent.workspace_id = new.workspace_id
      and parent.id = new.parent_id
    for update;

    if found and parent_placement <> new.placement then
      raise exception 'frontstage_page_placement_mismatch'
        using constraint = 'frontstage_pages_parent_child_placement';
    end if;
  end if;

  if tg_op = 'UPDATE' and new.placement is distinct from old.placement then
    if exists (
      select 1
      from frontstage_pages child
      where child.workspace_id = new.workspace_id
        and child.parent_id = new.id
        and child.placement <> new.placement
    ) then
      raise exception 'frontstage_group_placement_requires_empty_group'
        using constraint = 'frontstage_pages_parent_child_placement';
    end if;
  end if;

  return new;
end $$;

create trigger frontstage_pages_placement_integrity_trigger
before insert or update of workspace_id, parent_id, placement on frontstage_pages
for each row execute function enforce_frontstage_page_placement_integrity();
