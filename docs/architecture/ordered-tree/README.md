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

## Pagination contract

`tree/roots`, `tree/children/{id}`, `tree/descendants/{id}`, and `tree/search` return `{items, has_more, next_cursor}` instead of bare arrays. Pass `next_cursor` unchanged as `cursor` on the next request with the same query. An exhausted page has `has_more=false` and `next_cursor=null`. Template descriptors, OpenAPI, and the TypeScript SDK use this contract; upgrade callers together. `tree/ancestors/{id}` remains an array ordered from the root to the direct parent.

`limit` is a positive page size (default100, search20), not a tree capacity limit. The former1000-result and100-search-match caps are removed. `max_depth` is an optional positive depth filter; omission does not impose a depth cutoff, while results remain paged. The former default32 and maximum256 are removed. Integer representations and PostgreSQL ltree still have physical limits. Search page size counts matching nodes; `items` also contains ancestor context, which may recur across pages. Consumers merge by ID and preserve `is_match=true`.

Keyset pagination uses `sibling_rank COLLATE "C", id` for siblings and ancestor sibling ordering for depth-first descendant traversal. Cursors contain query context, an anchor ID, and a fixed-size fingerprint of the anchor's path/order, rather than the entire deep path. They bind model, scope, partition, query kind, and filters; they do not grant authorization. Anchor validation and page reads share a read-only repeatable-read transaction. A cursor for a different query returns400; deletion, reparenting, or an ordering change invalidating the anchor fingerprint returns409 `tree_stale_cursor`. Discard the cursor and reload.

Requests observe live data, without persistent snapshots. Traversal of static data has no omissions or duplicates. Concurrent insertion before the consumed boundary or movement across it can omit or repeat nodes; reloading obtains the current view. Consistent exports require a separate snapshot/export facility. Pagination bounds response size but does not eliminate descendant sorting or ancestor-context query costs; use execution plans and measured workloads to assess performance.

The organization endpoint `GET /api/console/settings/departments` uses the same envelope: no filter lists top-level departments; `parent_id` lists direct children; `prefix` searches with ancestor context; `ids` (comma-separated UUIDs) resolves saved selections. These three filters are mutually exclusive. Items retain department fields, roles, and distinct subtree member counts, adding `has_children` and `is_match`. Explicit-ID lookup uses immutable ID ordering, so reparenting alone does not invalidate its cursor. Member filtering queries the subtree in the backend independently of UI expansion. Organization views load on expansion and continuation; saving returns a targeted read instead of scanning every department.

Runtime protection remains owned by the interface kernel and deployment-level deadline/cancellation governance. The template adds no guessed capacity, timeout, or cache mechanism. Failures are explicit errors, never partial results presented as complete. Existing external block-tree array contracts keep their established semantics and only adapt to the internal shared query changes; this update does not claim cursor traversal for those endpoints.
