# #1963 External Interface Lifecycle Assembly Receipt

## Identity

- Delivery: [#1963](https://github.com/taichuy/1flowbase/issues/1963)
- Root: [#1893](https://github.com/taichuy/1flowbase/issues/1893)
- Input: `beta@fee6ae814b28e5136305fd545613520419608c21`
- Initial Product Assembly: `beta@3c6701837534bba1acef2ba7ae4a23730312da35`
- QA-1 remediation Assembly: `beta@a577d91bce7a9792108668316fd36ab81d31b019`
- QA-2 frozen candidate: `beta@9057a345474d43c0236c300206553d027a6959eb`
- QA-3 remediation Product Assembly: `beta@d8a20be4eda6fd1dd39d3b3a15e02d2b56692cee`
- QA-3 frozen documentation candidate: `beta@d354bf31c7cc4494d882461e976e64503758c159`
- EIL-F14R-D input Product Assembly: `beta@d8a20be4eda6fd1dd39d3b3a15e02d2b56692cee`
- EIL-F14R-D assembled Product candidate: `beta@093c19b8a542644d9faed5f8942e66784c30292b`
- Official plugins: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Delivery state: `QA_FAIL / NEEDS_REFRAME`
- Root AC state: candidate only; not settled

## Packet Assembly

| Packet | Commit / range | Result |
| --- | --- | --- |
| EIL-F01 | `5db3b40601d181e73b9700240c99dd4ae92387b6` | Computed External Endpoint Catalog and fail-closed merge rules |
| EIL-F02 | `56211c9873f23b9ef396d6444c339531c9dd9b52` | Exact Protocol/Operational Control allowlist |
| EIL-F03 | `265330263bdae348ea222abc0e28004055976c24` | Additive compiled Interface contribution collector |
| EIL-F04 | `bcb95203cb8e64ab9f9825d1cd3f2c18b62e546a` | Public providers and sign-up lifecycle |
| EIL-F03R | `bea6de107..885a9567b`, `259bd0412`, `8a2a3ddd5..39913b579` | Concrete narrow family dependencies; no `ApiState` Port erasure |
| EIL-F05A-D | `ba83d2673..db961e2d5` | Native read/cancel/resume-callback/file families |
| EIL-F06 | `cf59f3859` | Compatibility residual business operations |
| EIL-F07 | `4b79bfe91` | Dynamic runtime descriptor lifecycle |
| EIL-F08 | `3acd8d4a5..7627d7a66` | Console identity, membership, role and auth-center families |
| EIL-F09 | `c144ab6a9..ac1f2ce06` | Console application, runtime and assistant families |
| EIL-F10 | `c269fe209..54b2a6ccf` | Console data, model, provider and file families |
| EIL-F11 | `ad58895b9..1f8ea7bc2` | Console extension, MCP, network and infrastructure families |
| EIL-F12 | `b08374b3f..379496b1d` | Console workspace, frontstage and UI families |
| EIL-F13 | `1bee7198c..3ab5c6e1e` | Console system, i18n, docs, billing and backup families |
| EIL-F14 | `3c6701837534bba1acef2ba7ae4a23730312da35` | Production Catalog publication, zero-unclassified gate, direct AuthN residue removal and global boundary fixtures |
| EIL-F14R-C | `d8a20be4eda6fd1dd39d3b3a15e02d2b56692cee` | Exact Console operation-spec set, explicit authenticated operation ownership, complete Frontstage specifications and post-migration fixture alignment |
| EIL-F14R-D1 | `b3e461f2e1a2faa41502e09d622fd89a6b31846a` | Typed Console operation compilation source with deterministic complete-set failure reporting |
| EIL-F14R-D2 | `2005de7cbe49774fced125d80f126aab5066d78a` | Registry Binding ownership and migration disposition derived from one compiled snapshot; System Backup residuals enter typed lifecycle |
| EIL-F14R-D3 | `093c19b8a542644d9faed5f8942e66784c30292b` | Candidate-bound validation assets and input-Assembly permission-equivalence baseline |
| EIL-F14R-D4 | `d682d8f1e08db8b6bc74acedbb8837d64dbb388d` | Global publication evidence and QA-4 governance freeze |
| EIL-F14R-D5 | `db57f9adf8c4c11ad4e43eaecae8059e2551ef23` | QA-4 history and QA harness resource isolation only; Product Assembly unchanged |

Every row above is a serial commit or inclusive serial range on `beta`; the Git history is the authoritative per-file write-set ledger. No packet changed database schema, migrations, external DTOs, permissions, Runtime/plugin wire, or official plugin source.

## Final Ownership

```text
api-server Composition Root
→ project explicit family dependencies
→ family Adapter holding Store/Service/Runtime narrow ports
→ compiled Definition + Binding + exactly-one Handler contribution
→ InterfaceContributionCollector deterministic merge
→ one DynamicInterfaceRegistry boot snapshot
→ External Endpoint Catalog exact publication
→ Protocol Adapter → AuthN → AuthZ → Admission → Hooks
→ Handler → Application/Domain/Runtime Port
→ Terminal Receipt → Protocol Projection
```

- The collector never receives `ApiState`, Store/Registry containers, Runtime Host, Router or a dependency map.
- Native, Compatibility, Workflow and MCP production Ports are implemented by concrete adapters, not `ApiState` or aliases/casts of it.
- Console Assistant streaming now uses the frozen Authentication activation; the already-authenticated-principal path remains only for post-upgrade WebSocket frames.
- System backup coordinator controls retain their explicit bounded transport/maintenance ownership; migrated backup business operations use frozen Interface bindings.

## External Endpoint Catalog

The production Catalog is computed from Root mounts, `ConsoleRouteAssembly`, generated static/dynamic OpenAPI operations, MCP protocol methods, frozen `CompiledInterfaceRegistry` bindings and the exact control allowlist. Router construction fails closed for:

- `UNCLASSIFIED` rows;
- unknown binding references;
- duplicate source identities;
- conflicting business/control classifications;
- conflicting canonical binding identities.

The frozen QA batch must report the final total and the four classification counts. Required invariants are `UNCLASSIFIED=0`, Business Direct Route bypass `=0`, production fallback `=0`, dual-run `=0`, double-write `=0`, second Registry `=0`.

## Acceptance State After QA Attempt 2

| Acceptance | State |
| --- | --- |
| EIL-001/002/005/006/007/010 | `FAIL` |
| EIL-003/004 | `NOT_SETTLED` |
| EIL-008/009 | `PASS` |
| Root AC-001/003 | `NOT_SETTLED` |
| Root AC-002/007/009/010 | `FAIL` |

## QA Governance

Both centralized QA attempts are retained as immutable local evidence under:

`tmp/test-governance/1963-external-interface-lifecycle/`

Attempt 2 failed on the same operation-spec completeness root class as Attempt 1 and triggered problem framing. Root subsequently approved the finite EIL-F14R-C exact-set remediation recorded below. One third fresh centralized QA is authorized only for the newly frozen documentation Assembly; #1963 and #1893 remain OPEN, with no push or Root AC settlement.

## Fresh QA Attempt 1 — retained failure history

- Frozen candidate: `beta@1322d7fd03fb1c1b76fbe7c166bec8d42e3de9ff`
- Result: `QA_FAIL`
- Rows: `9 PASS / 6 FAIL / 0 UNRUN`
- Automated evidence: `2022 passed / 606 failed / 2 ignored`
- Independent roots:
  - production boot-plan compilation lacked the explicit `i18n.catalog.view` Core operation specification;
  - the pre-#1963 dependency gate rejected all concrete durable Adapter fields, conflicting with the approved EIL-F03R Composition Root → explicit family Adapter boundary.
- Raw evidence remains unchanged under `tmp/test-governance/1963-external-interface-lifecycle/row-*.log` and the attempt-1 local QA receipt.

Finite remediation packets:

| Packet | Commit | Result |
| --- | --- | --- |
| EIL-F14R-A | `80b8508de849ef6f8636f10a478b17fc64cc0f7d` | Register `i18n.catalog.view` as an explicit authenticated Core operation without changing its existing workspace restriction or API contract |
| EIL-F14R-B | `a577d91bce7a9792108668316fd36ab81d31b019` | Preserve SQL/raw-pool/Runtime-Host prohibitions while allowing approved explicit durable Adapter fields; add positive and negative structural fixtures |

These fixes were mechanically compiled only. Attempt 1 was not rewritten or rerun. The required second fresh frozen batch is recorded below; it did not make any additional EIL or Root AC candidate GREEN.

## Fresh QA Attempt 2 — retained failure history

- Frozen candidate: `beta@9057a345474d43c0236c300206553d027a6959eb`
- Product Assembly under reverification: `beta@a577d91bce7a9792108668316fd36ab81d31b019`
- Result: `QA_FAIL`
- Rows: `11 PASS / 4 FAIL / 0 UNRUN`
- Automated evidence: `2195 passed / 604 failed / 3 ignored`
- Severity: `Blocking 1 / High 1 / Warning 2`
- Passing rows: QA rows `01–04`, `07`, `09–14`
- Failing rows: QA rows `05`, `06`, `08`, `15`
- Endpoint classification counts: unavailable because the production Catalog did not publish

The primary blocker is the missing explicit Core or HostExtension operation specification for `frontstage.blocks.update`. Attempt 1 stopped on the same completeness root at `i18n.catalog.view`; therefore this is the second centralized failure of one root class, not authorization for another one-operation repair.

After excluding the boot-plan cascade, six direct API assertions remain. Three encode stale pre-#1963 structural expectations: direct Workflow `.authenticate(...)`, all Frontstage routes being `Authenticated`, and runtime i18n remaining `Authenticated` instead of its explicit `ConsoleOperation`. The other three source-anchor assertions for overview, run-conversation messages and trace-tree projections remain unresolved because the production boot failure prevents complete behavior evidence. Legacy authentication or route bypasses must not be restored to satisfy them.

Immutable Attempt-2 evidence is retained under:

`tmp/test-governance/1963-external-interface-lifecycle/attempt-2/`

## Reframe Gate

The user approved EIL-F14R-C after a bounded read-only inventory proved 26 missing specifications, all in the `frontstage.*` family. The Packet now:

- separates typed route selection from authorization kind;
- preserves Authenticated authorization for explicitly owned Frontstage and runtime i18n operations;
- validates the complete production `ConsoleOperation`/Core/HostExtension/projected specification exact set and reports every sorted missing, extra and duplicate identity in one failure;
- retains unique per-route Interface identities for multi-route authorization profiles;
- aligns six stale source-structure fixtures with the post-F03R/F14 typed Adapter owners.

Mechanical assembly evidence is `cargo fmt --all --check` PASS, `git diff --check` PASS, `cargo check -p api-server --tests` PASS and the minimum production exact-set publication probe `1 passed / 0 failed / 1240 filtered`. No per-Packet regression or QA was run. The next commit freezes this Receipt with the product Assembly above; that document HEAD is the sole QA-3 candidate.

## Fresh QA Attempt 3 — retained failure history

- Frozen candidate: `beta@d354bf31c7cc4494d882461e976e64503758c159`
- Product Assembly: `beta@d8a20be4eda6fd1dd39d3b3a15e02d2b56692cee`
- Result: `QA_FAIL`
- Rows: `10 PASS / 5 FAIL / 0 UNRUN`
- De-duplicated automated evidence: `2210 passed / 552 failed / 1 ignored`
- Raw automated executions: `2250 passed / 553 failed / 3 ignored`
- Severity: `Blocking 2 / High 2 / Warning 4`
- Endpoint classification counts: unavailable because the production Catalog did not publish

The two Blocking findings are:

1. `http.console.i18n.catalog.get.v1` is published by both the runtime-i18n family contribution and the Core owned-operation projection, so Router/Catalog assembly rejects the duplicate binding.
2. The live Console migration crosswalk has no disposition for at least `frontstage.blocks.delete`, so release-cohort migration planning cannot compile.

The exact-set compiler itself passes, but its repair exposed that operation identity, Interface binding ownership and policy-migration disposition still have separate sources of truth. This is the same completeness root class for a third centralized QA cycle. Per the approved stop condition, no QA-4 or additional per-identity patch is authorized. Delivery returns to problem framing for a finite single-owner compilation model.

Additional High findings are incomplete fixture alignment (`4/6` of the six specified fixtures) and a stale Console hygiene baseline (`401 operations / 434 routes`, `46 errors`). The single PostgreSQL failure is environment lock-table exhaustion (`53200`); migration diff and schema residue are zero, so no candidate storage regression is proven.

Candidate acceptance disposition:

- PASS: `EIL-008`
- NOT SETTLED: `EIL-003`, `EIL-004`, `EIL-009`
- FAIL: `EIL-001`, `EIL-002`, `EIL-005`, `EIL-006`, `EIL-007`, `EIL-010`
- Root NOT SETTLED: `AC-001`, `AC-003`
- Root FAIL: `AC-002`, `AC-007`, `AC-009`, `AC-010`

Immutable Attempt-3 evidence is retained under:

`tmp/test-governance/1963-external-interface-lifecycle/attempt-3/`

## EIL-F14R-D assembled candidate — QA-4 pending

The approved finite remediation replaces the three parallel completeness truths with one typed
compilation path:

```text
Compiled ConsoleOperation inventory
        + Compiled Interface Registry bindings
        + Compiled policy-migration dispositions
                         ↓
           CompiledConsoleOperationSnapshot
                         ↓
      boot publication / Endpoint Catalog / hygiene projection
```

Publication now validates every active business route against its frozen Interface binding and
migration disposition before the Dynamic Interface Registry is published. Approved protocol and
operational controls are classified by the existing exact allowlist and are not forced to masquerade
as business bindings. The five residual System Backup coordinator business operations (`list`,
`create`, recovery `preflight`, `reauth`, and `intent`) now execute through the existing typed
System Backup family adapter; route DTOs, status codes, CSRF, root-cookie checks, password/digest
checks, maintenance lease and asynchronous handoff remain unchanged.

The minimum production compiler authenticity probe passed with:

| total | business | protocol | operational | unclassified |
| ---: | ---: | ---: | ---: | ---: |
| 1058 | 472 | 584 | 2 | 0 |

Static assembly findings at freeze are: Business Direct Route bypass `0`, production fallback `0`,
dual-run `0`, double-write `0`, and second Registry `0`. These are assembly facts only; EIL and Root
AC remain candidate-only until the single frozen QA-4 batch settles all 15 rows with zero failures
and zero unrun items.

The Console hygiene baseline is not an unreviewed current-candidate refresh. It is byte-derived from
the immutable QA-3 inventory of input Product Assembly
`d8a20be4eda6fd1dd39d3b3a15e02d2b56692cee`, with source SHA-256
`7bd227feb1bf913f3f059f100e8b18f77dede9bb134f7c3efe8bad00ca5a08b6`. This proves the former
113-item expansion set predates F14R-D. Frontstage and runtime-i18n routes retain their prior
Authenticated authorization while acquiring explicit typed owners; no Simple grant or permission
result was added.

Development-stage evidence is limited to `cargo fmt --all --check`, `git diff --check`,
`cargo check -p api-server --tests`, and the single production compiler authenticity probe. Packet
tests, Node, frontend, storage, Compose, Runtime and official-plugin suites were deliberately not run.
All QA-1/2/3 evidence remains unchanged. The assembled candidate was evaluated by fresh centralized
QA-4 under `tmp/test-governance/1963-external-interface-lifecycle/attempt-4/`.

## Fresh QA Attempt 4 — retained failure history

- Frozen documentation candidate: `beta@d682d8f1e08db8b6bc74acedbb8837d64dbb388d`
- Product Assembly: `beta@093c19b8a542644d9faed5f8942e66784c30292b`
- Result: `QA_FAIL`
- Rows: `11 PASS / 4 FAIL / 0 UNRUN`
- Observed automation: `1738 passed / 11 failed / 3 ignored`; the full API process was
  terminated before its final summary, so this is a lower bound rather than a complete batch count.
- Severity: `Blocking 0 / High 0 / Warning 6`
- Endpoint counts: `1058 total / 472 business / 584 protocol / 2 operational / 0 unclassified`

The QA-3 operation/Binding/migration completeness root did not recur. Production Catalog
publication, exact-set compilation, migration reconciliation, runtime/official-plugin evidence and
all zero-residue counters passed. Four red rows have one finite QA execution batch:

1. Row 05 exited `137` before the full api-server suite emitted failure bodies or a final summary.
2. Row 08 did not materialize the preserved hygiene evaluator into the fresh evidence directory.
3. The disposable PostgreSQL URL was exported QA-wide and overrode two dev-up fixture-local URLs.
4. Row 15 failed derivatively and could not use unauthenticated GitHub CLI access.

EIL-F14R-D5 changes no product, test, contract, database, migration, Runtime or plugin source. It
materializes the fixed Row-08 evaluator, scopes the disposable database only to API/storage rows,
uses explicitly reduced Rust test concurrency after the reproduced resource kill, and obtains
read-only GitHub state through the existing Git credential without exposing it. The Product Assembly
remains frozen. Attempt-4 evidence remains immutable under
`tmp/test-governance/1963-external-interface-lifecycle/attempt-4/`; the next event is one fresh QA-5.

## Fresh QA Attempt 5 — final failed batch; reframe required

- Frozen documentation candidate: `beta@db57f9adf8c4c11ad4e43eaecae8059e2551ef23`
- Product Assembly: `beta@093c19b8a542644d9faed5f8942e66784c30292b`
- Result: `QA_FAIL`
- Rows: `11 PASS / 4 FAIL / 0 UNRUN`
- Observed automation: `1750 passed / 10 failed / 3 ignored`; Row 05 did not emit a final
  harness summary, so the intended-suite total is unavailable.
- Severity: `Blocking 0 / High 0 / Warning 6`

The architecture closure itself is candidate-bound and green where complete evidence exists:

- Endpoint Catalog: `1058 total / 472 business / 584 protocol / 2 operational / 0 unclassified`.
- Console inventory: `401 operations / 434 interfaces / 434 routes`.
- Hygiene: `0 findings / 0 errors / 0 warnings`.
- Direct bypass, fallback, dual-run, double-write and second Registry: all `0`.
- The QA-3 operation/Binding/migration completeness root did not recur.

The batch still fails Rows 01, 05, 10 and 15. Row 01 is a stale QA harness count (`25` versus the
26 matching Receipt rows). Row 05 observed `188 ok / 9 FAILED` statuses, then made no log progress
for about 110 minutes while retaining roughly 9 GiB RSS; after more than two hours Root authorized a
single SIGTERM of only the stuck test child so the original batch could settle without rerun or
substitution. Row 10 reproduced PostgreSQL `53200` in one final settings-migration fixture even on a
disposable PostgreSQL 18 instance with `max_locks_per_transaction=256`; migration diff and new
schema residue remain zero. Row 15 is derivative of the incomplete full API evidence and prior red
rows; read-only GitHub OPEN-state capture and paired integrity passed.

Because the full API nontermination/resource root has now failed two consecutive centralized QA
attempts, long-running-work requires returning to problem framing. No QA-6 is authorized in this
execution. Candidate states are: EIL-001/002/003/005/008 `PASS`; EIL-004/006/007/009
`NOT_SETTLED`; EIL-010 `FAIL`; Root AC-002/010 candidate `PASS`, with AC-001/003/007/009
`NOT_SETTLED`. No EIL or Root acceptance is formally settled.

#1963 and #1893 remain OPEN. No EIL or Root AC is settled, no push occurred, and no product, test, migration, protocol or official-plugin source was changed after the frozen candidate.
