alter table model_definitions
  add column if not exists source_kind text not null default 'main_source';

alter table model_definitions
  add column if not exists external_resource_key text null;

do $$
begin
  if not exists (
    select 1
    from pg_constraint
    where conname = 'model_definitions_source_kind_check'
      and conrelid = 'model_definitions'::regclass
  ) then
    alter table model_definitions
      add constraint model_definitions_source_kind_check
      check (source_kind in ('main_source', 'external_source'));
  end if;
end $$;

create unique index if not exists model_definitions_external_resource_uidx
  on model_definitions (data_source_instance_id, external_resource_key)
  where source_kind = 'external_source'
    and data_source_instance_id is not null
    and external_resource_key is not null;

alter table model_fields
  add column if not exists external_field_key text null;
