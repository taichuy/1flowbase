---
memory_type: feedback
feedback_category: interaction
topic: 长任务先探索、Root packetize、全部开发后集中测试
summary: 用户要求长任务先由 subagent 一次探索，Root 汇总路径、现状、目标、AC 与验收测试后分发明确 Work Packet；全部开发装配完成后只对总任务启动一个集中 QA，不做零碎探索和逐包测试。当前暂停时间预算，不在 skills 或 Issue 写 P50/P80、ETA 或硬停止时间。
keywords:
  - long-running
  - subagent
  - orchestration
  - context
  - delivery
  - work packet
  - batch qa
match_when:
  - 诊断长时间运行的多 agent Codex 任务
  - 设计 Root agent、开发 agent 与 QA agent 的职责
  - 优化长任务提示词、上下文继承或 agent 生命周期
created_at: 2026-07-18 12
updated_at: 2026-07-18 17
last_verified_at: 2026-07-18 17
decision_policy: direct_reference
scope:
  - user interaction
  - long-running Codex tasks
---

# 长任务保留中央调度与 Delivery 内开发上下文连续

## 时间

`2026-07-18 12`

## 规则

- 先由一个只读 subagent 探索相关路径、当前行为、依赖、测试入口和未知；不让每个开发 agent 重新探索。
- Root agent 是唯一调度者、packetizer、assembly owner 和 Control Ledger owner；汇总说明、现状、目标、AC 与 Test Batch 后，再分发边界明确的 Work Packet。
- 开发 agent 在同一 Delivery 内可以连续复用上下文，但每个 Packet 有独立代码结果和明确写集合；同时 active 的 Packet 写集合必须互斥，新 Delivery 使用最小 handoff。
- Root 下全部开发与 fixture Packet 装配完成后，只对冻结 assembly 启动一个 fresh QA；不做 per-packet / per-Delivery reviewer、回归或反复测试。
- QA 一次性返回完整 blocker 集，Root 再分发 fix Packet；全部修复装配后才启动新的单一 QA。
- 当前暂停时间预算治理；skills 与 GitHub Issue 不写 P50/P80、ETA、目标 / 硬停止 wall time 或耗时校准。
- 为后续调优只保留可验证的非时间事件计数，例如 first batch pass、Packet / fix Packet、needs-split、assembly conflict、agent context、验证运行与 QA 轮次；当前不为这些计数设目标值。

## 原因

一次探索避免开发 agent 重复熟悉代码，Root packetization 保持范围和 assembly 单一真值；先完成全部开发再集中测试，可以避免每个小变更都重新编译、review 和 QA。时间数字在工作模式尚未稳定前会制造错误优化目标，因此先移除。

## 适用场景

- Issue Tree 长计划
- 多 worktree、多 agent 开发
- 跨仓库或多阶段 Delivery
- 长时间自主 Codex 任务
