# #1958 Compatibility Interface Migration Assembly Receipt

## Candidate status

- Delivery: [#1958](https://github.com/taichuy/1flowbase/issues/1958)
- Root: [#1893](https://github.com/taichuy/1flowbase/issues/1893)
- Architecture baseline: [#1944](https://github.com/taichuy/1flowbase/issues/1944)
- Input: `beta@c2bd2c58f7e3e90bdb110b6fc0245c690ed3fbb4`
- Initial product assembly: `beta@b6ac2ffa8480f8345298fb44b71c6febf03fab07`
- Initial frozen assembly: `beta@f38227d24b8ed9e3e0f29493099bb10f7829f4b0`
- Replacement frozen assembly: `beta@a6ad997f18fce23a6a0b8190ee383ff45b146512`
- Prior final QA assembly: `beta@9c367ddcb19e1da7f89247d6fe5808afd70521f6`
- Fixture-remediation QA assembly: `beta@3426bebde34796b8266643c472d6eb8a9b4c8be2`
- Internal-visibility fix assembly: `beta@4741e0f65df23afbe8e1329c9115a063bf37a07b` plus this receipt refresh
- Final assembly: receipt refresh commit；集中 QA 必须用 `git rev-parse HEAD` 自动捕获完整 SHA
- Official plugins baseline: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Result: `REASSEMBLED / FRESH_QA_PENDING`
- Evidence root: `tmp/test-governance/1958-compatibility-interface-migration/`

本 Receipt 只冻结开发 assembly 的内容与验收矩阵，不提前声明 CIM AC 或 QA 通过。首次 fresh
centralized QA 对 `f38227d24b8ed9e3e0f29493099bb10f7829f4b0` 完成 21/21 rows，结果为
`16 PASS / 5 FAIL / 0 UNRUN`、1991 passed / 542 failed。完整 blocker 集已一次性转换为下方
fix batch。Replacement QA 对 `a6ad997f18fce23a6a0b8190ee383ff45b146512` 完成 21/21 rows，
结果为 `18 PASS / 3 FAIL / 0 UNRUN`、2512 passed / 2 failed；两个 distinct blocker 已转换为
CI-F03/CI-F04。随后对 `9c367ddcb19e1da7f89247d6fe5808afd70521f6` 的 21-row QA 记录为
`17 PASS / 4 FAIL / 0 UNRUN`、3670 passed / 6 failed；后续审计证明其中 Set-Cookie finding
来自 Frontstage integration fixture 缺失 required `ExtensionBootSnapshot`，不是完整生产装配下的
产品回归，且 frontend style-boundary 定向复跑 16/16。CI-F06 修复该 fixture 并保留历史证据；
对 `3426bebde34796b8266643c472d6eb8a9b4c8be2` 的 fresh QA 完成 21/21 rows，结果为
`18 PASS / 3 FAIL / 0 UNRUN`、2486 passed / 0 failed 加一个 API test compilation failure。
该 E0433 由 CI-F06 `180376947...` 删除仍被三个 crate-internal tests 使用的 `pub(crate)` 类型重导出
引入，不是输入 `9c367ddcb...` 遗留。CI-F08 恢复该窄重导出；fresh QA 必须对本 refresh 后
不变的完整 HEAD 从 QA-001 重启全部 21 rows。

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
| CI-F06 | `180376947baef0b62cd3a89e4fc421749d72d49b` | ASSEMBLED | 生产/测试共用 BootSnapshot compiler、完整 Frontstage Router fixture、有效/无效/fail-closed sign-in 行为验证 |
| CI-F07 | this receipt refresh commit | ASSEMBLED | 历史 QA 分类校正、RED/GREEN 证据与 fresh candidate freeze |
| CI-F08 | `4741e0f65df23afbe8e1329c9115a063bf37a07b` | ASSEMBLED | 恢复 crate-internal Snapshot query 类型重导出；完整 api-server all-targets no-run 编译通过 |
| CI-F09 | this receipt refresh commit | ASSEMBLED | E0433 提交归因校正、visibility fix 与 fresh candidate freeze |

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

## Final QA classification correction and CI-F06

The QA run against `9c367ddcb19e1da7f89247d6fe5808afd70521f6` remains historical `QA_FAIL`
evidence; its artifacts are not deleted or relabelled as a pass. A bounded RED replay exposed the
actual response before header access: `HTTP 500` with `extension boot snapshot is unavailable`. The
five Frontstage tests manually constructed `ApiState` with `extension_boot_snapshot: None`, while
production startup always compiles and publishes the snapshot before Router construction.

CI-F06 extracts the snapshot compilation already owned by the production Composition Root and uses
that same compiler from production startup and the Frontstage integration fixture. It does not add a
second Graph, Registry, Authentication truth or a missing-snapshot fallback. Directed GREEN is 7/7:
the original five consumers pass; valid credentials return `csrf_token` and `Set-Cookie`; invalid
credentials return 401 without a cookie; and a deliberately removed snapshot remains fail-closed at
500 without a cookie. The frontend `registry.test.tsx` directed replay is 16/16 with no Delivery
`web/` diff, so the prior single failure is recorded as non-causal timing/environment fluctuation.

Therefore `SAME_ROOT_RECURRED=NO`. CIM-AC-004 is `PENDING_REVERIFY` until the candidate-bound full
Router and Frontstage rows pass in fresh QA. CIM-AC-012 remains pending until all 21 rows have zero
failures and zero unrun items.

## Fixture-remediation QA and CI-F08

The fresh QA against `3426bebde34796b8266643c472d6eb8a9b4c8be2` remains historical `QA_FAIL`
evidence: `18 PASS / 3 FAIL / 0 UNRUN`, 2486 passed / 0 failed plus one compilation failure. QA-005
failed before API assertions because CI-F06 commit
`180376947baef0b62cd3a89e4fc421749d72d49b` removed the
`DurableHostInfrastructureProvidersViewQuery` `pub(crate)` re-export while three crate-internal tests
still used that path. QA-019 and QA-021 were derived failures from that single compile root.

CI-F08 restores only the type-level crate-internal re-export. It does not expose the internal
`boot_snapshot` module, change external visibility, or affect product behavior. Assembly evidence is
`cargo test -p api-server --all-targets --no-run` exit 0 with every unit, bin and integration test
executable produced. The public `compile_extension_boot_snapshot` Composition Root remains a
non-blocking boundary warning: production crates must not consume it as a general SDK.

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
| CIM-AC-004 | complete-snapshot real Router valid/invalid/cookie/fail-closed tests | PENDING REVERIFY |
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
