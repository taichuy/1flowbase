# Legacy EventBus recovery ownership

The host's live EventBus publishes to `MemoryEventBus::lossless()`. Durable legacy
snapshots remain a separate recovery obligation. Migration
`20261017210000_legacy_event_recovery_ownership.sql` atomically renames existing
relations without copying, acknowledging, replaying or deleting their data:

| Previous relation | Recovery owner |
| --- | --- |
| `host_event_bus_pending` | `legacy_event_recovery_pending` |
| `credit_event_handoffs` | `legacy_credit_handoff_receipts` |
| `host_event_bus_counter` | `legacy_event_recovery_state` |

`PgLegacyEventRecovery` owns these relations. `RecoveryEventBus` delegates only
historical take/inspection to it; live publication stays in memory. Explicit
legacy credit handoff/import remains available through the existing capability,
never by boot-time historical scanning. Model facts frozen with native publication
version 1 keep their original topic/body and `legacy_handoff_sequence` receipt;
new version 2 notifications retain their existing frozen per-subscriber outbox.

A handoff receipt proves enqueue, not subscriber processing or a monetary effect.
An explicit destructive take removes the pending snapshot but retains its credit
receipt. Missing source rows do not invalidate receipts. Sequence allocation and
restore keep the high-water mark and existing cutover provenance. Unknown topics,
UTF-8 topic bytes (including NUL), and exact serialized JSON text are retained.
Neither migration nor recovery re-executes credit commands.

Historical 18 pending / 18 receipts were audited against the prior cutover
snapshot on 2026-10-17: all three complete row sets (including state) matched.
All pending items were `credit.events`, each associated with a published source;
there was no consumer ACK evidence. They therefore remain recovery-pending.
This is retirement of the generic storage ownership and old relation names,
not deletion of unresolved historical obligations or a storage-volume reduction.

Selective settings backups emit the recovery relation names. The original
`1flowbase.settings-rows/v1` names are accepted only as explicit archive aliases;
header declarations, prerequisites and records resolve together. Duplicate aliases,
undeclared record names and conflicting immutable rows fail closed. Restoring a
historical snapshot retains the original explicit restore semantics: a handoff
receipt alone cannot prove a snapshot was consumed, so it cannot authorize
suppressing or automatically replaying that snapshot.

Deploy the new host and migration together while the old host is stopped: old
binaries query the former relation names. Rehearse against an isolated database,
retain a database backup, verify migration checksums and exact recovery rows, then
start the candidate with the ordinary migration runner. On a failed migration its
transaction preserves all old objects. After a committed rename, reverting only
the binary is insufficient; a coordinated reverse rename must account for any
post-cutover recovery writes. Never repair migration checksums or delete history
to make an old binary boot. Physical deletion needs separate disposition evidence
for every pending obligation and a continuing owner for permanent replay identity.
