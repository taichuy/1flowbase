alter table model_provider_main_model_distribution_rules
    add column if not exists distribution_rule_version text;

update model_provider_main_model_distribution_rules
set distribution_rule_version = case distribution_rule
    when 'none' then '1'
    when 'builtin.none' then '1'
    when 'round_robin' then '1'
    when 'builtin.round_robin' then '1'
    when 'retry_round_robin' then '1'
    when 'builtin.retry_round_robin' then '1'
    when '@taichuy/session_retry' then '1.0.0'
    else distribution_rule_contract_version
end
where distribution_rule_version is null;

alter table model_provider_main_model_distribution_rules
    alter column distribution_rule_version set not null,
    alter column distribution_rule_version set default '1';

alter table model_provider_main_model_distribution_rules
    add constraint model_provider_main_distribution_rule_version_check
    check (length(distribution_rule_version) between 1 and 64);
