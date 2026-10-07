# PostgreSQL tree storage

`parent_id` is the authoritative relationship; `sibling_rank` defines sibling order. The storage layer maintains a private, non-editable `tree_path ltree` column derived from stable UUID labels without hyphens. Renaming and reordering do not change paths. The column is not a public business field; custom fields cannot claim its name.

Coverage includes main-source `core/ordered_tree/v1` tables, `departments`, and `frontstage_block_nodes`. The separate `frontstage_pages` hierarchy is outside this template.

## Deployment and migration

The implementation uses PostgreSQL's bundled contrib `ltree` extension (version1.3 in the current PG18 environment), without adding Node or Rust dependencies. The migration installs it in `public`; the migration account needs extension-creation privileges and query connections must resolve public extension operators. Extension objects survive isolated test-schema cleanup.

The migration backfills existing trees and fails atomically on column conflicts, cycles, or unreachable nodes. It never silently reparents user data. New trees use the same storage installer for paths and GiST expression indexes. Back up and measure migration duration on representative data before rollout; backfill DDL requires table locks.

## Writes and reads

Structure commands retain model/scope/partition transaction locks. Database triggers derive a node's path from its parent and rewrite descendants atomically when it moves. Rollback includes path changes. Direct path or identity/partition changes are rejected. Leaf/subtree deletion and cycle protection retain their contracts.

Descendant matching uses `ARRAY[tree_path] <@ root.tree_path`, with the same single-path semantics. The `gist__ltree_ops` expression index stores signatures and PostgreSQL rechecks matches, avoiding full deep-path index entries in `gist_ltree_ops`. Ancestor queries use path relationships or parent traversal. Both must include `scope_id` and `tree_partition_id`; paths are not authorization. GiST uses official defaults, without capacity limits inferred from development hardware. Sibling ordering remains based on `sibling_rank`, not lexical UUID paths.

Organization member filtering and deduplication run in PostgreSQL; role and department projections are fetched in batches. The navigation root includes unassigned users without creating implicit memberships or changing direct-department role authorization.

## Verification and rollback

Isolated CI covers migration backfill, create/move/delete/rollback, scope isolation, and concurrent cycle protection, and captures path-query plans. Performance assessment must include read cost and move write amplification; adding an index alone does not prove improvement. Lossy signatures can yield false-positive candidates, but PostgreSQL rechecks preserve exact results. Greater depth can reduce signature selectivity without imposing a business depth cap.

On a source rollback, retain the extension, column, and maintenance triggers. If older code cannot pass existing tree behavior checks with those triggers present, roll forward with a fix. Removing the extension or column requires a separate data migration.

Official reference: [PostgreSQL18 ltree documentation](https://www.postgresql.org/docs/18/ltree.html) and the upstream `contrib/ltree/_ltree_gist.c` implementation.
