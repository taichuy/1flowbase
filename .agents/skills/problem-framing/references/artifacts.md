# Planning Artifacts

只生成当前阶段需要的产物。模板是信息上限，不为完整感填充无价值字段。计划形态只使用 Single Issue 或 Issue Tree。

- [Discussion Brief](#discussion-brief)
- [Single Issue](#single-issue)
- [Issue Tree](#issue-tree)
- [ADR](#adr)
- [Implementation Handoff](#implementation-handoff)

## Discussion Brief

方向尚未收敛且需要保存事实、未知或决策点时使用。

```md
# Discussion Brief

## Current State
- 已确认事实与证据：
- 假设 / 未知：

## Desired Outcome

## Success And Stop Conditions

## Constraints And Authority

## Decisions Needed
```

## Single Issue

普通任务使用一个完整、可直接执行和验收的 issue。

```md
## Issue 元数据
- 计划类型：Single Issue
- 分级：grade:g0/g1/g2/g3/g4
- 任务形态：greenfield / existing-codebase / hybrid-foundation
- 标签：plan:single、grade:*、phase:*、area:*

## 预期结果

## 现状与证据
- 已确认事实：
- 待验证假设：

## 范围、权限与边界
- 范围内：
- 非目标：
- 已授权动作：
- 需要额外确认的动作：

## 方案结论
- 采用方向与关键取舍：
- 复杂度归属：

## 验收点账本
| 编号 | 可观察结果 | 证据 | 结算阶段 |
| --- | --- | --- | --- |
| AC-001 |  |  | 本地 / QA / CI-beta / 用户验收 |

## 验证与预算
- 最小结果证据：
- 延后证据：
- 资源或时间边界：

## 停止与重构条件

## 生命周期
- 当前阶段：
- 关闭条件：
```

## Issue Tree

长计划使用两层结构：一个 Root 和若干 Delivery。Root 是计划、进度和用户验收的唯一真值；Delivery 是可独立集成的纵向结果，不继续拆子 issue。

```text
[Issue Tree Root]
  ├─ [Delivery A：可集成结果]
  ├─ [Delivery B：可集成结果]
  └─ [Delivery C：可集成结果]
```

### Root Issue

```md
## Issue 元数据
- 计划类型：Issue Tree
- 树角色：Root
- 分级：grade:g2/g3/g4
- 任务形态：greenfield / existing-codebase / hybrid-foundation
- 标签：plan:tree、parent-issue、grade:*、phase:*、area:*
- Delivery issues：

## 最终结果

## 现状、证据与关键决策

## 总范围、权限与非目标

## Root 验收点账本
| 编号 | 可观察结果 | 证据 | 负责 Delivery | 状态 |
| --- | --- | --- | --- | --- |
| AC-001 |  |  |  | 未结算 |

## Delivery Map
| Delivery | 纵向结果 | 结算 Root AC | 依赖 | 集成证据 |
| --- | --- | --- | --- | --- |

## Control Ledger
- 当前已集成基线：
- 当前 Delivery、owner 与状态：
- candidate / review / integration 证据：
- Delivery 预算与当前消耗：
- 剩余验收风险：
- 活动 agent、worktree、进程与端口：
- 已知阻塞 / 不确定性 / 外部扰动：
- 下一可控结果：

## 验证、回滚与资源边界

## 停止与重构条件

## 生命周期
- 当前阶段：
- Root 关闭条件：全部 Root AC 有证据，最终 QA 通过，用户验收完成。
```

### Delivery Issue

```md
## Issue 元数据
- 计划类型：Issue Tree
- 树角色：Delivery
- 父 issue：#<root>
- 分级：grade:g1/g2/g3/g4
- 任务形态：greenfield / existing-codebase / hybrid-foundation
- 标签：plan:tree、child-issue、grade:*、phase:*、area:*

## 纵向交付结果

## 结算的 Root AC

## 范围与所有权
- 范围内：
- 非目标：
- 独占模块 / 集成边界：

## 验收证据
| 编号 | 可观察结果 | 证据 |
| --- | --- | --- |

## 执行与验证预算
- 主要开发 owner / worktree：
- 预计时间、影响面与 agent 上限：
- 最小本地证据与延后门禁：
- candidate review 层级：

## 停止与上报条件

## 完成条件
- 结果已进入 Root 的集成基线，Root 账本已更新，证据可复核。
```

## Artifact Rules

- 已确认事实带来源；假设保持可挑战。
- AC 描述用户或系统可观察结果；测试、构建和 hygiene 是证据，不是需求本身。
- Delivery 必须产生纵向、可集成结果并减少 Root 验收风险；类型、mapper、migration、测试或评论等实现步骤不单独建 issue。
- Root 获批后即授权执行正文列出的 Delivery；Delivery 不重复等待用户批准。扩大 Root 边界时返回 `problem-framing`。
- 一个计划只有一个在线真值。重构计划时替换旧结构并关闭 superseded 节点，只保留证据链接，不并行维护两套计划。

## ADR

只用于不可逆架构、核心 contract、source of truth、数据所有权或长期边界决策。ADR 是 Root / Single Issue 的证据，不作为额外计划层。

```md
# ADR: <title>

## Status
Proposed

## Context

## Decision

## Rationale And Mechanism

## Alternatives Rejected

## Risks And Reversibility

## Evidence
```

## Implementation Handoff

只传递当前交付所需的最小稳定上下文，不复制完整 issue、diff 或执行过程。

```md
# Implementation Handoff

## Outcome

## Scope And Authority

## Acceptance Evidence

## Integrated Baseline And Owned Areas

## Budget And Evidence Tier

## Decisions And Gotchas

## Stop Or Escalate If
```
