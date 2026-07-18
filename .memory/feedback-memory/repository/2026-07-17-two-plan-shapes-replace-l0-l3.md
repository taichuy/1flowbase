---
memory_type: feedback
feedback_category: repository
topic: 计划只使用 Single Issue 与两层 Issue Tree
summary: 普通任务使用 Single Issue；长计划使用 Root → 纵向 Delivery 两层 Issue Tree。新模型直接替换 L0/L1/L2/L3，不并行保留；Root 一次批准既定 Delivery，用户只验收 Root。
keywords:
  - problem-framing
  - Single Issue
  - Issue Tree
  - Root
  - Delivery
  - vertical slice
  - long-running work
match_when:
  - 创建或调整 issue 计划
  - 使用 problem-framing
  - 多 agent 或跨上下文长任务
  - 重构旧 issue tree
created_at: 2026-07-17 22
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
decision_policy: direct_reference
scope:
  - .agents/skills/problem-framing
  - .agents/skills/test-driven-development
  - GitHub issue planning
---

# Two Plan Shapes Replace L0-L3

## 时间

`2026-07-17 22`

## 规则

- 计划形态只有两种：普通任务的 Single Issue，以及长计划的两层 Issue Tree。
- Issue Tree 只有一个 Root 和若干纵向 Delivery；Delivery 不继续拆 issue。
- Root 是计划、进度与用户验收唯一真值。Root 一次批准正文列出的 Delivery，用户只验收 Root。
- 不能独立减少 Root AC 风险并进入集成基线的内容是实现步骤，不是 Delivery。
- 新模型直接替换旧 L0/L1/L2/L3；重构时关闭 superseded 节点，不维护两套活动计划。
- 长任务进展按已集成 Root AC 证据衡量，不按局部 commit、测试数、评论或子 issue 数衡量。

## 原因

#1303 的旧四层树包含 55 个节点和 40 个横向 L3，局部 contract、storage、protocol 提交很多，却长期没有形成 `/api/agent/v3/runs` 的端到端验收结果。层级和过程控制超过了结果本身，增加上下文、评论、编译与集成成本。

## 适用场景

- 普通功能、缺陷、重构或 contract 需求选择计划形态。
- 多仓库、多 agent、跨上下文、migration 或 rollout 长计划。
- 判断某项工作应成为 GitHub Delivery 还是内部 implementation handoff。
- 重构已有 parent/child issue 树。
