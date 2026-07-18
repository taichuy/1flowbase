---
name: problem-framing
description: 1flowbase 需求对齐与动工前决策 Skill。用于功能、缺陷、交互、重构、规则、文档、架构、数学或算法表达、状态、权限、数据、API contract 或跨前后端需求；从证据中收敛目标、成功标准、边界、验证和停止条件，以保守 / 平衡 / 激进三个方向帮助用户拍板，并在确认后选择 Single Issue（普通任务）或两层 Issue Tree（长计划）。确认前不实现；纯查询、机械精确改动或用户明确要求直接实现时可跳过。
---

# Problem Framing

## Outcome

把请求收敛成可决策、可执行、可验收的结果，不替实现者规定完整路径。完成时，现状有证据，结果可观察，范围、owner 与授权闭合，验证足以结算风险，并有唯一建议和停止条件。

本 Skill 只形成决策，不修改产品代码、测试、migration、schema 或运行时行为。

## Reasoning Catalysts

`先推理后结论`：先定义理想结果，再用`第一性原理`拆出事实、隐藏因果、硬约束与失败模式；用`奥卡姆剃刀`选择足以解释证据的最小机制；先`升温发散`真实方向，再`降温收敛`唯一建议，最终`通俗易懂`但不牺牲准确性。

优先用高信息关系、反例或最小案例催化，不堆模型已知常识和同义说明。催化词只改变搜索方向，不替代证据、领域精度或硬边界，也不作为口号复述。

## Architecture Catalysts

- `Deep Modules / Information Hiding`：公共接口只暴露调用方决策所需的最小充分信息；状态判断、协议细节与兼容分支留在内部。
- `Conservation of Complexity + Requisite Variety`（`Tesler's Law` / `Ashby's Law`）：必要复杂度不能消失；由拥有足够状态与动作空间的语义 owner 吸收。
- `Observability × Controllability ⇒ Ownership`：看不见相关状态或不能控制其转移的模块，不拥有该复杂度。
- `Proven Mechanisms over Ad-hoc Rules`：优先成熟数学关系、算法、数据结构、状态机、约束与调度机制，不用临时规则堆叠代替。

用以下 complexity placement heuristic 选择 owner；这是本 Skill 的架构判定式，不是经典控制论原公式：

```text
owner*(x) =
  argmin_m [C_leak(m) + C_coordination(m) + C_failure(m)]

subject to:
  SourceOfTruth_m(x)
  ∧ Observable_m(x)
  ∧ Controllable_m(x)
  ∧ Variety_m ≥ Variety_x
```

`C_leak` 是泄漏给调用方的兼容、分支与隐式约定；`C_coordination` 是跨 owner 协调成本；`C_failure` 是复杂度错置造成的失败成本。

## Decision Field

把请求看作受约束决策；用关系筛选信息，不在回答中机械复述字段：

```text
证据 -> 现状 -> 与目标的差距 -> 可观察成功标准
source of truth / owner -> 必要复杂度
授权 / contract -> 可行方向
失败风险 -> 验证强度
潜在决策变化 × 影响 > 获取成本 -> 新证据
```

维持以下守恒关系：

- 用户描述提供线索，结论强度不超过证据；安全、数据、权限与已确认 contract 是硬边界，工作偏好只改变方向权重。
- 方案范围不超过授权与非目标；新增范围同时产生成功标准、owner、证据和预算债务。
- 只处理会改变可行域或推荐的未知；其他缺口使用有界假设。下一步不能减少决策残差时停止。
- 后端是 contract 与状态唯一数据来源；前端不承担输出兼容，接口字段保持后端 DTO / 领域语义原名。

## Control Loop

```text
[证据与结果差距] -> [三个真实方向] -> [唯一建议] -> [用户决策]
       ^                                      |
       +---- 边界、语义或授权发生变化 --------+
```

- 已有事实足以判定可行域、约束冲突或推荐时直接收敛；否则只获取可能改变结论的最小证据，能查明的事实不询问用户。
- 三个方向解决同一目标，在范围、复杂度归属或风险偏好上有真实差异；缺少关键事实时集中追问并给推荐默认值。
- 证据与用户方案冲突时说明后果和更小可行方向；狭窄需求不扩张，每条信息只表达一次。

## Response Contract

输出顺序固定为：`现状` → `需求分析` → `三个方向（升温发散：保守 / 平衡 / 激进）` → `最终建议（降温收敛）`。

- `现状`：已确认事实、证据和会改变决策的未知。
- `需求分析`：理想结果、可观察成功标准、关键约束、隐藏因果与复杂度归属。
- 每个方向只写 `方案内容` 与 `综合收益`；综合收益同时包含代价和主要失败模式，不用不可执行方向凑数。
- `最终建议`：唯一推荐、理由、成功与停止口径，以及需要确认的事项。
- 普通需求保持短，复杂问题才展开；UI、UX、状态或复杂关系用短 ASCII 图表达主路径。

## Decision Gate

- 用户未确认方向时停止，不进入实现；用户明确要求直接实现时可跳过 issue 确认，但必须把结果、成功标准、权限、验证与停止条件交给 implementation Skill。
- 方向确认后只选择 Single Issue 或两层 Issue Tree；读取 `references/issue-lifecycle.md`。长计划还需满足 `references/long-running-work.md` 的 Delivery readiness。
- 新问题改变目标、source of truth、contract、数据影响、权限、用户内容或成功标准时，回到本 Skill；既定边界内的局部实现选择不重复请求批准。

## Reference Routes

| 信号 | 读取 |
| --- | --- |
| issue、Issue Tree、ADR、discussion brief、handoff | `references/artifacts.md` |
| 计划形态、grade、labels、批准与关闭 | `references/issue-lifecycle.md` |
| 长计划、多 agent、跨上下文或持续集成控制 | `references/long-running-work.md` |
| defaults、contract、schema、state、permissions、migration、history、runtime behavior、user content | `references/domain-matrix.md` |
| 高风险方向比较或反方评审 | `references/options-and-red-team.md` |
| 新公共抽象、接口、flag、通用 helper、重复校验或 pass-through | `../_shared/design-rules.md` |
| 只需校准输出尺度 | `references/examples.md` |

获批后使用对应 implementation Skill 与 `test-driven-development`；验收和交付使用 `qa-evaluation`。
