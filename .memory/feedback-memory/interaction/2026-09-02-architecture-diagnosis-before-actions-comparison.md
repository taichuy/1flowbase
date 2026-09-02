---
memory_type: feedback
feedback_category: interaction
topic: 架构迁移先诊断现状再启动跨分支 Actions 对比
summary: 用户要求了解架构迁移进度与当前接口事实时，先完成只读诊断和验证方案对齐；即使允许推送 beta，也不得提前触发 main/beta GitHub Actions 对比。
keywords:
  - architecture migration
  - beta
  - GitHub Actions
  - quality gate comparison
  - diagnosis first
match_when:
  - 用户要求检查 beta 架构迁移现状并把 main/beta 质量门禁作为后续完整性验证方案
  - 用户允许推送 beta，但明确当前阶段只是了解情况
created_at: 2026-09-02 13
updated_at: 2026-09-02 13
last_verified_at: 2026-09-02 13
decision_policy: direct_reference
scope:
  - repository architecture diagnosis
  - GitHub Actions quality gates
  - beta delivery workflow
---

# 架构迁移先诊断，再比较 Actions

## 规则

当请求同时包含“检查当前架构迁移状态”和“后续用 GitHub Actions 比较 main/beta”时，当前阶段先输出接口现状、迁移边界、证据强度、未知与比较方案。除非用户随后确认进入验证阶段，否则不推送 beta、不触发跨分支门禁，也不把历史 Actions 结果冒充当前候选证据。

## 原因

架构代码是否已装配、功能是否已证明完整、CI 是否正式结算是三个不同状态。提前运行跨分支门禁会跳过对比较口径和门禁版本差异的对齐。

## 适用场景

适用于 beta worktree 架构审计、接口迁移完整性诊断，以及把 main/beta GitHub Actions 作为下一阶段证据的任务。
