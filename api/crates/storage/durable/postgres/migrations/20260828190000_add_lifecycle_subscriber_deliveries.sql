alter table lifecycle_outbox
    add column if not exists graph_fingerprint text;

create table if not exists lifecycle_outbox_deliveries (
    event_id uuid not null references lifecycle_outbox(event_id) on delete cascade,
    subscriber_id text not null check (length(btrim(subscriber_id)) > 0),
    handler_id text not null check (length(btrim(handler_id)) > 0),
    handler_version text not null check (length(btrim(handler_version)) > 0),
    status text not null default 'pending'
        check (status in ('pending', 'claimed', 'delivered')),
    attempt_count integer not null default 0 check (attempt_count >= 0),
    available_at timestamptz not null default now(),
    claimed_by uuid,
    claimed_at timestamptz,
    delivered_at timestamptz,
    last_error text,
    primary key (event_id, subscriber_id),
    check (
        (status = 'pending' and claimed_by is null and claimed_at is null and delivered_at is null)
        or (status = 'claimed' and claimed_by is not null and claimed_at is not null and delivered_at is null)
        or (status = 'delivered' and claimed_by is null and claimed_at is null and delivered_at is not null)
    )
);

create index if not exists lifecycle_outbox_deliveries_pending_idx
    on lifecycle_outbox_deliveries (available_at, event_id, subscriber_id)
    where status = 'pending';

create index if not exists lifecycle_outbox_deliveries_stale_claim_idx
    on lifecycle_outbox_deliveries (claimed_at, event_id, subscriber_id)
    where status = 'claimed';
