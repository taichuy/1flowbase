# Gate execution and evidence reuse

[中文](README.md#门禁执行与结果复用)

Full project acceptance uses `clippy --all-targets` to type-check the same package
groups. Its CI matrix and required receipts omit the three redundant `cargo check`
shards. The PR baseline has no Clippy step and keeps those check shards. Explicit
check commands remain available. Clippy does not replace feature-specific gates
or behavior tests.

Full CI runs the equivalent API behavior suite once in its four coverage shards.
Complete package behavior suites for control-plane, orchestration-runtime,
runtime-extension-host and storage-durable-postgres also belong to their coverage
owners. The first three retain explicit `cargo test --doc` steps, including the
Host compile-fail case and future examples. Only ordinary runtime/storage shards
under `QUALITY_GATE_SCOPE=ci` omit the three duplicated packages. Standalone/PR
shards, `test-backend` and `verify-backend all` retain their full selection. All
control-plane-postgres-tests still run in four storage partitions, with the
model-usage-report-postgres Node integration in partition one.
Each API shard must prove successful nonzero
execution, package/partition/SHA, inventory counts and nonempty profiles. Merge
checks a complete disjoint runnable-test union and counts ignored tests separately
from nextest partition/filter exclusions. Old artifacts cannot fill missing shards.

`scripts/node/verify/backend.js` owns Cargo command construction and validation of
protected test executions. With no arguments, `test-backend` and `verify-backend all`
run static checks and the workspace test plan once; that plan already includes
image/multimodal scenarios. The standalone `image-llm-vision` scope remains available.
Each protected command still requires actual
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
instrumentation need separate assessment. Executed test identities confirm that
the default-feature consistency-control-runtime/storage/api, official-i18n-seed
and image-llm-vision suites already run in ordinary/coverage package tests. Full
CI omits those five repeated owners, reducing required components from 29 to 24.
Their manual scopes and the PR seed/image-llm gates remain available. The special
features and unique cases of history-maintenance retain a separate owner.
Foundation gates, coverage thresholds and rejection of missing, failed or
wrong-SHA receipts remain unchanged.

The frontend owns the five consumer-contract tests. `frontend-pr` runs the targeted
contracts explicitly; repository `frontend/all` covers them through the union of
normal tests and page regression. Tooling no longer executes them again.
`test-frontend full` alone does not include the entire page-regression suite.

Scoped developer commands pass selections to the native runner:
`test-backend.js --package <crate> [--lib | --test <target> | --bin <target>] [--filter <name>] [--exact]`
and `test-frontend.js fast [--] <file> [-t <pattern>]`. Backend selection requires
a package. Unsupported, missing or conflicting arguments fail before execution
instead of silently running the full suite. Explicit frontend files bypass the
fast-suite exclusions; name-only selections retain that suite. Each selection
validates its own native Vitest JSON report for actual passing tests, so an
all-skipped run cannot succeed. Resource budgets still apply.
Full CI executes a real scoped Rust command and a scoped
frontend command in their existing owner jobs.

Protocol conformance owns official provider tests, plugin packaging and real wire
and runtime contracts. Repository Rust behavior belongs to the ordinary/coverage
owners above; Node tool tests belong to `repo-tooling`. The protocol job no longer
compiles and executes them again. Full CI still requires candidate receipts from
all these owners; a standalone protocol pass does not establish repository-wide
acceptance. Provider-only cases, nonzero execution checks, transport/error oracles
and paired source/artifact provenance remain intact.

API coverage uses the [native LLVM `%Nm` merge pool](https://clang.llvm.org/docs/SourceBasedCodeCoverage.html),
with N derived from test parallelism. The runtime locks and merges profiles per
binary instead of retaining a full profile for every test process through `%p`.
Partitions, execution counts, SHA checks and coverage thresholds are unchanged.
Before its test run, the first shard compares complete coverage counters for the
old and pooled patterns using the same real instrumented binary. Deliberately
omitting one execution must change the counters. A failed proof prevents a
successful shard receipt. The proof travels with shard one's artifact; final
coverage still merges profiles from the complete API test suite.

`cargo llvm-cov report` scans executables in the target directory, including old
cached objects. Before building the current inventory, both shards and merge use
native `cargo clean --package api-server` to rebuild the measured package. This
prevents retired source and old instantiations from inflating denominators while
preserving other dependency caches and existing SDK workers. It follows
[cargo-llvm-cov's measured-package cleanup](https://github.com/taiki-e/cargo-llvm-cov/blob/v0.9.1/src/clean.rs),
without excluding source to improve coverage. Merge rejects API-owned files absent
from the candidate and logs the actual LLVM object list. The CI cache probe must
detect stale-object contamination and recover the same current counters as a clean
baseline after package cleanup.

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

The common API fixture uses `PostgresTestSchema::create()` to give each test a
separate empty database and schema with explicit startup search_path, then runs
the production Migrator and complete business bootstrap. Filesystem roots remain
isolated per fixture, and the database guard only cleans up its owned temporary
database. Historical migration and restore fixtures keep their existing entry
points.

React Doctor reviews bind the complete target source and only the supporting
invariant, direct consumer and relevant tests. Unrelated router/settings-shell
changes should not invalidate local component reasoning. Narrowing evidence does
not remove raw diagnostics or relax review of new rules. Sign-in filing HTML
explicitly trusts publishers authorized by `system.branding.save`; its review binds
both write authorization and verbatim rendering evidence. This does not authorize
arbitrary untrusted HTML.
