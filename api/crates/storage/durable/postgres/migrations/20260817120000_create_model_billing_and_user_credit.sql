create table model_pricing_rules (
    id uuid primary key,
    provider_code text not null,
    upstream_model_id text not null,
    input_token_unit_size bigint not null check (input_token_unit_size > 0),
    input_token_unit_price numeric(38, 18) not null check (input_token_unit_price >= 0),
    output_token_unit_size bigint not null check (output_token_unit_size > 0),
    output_token_unit_price numeric(38, 18) not null check (output_token_unit_price >= 0),
    cache_hit_token_unit_size bigint not null check (cache_hit_token_unit_size > 0),
    cache_hit_token_unit_price numeric(38, 18) not null check (cache_hit_token_unit_price >= 0),
    currency_code text not null default 'USD' check (currency_code = 'USD'),
    effective_from timestamptz not null,
    effective_to timestamptz check (effective_to is null or effective_to > effective_from),
    timezone text not null default 'UTC',
    weekday_mask smallint not null default 127 check (weekday_mask between 1 and 127),
    local_time_start time,
    local_time_end time,
    priority integer not null default 0 check (priority >= 0),
    enabled boolean not null default true,
    source_kind text not null default 'manual' check (source_kind in ('official', 'manual')),
    source_catalog_id text,
    source_version text,
    source_checksum text,
    extensions jsonb not null default '{}'::jsonb check (jsonb_typeof(extensions) = 'object'),
    created_by uuid,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    check ((local_time_start is null) = (local_time_end is null)),
    check (local_time_start is null or local_time_start < local_time_end),
    check ((source_kind = 'official' and source_catalog_id is not null) or source_kind = 'manual')
);

create unique index model_pricing_rules_official_catalog_id_uidx
    on model_pricing_rules (source_catalog_id)
    where source_kind = 'official';
create index model_pricing_rules_match_idx
    on model_pricing_rules (provider_code, upstream_model_id, enabled, priority desc, effective_from desc);

