alter table model_fields
  add column if not exists api_required boolean not null default false;

update model_fields
set api_required = true
where is_required = true
  and is_writable = true
  and is_system = false;
