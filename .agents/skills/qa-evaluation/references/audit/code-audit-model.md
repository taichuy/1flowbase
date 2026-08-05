# Evidence-Driven Code Audit Model

## Goal

把代码审计限制为可复查的 QA finding：按当前范围加载必要参考卡，收集候选绑定证据，排除合法反例，再由 Root 统一严重级别。审计默认只报告，不直接修改、删除或重构代码资产。

## Invariants

```text
Finding = Rule ∧ Evidence ∧ Impact ∧ LegalNegativeChecked
```

- 没有直接证据时写 `Unverified / 未验证`，不下确定结论。
- 静态相似、命名直觉、文件行数或“存在更高级方案”只能产生调查候选。
- 当前任务只把本次引入的问题作为 blocker；既有债默认 warning，除非 Issue 明确纳入。
- candidate identity、artifact freshness、环境和未运行证据属于结论的一部分。
- QA 只拥有证据聚合与分级，不拥有产品语义、schema、状态、权限或用户内容。

### Routing

| 信号 | 加载 |
| --- | --- |
| 数据库、索引、query plan、capacity、JSONB、retention、ephemeral | `database-query-ephemeral.md` |
| 算法、数据结构、状态机、并发、幂等、语义重复 | `algorithms-state-concurrency.md` |
| 日志、trace、旁路、correlation、live/durable seam | `observability-log-pipeline.md` |
| 测试生命周期、短命测试、harness、合并或删除候选 | `test-asset-lifecycle.md` |
| AI Gateway、MCP Gateway、Application Backend、Native React / 低代码 | `foundation-audit-cards.md` |
| helper / manager / wrapper、死代码、过度抽象 | 复用 `../governance/maintainability-dead-abstraction.md` |

只加载命中信号的卡片；完整项目审计也先建风险矩阵，再选卡片，不默认把全部 reference 注入上下文。

## Evidence

按强度从高到低使用：

1. 候选绑定的运行态、数据库、浏览器、CI artifact 或失败 fixture。
2. 真实 route/service/repository/consumer 的定向测试、状态与契约证据。
3. 调用点、注册表、DTO/schema、查询与历史 churn 的源码证据。
4. 静态启发式候选；不能单独支持 blocker。

每个 finding 记录位置、Rule、Evidence、Impact、Legal negative、Severity、Candidate identity、Artifact freshness 与 Unverified 限制。

### Subagent Protocol

- 只有审计域彼此独立且并行能降低上下文污染时，启动 1～3 个只读 subagent。
- 推荐切片：数据/查询/性能；契约/状态/算法；测试/重复/可观测性。
- subagent 只返回 evidence-backed finding、合法反例、未验证项和资源实耗，不修改文件、Issue 或远端，不得嵌套调度。
- Root 负责去重、交叉验证、严重级别、范围与最终报告；subagent verdict 不是最终 QA 结论。

## Legal Negatives

- 单调用方层承担事务、权限、错误映射或外部协议边界时，不是空转抽象。
- 标题或代码形状相同但 owner、invariant、scope 或失败模式不同，不是语义重复。
- 未运行重型证据且已按 lane 延后时，不把“未运行”误报为失败。
- warningFiles 非空但 component 通过时，保持 advisory，不自动失败。

## Severity

- `Blocking/High`：必须有当前范围内可复现的契约、安全、数据、状态、权限或证据真实性破坏。
- `Medium/Low warning`：已有影响证据，但不阻断当前结果，或属于既有治理债。
- `Advisory`：替代机制、容量规划或维护机会，尚无当前失败证据。
- `Unverified`：证据身份、环境、运行路径或合法反例无法确认。

严重级别仍以 `../governance/severity-rules.md` 为真值；本卡只增加审计证据门槛。

## Resource Boundary

- Dev Acceptance 只审计改动路径与直接传播；PR 使用既有 component evidence；Project Health 才轮转深挖。
- 不因代码审计自动运行全仓 Cargo、coverage、load/soak、生产查询或浏览器全回归。
- 所有生成型审计产物写入 `tmp/test-governance/`；单次完整审计低于 1 小时。
- 审计发现需要修复时，先报告；产品、schema、索引、状态或删除动作进入独立 Issue。

## Stop Conditions

- 规则只能依赖关键词、文本相似、命名印象或主观“最佳实践”。
- candidate、artifact freshness、环境或真实消费者无法确认。
- 审计要求修改产品语义、用户内容、schema、索引、状态、权限或自动删除资产。
- 新 reference 开始复制现有专项真值，而不是路由和补充缺口。
- 继续取证不再改变结论，或会突破 lane / 1 小时资源边界。
