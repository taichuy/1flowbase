drop index if exists frontstage_page_visibility_rules_root_uidx;

create unique index frontstage_page_visibility_rules_root_uidx
  on frontstage_page_visibility_rules (workspace_id, role_id)
  where page_id is null and tab_id is null;
