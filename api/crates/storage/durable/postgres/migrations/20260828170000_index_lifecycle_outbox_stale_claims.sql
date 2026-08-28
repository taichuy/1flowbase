create index if not exists lifecycle_outbox_claimed_idx
    on lifecycle_outbox (claimed_at, occurred_at, event_id)
    where status = 'claimed';
