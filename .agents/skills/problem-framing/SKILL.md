---
name: problem-framing
description: 1flowbase 需求对齐与动工前决策 Skill。用于功能、缺陷、交互、重构、规则、文档、架构、数学或算法表达、状态、权限、数据、API contract 或跨前后端需求；从证据中收敛目标、成功标准、边界、验证和停止条件，以保守 / 平衡 / 激进三个方向帮助用户拍板，并在确认后选择 Single Issue（普通任务）或两层 Issue Tree（长计划）。确认前不实现；纯查询、机械精确改动或用户明确要求直接实现时可跳过。
---

# Problem Framing

## Outcome

把请求收敛成可决策、可执行、可验收的结果定义，而不是替实现者规定推理步骤。

完成需求对齐意味着：事实与未知可区分，目标和非目标清楚，复杂度归属合理，成功标准可观察，验证与权限边界明确，并有唯一建议及停止条件。

本 Skill 只对齐需求，不修改产品代码、测试、migration、schema 或运行时行为。

## Decision Frame

- 从代码、文档、issue、日志或运行结果确认现状；用户描述是问题线索，不是证据边界。
- 先定义理想结果和“完成”的可观察含义，再分析隐藏因果、约束、失败模式与未知。
- 区分真正不变量、授权边界和工作偏好；绝对词只用于安全、数据、权限或已确认 contract 等硬边界。
- 把必要复杂度放在最理解语义、最靠近变化源的 owner；接口保持窄而深，不向调用方扩散分支和隐式约定。
- 方案优先复用能降低复杂度或风险的成熟算法、状态机、约束、数据结构或调度机制；不为展示技术增加抽象。
- 每条信息只表达一次。保留目标、证据、关键取舍、成功标准、验证、权限和停止条件，删去重复流程、风格要求和无关背景。

## Response

保持以下顺序；普通需求可以压缩内容，但不合并或省略“需求分析”和“最终建议”。

```markdown
## 现状
已确认事实、证据，以及会影响决策的未知。

## 需求分析
理想结果、成功标准、关键约束、隐藏因果与复杂度归属。

## 三个方向（升温发散）
### 保守
- 方案内容：...
- 综合收益：收益、代价与主要失败模式。

### 平衡
- 方案内容：...
- 综合收益：收益、代价与主要失败模式。

### 激进
- 方案内容：...
- 综合收益：收益、代价与主要失败模式。

## 最终建议（降温收敛）
唯一推荐、关键理由、成功与停止口径，以及需要用户确认的事项。
```

三个方向解决同一个目标，并在范围、复杂度归属或风险偏好上存在真实差异。普通需求保持短；复杂度来自问题本身时再展开。涉及 UI、UX、状态或复杂关系时，用短 ASCII 图表达主路径。

## Decision Gate

- 用户尚未确认方向时停止，不进入实现。
- 缺少会改变 source of truth、contract、数据、权限、用户内容或验收的关键信息时，集中追问并给推荐默认值。
- 用户明确要求直接实现时，可以跳过 issue 确认；仍需把成功标准、权限、验证和停止条件交给实现 Skill。
- 方向确认后，计划只使用两种形态：普通任务使用 Single Issue；长计划使用两层 Issue Tree。读取 `references/issue-lifecycle.md`。
- 新问题扩大已确认目标、数据影响、权限或 contract 时，停止实现并回到本 Skill；局部实现选择不重复请求批准。

## Boundaries

- 能从现有证据确认的事实不询问用户。
- 不把狭窄需求扩张成路线图、平台重设计或无关清理。
- 后端是 contract 与状态唯一数据来源；前端不承担输出兼容，接口字段保持后端 DTO / 领域语义原名。
- 用户方案与证据或硬约束冲突时，说明冲突、后果和更小的可行方向。

## Progressive Disclosure

- 需要 issue、Issue Tree、ADR、discussion brief 或 implementation handoff：读取 `references/artifacts.md`。
- 选择 Single Issue / Issue Tree、grade、labels、批准和关闭语义：读取 `references/issue-lifecycle.md`。
- 长计划、多 agent、跨上下文或需要持续集成控制：读取 `references/long-running-work.md`。
- 涉及 defaults、contract、schema、state、permissions、migration、history、runtime behavior 或 user content：读取 `references/domain-matrix.md`。
- 高风险方向比较或反方评审：读取 `references/options-and-red-team.md`。
- 新增公共抽象、接口、flag、通用 helper、重复校验或 pass-through：读取 `../_shared/design-rules.md`。
- 只需校准输出尺度：读取 `references/examples.md`。

方向和计划获批后，使用对应 implementation Skill 与 `test-driven-development`；验收和交付使用 `qa-evaluation`。
