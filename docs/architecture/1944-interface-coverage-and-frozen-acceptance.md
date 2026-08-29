# #1944 Interface Coverage Inventory and Frozen Acceptance Matrix

## Frozen input

- Main: `beta@ff4cc74ab073256419884d3d96e0b3defcb36d45`
- Official plugins: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Machine-readable inventory: `api/apps/api-server/src/_tests/fixtures/interface_coverage_inventory.1944.json`
- Inventory cardinality: 10 entries across 9 finite families.

The inventory freezes externally observable behavior rather than claiming that every existing route already uses `InterfaceInvocationKernel`. Console HTTP, Public Auth, Application Native API, OpenAI/Anthropic/Responses compatibility, SSE/WebSocket, MCP JSON-RPC, `/api/ex`, real background workers, and the current production Kernel path are all represented. Each machine-readable row records protocol shape, authentication, principal/scope, authorization/CSRF, lifecycle policies, target ownership, side effects, tests/consumers, and a source anchor.

## Approved migration boundary

Only these four real production slices are approved for #1944 migration:

1. Public: `GET /api/public/auth/login-instances`.
2. Console/User: `GET /api/console/settings/host-infrastructure/providers`.
3. Application + stream: `POST /api/agent/v1/runs` including its existing SSE projection.
4. MCP/User API key: `POST /api/mcp/:instance_id` for the existing JSON-RPC tool lifecycle.

Compatibility routes, `/api/ex`, WebSocket variants, sign-in mutation, and background workers remain frozen external-behavior regression families. They are not silently rewritten in this Delivery. A route may move into the migration scope only when the same input/principal/state yields identical allow/deny, row scope, state mutation, DTO, status/error, stream ordering, transaction/outbox, runtime dispatch, audit, and receipt observations.

## Frozen Acceptance Matrix

| Row | ARC AC | Required fixture/evidence |
| --- | --- | --- |
| AM-01 | 001, 014 | Source-anchored inventory and four-path before/after equivalence |
| AM-02 | 002, 003, 009 | Definition/Binding/Compiled Plan compiler matrix |
| AM-03 | 004, 005 | Public/User/Application compile and runtime negatives |
| AM-04 | 006, 007, 008, 012 | Lifecycle/attempt/stream/receipt state matrix |
| AM-05 | 010, 011, 013 | Handler ownership and extension capability matrix |
| AM-06 | 015 | Cargo and Node controlled negatives |
| AM-07 | 016 | Fresh frozen-assembly manifest with zero unrun rows |

## Frozen centralized Test Batch

No command below may run before IF-F01 through IF-F07 and all fixtures are committed and the assembly SHA is frozen. The single fresh QA manifest must contain every row and record command, cwd, start/end identity, exit code, log path, result count, and unrun reason.

1. Identity and cleanliness for both repositories; process/port baseline.
2. `cargo fmt --all --check`.
3. `interface-runtime` contract/compiler/lifecycle/receipt tests and compile-fail fixtures.
4. `access-control` plus Principal/AuthN/AuthZ negatives.
5. Full `api-server` unit and integration suite, including the four vertical slices.
6. stream/error/deadline/cancel/idempotency/CSRF/row-scope regression.
7. dependency boundaries and repository Node controlled negatives.
8. `runtime-core`, `orchestration-runtime`, and `runtime-extension-host` suites.
9. Official plugin Node suites and all executable builds.
10. Ignored real-Host conformance and official-repository zero unexpected diff.
11. Migration count/diff, PostgreSQL residue, and unexpected schema diff checks.
12. Compose/dev-up/deploy/rollback and legacy-orphan gates.
13. `cargo check --workspace` and locked/offline `cargo metadata`.
14. `git diff --check`, final paired identity/cleanliness, process/port cleanup.
15. Manifest completeness assertion: frozen Test Batch unrun rows equal zero.

Warnings, logs, manifests, counts, and receipts belong under `tmp/test-governance/1944-interface-lifecycle/`.

## Gap ledger

| Family | Current gap | #1944 handling |
| --- | --- | --- |
| Compatibility HTTP/WebSocket | Protocol adapters still own protocol DTO/error/stream projection | Regression only; no semantic rewrite |
| `/api/ex` | Cookie/User API key credential branching and sync/async response policy are route-owned | Regression only; requires separate equivalence proof before migration |
| Public sign-in | Credential verification and session issuance intentionally meet inside `AuthKernel` | Regression only; raw credential remains in trusted AuthN adapter |
| Internal/background | No evidence for a stable System/Plugin Principal profile | Do not invent a principal; retain worker ownership |
| Dynamic route expansion | No approval for arbitrary runtime route injection | Rejected; bindings compile before publish |

This finite boundary is the stop condition for route migration: any newly discovered family or behavior-changing requirement returns to the #1944 Ledger before product code is modified.
