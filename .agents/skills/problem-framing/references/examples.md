# Examples

## Vague Feature Request

用户说：“优化一下 Settings 页面流程。”

回复形态：

```md
现状
- 这个请求会改变产品流程，但还没有定义主操作和成功标准。
- Settings 已有共享 surface 和紧凑运营型 UI 的项目规则。

方向
- 先收敛目标、受影响的 settings 区域、用户主路径和成功标准。

风险收益
- 收益：避免重设计无关 settings surface。
- 风险：跳过收敛会让 UI 改动未经批准地改变导航或状态归属。

建议
- 建议先产出短 discussion brief，用户确认后再交给 `frontend-development`。
```

## Ordinary Requirement

用户说：“给列表加一个按更新时间排序。”

回复形态：

```md
现状
- 这是一个普通需求，但仍会影响列表默认行为和用户预期。
- 当前还没确认是默认排序、可选排序，还是只在某个页面生效。

方向
- 做法 A：只在目标列表增加可选排序。
- 做法 B：把更新时间作为目标列表默认排序。
- 做法 C：抽成通用列表排序能力，供同类列表复用。

风险收益
- 收益：实现范围小，验证清晰。
- 风险：如果不确认作用范围，可能误改全局列表行为。

建议
- 建议选做法 A：只在目标列表增加可选排序，不改默认排序。需要你确认作用页面和是否改变默认行为。
```

## Cross-Domain Architecture Request

用户说：“统一 defaults，修掉 frontend/backend 漂移。”

回复形态：

```md
现状
- 这会影响前端 fallback、后端 defaults、运行时 contracts、已落库数据和可能的 migration 行为。
- 即使都被叫作 defaults，它们也不是同一个概念。

方向
- 在设计 API 或 migration 前，先建立 domain matrix。
- 对比 conservative / balanced / aggressive 三个方案。

风险收益
- 收益：保护用户历史数据和运行时 contracts。
- 风险：扩大清理范围可能引入 silent behavior change。

建议
- 建议在矩阵证明 source of truth 和历史影响后，再选择 balanced 方案。
```

## Issue Hierarchy Request

用户说：“这个需求要拆 parent / child issue。”

回复形态：

```md
现状
- 这个需求需要先区分结构层级和风险分级。
- `level:*` 表示 issue 在树上的位置，`grade:*` 表示风险和规划强度。

方向
- 默认：先用 Standalone Complete Issue 承载目标、范围、取舍、验收和执行边界。
- 升级：只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才拆 issue 树。
- L0：项目级总问题，挂多个 L1 决策 issue。
- L1：架构决策 / ADR，挂多个 L2 workstream。
- L2：backend / frontend / QA 等工作流，挂多个 L3 执行任务。
- L3：单个可执行任务，不再继续拆 child；Standalone 和 L3 允许 agent 在确认后进入实现。

风险收益
- 收益：父子关系可检索，验收路径清晰。
- 收益：提前拆 L3 能限制 AI 的实现边界，避免过度抽象、范围扩张和耗时失控。
- 收益：L0 只整理事实，L1 只做决策，L2 只拆工作流，L3 才执行，能避免 planning 和 implementation 混在一起。
- 风险：小需求默认拆树会增加 issue 噪音，并让 L3 缺少完整需求上下文。
- 风险：跨层挂载会让 parent/child 语义混乱；让 L2 直接实现会把 workstream 变成大包任务。

建议
- 建议默认创建 Standalone Complete Issue，并打 `level:standalone` 和对应 `grade:*`。
- 如果用户批准拆树，再给每个 issue 同时打 `level:*` 和 `grade:*`，并只挂直接下一层 child issue；进入实现前，先把 L2 拆成 L3。
- AI 可以在 L0 整理事实和冲突，在 L1 提供方案和风险，在 L2 拆依赖和顺序，在 L3 写代码、测试和验证；AI 不应该自己批准 L1，也不应该在 L3 改架构边界。
```
