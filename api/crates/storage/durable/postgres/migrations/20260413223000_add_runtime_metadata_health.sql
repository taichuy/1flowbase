alter table model_definitions
  drop constraint if exists model_definitions_scope_kind_check;

alter table model_definitions
  add column if not exists availability_status text;

update model_definitions
set availability_status = 'available'
where availability_status is null;

alter table model_definitions
  alter column availability_status set default 'available',
  alter column availability_status set not null;

alter table model_definitions
  add constraint model_definitions_scope_kind_check
  check (scope_kind in ('system', 'workspace'));

alter table model_definitions
  drop constraint if exists model_definitions_availability_status_check;

alter table model_definitions
  add constraint model_definitions_availability_status_check
  check (availability_status in ('available', 'unavailable', 'broken'));

alter table model_fields
  add column if not exists availability_status text;

update model_fields
set availability_status = 'available'
where availability_status is null;

alter table model_fields
  alter column availability_status set default 'available',
  alter column availability_status set not null;

alter table model_fields
  drop constraint if exists model_fields_availability_status_check;

alter table model_fields
  add constraint model_fields_availability_status_check
  check (availability_status in ('available', 'unavailable', 'broken'));
