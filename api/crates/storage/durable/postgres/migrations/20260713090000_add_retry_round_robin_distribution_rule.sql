alter table model_provider_main_model_distribution_rules
    drop constraint model_provider_main_model_distribution_rules_rule_check;

alter table model_provider_main_model_distribution_rules
    add constraint model_provider_main_model_distribution_rules_rule_check
    check (distribution_rule in ('none', 'round_robin', 'retry_round_robin'));
