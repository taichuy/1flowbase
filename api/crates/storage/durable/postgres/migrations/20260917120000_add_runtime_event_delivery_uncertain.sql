-- A delivery whose protocol write started but could not be proven is neither
-- pending (replayable) nor acked (proven). It leaves the automatic replay set
-- and waits for explicit recovery.
alter table runtime_events
    add column delivery_uncertain_at timestamptz,
    drop constraint runtime_events_delivery_status_check,
    drop constraint runtime_events_delivery_state_check,
    add constraint runtime_events_delivery_status_check
        check (delivery_status is null
               or delivery_status in ('pending', 'claimed', 'acked', 'uncertain')),
    add constraint runtime_events_delivery_state_check
        check (
            (delivery_status is null
                and delivery_claim_token is null
                and delivery_lease_expires_at is null
                and delivery_acked_at is null
                and delivery_uncertain_at is null)
            or (delivery_status = 'pending'
                and delivery_claim_token is null
                and delivery_lease_expires_at is null
                and delivery_acked_at is null
                and delivery_uncertain_at is null)
            or (delivery_status = 'claimed'
                and delivery_claim_token is not null
                and delivery_lease_expires_at is not null
                and delivery_acked_at is null
                and delivery_uncertain_at is null)
            or (delivery_status = 'acked'
                and delivery_claim_token is not null
                and delivery_lease_expires_at is not null
                and delivery_acked_at is not null
                and delivery_uncertain_at is null)
            or (delivery_status = 'uncertain'
                and delivery_claim_token is not null
                and delivery_lease_expires_at is not null
                and delivery_acked_at is null
                and delivery_uncertain_at is not null)
        );
