# Gate execution and evidence reuse

[中文](README.md#门禁执行与结果复用)

Full project acceptance uses `clippy --all-targets` to type-check the same package
groups. Its CI matrix and required receipts omit the three redundant `cargo check`
shards. The PR baseline has no Clippy step and keeps those check shards. Explicit
check commands remain available. Clippy does not replace feature-specific gates
or behavior tests.

Full CI runs the equivalent API behavior suite once in its four coverage shards.
Control-plane library/integration tests also belong to coverage, with an explicit
`cargo test --doc` step preserving current and future documentation examples.
Ordinary diagnostic commands remain available. Storage suites have different test
sets and retain both owners. Required aggregate components decrease from 34 to 29;
coverage thresholds remain unchanged. Each API shard must prove successful nonzero
execution, package/partition/SHA, inventory counts and nonempty profiles. Merge
checks a complete disjoint runnable-test union and counts ignored tests separately
from nextest partition/filter exclusions. Old artifacts cannot fill missing shards.

`scripts/node/verify/backend.js` owns Cargo command construction and validation of
protected test executions. `test-backend` reuses its static check, image/multimodal
scenarios, and workspace test plan. Each protected command still requires actual
passing tests: adjacent success, ignored tests, compilation, or absent output do
not establish execution evidence.

`tooling data-contracts` shares one migration inventory and source analysis results
across the existing schema, raw JSONB, query-contract, and capacity JSON/Markdown
reports. Schema errors and query-contract failures still block. Raw JSONB findings
and growth planning remain advisory, and missing capacity measurements remain
unverified. Missing or malformed required policies, including query contracts,
still fail. Standalone report commands remain available for targeted diagnosis.

Reuse between targeted and full suites requires equivalent tests and execution
conditions. Differences in nextest/libtest, stack size, features, and coverage
instrumentation need separate assessment. Consistency, foundation, coverage, and
protected image-llm scenarios remain in the acceptance plan.

The frontend owns the five consumer-contract tests. `frontend-pr` runs the targeted
contracts explicitly; repository `frontend/all` covers them through the union of
normal tests and page regression. Tooling no longer executes them again.
`test-frontend full` alone does not include the entire page-regression suite.

Unified governance acceptance requires full Rust tests and complete CI on a frozen
candidate. Record passed, failed, ignored/skipped tests and core scenarios, plus
compilation time, test time, CI wall time, and cumulative runner time. The aggregate
must satisfy the candidate/component checks and succeed; targeted script tests do
not establish overall completion. Deduplication may reduce execution counts but
must preserve core scenarios. Historical durations are references with explicit
source/environment/cache differences; unmeasured values remain unknown.

Local backend budgets are `min(CPUs, max(4, floor(CPUs / 2)))` and
`min(effective memory, max(8 GiB, effective memory / 2))`. Both quotas apply to the
aggregate `dev-rust.slice`; the parent and frontend sibling remain unrestricted by default. GitHub Actions verification
and warmup use all runner CPUs and do not apply local memory quotas. Installation
and operating-system limits are owned by [scripts/README](../../../scripts/README.md);
this does not introduce admission coordination across frontend, databases, and
other sessions.

Migration rejection tests run the formal migration set on a dedicated connection
and explicitly close that connection after the expected error. This releases any
retained session advisory lock without waiting for pool eviction. Successful
migrations still use the production entry point. Atomic rollback, money, and
historical checksum assertions remain; production locking and rejection scenarios
are retained.

The common API fixture can clone a command-owned PostgreSQL template prepared only
with formal migrations. Every test still owns a separate database and filesystem
roots, uses explicit startup search_path, runs the production Migrator's
version/checksum validation, and performs full business bootstrap. Existing empty
database and historical migration/restore fixtures keep their original path.
Templates are never cached across commands or CI runs.

The Node runner builds the test-only `with-postgres-template` binary first, then
executes it directly around the test command without nesting Cargo builds. The
wrapper waits for its child and cleans up only its own template/clones on normal
exit, failure and handled INT/TERM. SIGKILL cannot guarantee process cleanup.
Phase metrics live under `tmp/test-governance/postgres-fixtures/`: fresh/clone
measure database creation; migration includes connection and formal migration or
validation; bootstrap covers remaining fixture initialization; prepare and cleanup
measure template preparation and teardown. A serial-work model is
`T_fresh = N × (F + M + B + D)` versus
`T_template = P + N × (C + V + B + D) + D_template`, for N fixtures, empty-database
creation F, migration M, clone C, validation V, bootstrap B, per-database cleanup D
and template preparation P. This does not directly predict parallel wall time or
establish a measured speedup. Retention requires actual
fresh/clone measurements, isolation/cleanup evidence and final CI metrics.

React Doctor reviews bind the complete target source and only the supporting
invariant, direct consumer and relevant tests. Unrelated router/settings-shell
changes should not invalidate local component reasoning. Narrowing evidence does
not remove raw diagnostics or relax review of new rules. Sign-in filing HTML
explicitly trusts publishers authorized by `system.branding.save`; its review binds
both write authorization and verbatim rendering evidence. This does not authorize
arbitrary untrusted HTML.
