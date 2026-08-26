alter table role_data_model_policies
  add column if not exists can_create_override boolean;
