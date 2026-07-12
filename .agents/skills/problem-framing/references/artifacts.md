# Planning Artifacts

只生成当前阶段需要的产物。模板是结构上限，不要求为空字段填充无价值内容。

## Discussion Brief

用于方向尚未收敛，但事实、范围或决策点需要单独保存的复杂需求。普通对齐直接使用 `SKILL.md` 输出。

```md
# Discussion Brief

## Current State
- 已确认事实与证据：
- 假设 / 未知：

## Goal And Non-Goals

## Constraints And Invariants

## Decisions Needed

## Evidence Needed
```

## Issue Draft

Issue 字段和顺序以本节为唯一模板；层级、分级、标签和生命周期读取 `issue-lifecycle.md`。

```md
## Issue 元数据
- 标题：[状态]标题
- Issue 类型：Standalone Complete Issue | Issue Tree Node
- 层级：level:standalone | level:l0/l1/l2/l3
- 分级：grade:g0/g1/g2/g3/g4
- 标签：
- 父 issue：
- 子 issue：

## 需求复述

## 目标与非目标
- 目标：
- 非目标：

## 事实、假设与不变量
- 已确认事实及证据：
- 待验证假设：
- 不可协商不变量：

## 方案结论
- 采用方向与关键取舍：
- 不采用方向及原因：

## 任务形态
- greenfield | existing-codebase | hybrid-foundation
- 旧债策略：

## 范围与执行边界
- 范围内：
- 范围外：
- 主要模块：
- 停止 / 升级条件：

## 验收点账本
| 编号 | 类型 | 可观察结果 | 证据 | 结算阶段 |
| --- | --- | --- | --- | --- |
| AC-001 | 新需求 / 回归断言 |  |  | 本地 / QA / CI-beta / 用户验收 |

## 验证与预算
- 验证方式：
- 延后到 QA / CI 的证据：
- 复杂度与验证预算：

## 生命周期
- 当前阶段：
- 关闭条件：
```

Rules:

- `已确认事实` 带来源；`待验证假设` 保持可被挑战。
- Standalone 与 L3 必须有明确执行边界；L0/L1/L2 不直接进入实现。
- 默认使用 Standalone Complete Issue、`level:standalone`，父子 issue 写 `无`。
- issue 树只记录直接父子关系，不跨层挂载。
- AC 点是需求结算口径；构建、测试和 hygiene 只提供证据。
- 只保留真实决策和边界，不把实现杂项包装成待用户拍板事项。

## ADR Draft

只用于不可逆架构、核心 contract、source of truth、数据所有权或长期边界决策。

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

用户已确认方向或 issue，且需要把最小上下文交给实现 Skill 时使用。它不是完整 implementation plan。

```md
# Implementation Handoff

## Approved Direction

## Scope
- In scope:
- Out of scope:

## Constraints And Acceptance Points

## Files / Areas To Inspect First

## Verification Evidence

## Context Capsule
- Built / changed:
- Lives in:
- Decisions / gotchas:
- Extend from:

## Stop / Escalate If
```

Handoff 只引用已确认边界。context capsule 保存位置、决策、风险和扩展入口，不复制代码或完整 diff；实现中出现新决策时回到 `problem-framing`。
