# Foundation Contract Gates

## Goal

在既有 `Dev Acceptance / PR Merge / Project Health` lane 之上增加四基座契约轴，让通用 frontend/backend green 不再冒充具体基座已结算：

- `AI Gateway`
- `MCP Gateway`
- `Application Backend`
- `Native React frontend blocks`

路由真值由 `scripts/node/foundation-contracts/core.js` 拥有；workflow 只调度 fast/full pack，不重新定义产品语义。

## Contract Boundaries

| 基座 | Fast evidence 主要拦截 | Full evidence 延后项 |
| --- | --- | --- |
| AI Gateway | 协议投影、状态协议和主仓 workflow contract 漂移 | Provider 实包、并发、完整 transport、paired SHA |
| MCP Gateway | `mcp_list -> mcp_get -> mcp_call`、ACL、mapping 与调用契约漂移 | Bundle、上游 MCP、存储与完整大结果矩阵 |
| Application Backend | Data Model 定义、runtime API、scope/ACL 的快速契约漂移 | migration/reconcile、物理 schema、metadata preservation、coverage |
| Native React | 标准 Component、compiler/runtime ABI、dependency lock、capability、ShadowRoot 与 host ABI | 全页面、浏览器、移动端、缓存和视觉回归 |

`mcp_result` 是大结果或 durable receipt 的内部续取证据，不属于核心三入口；只有 result delivery / receipt 风险被命中时才追加 continuation pack。

`frontstage-governance-hygiene` 只结算页面树、可见性、存储约束与 settings registry 边界；它不结算 Native React Component、compiler/runtime ABI、dependency lock、capability guard 或 runtime conformance。

## Receipt Contract

统一 receipt 位于 `tmp/test-governance/foundation-contracts/`，至少包含：candidate SHA、lane、event、changed-file trigger、被选择的基座、组合缝隙、required / executed pack、各命令的日志路径与退出码、Cargo / Vitest / Node 已执行 passing / failed 用例数、status、exit code、warning、error、未覆盖项和延后证据。

`warning` 和非空 `warningFiles` 保持可见但不改变 passed；只有显式 failed component、非零 exit code、error/blocker 或缺失 / 重复的已选择 component receipt、未执行完整 required pack、重复命令 ID 或 测试零用例才失败；不得用计划回填缺失的执行证据。

## Resource Boundary

- 本地默认只运行受影响 fast pack；完整 provider/browser/migration/coverage 留 CI/nightly。
- PR / `beta` 的统一 `verify` 质量门禁调用 fast component；nightly/manual 的统一 `quality gate` 调用四基座并纳入 aggregate。可复用 workflow 不建立独立 PR/push 门禁，最长执行路径必须低于 60 分钟。
- 完整体检先在线运行全部门禁；剩余持续线上失败的门禁不超过 3 项时，才转本地定向定位，再用 GitHub Actions 验证修复。其他 gate 不随局部修复无条件重跑；最终 aggregate 使用同一冻结 candidate 的完整证据。
- 管理员保留手动合并判断；不得把本规则扩张为 required check、branch protection 或 ruleset 变更。

## Deterministic Evidence And Legal Negatives

路由 fixture 必须覆盖四个基座正例，以及 docs-only、locale-only、无关 CSS 等合法反例。共享 `interface-runtime` / `extension-contracts` 的 Rust 源码、测试与 Cargo manifest，以及 `runtime-core` 的 `runtime_backend` Port 源码与对应测试，必须选择三个后端基座；Native React 不因此触发。crate 规则文档和无关 runtime 模块保持未选择。规则变更还必须证明：

- 把 `mcp_result` 放进核心三入口会失败；
- warning-only receipt 仍 passed，error/blocker 才 failed；
- receipt 缺 candidate SHA 或 selected component、required command 缺项 / 重复、测试零用例会失败；合法未选基座无需 receipt；
- AI full workflow 不恢复成 every-PR 90-minute gate。

## Stop Conditions

- 需要改变产品 API、DTO、数据库、migration、状态、权限、runtime、用户内容或 UI；
- 需要修改 required check、branch protection、ruleset 或管理员合并权限；
- 无法形成有限 pack，只能继续叠加全仓测试；
- 新规则没有确定性反例或只能依赖主观源码判断。
