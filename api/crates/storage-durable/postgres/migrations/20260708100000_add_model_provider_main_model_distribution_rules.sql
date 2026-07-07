create table if not exists model_provider_main_model_distribution_rules (
    id uuid not null primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    scope_id uuid generated always as (workspace_id) stored,
    provider_code text not null,
    model_id text not null,
    distribution_rule text not null default 'none',
    created_by uuid not null references users(id),
    updated_by uuid not null references users(id),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint model_provider_main_model_distribution_rules_identity_key
        unique (workspace_id, provider_code, model_id),
    constraint model_provider_main_model_distribution_rules_provider_fk
        foreign key (workspace_id, provider_code)
        references model_provider_main_instances (workspace_id, provider_code)
        on delete cascade,
    constraint model_provider_main_model_distribution_rules_provider_code_check
        check (btrim(provider_code) <> ''),
    constraint model_provider_main_model_distribution_rules_model_id_check
        check (btrim(model_id) <> ''),
    constraint model_provider_main_model_distribution_rules_rule_check
        check (distribution_rule in ('none', 'round_robin'))
);

create index if not exists model_provider_main_model_distribution_rules_scope_created_id_idx
    on model_provider_main_model_distribution_rules (scope_id, created_at, id);

create index if not exists model_provider_main_model_distribution_rules_provider_rule_idx
    on model_provider_main_model_distribution_rules (workspace_id, provider_code, distribution_rule);
