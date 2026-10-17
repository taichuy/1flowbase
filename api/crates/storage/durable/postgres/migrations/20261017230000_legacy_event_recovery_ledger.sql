-- Forward-only consolidation. Applied migrations and retained bodies are immutable.
-- SQLx owns the surrounding transaction; failures leave all three source tables intact.
lock table legacy_event_recovery_state, legacy_event_recovery_pending,
    legacy_credit_handoff_receipts in access exclusive mode;

create table legacy_event_recovery_ledger (
    sequence bigint primary key check (sequence >= 0),
    disposition text not null,
    topic bytea,
    payload text,
    payload_digest bytea,
    value_size_bytes bigint check (value_size_bytes >= 0),
    credit_event_id uuid,
    cutover_id uuid,
    origin text check (origin in ('live', 'legacy_snapshot')),
    handed_off_at timestamptz,
    last_sequence bigint check (last_sequence >= 0),
    created_at timestamptz,
    constraint legacy_event_recovery_row_shape check (coalesce((
        (sequence = 0 and disposition = 'state' and last_sequence is not null
            and cutover_id is not null and created_at is not null
            and topic is null and payload is null and payload_digest is null
            and value_size_bytes is null and credit_event_id is null
            and origin is null and handed_off_at is null)
        or
        (sequence > 0 and disposition in ('pending', 'taken')
            and last_sequence is null and created_at is null
            and ((credit_event_id is null and cutover_id is null and origin is null and handed_off_at is null)
                or (credit_event_id is not null and cutover_id is not null and origin is not null and handed_off_at is not null))
            and (payload_digest is null or octet_length(payload_digest) = 32)
            and ((disposition = 'pending' and topic is not null and payload is not null
                    and payload_digest is not null and value_size_bytes is not null
                    and payload_digest = sha256(convert_to(payload, 'UTF8')))
                or (disposition = 'taken' and payload is null
                    and ((topic is not null and payload_digest is not null and value_size_bytes is not null)
                        or (topic is null and payload_digest is null and value_size_bytes is null and credit_event_id is not null)))))
    ),false))
);
create unique index legacy_event_recovery_credit_identity_idx
    on legacy_event_recovery_ledger(credit_event_id) where credit_event_id is not null;
create index legacy_event_recovery_pending_topic_idx
    on legacy_event_recovery_ledger(topic, sequence) where disposition = 'pending';

insert into legacy_event_recovery_ledger(sequence,disposition,last_sequence,cutover_id,created_at)
    select 0,'state',last_sequence,cutover_id,created_at from legacy_event_recovery_state;
insert into legacy_event_recovery_ledger(sequence,disposition,topic,payload,payload_digest,
    value_size_bytes,credit_event_id,cutover_id,origin,handed_off_at)
    select coalesce(p.sequence,r.sequence),case when p.sequence is null then 'taken' else 'pending' end,
        p.topic,p.payload,sha256(convert_to(p.payload,'UTF8')),p.value_size_bytes,
        r.event_id,r.cutover_id,r.origin,r.handed_off_at
    from legacy_event_recovery_pending p full join legacy_credit_handoff_receipts r on r.sequence=p.sequence;

-- A receipt at the same sequence must belong to that exact pending credit identity.
-- The old FK handles missing receipts; this guard also rejects unrelated sequence collisions.
do $$ begin
    if exists(select 1 from legacy_event_recovery_pending p
        join legacy_credit_handoff_receipts r using(sequence)
        where p.credit_event_id is distinct from r.event_id) then
        raise exception 'inconsistent legacy recovery receipt identity';
    end if;
end $$;

drop table legacy_event_recovery_pending;
drop table legacy_credit_handoff_receipts;
drop table legacy_event_recovery_state;
comment on table legacy_event_recovery_ledger is
    'Legacy recovery only: sequence 0 owns allocation state; positive rows retain pending snapshots or permanent explicit-take identities. Take is not consumer ACK. Live EventBus publishes only to memory.';
comment on column legacy_event_recovery_ledger.payload_digest is
    'SHA256 of the exact UTF8 serialized payload; survives explicit take. NULL only for historical receipt-only identities whose original body is unavailable.';
comment on column credit_event_outbox.published_at is
    'Legacy enqueue evidence only; legacy_event_recovery_ledger retains independent snapshots and handoff identities, not consumer acknowledgement.';
