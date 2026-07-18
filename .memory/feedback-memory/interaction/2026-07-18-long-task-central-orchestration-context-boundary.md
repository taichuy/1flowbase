---
memory_type: feedback
feedback_category: interaction
topic: 长任务保留中央调度与 Delivery 内开发上下文连续，问题应定位在嵌套调度和跨 Delivery 复用
summary: 用户认可主 agent 负责调度，也希望开发 agent 在同一交付域保持上下文；优化时不要把 delegation 或连续性本身判为问题，应限制开发上下文寿命到单个 Delivery，并避免开发 agent 再嵌套调度子 agent。
keywords:
  - long-running
  - subagent
  - orchestration
  - context
  - delivery
match_when:
  - 诊断长时间运行的多 agent Codex 任务
  - 设计 Root agent、开发 agent 与 QA agent 的职责
  - 优化长任务提示词、上下文继承或 agent 生命周期
created_at: 2026-07-18 12
updated_at: 2026-07-18 12
last_verified_at: 2026-07-18 12
decision_policy: direct_reference
scope:
  - user interaction
  - long-running Codex tasks
---

# 长任务保留中央调度与 Delivery 内开发上下文连续

## 时间

`2026-07-18 12`

## 规则

- 主 agent 作为唯一调度者、集成者和 Root Control Ledger owner 是正确结构，不应把中央调度本身判为问题。
- 开发 agent 在同一个 Delivery 内保持上下文连续是期望行为；优先通过同一 agent 的 follow-up 延续。
- 新的独立 Delivery 更适合使用新的开发 agent 与独立 worktree，通过最小 handoff 继承稳定上下文。
- 需要优化的是开发 agent 再嵌套调度、跨 Delivery 长期复用、完整历史 fork 和没有状态变化的高频轮询。

## 原因

中央调度能维护唯一集成基线和范围判断，Delivery 内连续上下文能减少重复探索。真正导致长任务收益低的是调度层级重复、上下文寿命无界，以及监督被实现成持续微管理。

## 适用场景

- Issue Tree 长计划
- 多 worktree、多 agent 开发
- 跨仓库或多阶段 Delivery
- 长时间自主 Codex 任务
