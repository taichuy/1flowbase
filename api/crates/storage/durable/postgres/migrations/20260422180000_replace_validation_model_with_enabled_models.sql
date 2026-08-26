alter table model_provider_instances
    add column enabled_model_ids text[] not null default '{}'::text[];

update model_provider_instances
set enabled_model_ids = case
    when validation_model_id is null then '{}'::text[]
    else array[validation_model_id]
end;

alter table model_provider_instances
    drop column validation_model_id,
    drop column last_validated_at,
    drop column last_validation_status,
    drop column last_validation_message;
