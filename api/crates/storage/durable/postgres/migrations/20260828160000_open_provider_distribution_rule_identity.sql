alter table model_provider_main_model_distribution_rules
    drop constraint if exists model_provider_main_model_distribution_rules_rule_check;

alter table model_provider_main_model_distribution_rules
    add column if not exists distribution_rule_contract_version text not null default '1',
    add column if not exists distribution_rule_config jsonb not null default '{}'::jsonb;

alter table model_provider_main_model_distribution_rules
    add constraint model_provider_main_model_distribution_rules_identity_format_check
    check (
        distribution_rule ~ '^[a-z0-9@][a-z0-9@._/-]{0,127}$'
        and length(distribution_rule_contract_version) between 1 and 64
        and jsonb_typeof(distribution_rule_config) = 'object'
    );
