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
| MCP Gateway | `mcp.list -> mcp.get -> mcp.call`、ACL、mapping 与调用契约漂移 | Bundle、上游 MCP、存储与完整大结果矩阵 |
| Application Backend | Data Model 定义、runtime API、scope/ACL 的快速契约漂移 | migration/reconcile、物理 schema、metadata preservation、coverage |
| Native React | 标准 Component、compiler/runtime ABI、dependency lock、capability、ShadowRoot 与 host ABI | 全页面、浏览器、移动端、缓存和视觉回归 |

`mcp.result` 是大结果或 durable receipt 的内部续取证据，不属于核心三入口；只有 result delivery / receipt 风险被命中时才追加 continuation pack。

`frontstage-governance-hygiene` 只结算页面树、可见性、存储约束与 settings registry 边界；它不结算 Native React Component、compiler/runtime ABI、dependency lock、capability guard 或 runtime conformance。

## Receipt Contract

统一 receipt 位于 `tmp/test-governance/foundation-contracts/`，至少包含：candidate SHA、lane、event、changed-file trigger、被选择的基座、组合缝隙、执行 pack、status、exit code、warning、error、未覆盖项和延后证据。

`warning` 和非空 `warningFiles` 保持可见但不改变 passed；只有显式 failed component、非零 exit code、error/blocker 或缺失的已选择 component receipt 才失败。

## Resource Boundary

- 本地默认只运行受影响 fast pack；完整 provider/browser/migration/coverage 留 CI/nightly。
- PR / `beta` fast workflow 的最长执行路径必须低于 60 分钟。
- 少于 3 个基座反复失败时：本地单基座 → GitHub Actions 单基座 → `auto/all` 全量；不得用重复全量运行代替根因定位。
- 管理员保留手动合并判断；不得把本规则扩张为 required check、branch protection 或 ruleset 变更。

## Deterministic Evidence And Legal Negatives

路由 fixture 必须覆盖四个基座正例，以及 docs-only、locale-only、无关 CSS 等合法反例。规则变更还必须证明：

- 把 `mcp.result` 放进核心三入口会失败；
- warning-only receipt 仍 passed，error/blocker 才 failed；
- receipt 缺 candidate SHA 或 selected component 会失败；
- AI full workflow 不恢复成 every-PR 90-minute gate。

## Stop Conditions

- 需要改变产品 API、DTO、数据库、migration、状态、权限、runtime、用户内容或 UI；
- 需要修改 required check、branch protection、ruleset 或管理员合并权限；
- 无法形成有限 pack，只能继续叠加全仓测试；
- 新规则没有确定性反例或只能依赖主观源码判断。
