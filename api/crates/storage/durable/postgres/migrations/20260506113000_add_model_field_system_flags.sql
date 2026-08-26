alter table model_fields
  add column if not exists is_system boolean not null default false;

alter table model_fields
  add column if not exists is_writable boolean not null default true;
