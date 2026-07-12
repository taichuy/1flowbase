# Issue Lifecycle

只在用户需要开发计划、issue 草案、层级、分级、标签或生命周期时读取。需求对齐格式以 `SKILL.md` 为准，本文件不重复定义。

## Default Shape

默认使用 **Standalone Complete Issue**：一个 issue 同时承载目标、取舍、范围、验收点、执行边界和关闭条件。

只有以下情况升级为 issue 树：

- 用户明确要求 parent / child issue。
- 已存在必须遵守的 parent issue。
- 单体 issue 无法安全承载多个独立决策或 workstream。

升级会增加层级、同步和验收成本；提出树形结构时必须说明为什么单体 issue 不够。

## Grades

`grade:*` 表示风险和规划强度，不表示父子结构。

| Grade | Use When | Required Planning Evidence |
| --- | --- | --- |
| `grade:g0` | 纯查询、机械精确改动或用户明确直接实现 | 最终说明跳过原因 |
| `grade:g1` | 单点、低风险需求，无数据或 contract 风险 | 已确认方向、范围和定向验收 |
| `grade:g2` | 子系统行为变化，需要测试或 QA | 完整 issue、AC 点和验证预算 |
| `grade:g3` | 跨前后端、状态、权限、schema 或 runtime contract | 三方向、边界证据、完整 issue |
| `grade:g4` | 历史数据、migration、用户内容、核心 contract 或不可逆决策 | Domain Matrix、red-team、按需 ADR 和 rollback / preview 证据 |

选择能覆盖真实风险的最低 grade，不为显得完整升级。

## Levels

`level:*` 表示 issue 的结构位置。

| Level | Owns | May Enter Implementation |
| --- | --- | --- |
| `level:standalone` | 完整需求上下文、决策、验收与执行边界 | 用户确认后可以 |
| `level:l0` | 总目标、事实、冲突、总范围与总验收 | 不可以 |
| `level:l1` | 单个架构、contract 或 source-of-truth 决策 | 不可以 |
| `level:l2` | 单个 workstream、依赖与交付顺序 | 不可以 |
| `level:l3` | 单一可执行任务、局部验收与停止条件 | 用户确认后可以 |

树只允许 `L0 → L1 → L2 → L3` 的直接父子关系。每层可有多个 sibling；L3 不继续拆 child。实现阶段不得用 L3 修改已批准的 L1 决策或 L2 边界。

## Task Archetype

issue 正文必须标明任务形态，用于确定验收与旧债策略：

| Archetype | Use When | Acceptance / Debt Bias |
| --- | --- | --- |
| `greenfield` | 新能力、新模块或空白子系统 | 验证基础可运行、入口和最小回归；基础缺口可作 blocker |
| `existing-codebase` | 既有系统增量修改 | 只阻断本次引入问题；既有债务默认 warning 或后续 issue |
| `hybrid-foundation` | 既有系统内新增承载后续功能的 foundation | 先验证稳定入口与扩展边界；后续功能独立结算 |

## Labels

每个 issue 使用一个 `level:*`、一个 `grade:*` 和一个 `phase:*`。按实际范围增加 `area:*`；子 issue 增加 `child-issue`。

推荐阶段：

- `phase:proposed`
- `phase:approved`
- `phase:in-progress`
- `phase:qa`
- `phase:user-acceptance`
- `phase:blocked`
- `phase:done`

标题使用 `[状态]标题`，状态与 `phase:*` 同步。GitHub issue 标题和正文默认中文；labels、代码标识符、API 路径、文件路径和命令保持原文。

## Lifecycle

```text
proposed -> approved -> in-progress -> qa -> user-acceptance -> done
                         \-> blocked -> approved / in-progress
```

- AI 可以起草、实施和提供证据，但不能替用户批准关键决策或用户验收。
- 方向确认只授权创建 issue；issue 确认后才授权实现，除非用户明确跳过 issue。
- `phase:done` 需要 AC 点结算和用户验收；测试或构建通过不能替代产品验收。
- 新问题扩大目标、contract、数据影响或 parent 边界时，回到 `problem-framing`，不要在执行 issue 内隐式吸收。

## Acceptance Ledger

使用稳定编号 `AC-001`、`AC-002`。每个 AC 点写清：

- 可观察结果。
- 证据来源。
- 结算阶段：本地、QA、CI / beta 或用户验收。

后续修改只描述 delta；旧 AC 点保留为回归断言。机械质量门禁只能作为证据，不能替代 AC 点结论。
