alter table model_provider_main_model_distribution_rules
    add column if not exists excluded_provider_instance_ids uuid[] not null default '{}';
