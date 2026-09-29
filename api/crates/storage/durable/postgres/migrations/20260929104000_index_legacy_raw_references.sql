-- Preserve the exact legacy-reference guard while avoiding a scan of every
-- runtime event for each archive part during explicit historical maintenance.
-- Equality on this expression implies the partial predicate; unrelated events
-- consume no entries and legacy importers continue to maintain it normally.
create index runtime_events_legacy_archive_part_reference
    on runtime_events ((payload->'_client_archive_ref'->>'part_id'))
    where (payload->'_client_archive_ref'->>'part_id') is not null;
