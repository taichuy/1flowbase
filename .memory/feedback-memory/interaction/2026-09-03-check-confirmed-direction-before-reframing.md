---
memory_type: feedback
feedback_category: interaction
topic: 诊断后续方向前先核对已确认决策
summary: 当现象已有已确认但未落地的修复方向时，先检索项目记忆和历史证据，不把局部技术优化误当作新建议。
keywords:
  - confirmed direction
  - problem framing
  - dev-up
  - reset password
match_when:
  - 为重复出现的已知问题提出修复方向
  - 当前实现可能落后于用户已确认的决策
created_at: 2026-09-03 07
updated_at: 2026-09-03 07
last_verified_at: 2026-09-03 07
decision_policy: direct_reference
scope:
  - problem-framing
  - scripts/node/dev-up
---

# 先核对已确认方向

## 规则

对重复现象做根因诊断后，在提出新实现方向前，先检索同主题项目记忆和历史决策。已有方向未落地时，应报告“方案未实现 / 发生回退”，不得用更局部的方案重新替代。

## 原因

用户已于 2026-08-23 确认 `reset_root_password` 应替换为通用 JS 数据库账号密码重置脚本；本次 AI 未先核对该决策，误推荐“缩小 Rust 依赖边界”。

## 适用场景

历史待办、阶段决策、同一故障再现、当前代码与已批准设计不一致的诊断。
