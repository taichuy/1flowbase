alter table model_definitions
  add column if not exists external_table_id text null;

create unique index if not exists model_definitions_external_table_uidx
  on model_definitions (data_source_instance_id, external_table_id)
  where source_kind = 'external_source'
    and data_source_instance_id is not null
    and external_table_id is not null;
