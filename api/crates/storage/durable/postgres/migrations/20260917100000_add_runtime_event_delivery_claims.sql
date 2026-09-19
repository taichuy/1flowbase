alter table runtime_events
    add column delivery_status text,
    add column delivery_claim_token uuid,
    add column delivery_generation bigint not null default 0,
    add column delivery_lease_expires_at timestamptz,
    add column delivery_acked_at timestamptz,
    add constraint runtime_events_delivery_status_check
        check (delivery_status is null or delivery_status in ('pending', 'claimed', 'acked')),
    add constraint runtime_events_delivery_state_check
        check (
            (delivery_status is null
                and delivery_claim_token is null
                and delivery_lease_expires_at is null
                and delivery_acked_at is null)
            or (delivery_status = 'pending'
                and delivery_claim_token is null
                and delivery_lease_expires_at is null
                and delivery_acked_at is null)
            or (delivery_status = 'claimed'
                and delivery_claim_token is not null
                and delivery_lease_expires_at is not null
                and delivery_acked_at is null)
            or (delivery_status = 'acked'
                and delivery_claim_token is not null
                and delivery_lease_expires_at is not null
                and delivery_acked_at is not null)
        );

create index runtime_events_pending_delivery_idx
    on runtime_events (flow_run_id, sequence asc)
    where delivery_status in ('pending', 'claimed');
