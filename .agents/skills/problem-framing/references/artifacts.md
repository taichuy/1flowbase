# Planning Artifacts

只使用当前请求需要的产物。产物要短到用户能一轮审完并拍板。

## Discussion Brief

```md
# Discussion Brief

## Current State
- 已确认事实：
- 假设：
- 未知点：

## Goal

## Scope
- 范围内：
- 范围外：

## Success Criteria

## Invariants

## Risks And Failure Modes

## Decisions Needed
```

## Issue Draft

```md
## Issue 元数据
- 标题：[状态]标题
- Issue 类型：Standalone Complete Issue | Issue Tree Node
- 层级：level:standalone | level:l0/l1/l2/l3
- 分级：
- 标签：
- 父 issue：
- 子 issue：

## 需求复述

## 目标
- 本次要达成什么：

## 非目标
- 本次明确不做什么：

## 已确认事实

## 不可协商不变量

## 待验证假设

## 待决策事项

## 方案结论
- 采用方向：
- 关键取舍：

## 不采用方案

## 范围边界
- 范围内：
- 范围外：

## 验收证据
- 用户可见结果：
- 测试 / 命令 / 日志 / 截图 / 接口响应：
- 延后到 QA 或 CI 的证据：

## 执行边界
- 主要文件 / 模块：
- 不允许扩大到：
- 停止 / 升级条件：

## 预算
- 复杂度预算：
- 验证预算：
- 追问预算：

## 生命周期
- 当前阶段：
- 关闭条件：
```

Rules:

- GitHub issue 标题和正文默认中文；labels、代码标识符、API 路径、文件路径和命令保持原文。
- `已确认事实` 必须带证据来源。
- `待验证假设` 必须保持可被挑战，不能写成已决设计。
- `待决策事项` 必须是用户需要拍板的真实决策，不是实现杂项。
- `Issue 元数据` 必须按 `references/issue-lifecycle.md` 填写层级、分级和标签。
- 默认使用 `Issue 类型：Standalone Complete Issue` 和 `level:standalone`；`父 issue`、`子 issue` 写 `无`。
- 只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才输出 `Issue Tree Node` 草案。
- issue 树中 `父 issue` 必须指向上一层 issue；`子 issue` 只列直接下一层 issue。
- L0 记录事实、冲突和总清单；L1 记录用户批准的决策；L2 记录工作流和依赖顺序；L3 记录单一执行任务。
- Standalone 和 L3 issue 必须填写 `执行边界`；L2 issue 不得直接当实现任务使用。
- issue 标题必须使用 `[状态]标题`，并和 `phase:*` 标签同步。

## ADR Draft

```md
# ADR: <title>

## Status
Proposed

## Context

## Decision

## Rationale

## Alternatives Considered

## Rejected Options

## Risks

## Rollback

## Tests / Evidence
```

## Implementation Handoff

```md
# Implementation Handoff

## Approved Direction

## Scope
- In scope:
- Out of scope:

## Constraints

## Files / Areas To Inspect First

## Tests To Add First

## Verification Evidence

## Stop / Escalate If
```

Rules:

- 除非用户明确要求，handoff 不是完整 implementation plan；完整开发计划必须回到 Standalone Complete Issue 或已批准 issue 树。
- Handoff 只能从已确认 Standalone Complete Issue，或已批准 L1、已收敛 L2 和明确 L3 生成。
- 每个实现任务都必须能追溯到已批准范围或验收证据。
- 实现中发现新决策时，回到 `problem-framing`。
