# #1958 Compatibility Interface Migration Assembly Receipt

## Candidate status

- Delivery: [#1958](https://github.com/taichuy/1flowbase/issues/1958)
- Root: [#1893](https://github.com/taichuy/1flowbase/issues/1893)
- Architecture baseline: [#1944](https://github.com/taichuy/1flowbase/issues/1944)
- Input: `beta@c2bd2c58f7e3e90bdb110b6fc0245c690ed3fbb4`
- Initial product assembly: `beta@b6ac2ffa8480f8345298fb44b71c6febf03fab07`
- Initial frozen assembly: `beta@f38227d24b8ed9e3e0f29493099bb10f7829f4b0`
- Replacement frozen assembly: `beta@a6ad997f18fce23a6a0b8190ee383ff45b146512`
- Final fix product assembly: `beta@37887d9e55f976bd21d5e424890883bfb03cba27` plus this receipt refresh
- Final assembly: receipt refresh commit；集中 QA 必须用 `git rev-parse HEAD` 自动捕获完整 SHA
- Official plugins baseline: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Result: `REASSEMBLED / FINAL_QA_PENDING`
- Evidence root: `tmp/test-governance/1958-compatibility-interface-migration/`

本 Receipt 只冻结开发 assembly 的内容与验收矩阵，不提前声明 CIM AC 或 QA 通过。首次 fresh
centralized QA 对 `f38227d24b8ed9e3e0f29493099bb10f7829f4b0` 完成 21/21 rows，结果为
`16 PASS / 5 FAIL / 0 UNRUN`、1991 passed / 542 failed。完整 blocker 集已一次性转换为下方
fix batch。Replacement QA 对 `a6ad997f18fce23a6a0b8190ee383ff45b146512` 完成 21/21 rows，
结果为 `18 PASS / 3 FAIL / 0 UNRUN`、2512 passed / 2 failed；两个 distinct blocker 已转换为
CI-F03/CI-F04。Final QA 必须对本 refresh 后不变的完整 HEAD 从 QA-001 重启全部 21 rows。

## Packet ledger

| Packet | Commit | Status | Actual write set |
| --- | --- | --- | --- |
| CI-M01 | `830206a3b56ed7f2a3a7171529b9de2d478a74db` | ASSEMBLED | finite inventory、equivalence matrix 与 CIM fixture |
| CI-M02 | `22f32c84d518e544ead1180341025afae7e24b2e` | ASSEMBLED | blocking compatibility Definition/Binding/Handler 与 route projection |
| CI-M03 | `71cedf6a2360cb6583431ead284f4e115a223507` | ASSEMBLED | SSE/WebSocket server-stream plan、sealed actor、terminal 与 control-plane actor ports |
| CI-M04 | `a2989828de1cf0bd73f3819f3be1d57289e299f5` | ASSEMBLED | Public sign-in typed mutation、AuthKernel adapter 与 cookie projection fixture |
| CI-M05 | `563a26acd9b6f3c545e8323a64582ccfd7bb9141` | ASSEMBLED | `/api/ex/*` frozen AuthN/CSRF、typed User Handler、sync/async projection 与 fixture |
| CI-M06 | `b6ac2ffa8480f8345298fb44b71c6febf03fab07` | ASSEMBLED | legacy production path removal、Rust/Node bypass gates、inventory source anchor |
| CI-M07 | this receipt commit | ASSEMBLED | ADR、#1944→#1958 architecture map 与 candidate receipt |
| CI-F01 | `1a567aebdeb40541fe950c6884d9e6af334c0122` | ASSEMBLED | absolute static protocol RouteIdentity、external-path controlled fixtures、`/api/ex` rustfmt-stable exactly-one AuthN gates |
| CI-F02 | this receipt refresh commit | ASSEMBLED | attempt-1 QA receipt、fix batch 与 replacement candidate freeze |
| CI-F03 | `e4b7c7a7d94f64fcd121996d8b138ade0ba1063f` | ASSEMBLED | HostExtension Authentication activation 与共享 BuiltIn factory 并存、preservation fixture |
| CI-F04 | `37887d9e55f976bd21d5e424890883bfb03cba27` | ASSEMBLED | Public sign-in Route projection 与 typed Handler exactly-one execution owner fixture |
| CI-F05 | this receipt refresh commit | ASSEMBLED | replacement QA receipt、最终 fix batch 与 final candidate freeze |

Packet 阶段只执行 `cargo fmt`、`cargo check -p api-server` 和
`cargo check -p api-server --tests` 作为安全装配；没有运行 Packet 测试、reviewer 或 QA。

## Fresh QA attempt 1 and fix batch

Attempt 1 evidence is stored at
`tmp/test-governance/1958-compatibility-interface-migration/`. It found three Blocking roots:

1. `RouteIdentity` incorrectly restricted HTTP bindings to `/api/*`, so `/v1/*` and `/responses`
   compatibility definitions panicked during Registry compilation;
2. Rust and Node `/api/ex` source fixtures searched for the whitespace-sensitive literal
   `boot_snapshot.authenticate(` and misread rustfmt's chained call as zero owners, although the
   production route contains exactly one `.authenticate(` call;
3. the development QA database already contained 396 `test_<32hex>` schemas and ended at 399.

CI-F01 accepts bounded absolute static protocol paths while continuing to reject relative,
scheme-relative, query/fragment-bearing and whitespace/control paths. It also makes the two AuthN
source gates rustfmt-stable. The database blocker was remediated outside Git with the repository's
scoped `dev-db-maintenance` tool: dry-run selected exactly 399 `test_<32hex>` schemas in the local
`1flowbase` development database, then the identical apply plan removed all 399. No business schema,
migration or user data was selected.

## Replacement QA and final fix batch

Replacement QA executed all 21 rows against
`a6ad997f18fce23a6a0b8190ee383ff45b146512`: `18 PASS / 3 FAIL / 0 UNRUN`, 2512 passed / 2 failed,
Blocking 2 / High 0, with `SAME_ROOT_RECURRED=NO`. Migration rehearsal was 261/0, delivery migration
diff remained zero and test-schema residue was `0 → 0`.

CI-F03 closes the first distinct blocker: activating a HostExtension Authentication factory no
longer removes an unrelated BuiltIn activation solely because both use the same adapter identity.
Both frozen activation identities remain factory-bound and registry publication still rejects
duplicate activation identities. CI-F04 corrects the Public sign-in ownership fixture to verify that
the protocol Route owns terminal receipt and cookie projection while the typed Handler owns the
single AuthKernel `login` execution. The production route already preserved that ordering and
external cookie behavior; no product fallback or second execution path is added.

## Final route ownership

```text
Static Protocol Route
→ Frozen Binding / Compiled Invocation Plan
→ activated AuthN factory → sealed Principal
→ core AuthZ → ordered extension vetoes
→ core Admission → ordered extension vetoes
→ frozen Hooks
→ exactly-one Typed Handler
→ existing Application / Domain / Runtime Port
→ exactly-one Terminal Receipt
→ HTTP / SSE / WebSocket projection
```

| Family | Binding/profile | Business and projection owner preserved |
| --- | --- | --- |
| OpenAI Chat/Responses/Compact blocking | typed HTTP / Application | translation、Native run、provider transport、Runtime、OpenAI DTO/error |
| Anthropic Messages blocking | typed HTTP / Application | translation、Native run、Runtime、Anthropic DTO/error |
| OpenAI/Anthropic SSE | typed server-stream / Application | protocol mapper、disconnect/cancel、durable terminal |
| Native/Responses WebSocket | typed server-stream / Application | socket actor、frame mapper、resume/cancel、terminal |
| Public sign-in | typed unary / Public | AuthKernel、SessionIssuer、transaction、audit、Set-Cookie |
| `/api/ex/*` | typed unary / User | session/API-key、CSRF、row scope、WorkflowExtensionRunService、Runtime |

Internal/background workflow schedule remains `HOLD`; it has no Interface registration in this
assembly.

## Removed production compatibility paths

- raw bearer `PreparedCompatibleTurn` and non-actor stream openers;
- route-local OpenAI/Anthropic blocking and callback-resume executors;
- duplicate `/api/ex/*` `require_session` / `require_csrf` owner;
- stale WebSocket credential-to-header conversion branch;
- route-local provider transport staging helper superseded by the typed compatibility handler.

Legacy event-forwarding machinery that remains solely under `#[cfg(test)]` is protocol regression
fixture code and is excluded from production compilation. The Node boundary rejects reintroducing
the removed symbols into production protocol sources.

## CIM acceptance candidate map

| AC | Candidate evidence | Pre-QA status |
| --- | --- | --- |
| CIM-AC-001 | finite 7-entry inventory and source-anchor fixture | ASSEMBLED |
| CIM-AC-002 | explicit blocking bindings and compatibility route tests | ASSEMBLED |
| CIM-AC-003 | server-stream/WebSocket typed runtime and terminal fixtures | ASSEMBLED |
| CIM-AC-004 | Public sign-in plan plus existing valid/invalid/cookie/audit tests | ASSEMBLED |
| CIM-AC-005 | workflow extension plan plus existing auth/CSRF/sync/async/Runtime tests | ASSEMBLED |
| CIM-AC-006 | registry exactly-one compiler negatives and source owner gates | ASSEMBLED |
| CIM-AC-007 | typed ports; dependency and infrastructure-import boundaries | ASSEMBLED |
| CIM-AC-008 | deleted legacy path set and Rust/Node residue gates | ASSEMBLED |
| CIM-AC-009 | migration/API/permission/runtime/plugin comparison deferred to frozen QA | PENDING QA |
| CIM-AC-010 | inventory HOLD row and no Internal binding gate | ASSEMBLED |
| CIM-AC-011 | #1944 publish negatives plus complete catalog/factory/handler assembly | ASSEMBLED |
| CIM-AC-012 | candidate-bound fresh command manifest, failed=0, unrun=0 | PENDING QA |

## Frozen Test Batch

The centralized manifest must cover all #1958 Test Batch rows: interface-runtime; access-control;
full api-server unit/integration; Public sign-in; compatibility blocking/SSE/WebSocket; `/api/ex`;
AuthN/AuthZ/row-scope/CSRF/deadline/cancel/idempotency; Node/Cargo residue and dependency boundaries;
runtime-core/orchestration-runtime/runtime-extension-host; migration rehearsal and zero diff;
frontend consumers; Compose/dev-up/deploy/rollback; fmt/workspace check/locked-offline metadata/diff;
official plugin Node, 9 executable builds and real Host conformance; final paired identity and
cleanliness. Unrun rows must be zero.

No push is authorized. #1958, #1893 and #1944 remain open; this receipt does not settle Root AC.
