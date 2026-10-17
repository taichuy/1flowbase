-- SQLx applies this migration in one transaction. Rename the existing objects,
-- retaining every pending obligation, permanent receipt, FK and counter value.
-- No history is replayed, acknowledged, cleared, or reconstructed here.
alter table host_event_bus_pending rename to legacy_event_recovery_pending;
alter table credit_event_handoffs rename to legacy_credit_handoff_receipts;
alter table host_event_bus_counter rename to legacy_event_recovery_state;
alter index host_event_bus_pending_topic_sequence_idx
    rename to legacy_event_recovery_pending_topic_sequence_idx;

-- Constraint renames also rename their backing indexes; retain the same objects.
alter table legacy_event_recovery_pending
    rename constraint host_event_bus_pending_pkey to legacy_event_recovery_pending_pkey;
alter table legacy_event_recovery_pending
    rename constraint host_event_bus_pending_sequence_check to legacy_event_recovery_pending_sequence_check;
alter table legacy_event_recovery_pending
    rename constraint host_event_bus_pending_credit_event_id_key to legacy_event_recovery_pending_credit_event_id_key;
alter table legacy_event_recovery_pending
    rename constraint host_event_bus_pending_value_size_bytes_check to legacy_event_recovery_pending_value_size_bytes_check;
alter table legacy_event_recovery_pending
    rename constraint host_event_bus_pending_credit_event_id_sequence_fkey to legacy_event_recovery_pending_credit_event_id_sequence_fkey;
alter table legacy_credit_handoff_receipts
    rename constraint credit_event_handoffs_pkey to legacy_credit_handoff_receipts_pkey;
alter table legacy_credit_handoff_receipts
    rename constraint credit_event_handoffs_sequence_key to legacy_credit_handoff_receipts_sequence_key;
alter table legacy_credit_handoff_receipts
    rename constraint credit_event_handoffs_sequence_check to legacy_credit_handoff_receipts_sequence_check;
alter table legacy_credit_handoff_receipts
    rename constraint credit_event_handoffs_origin_check to legacy_credit_handoff_receipts_origin_check;
alter table legacy_credit_handoff_receipts
    rename constraint credit_event_handoffs_event_id_sequence_key to legacy_credit_handoff_receipts_event_id_sequence_key;
alter table legacy_event_recovery_state
    rename constraint host_event_bus_counter_pkey to legacy_event_recovery_state_pkey;
alter table legacy_event_recovery_state
    rename constraint host_event_bus_counter_singleton_check to legacy_event_recovery_state_singleton_check;
alter table legacy_event_recovery_state
    rename constraint host_event_bus_counter_last_sequence_check to legacy_event_recovery_state_last_sequence_check;
alter table legacy_event_recovery_state
    rename constraint host_event_bus_counter_cutover_id_key to legacy_event_recovery_state_cutover_id_key;

comment on table legacy_event_recovery_pending is
    'Retained legacy recovery snapshots, including model lifecycle v1; explicit poll/take is not consumer processing acknowledgement. Live host events use memory.';
comment on table legacy_credit_handoff_receipts is
    'Permanent legacy credit handoff identity; retained after take to prevent replay, not proof of consumer acknowledgement.';
comment on table legacy_event_recovery_state is
    'Transaction-owned legacy recovery sequence high-water mark and cutover identity; not a live EventBus publication store.';
comment on column credit_event_outbox.published_at is
    'Legacy enqueue evidence only; permanent historical handoff identity is retained separately by legacy_credit_handoff_receipts. No consumer acknowledgement is implied.';
