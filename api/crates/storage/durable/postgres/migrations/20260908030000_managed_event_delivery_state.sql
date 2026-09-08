-- Preserve frozen facts and targets. Pre-fencing claims cannot safely be acknowledged.
alter table lifecycle_outbox_deliveries
    add column claim_id uuid,
    add column claim_expires_at timestamptz,
    add column pause_reason text,
    add column paused_at timestamptz;

alter table lifecycle_outbox_deliveries drop constraint lifecycle_outbox_deliveries_status_check;
alter table lifecycle_outbox_deliveries drop constraint lifecycle_outbox_deliveries_check;

update lifecycle_outbox_deliveries
set status = 'pending', claimed_by = null, claimed_at = null,
    last_error = 'legacy claim requires fenced redelivery'
where status = 'claimed';

alter table lifecycle_outbox_deliveries
    add constraint lifecycle_outbox_deliveries_status_check check (status in ('pending','claimed','delivered','paused')),
    add constraint lifecycle_outbox_deliveries_pause_reason_check check (pause_reason in (
        'frozen_graph_unavailable','frozen_handler_unavailable','authority_revoked','installation_inactive','retry_budget_exhausted'
    )),
    add constraint lifecycle_outbox_deliveries_claim_state_check check (
        (status = 'claimed' and claimed_by is not null and claimed_at is not null and claim_id is not null
            and claim_expires_at is not null and claim_expires_at > claimed_at and delivered_at is null)
        or (status <> 'claimed' and claimed_by is null and claimed_at is null and claim_id is null and claim_expires_at is null
            and ((status = 'delivered' and delivered_at is not null) or (status <> 'delivered' and delivered_at is null)))
    ),
    add constraint lifecycle_outbox_deliveries_pause_state_check check (
        (status = 'paused' and pause_reason is not null and paused_at is not null)
        or (status <> 'paused' and pause_reason is null and paused_at is null)
    );

create index lifecycle_outbox_deliveries_expired_claim_idx
    on lifecycle_outbox_deliveries (claim_expires_at, event_id, subscriber_id) where status = 'claimed';
create index lifecycle_outbox_deliveries_paused_idx
    on lifecycle_outbox_deliveries (pause_reason, event_id, subscriber_id) where status = 'paused';
-- Expiration is owned by the persisted claim, not a later claimant's chosen lease duration.
drop index lifecycle_outbox_deliveries_stale_claim_idx;
create unique index lifecycle_outbox_deliveries_claim_id_idx
    on lifecycle_outbox_deliveries (claim_id) where claim_id is not null;
