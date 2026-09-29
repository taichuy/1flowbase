# Retained 62m sample replay

The ignored `real_62m_storage_compaction_lossless_and_physical_receipt` test is
authored for the single assembled Test Batch. It does not run in ordinary tests.
The parent `_tests/mod.rs` needs `mod storage_compaction_replay;`.

Set `DATABASE_URL` to the development PostgreSQL database containing the retained
sample in `public`. The official fixture helper creates `test_<uuid>` schemas in
that same database. Set these explicit inputs before running the ignored test:

- `STORAGE_REPLAY_RECEIPT`: absolute path to `live-audit-final-valid-session.json`.
- `STORAGE_REPLAY_COSTS`: absolute path to `62m-session-storage-costs.json`.
- `STORAGE_REPLAY_OUTPUT_DIR`: absolute artifact directory under
  `tmp/test-governance/`.
- `STORAGE_REPLAY_CANDIDATE_SHA`: optional full frozen assembly SHA; when set it
  must equal `git rev-parse HEAD`.

The fixture checks the receipt attachment hash, exactly 15 run identities,
51,081 source events, 85,201 source frames and 96 captures. It seeds the official
migration prefix before `20260929100000`, removes the seed run and its cascading
runtime rows, copies only the selected task rows, and fills missing FK parents
using the actual official prefix FK graph, checked against source `pg_catalog`
metadata. Source fingerprints bind a read-only transaction to `public`; isolated
fingerprints bind to the destination, with transaction-local search_path restored
before reuse. It copies finite conversation-message and
observation ownership child rows. An additional run, missing parent, incompatible
prefix column, cross-schema parent or unsupported partition layout stops replay.
No application-wide or whole-database source copy is permitted.

The copy transaction qualifies all destination writes with its guarded isolated
schema. `session_replication_role=replica` is local to that connection and is reset
to `origin` before every destination FK and run/task/archive owner is audited.
Source `public` tables are selected only. The schema guards reclaim both replay
and latest-empty-baseline schemas, including on errors.

The candidate upgrades through `run_migrations` and invokes the real
`compact_retained_runtime_storage(&run_ids, 256)` mover and its reentry. It compares
full original event values and header/ID/order identity, exact Native body Strings,
client pages and all non-raw sections, node filters, request focus, source directory
references, every raw frame and a public raw-section traversal per capture. Raw
sections share capture content across Steps, so that content is traversed once per
request instead of once per repeated semantic history Step.

`storage-compaction-replay.json` lists all task-domain and finite support/default
relations with row-value, heap/auxiliary, index, TOAST and gross allocated sizes.
It records old/latest empty baselines, old gross, formal upgrade before mover,
live allocation after mover, and an isolated `VACUUM FULL` layout equivalent.
The latter does not claim live source file reclamation or CPU/latency/WAL savings.
The old 184.103 MiB receipt omitted Step and other retained business costs; this
fixture counts their directories, references and relation/index/TOAST allocation.

Only counts, hashes, sizes and notes are exported. Original payloads and database
credentials are never printed; failures expose a fixed phase, error digest and
only PostgreSQL SQLSTATE/constraint/table metadata instead of a raw error chain.
No fixture execution, build or service restart was performed when
these files were authored.
