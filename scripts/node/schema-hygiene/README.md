# Schema Hygiene Rules

## Profiles

- `managed_table` is the default for every unmarked physical table.
- `dynamic_model_table` is for generated workspace-scoped user data tables.
- `registered_system_table` is a fixed physical table registered into metadata.

## registered_system_table

`registered_system_table` means fixed physical table, metadata registration, and read-only field template.

It is not a schema hygiene exemption. The scanner still checks required physical columns, primary key shape, scope, indexes, constraints, and parse failures. If an existing registered system table misses a fixed-template requirement, the gate must report the difference first; schema repair belongs in a separate migration issue.

Metadata systems may manage display configuration, actions, views, and relation metadata for these tables, but must not add, delete, rename, or physically change registered system table columns.

## physical_contract_table

The payload split and archive storage migrations from 2026-09-28/29 use physical
identities rather than generic platform lifecycle columns. Their explicit entries
in `tableProfiles` select `physical_contract_table`; `physicalTableContracts`
requires exact primary keys, column types/nullability, owner foreign keys and
ON DELETE actions, uniqueness, integrity CHECKs, and lookup index prefixes.
A missing contract declaration fails. These checks do not accept exemption skips.
Unmarked tables still use the unchanged `managed_table` rules.

- `node_runs` remains a scanned registered system table. Its fixed field template
  matches `NODE_RUNS_FIELDS` in `api/crates/domain/src/builtin_data_model.rs` and
  the physical directory after `20260928190000_node_run_details.sql`. Its separate
  physical contract also checks its run ownership and identity. Payloads belong
  to `node_run_details`, keyed by `(node_run_id, section)`; `node_run_records` is
  an adapter view rather than another physical table.
- Archive heads permit an unbound nullable `flow_run_id` before capture binding.
  Parts and blocks belong to the request head; block locators retain a composite
  `(request_id, block_id)` owner FK. Codec contracts use the latest packed
  directory migration `20260929143000_dense_metadata_and_packed_archive_directories.sql`.
- Native snapshot items and manifests use an application/scope composite owner
  FK and application/hash uniqueness. References use `(manifest_id, item_id)`;
  their item lookup index and both owner references remain required.
- Observation reclamation ownership uses `content_id` as both PK and canonical
  content FK. It marks only archive-created bodies, with owner cascade cleanup.

Physical CHECK comparison ignores whitespace, but requires the declared SQL
expression; it does not prove SQL semantic equivalence. Table/ALTER CHECK drops
are removed from the inventory so retired checks cannot satisfy current rules.
Inline CHECKs carry their PostgreSQL-generated constraint names so their drops
also remove them from the inventory. Trigger immutability,
view writes, runtime scope validation and body reconstruction require behavioral
verification beyond this source gate. The `_tests/physical-contract.test.js`
fixtures exercise real migration inventory and controlled missing/wrong contract
properties; run them with the centralized tooling test batch.