create table workspace_billing_settings (
    workspace_id uuid primary key references workspaces(id) on delete cascade,
    billing_enabled_at timestamptz not null default now(),
    catalog_revision bigint not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

insert into workspace_billing_settings (workspace_id, billing_enabled_at)
select id, now() from workspaces
on conflict (workspace_id) do nothing;

create function ensure_workspace_billing_settings() returns trigger language plpgsql as $$
begin
    insert into workspace_billing_settings (workspace_id, billing_enabled_at)
    values (new.id, now())
    on conflict (workspace_id) do nothing;
    return new;
end;
$$;

create trigger workspaces_ensure_billing_settings
after insert on workspaces
for each row execute function ensure_workspace_billing_settings();

create table user_credit_accounts (
    id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    user_id uuid not null references users(id) on delete cascade,
    credit_unit text not null default 'USD' check (credit_unit = 'USD'),
    charge_enabled boolean not null default true,
    current_balance numeric(38, 18) not null default 0,
    reserved_amount numeric(38, 18) not null default 0 check (reserved_amount >= 0),
    revision bigint not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    unique (workspace_id, user_id, credit_unit)
);

create index user_credit_accounts_workspace_updated_idx
    on user_credit_accounts (workspace_id, updated_at desc, id desc);

insert into user_credit_accounts (id, workspace_id, user_id, credit_unit, charge_enabled)
select gen_random_uuid(), membership.workspace_id, membership.user_id, 'USD',
       not exists (
           select 1
           from user_role_bindings binding
           join roles role on role.id = binding.role_id
           where binding.user_id = membership.user_id
             and role.scope_kind = 'system'
             and role.code = 'root'
       )
from workspace_memberships membership
on conflict (workspace_id, user_id, credit_unit) do nothing;

create function ensure_workspace_member_credit_account() returns trigger language plpgsql as $$
begin
    insert into user_credit_accounts (id, workspace_id, user_id, credit_unit, charge_enabled)
    values (
        gen_random_uuid(), new.workspace_id, new.user_id, 'USD',
        not exists (
            select 1
            from user_role_bindings binding
            join roles role on role.id = binding.role_id
            where binding.user_id = new.user_id
              and role.scope_kind = 'system'
              and role.code = 'root'
        )
    )
    on conflict (workspace_id, user_id, credit_unit) do nothing;
    return new;
end;
$$;

create trigger workspace_memberships_ensure_credit_account
after insert on workspace_memberships
for each row execute function ensure_workspace_member_credit_account();

alter table runtime_credit_ledger
    add column transaction_id uuid,
    add column account_id uuid references user_credit_accounts(id) on delete restrict,
    add column billing_session_id uuid,
    add column actor_user_id uuid,
    add column actor_plugin_id text,
    add column reserved_after numeric(38, 18),
    add column source_type text,
    add column source_id text,
    add column metadata jsonb not null default '{}'::jsonb;

update runtime_credit_ledger set transaction_id = id where transaction_id is null;
alter table runtime_credit_ledger alter column transaction_id set not null;
create unique index runtime_credit_ledger_account_idempotency_uidx
    on runtime_credit_ledger (account_id, idempotency_key)
    where account_id is not null;
create index runtime_credit_ledger_account_created_idx
    on runtime_credit_ledger (account_id, created_at desc, id desc);
create index runtime_credit_ledger_workspace_user_created_idx
    on runtime_credit_ledger (workspace_id, user_id, created_at desc, id desc);
create index runtime_credit_ledger_billing_session_idx
    on runtime_credit_ledger (billing_session_id);

alter table billing_sessions
    add column user_id uuid references users(id) on delete set null,
    add column account_id uuid references user_credit_accounts(id) on delete restrict,
    add column pricing_rule_id uuid,
    add column reserved_amount numeric(38, 18) not null default 0 check (reserved_amount >= 0),
    add column actual_amount numeric(38, 18),
    add column reservation_expires_at timestamptz,
    add column last_heartbeat_at timestamptz;

alter table runtime_cost_ledger
    add column billing_session_id uuid references billing_sessions(id) on delete set null;

create unique index runtime_cost_ledger_billing_session_uidx
    on runtime_cost_ledger (billing_session_id)
    where billing_session_id is not null;

alter table runtime_credit_ledger
    add constraint runtime_credit_ledger_billing_session_fk
    foreign key (billing_session_id) references billing_sessions(id) on delete set null;

create index billing_sessions_reserved_expiry_idx
    on billing_sessions (reservation_expires_at, id)
    where status = 'reserved';

create table credit_event_outbox (
    event_id uuid primary key,
    workspace_id uuid not null references workspaces(id) on delete cascade,
    account_id uuid references user_credit_accounts(id) on delete set null,
    event_type text not null,
    payload jsonb not null,
    created_at timestamptz not null default now(),
    published_at timestamptz,
    locked_by text,
    locked_until timestamptz,
    delivery_attempts integer not null default 0,
    last_error text
);

create index credit_event_outbox_pending_idx
    on credit_event_outbox (created_at, event_id)
    where published_at is null;

update runtime_usage_ledger usage
set price_snapshot = jsonb_build_object(
        'rule_kind', 'historical_zero',
        'billing_enabled_at', settings.billing_enabled_at,
        'reason', 'usage_before_billing_activation'
    ),
    cost_snapshot = jsonb_build_object(
        'normalized_cost', '0',
        'currency_code', 'USD',
        'pricing_match_status', 'historical_zero'
    )
from flow_runs runs
join workspace_billing_settings settings on settings.workspace_id = runs.scope_id
where usage.flow_run_id = runs.id
  and usage.created_at < settings.billing_enabled_at;

insert into runtime_cost_ledger (
    id, flow_run_id, span_id, usage_ledger_id, workspace_id,
    provider_instance_id, gateway_route_id, model_id, upstream_model_id,
    price_snapshot, raw_cost, normalized_cost, settlement_currency,
    cost_source, cost_status, created_at
)
select
    gen_random_uuid(), usage.flow_run_id, usage.span_id, usage.id, runs.scope_id,
    usage.provider_instance_id, usage.gateway_route_id, usage.model_id, usage.upstream_model_id,
    usage.price_snapshot, 0, 0, 'USD', 'historical_zero', 'rated', usage.created_at
from runtime_usage_ledger usage
join flow_runs runs on runs.id = usage.flow_run_id
join workspace_billing_settings settings on settings.workspace_id = runs.scope_id
where usage.created_at < settings.billing_enabled_at
  and not exists (
      select 1 from runtime_cost_ledger cost where cost.usage_ledger_id = usage.id
  );
