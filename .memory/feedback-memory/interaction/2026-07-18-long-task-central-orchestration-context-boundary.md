---
memory_type: feedback
feedback_category: interaction
topic: 长任务保留中央调度与 Delivery 内开发上下文连续，问题应定位在嵌套调度和跨 Delivery 复用
summary: 用户认可主 agent 负责调度，也希望开发 agent 在同一交付域保持上下文；单次 agent 自治任务以 30 分钟为目标、60 分钟必须回报，不能用 2～3 分钟误判超支；多小时 Delivery 通过同一上下文 follow-up 续段。
keywords:
  - long-running
  - subagent
  - orchestration
  - context
  - delivery
  - control interval
  - time budget
match_when:
  - 诊断长时间运行的多 agent Codex 任务
  - 设计 Root agent、开发 agent 与 QA agent 的职责
  - 优化长任务提示词、上下文继承或 agent 生命周期
created_at: 2026-07-18 12
updated_at: 2026-07-18 15
last_verified_at: 2026-07-18 15
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
- 单次 agent 自治任务默认 30 分钟做收敛检查，最迟 60 分钟返回状态；2～3 分钟通常不足以判定过度探索。
- Delivery 本身可以是数小时；路径仍成立时由 Root 对同一开发 agent follow-up 续发下一 control interval，不为续段丢弃上下文或重建调度中心。
- 超过 60 分钟不能静默继续；返回证据、实耗和下一可控结果，由 Root 判断续段或 reframe。预算耗尽不能改写完成证据。

## 原因

中央调度能维护唯一集成基线和范围判断，Delivery 内连续上下文能减少重复探索。真正导致长任务收益低的是调度层级重复、上下文寿命无界，以及监督被实现成持续微管理。

## 适用场景

- Issue Tree 长计划
- 多 worktree、多 agent 开发
- 跨仓库或多阶段 Delivery
- 长时间自主 Codex 任务
