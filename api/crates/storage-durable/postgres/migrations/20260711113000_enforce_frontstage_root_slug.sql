update frontstage_pages
set slug = 'p-' || left(replace(id::text, '-', ''), 10)
where parent_id is null
  and placement = 'topbar'
  and slug is null;

create unique index if not exists frontstage_pages_workspace_slug_uidx
  on frontstage_pages (workspace_id, slug)
  where slug is not null;

alter table frontstage_pages
  add constraint frontstage_pages_slug_format_check
  check (
    slug is null
    or slug ~ '^[a-z0-9](?:[a-z0-9-]{2,46}[a-z0-9])?$'
  );

alter table frontstage_pages
  add constraint frontstage_pages_root_slug_check
  check (
    (parent_id is null and placement = 'topbar' and slug is not null)
    or (not (parent_id is null and placement = 'topbar') and slug is null)
  );

create or replace function enforce_frontstage_page_placement_integrity()
returns trigger
language plpgsql
as $$
declare
  parent_placement text;
  parent_parent_id uuid;
begin
  if new.parent_id is not null then
    select parent.placement, parent.parent_id
    into parent_placement, parent_parent_id
    from frontstage_pages parent
    where parent.workspace_id = new.workspace_id
      and parent.id = new.parent_id
    for update;

    if found and not (
      parent_placement = new.placement
      or (
        parent_placement = 'topbar'
        and parent_parent_id is null
        and new.placement = 'sidebar'
      )
    ) then
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
        and not (
          child.placement = new.placement
          or (new.placement = 'topbar' and new.parent_id is null and child.placement = 'sidebar')
        )
    ) then
      raise exception 'frontstage_group_placement_requires_empty_group'
        using constraint = 'frontstage_pages_parent_child_placement';
    end if;
  end if;

  return new;
end $$;
