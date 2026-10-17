-- Existing lifecycle facts retain their original raw EventBus publication responsibility.
alter table lifecycle_outbox
    add column native_publication_version smallint not null default 1
        check (native_publication_version in (1, 2)),
    add column legacy_handoff_sequence bigint
        check (legacy_handoff_sequence is null or legacy_handoff_sequence > 0);
