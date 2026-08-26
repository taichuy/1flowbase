alter table frontstage_pages
  add column if not exists tooltip text,
  add column if not exists is_hidden boolean not null default false;
