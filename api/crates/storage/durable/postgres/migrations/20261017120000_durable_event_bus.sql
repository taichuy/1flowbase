-- A single transaction-owned counter orders committed publications across all topics.
-- Existing published_at is legacy enqueue evidence; do not replay or rewrite it here.
create table host_event_bus_counter (
    singleton boolean primary key default true check (singleton),
    last_sequence bigint not null default 0 check (last_sequence >= 0),
    cutover_id uuid not null unique default gen_random_uuid(),
    created_at timestamptz not null default now()
);
insert into host_event_bus_counter(singleton) values (true);

-- Permanent handoff identity outlives destructive poll; no business ACK is implied.
create table credit_event_handoffs (
    event_id uuid primary key,
    sequence bigint not null unique check (sequence > 0),
    cutover_id uuid not null,
    origin text not null check (origin in ('live', 'legacy_snapshot')),
    handed_off_at timestamptz not null default now(),
    unique(event_id, sequence)
);

create table host_event_bus_pending (
    sequence bigint primary key check (sequence > 0),
    -- UTF-8 bytes preserve the EventBus string namespace, including embedded NUL.
    topic bytea not null,
    -- Serialized JSON has no literal NUL, and avoids jsonb numeric/string restrictions.
    payload text not null,
    credit_event_id uuid unique,
    value_size_bytes bigint not null check (value_size_bytes >= 0),
    foreign key (credit_event_id, sequence) references credit_event_handoffs(event_id, sequence)
        on delete restrict deferrable initially deferred
);
create index host_event_bus_pending_topic_sequence_idx
    on host_event_bus_pending(topic, sequence);
comment on table host_event_bus_pending is
    'Durable pending EventBus queue; poll is destructive take, not consumer processing acknowledgement.';
comment on column credit_event_outbox.published_at is
    'Legacy: memory enqueue only. Durable handoff is evidenced separately by credit_event_handoffs.';
