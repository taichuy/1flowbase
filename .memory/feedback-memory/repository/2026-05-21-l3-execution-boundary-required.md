---
memory_type: feedback
feedback_category: repository
topic: L3 执行任务层是 AI 实现控制边界
summary: 旧 L0/L1/L2/L3 计划模型已于 2026-07-17 被 Single Issue / 两层 Issue Tree 取代；本条只保留历史原因，不再作为当前规则使用。
keywords:
  - issue hierarchy
  - level:l3
  - superseded
created_at: 2026-05-21 00
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
decision_policy: verify_before_decision
status: superseded
superseded_by: .memory/feedback-memory/repository/2026-07-17-two-plan-shapes-replace-l0-l3.md
scope:
  - .agents/skills/problem-framing
  - .memory/feedback-memory/repository
---

# Superseded L3 Execution Boundary

本条记录旧四层 issue 模型曾用于防止大任务直接进入实现。该模型在 #1303 实践中产生过多横向节点、局部完成和延迟集成，已被用户明确废止。

当前规则读取 `2026-07-17-two-plan-shapes-replace-l0-l3.md`：普通任务使用 Single Issue；长计划使用 Root → 纵向 Delivery，两层结构直接替换 L0/L1/L2/L3。
