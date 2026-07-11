alter table frontstage_page_visibility_rules
  add column if not exists tab_id uuid;

alter table frontstage_page_visibility_rules
  add constraint frontstage_page_visibility_rules_tab_fkey
  foreign key (workspace_id, tab_id)
  references frontstage_page_tabs (workspace_id, id)
  on delete cascade;

alter table frontstage_page_visibility_rules
  add constraint frontstage_page_visibility_rules_target_check
  check (
    (page_id is null and tab_id is null)
    or (page_id is not null and tab_id is null)
    or (page_id is null and tab_id is not null)
  );

create unique index if not exists frontstage_page_visibility_rules_tab_uidx
  on frontstage_page_visibility_rules (workspace_id, tab_id, role_id)
  where tab_id is not null;

create index if not exists frontstage_page_visibility_rules_workspace_tab_idx
  on frontstage_page_visibility_rules (workspace_id, tab_id)
  where tab_id is not null;
