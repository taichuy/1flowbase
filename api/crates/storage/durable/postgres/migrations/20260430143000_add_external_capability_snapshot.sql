alter table model_definitions
  add column if not exists external_capability_snapshot jsonb null;
