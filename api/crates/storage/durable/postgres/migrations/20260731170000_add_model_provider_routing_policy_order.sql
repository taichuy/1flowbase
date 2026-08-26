alter table model_provider_main_instances
    add column if not exists revision bigint not null default 0;

alter table model_provider_main_instances
    add constraint model_provider_main_instances_revision_check
    check (revision >= 0);

alter table model_provider_main_model_distribution_rules
    add column if not exists provider_instance_ids uuid[] not null default '{}';
