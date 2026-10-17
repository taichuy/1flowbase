# Legacy EventBus recovery ledger

Live host EventBus publication uses `MemoryEventBus::lossless()`. PostgreSQL owns
only retained recovery obligations and the exact model native-publication-v1
compatibility protocol. `PgLegacyEventRecovery` exposes historical inspection,
reveal, explicit take and exact-ID credit import. It does not implement the
general `EventBus` or live `CreditEventHandoff` publication traits. Tests construct
historical generic entries through fixture SQL rather than a production writer.

Forward SQLx migration `20261017230000_legacy_event_recovery_ledger.sql`
consolidates the intermediate three relations into one physical table,
`legacy_event_recovery_ledger`. Applied migration files remain unchanged. The
transaction locks the old tables, copies every retained field, rejects ambiguous
receipt/sequence collisions, and drops the three old physical tables only after
the complete copy succeeds. Failure rolls back both schema and data changes.

| Ledger row | Retained information |
| --- | --- |
| `sequence = 0`, `disposition = state` | Original sequence high-water mark, cutover UUID and creation timestamp |
| Positive `pending` entry | Original sequence, UTF-8 topic bytes, exact serialized JSON text, size and SHA256 digest; credit receipt fields on this same row when present |
| Positive `taken` entry | Permanent identity, topic, digest, size and receipt fields; body released by explicit take |
| Historical credit receipt without pending body | `taken` identity with exact credit UUID, cutover, origin and handoff time; unavailable topic/body/digest/size remain NULL |

There are three ledger indexes: the primary key, a partial unique credit-identity
index and a partial pending topic/sequence index. The previous three-table
structure had eight indexes. Pending credit entries no longer duplicate their
sequence/identity across separate pending and receipt rows. Pending bodies remain
independent snapshots because source outboxes are mutable and workspace deletion
can cascade through them. The digest adds identity bytes; actual PostgreSQL page
usage must be measured on the deployed candidate, separately from relation/index
counts. This change does not claim deletion of unresolved historical bodies.

All recovery mutations lock state row zero before allocating or taking work.
Allocation uses the greater of the saved high-water mark and every ledger entry
identity, including tombstones. It never infers a fixed backlog count. Inspection
loads scalar metadata only; reveal and FIFO poll retain the original string/JSON
protocol, including unknown topics, Unicode, escaped NUL and unrestricted JSON
number representations. Poll changes `pending` to `taken`, clears only its body,
and commits before returning the decoded value. It is a destructive take, never
proof of subscriber processing, billing or business acknowledgement.

Exact-ID credit import retains the prior envelope serialization and updates the
source atomically with its independent snapshot and receipt. Receipts remain
authoritative after source deletion. Model facts frozen with native publication
version 1 retain their exact original topic/body and source
`legacy_handoff_sequence`; lost lifecycle ACK or a retry after take returns the
same sequence without writing a new entry. New version 2 notification publication
keeps its existing per-subscriber outbox behavior and does not persist generic
recovery events.

Selective settings backups export only the canonical ledger. Original
`host_event_bus_pending` / `credit_event_handoffs` / `host_event_bus_counter` and
intermediate `legacy_event_recovery_pending` / `legacy_credit_handoff_receipts` /
`legacy_event_recovery_state` archives are accepted as complete, exact historical
schemas. A private disk spool joins pending entries with matching receipts,
preserves receipt-only identities and state provenance, then feeds the canonical
validation/write path. No target writes occur during normalization. Duplicate
aliases, duplicate identities, undeclared row aliases, partial schemas, missing or
mismatched credit receipts, changed digests/bodies and immutable identity conflicts
fail before any restore mutation. Prepared restore holds the state lock through
its serializable transaction. State merges never rewind the target high-water or
replace an existing history's cutover; a pristine target adopts source provenance.

Taken state dominates an older pending backup, for credit and generic entries
alike. Restore cannot refill a known taken entry, even if the source disappeared
or the host restarted. Pre-ledger generic takes left no identity; some old model
v1 takes are known only through the source's `legacy_handoff_sequence`. Their
missing topic/body digest cannot be reconstructed honestly. Incoming pending work
whose sequence is absent from a target ledger but lies inside that target's
historical high-water range is rejected as ambiguous. This also prevents an old
pending archive from reviving those pre-ledger model/generic takes. A fresh target
without prior high-water or entry history can restore the archive normally.
No `published_at`, lifecycle `delivered` state or missing body is invented into a
consumer ACK, and no unknown digest is fabricated from a mutable source.

Deploy the candidate and forward migration together while the old host is
stopped. Rehearse on an isolated database, retain the original database backup,
verify migration checksums and projected original fields, then start through the
ordinary migration runner. Old binaries query removed tables and cannot be
restarted against the consolidated schema. Reverting only the binary is
insufficient; any coordinated rollback must account for post-cutover writes and
taken identities. Never repair applied checksums or erase recovery history to
make an old binary boot.
