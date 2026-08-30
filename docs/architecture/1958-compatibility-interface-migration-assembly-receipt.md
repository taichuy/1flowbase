# #1958 Compatibility Interface Migration Assembly Receipt

## Candidate status

- Delivery: [#1958](https://github.com/taichuy/1flowbase/issues/1958)
- Root: [#1893](https://github.com/taichuy/1flowbase/issues/1893)
- Architecture baseline: [#1944](https://github.com/taichuy/1flowbase/issues/1944)
- Input: `beta@c2bd2c58f7e3e90bdb110b6fc0245c690ed3fbb4`
- Product assembly before this receipt: `beta@b6ac2ffa8480f8345298fb44b71c6febf03fab07`
- Final assembly: CI-M07 receipt commit；集中 QA 必须用 `git rev-parse HEAD` 自动捕获完整 SHA
- Official plugins baseline: `main@8bf11605b02a0df8dd01271875f1ec3d182c0d3a`
- Result: `ASSEMBLED / QA_PENDING`
- Evidence root: `tmp/test-governance/1958-compatibility-interface-migration/`

本 Receipt 只冻结开发 assembly 的内容与验收矩阵，不提前声明 CIM AC 或 QA 通过。唯一 fresh
centralized QA 必须对 CI-M07 后不变的完整 HEAD 执行；failed 或 unrun 非零时结果只能是
`QA_FAIL`。

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

Packet 阶段只执行 `cargo fmt`、`cargo check -p api-server` 和
`cargo check -p api-server --tests` 作为安全装配；没有运行 Packet 测试、reviewer 或 QA。

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
