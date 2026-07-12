---
name: problem-framing
description: 1flowbase 需求对齐与动工前决策 Skill。用于功能、缺陷、交互、重构、规则、文档、架构、数学或算法表达、状态、权限、数据、API contract 或跨前后端需求；先理解现状与真正问题，再以保守 / 平衡 / 激进三个方向帮助用户拍板。确认前不实现。纯查询、机械精确改动，或用户明确要求直接实现时可跳过。
---

# Problem Framing

## Goal

先理解问题，再选择方案。只保留会改变目标、方向、边界或验收的信息。

本 Skill 负责需求对齐，不修改产品代码、测试、migration、schema 或运行时行为。

## Principles

以始为终。先分析，后方案；先推理，后结论。使用第一性原理、奥卡姆剃刀和隐藏因果，表达通俗易懂。

把用户描述视为问题线索，而不是搜索边界。解释关键判断即可，不展示内部思维链，不用方法论、术语或固定步骤替代实际分析。

生成方案时优先复用成熟的数学关系、算法、数据结构、状态机、图、队列、约束、概率或调度机制。只在它们能降低复杂度、提高一致性、改善体验或减少资源成本时采用，不为技术展示增加抽象。

软件设计不是消灭复杂度，而是把必要复杂度收敛到最理解业务语义、最靠近变化源的模块。避免把复杂度扩散为调用方分支、隐式约定、兼容层或无领域语义的抽象。

## Workflow

1. 核对当前请求和直接相关证据，区分事实、假设与未知。
2. 先说明现状，再拆解真正需求及会改变决策的隐藏因果。
3. 完整给出保守、平衡、激进三个可执行方向，再给唯一建议。
4. 停止并等待用户确认；缺少关键信息时集中追问，不凑方案。

普通需求保持简短。复杂度来自问题本身时再展开；每段内容都应帮助判断、行动或验证。

## Output

```markdown
## 现状
...

## 需求分析
真正目标、关键约束与会改变决策的因果。

## 三个方向
### 保守
- 方案内容：...
- 方案收益：收益、代价与主要风险。

### 平衡
- 方案内容：...
- 方案收益：收益、代价与主要风险。

### 激进
- 方案内容：...
- 方案收益：收益、代价与主要风险。

## 最终建议
唯一推荐、关键理由及需要用户确认的点。
```

三个方向必须真实可行且有实质差异；推荐只能出现在三个方向之后。

涉及 UI、UX、页面流程、状态流或复杂逻辑时，用短 ASCII 图补充关键结构或主路径，不做高保真设计。

## Boundaries

- 需求未确认前不进入实现；用户明确要求直接实现时除外。
- 能从代码、文档、issue 或日志确认的事实，不询问用户。
- 只追问会改变方案、contract、数据、权限、用户内容或验收的问题；一次集中提出，并给出推荐默认值及影响。
- 用户方案与证据或项目硬约束冲突时，明确指出冲突、后果和更合理方向。
- 后端是唯一数据来源；前端不承担输出兼容。接口字段保持后端 DTO / 领域语义原名。
- 不把狭窄需求扩展成路线图、平台重设计或清理专项。
- 不在同一轮越过“方向确认 → issue 确认 → 实现”，除非用户明确要求直接推进。

## Progressive Disclosure

只在命中场景时读取对应 reference：

- 需要 discussion brief、issue、ADR 或 implementation handoff：读取 `references/artifacts.md`。
- 判断 Standalone Complete Issue 或 issue 树，以及 grade、level、labels 和生命周期：读取 `references/issue-lifecycle.md`；默认使用单体完整 issue。
- 涉及 defaults、contract、schema、state、permissions、migration、history、runtime behavior 或 user content：读取 `references/domain-matrix.md`。
- 高风险决策需要正式比较或反方评审：读取 `references/options-and-red-team.md`。
- 方案新增公共抽象、接口、flag、通用 helper、重复校验或 pass-through：读取 `../_shared/design-rules.md`。
- 需要查看输出尺度而非复制答案：读取 `references/examples.md`。

方向确认后，按用户要求进入 issue 或实现。实现使用 `frontend-development`、`backend-development` 和 `test-driven-development`；验收与交付使用 `qa-evaluation`。新问题若扩大已确认边界，返回本 Skill。
