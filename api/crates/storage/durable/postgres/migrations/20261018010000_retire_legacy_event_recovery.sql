-- SQLx runs this forward-only retirement in a transaction. Operators must explicitly
-- reconcile and remove known legacy entries before upgrading; never delete unknown history.
lock table legacy_event_recovery_ledger, lifecycle_outbox,
    lifecycle_outbox_deliveries in access exclusive mode;

do $$ begin
    if exists(select 1 from legacy_event_recovery_ledger where sequence > 0) then
        raise exception 'legacy recovery ledger is not empty; explicit reconciliation is required';
    end if;
    if exists(
        select 1 from lifecycle_outbox source
        where source.contract_id = 'model_definition.committed'
          and source.contract_version = 'v1'
          and source.native_publication_version = 1
          and (source.status <> 'delivered' or exists(
              select 1 from lifecycle_outbox_deliveries delivery
              where delivery.event_id = source.event_id and delivery.status <> 'delivered'
          ))
    ) then
        raise exception 'unfinished legacy model publication responsibility requires reconciliation';
    end if;
end $$;

drop table legacy_event_recovery_ledger;
alter table lifecycle_outbox
    drop column native_publication_version,
    drop column legacy_handoff_sequence;
comment on column credit_event_outbox.published_at is
    'Source publication completion evidence, not subscriber acknowledgement. Typed integration delivery responsibility is retained independently in lifecycle_outbox and its per-target receipts.';
