create table if not exists lifecycle_outbox (
    event_id uuid primary key,
    transaction_id uuid not null,
    contract_id text not null check (length(btrim(contract_id)) > 0),
    contract_version text not null check (length(btrim(contract_version)) > 0),
    canonical_payload bytea not null,
    occurred_at timestamptz not null,
    status text not null default 'pending'
        check (status in ('pending', 'claimed', 'delivered')),
    attempt_count integer not null default 0 check (attempt_count >= 0),
    available_at timestamptz not null default now(),
    claimed_by uuid,
    claimed_at timestamptz,
    delivered_at timestamptz,
    last_error text,
    created_at timestamptz not null default now(),
    check (
        (status = 'pending' and claimed_by is null and claimed_at is null and delivered_at is null)
        or (status = 'claimed' and claimed_by is not null and claimed_at is not null and delivered_at is null)
        or (status = 'delivered' and claimed_by is null and claimed_at is null and delivered_at is not null)
    )
);

create index if not exists lifecycle_outbox_pending_idx
    on lifecycle_outbox (available_at, occurred_at, event_id)
    where status = 'pending';

create index if not exists lifecycle_outbox_transaction_idx
    on lifecycle_outbox (transaction_id, occurred_at, event_id);
